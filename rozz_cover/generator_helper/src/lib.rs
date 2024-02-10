extern crate common;
extern crate rand;

use colored::Colorize;
use common::{analyzer_error, analyzer_print};
use common::get_pid;
use libc;
use std::env;
use std::ptr::null_mut;
use std::fs::OpenOptions;
use serde::{Deserialize};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::ffi;
use rand::prelude::*;
use rand::distributions::WeightedIndex;
use std::borrow::{BorrowMut, Borrow};
use std::cmp::min;
use std::process::exit;
use std::time;

const PROBABILITY_FOR_PRINT_DEBUG: u8 = 1;

#[derive(Default, Debug, Clone, Deserialize)]
pub struct DeserializedMemoryDependency {
    statements: Vec<String>,
    statement_map: HashMap<String, usize>,
    graph: HashMap<usize, HashSet<usize>>,
}

#[derive(Default, Debug, Clone)]
pub struct Helper {
    pub creator: Vec<String>,
    pub creator_lineno: Vec<usize>,
    pub mem_dep: DeserializedMemoryDependency,
    pub statement_to_lineno_map: HashMap<String, usize>,
    pub graph: Vec<HashSet<usize>>,
    pub weights: Vec<u64>,
    pub rng: ThreadRng,
    // define for debugging
    pub debug_mode: bool,
    pub debug_output_path: String,
    pub total_relations_num: u64,
    pub history_picked: Vec<usize>,
    pub used_relations: HashMap<usize, HashMap<usize, u64>>,
}

impl Helper {
    pub fn add_creator(&mut self, rule_name: String) {
        self.creator_lineno.push(self.creator.len());
        self.creator.push(rule_name);
    }
    pub fn build_graph(&mut self) {
        for (index, rule_name) in self.creator.iter().enumerate() {
            self.statement_to_lineno_map.insert(rule_name.clone(), index);
        }

        for (_write_lineno, rule_name) in self.creator.iter().enumerate() {
            let write_mem_dep_index = match self.mem_dep.statement_map.get(rule_name) {
                Some(r) => r,
                None => {
                    // analyzer_error!("{}", rule_name);
                    // we don't care those who didn't visit any backend functions
                    self.graph.push(HashSet::default());
                    continue;
                }
            };
            let read_mem_dep_index_set = match self.mem_dep.graph.get(write_mem_dep_index) {
                Some(r) => r,
                None => {
                    // we don't care those who didn't visit any backend functions
                    self.graph.push(HashSet::default());
                    continue;
                }
            };
            let mut read_set_for_lineno = HashSet::default();
            for read_mem_dep_index in read_mem_dep_index_set {
                let read_rule_name = self.mem_dep.statements.get(read_mem_dep_index.clone()).unwrap();
                let read_lineno = self.statement_to_lineno_map.get(read_rule_name).unwrap();
                read_set_for_lineno.insert(read_lineno.clone());
            }
            self.graph.push(read_set_for_lineno);
        }
    }
    pub fn reset_weights(&mut self) {
        if self.debug_mode &&
            PROBABILITY_FOR_PRINT_DEBUG > rand::thread_rng().gen_range(0u8..=100u8) {
            self.print_used_relations();
        }
        if self.weights.len() != self.creator.len() {
            self.weights = vec![1; self.creator.len()]
        } else {
            for i in 0..self.weights.len() {
                self.weights[i] = 1;
            }
        }
        self.history_picked.clear();
    }

    pub fn record_used_relations(&mut self, write: usize, read: usize) {
        if !self.used_relations.contains_key(&write) {
            self.used_relations.insert(write.clone(), HashMap::default());
        }
        if !self.used_relations.get(&write).unwrap().contains_key(&read) {
            self.used_relations.get_mut(&write).unwrap().insert(read.clone(), 0);
        }
        *self.used_relations.get_mut(&write).unwrap().get_mut(&read).unwrap() += 1;
    }

    pub fn print_used_relations(&self) {
        let mut used: u64 = 0;
        for (_key, var) in &self.used_relations {
            used += var.keys().len() as u64;
        }
        analyzer_print!("used relations: {}/{}", used, self.total_relations_num);
        let timestamp = time::SystemTime::now()
            .duration_since(time::SystemTime::UNIX_EPOCH).unwrap().as_secs();
        let vec = serde_json::to_vec(&self.used_relations).unwrap();
        let output_path = self.debug_output_path.clone();
        std::fs::write(format!("{}/debuginfo-{}", output_path, timestamp), vec)
            .expect("cannot write debuginfo to filesystem");
    }
}

#[allow(unused)]
#[no_mangle]
pub extern "C" fn initialize_helper() -> *mut Helper {
    let mut helper = Box::new(Helper::default());
    analyzer_print!("helper initialization");
    // if MEM_DEP_JSON_PATH is not defined, then this function should not be called
    let json_path = match env::var("MEM_DEP_JSON_PATH") {
        Ok(r) => r,
        Err(e) => {
            analyzer_error!("MEM_DEP_JSON_PATH is not defined in var");
            return null_mut();
        }
    };

    let debug_mode = match env::var("MEM_DEP_DEBUG_MODE") {
        Ok(r) => {
            r == "true"
        }
        Err(_) => {
            false
        }
    };

    let debug_output_path = match env::var("MEM_DEP_DEBUG_OUTPUT_PATH") {
        Ok(r) => r,
        Err(_) => {
            "/tmp/experiment_debug_output/".to_string()
        }
    };
    let _ = std::fs::create_dir(&debug_output_path);

    helper.debug_mode = debug_mode;
    helper.debug_output_path = debug_output_path;
    let mut mem_dep_file = match OpenOptions::new().read(true).open(&json_path) {
        Ok(r) => r,
        Err(e) => {
            analyzer_error!("cannot create the file: {:?}; {:?}", json_path, e);
            return null_mut();
        }
    };
    let mut buf: Vec<u8> = vec![];
    mem_dep_file.read_to_end(&mut buf);
    let mem_dep: DeserializedMemoryDependency = match serde_json::from_slice(&buf) {
        Ok(r) => r,
        Err(e) => {
            analyzer_error!("cannot create the file: {:?}; {:?}", json_path, e);
            return null_mut();
        }
    };
    let mut total: u64 = 0;
    for (_key, var) in &mem_dep.graph {
        total += var.len() as u64;
    }
    helper.total_relations_num = total;
    helper.mem_dep = mem_dep;
    Box::into_raw(helper)
}

#[allow(unused)]
#[no_mangle]
pub extern "C" fn add_creator_line(helper: *mut Helper, original_rule: *mut libc::c_char) {
    unsafe {
        (*helper).add_creator(String::from(unsafe { ffi::CStr::from_ptr(original_rule) }
            .to_str().unwrap()));
    };
}

#[allow(unused)]
#[no_mangle]
pub extern "C" fn prepared(helper: *mut Helper) {
    unsafe {
        (*helper).build_graph();
        (*helper).reset_weights();
    };
}

#[allow(unused)]
#[no_mangle]
pub extern "C" fn update_weights(helper: *mut Helper, original_rule: *mut libc::c_char) {
    let helper = unsafe { (*helper).borrow_mut() };
    let rule_name = String::from(unsafe { ffi::CStr::from_ptr(original_rule) }
        .to_str().unwrap());
    match helper.statement_to_lineno_map.get(&rule_name) {
        Some(write_lineno) => {
            for read_lineno in &helper.graph[write_lineno.clone()] {
                helper.weights[read_lineno.clone()] += 1;
            }
            if helper.debug_mode {
                helper.history_picked.push(write_lineno.clone());
            }
        }
        None => {
            // analyzer_error!("{}", rule_name);
        }
    }
}

#[allow(unused)]
#[no_mangle]
pub extern "C" fn random_choice_with_weights(helper: *mut Helper) -> usize {
    let helper = unsafe { (*helper).borrow_mut() };
    let dist = WeightedIndex::new(&helper.weights).unwrap();
    let lineno = dist.sample(&mut helper.rng);
    if helper.debug_mode {
        for history_line in helper.history_picked.clone() {
            if history_line < helper.graph.len() &&
                helper.graph[history_line].contains(&lineno) {
                helper.record_used_relations(history_line, lineno);
            }
        }
    }
    return lineno;
}

#[allow(unused)]
#[no_mangle]
pub extern "C" fn reset_weights(helper: *mut Helper) {
    let helper = unsafe { (*helper).borrow_mut() };
    helper.reset_weights();
}

#[allow(unused)]
#[no_mangle]
pub extern "C" fn free_helper(helper: *mut Helper) {}

#[allow(unused)]
#[no_mangle]
pub extern "C" fn print_top_n_weights(helper: *mut Helper, n: usize) {
    let helper = unsafe { (*helper).borrow() };
    let mut vec: Vec<(String, u64)> = vec![];
    for (index, weight) in helper.weights.iter().enumerate() {
        vec.push((helper.creator[index].clone(), weight.clone()))
    }
    vec.sort_by(|a, b| b.1.cmp(&a.1));
    for i in 0..min(n, helper.weights.len()) {
        analyzer_print!("{:?}", vec[i]);
    }
}