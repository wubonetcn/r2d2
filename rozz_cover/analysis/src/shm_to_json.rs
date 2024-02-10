extern crate common;
extern crate clap;
extern crate indicatif;

mod utils;

use utils::{MemDependencyList, process_one_shm};
use common::{analyzer_error, analyzer_print};

use bincode;
use clap::{Arg, App};
use colored::Colorize;
use common::MemDependency;
use common::get_pid;
use std::fs;
use std::io::{Read, Write};
use serde_json;
use std::path::{PathBuf};
use serde::{Serialize, Deserialize};
use std::collections::{HashSet, HashMap};
use std::iter::FromIterator;
use std::process::Command;
use indicatif::ProgressIterator;
use std::borrow::Borrow;
use std::fs::DirEntry;
use std::io;

fn main() {
    let matches = App::new("shm_to_json tool")
        .version("1.0")
        .author("Chijin <tlock.chijin@gmail.com>")
        .about("deserialize shm from bincode to json")
        .arg(Arg::with_name("input")
            .short("i")
            .long("input")
            .value_name("input_path")
            .help("path of shm")
            .required(true))
        .arg(Arg::with_name("output")
            .short("o")
            .long("output")
            .value_name("output_path")
            .help("path of output json")
            .required(true))
        .arg(Arg::with_name("relation_output")
            .short("r")
            .long("relation")
            .value_name("relation_output")
            .help("if it is set, it will output relation to the path"))
        .get_matches();

    let input_path = matches.value_of("input").unwrap();
    let output_path = matches.value_of("output").unwrap();
    let input_path_buf = PathBuf::from(input_path);
    if !input_path_buf.exists() {
        analyzer_print!("file doesn't exist: {}", input_path);
    }

    let mut mem_list = MemDependencyList::default();
    let mut demangle_map: HashMap<String, String> = Default::default();
    if input_path_buf.is_file() {
        analyzer_print!("process file: {}", input_path);
        let mem_dep = process_one_shm(&input_path_buf, &mut demangle_map).unwrap();
        mem_list.mem_list.push(mem_dep);
    } else if input_path_buf.is_dir() {
        analyzer_print!("process dir: {}", input_path);
        let mut cnt = 0;
        let mut success = 0;
        let entities: Vec<io::Result<DirEntry>> = fs::read_dir(&input_path_buf).unwrap().collect();
        for entity in entities.iter().progress() {
            let path = entity.as_ref().unwrap().path();
            let res = process_one_shm(&path, &mut demangle_map);
            if res.is_ok() {
                success += 1;
                let mem_dep = res.unwrap();
                mem_list.mem_list.push(mem_dep);
            }
            cnt += 1;
        }
        analyzer_print!("processed: {}/{}", success, cnt);
    }
    let json_buf = serde_json::to_vec(&mem_list).unwrap();
    let mut output_fs = fs::OpenOptions::new()
        .read(true).write(true).create(true).truncate(true).open(output_path)
        .expect("cannot open output file");
    output_fs.write(&json_buf).expect("cannot write buf to output file");
    analyzer_print!("done! json has written to {}", output_path);
}