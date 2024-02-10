use common::{MemDependency, RegisterMsgInfo, GEPPointerAddress, GEPId, InstrPointerAddress, InstrId, TypeSize};
use common::get_pid;
use common::{analyzer_print, analyzer_error};
use colored::Colorize;
use std::collections::{HashMap, HashSet};
use serde::{Serialize, Deserialize};
use std::path::{PathBuf, Path};
use std::fs;
use std::io::{Read, Write, BufReader, BufRead};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::fs::{OpenOptions, remove_file};
use std::time::{SystemTime, UNIX_EPOCH};
use std::net::SocketAddr;
use memmap::MmapOptions;
use std::borrow::Borrow;
use std::hash::{Hash, Hasher};

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct MemDependencyList {
    pub mem_list: Vec<MemDependency>
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Graph {
    pub func_names: Vec<String>,
    pub func_name_map: HashMap<String, usize>,
    // {WriteFuncName : {ReadFuncName1, ...]} , }
    pub acc_graph: HashMap<usize, HashMap<usize, Vec<EdgeInfo>>>,
    pub gep_graph: HashMap<usize, HashMap<usize, Vec<EdgeInfo>>>,
    #[serde(skip)]
    pub active: bool,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct EdgeInfo {
    source_instr_id: u32,
    source_gep_id: u32,
    sink_instr_id: u32,
    sink_gep_id: u32,
}

#[derive(Debug, Default, Clone)]
pub struct MemAnalyzer {
    pub graph: Arc<Mutex<Graph>>,
    pub socket2info: Arc<Mutex<HashMap<SocketAddr, RegisterMsgInfo>>>,
    pub demangle_map: Arc<Mutex<HashMap<String, String>>>,
    pub output_path: PathBuf,
}


pub type RuleToCFuncMap = HashMap<String, String>;

#[allow(unused)]
impl MemAnalyzer {
    pub fn create_from_last_result(path: &PathBuf) -> Self {
        let mut last_graph_fs = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .expect("file doesn't exist");
        let mut buf: Vec<u8> = vec![];
        last_graph_fs.read_to_end(&mut buf).expect("cannot read buf from last result");
        let graph: Graph = serde_json::from_slice(&buf).expect("cannot deserialize it");
        let mut default = Self::default();
        let mut locked_graph = default.graph.lock().unwrap();
        *locked_graph = graph;
        drop(locked_graph);
        return default;
    }

    pub fn serialize_to_filesystem(&self, should_created: bool) -> bool {
        let locked_graph = self.graph.lock().unwrap();
        if !locked_graph.active {
            return false;
        }
        let buf = serde_json::to_vec(&*locked_graph).unwrap();
        let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let mut path = self.output_path.to_str().unwrap().to_string();
        path = path + &time.to_string();
        let mut file = OpenOptions::new().read(true).write(true)
            .create(true).truncate(true).open(&path).expect(
            &format!("cannot open the file: {:?}", path));
        file.write_all(&buf).expect(
            &format!("cannot write to the file: {:?}", path)
        );
        return true;
    }

    pub fn on_socket_connected(&mut self, addr: &SocketAddr, read_buf: &[u8]) {
        let register: RegisterMsgInfo = bincode::deserialize(read_buf)
            .expect(format!("cannot deserialize register info: {:?}", read_buf).as_str());
        let mut locked_socket2info = self.socket2info.lock().unwrap();
        locked_socket2info.insert(addr.clone(), register);
    }
    pub fn on_socket_closed(&mut self, addr: &SocketAddr) {
        let mut locked_socket2info = self.socket2info.lock().unwrap();
        let register_info = match locked_socket2info.get(addr) {
            Some(r) => r.clone(),
            None => {
                analyzer_error!("the socket doesn't exist on socket2info: {:?}", &addr);
                return;
            }
        };
        // analyzer_print!("closed: {:?}", register_info);
        drop(locked_socket2info);
        let mut locked_demangle_map = self.demangle_map.lock().unwrap();
        let mem_dep = match process_one_shm(&register_info.path, &mut locked_demangle_map) {
            Ok(r) => r,
            Err(e) => {
                analyzer_error!("cannot process shm: {:?}", &register_info);
                remove_file(&register_info.path).unwrap();
                return;
            }
        };
        drop(locked_demangle_map);
        let (gep_edges, acc_eges) = match std::env::var_os("DISABLE_MINI_COLLECTION") {
            Some(_) => { build_graph(&mem_dep) }
            None => {
                (vec![], build_graph_with_mini_collection(&mem_dep))
            }
        };

        let mut locked_graph = self.graph.lock().unwrap();
        for func_name in &mem_dep.call_seq {
            if !locked_graph.func_name_map.contains_key(func_name) {
                let index = locked_graph.func_names.len();
                locked_graph.func_names.push(func_name.clone());
                locked_graph.func_name_map.insert(func_name.clone(), index);
            }
        }
        for (source, sink, edge_info) in gep_edges.into_iter() {
            let source_name = &mem_dep.call_seq[source];
            let sink_name = &mem_dep.call_seq[sink];
            let source_index = locked_graph.func_name_map.get(source_name).unwrap().clone();
            let sink_index = locked_graph.func_name_map.get(sink_name).unwrap().clone();
            match locked_graph.gep_graph.get_mut(&source_index) {
                Some(hashmap) => {
                    hashmap.insert(sink_index, edge_info);
                }
                None => {
                    let mut hashmap: HashMap<usize, Vec<EdgeInfo>> = Default::default();
                    hashmap.insert(sink_index, edge_info);
                    locked_graph.gep_graph.insert(source_index, hashmap);
                }
            };
        }
        for (source, sink, edge_info) in acc_eges.into_iter() {
            let source_name = &mem_dep.call_seq[source];
            let sink_name = &mem_dep.call_seq[sink];
            let source_index = locked_graph.func_name_map.get(source_name).unwrap().clone();
            let sink_index = locked_graph.func_name_map.get(sink_name).unwrap().clone();
            match locked_graph.acc_graph.get_mut(&source_index) {
                Some(hashmap) => {
                    if hashmap.contains_key(&sink_index) {
                        continue;
                    } else {
                        hashmap.insert(sink_index, edge_info);
                    }
                }
                None => {
                    let mut hashmap: HashMap<usize, Vec<EdgeInfo>> = Default::default();
                    hashmap.insert(sink_index, edge_info);
                    locked_graph.acc_graph.insert(source_index, hashmap);
                }
            };
        }
        locked_graph.active = true;
        remove_file(&register_info.path).unwrap();
    }
}

// return the relationship vec based on accurate store/load
pub fn build_graph_with_mini_collection(mem_dep: &MemDependency)
                                        -> Vec<(usize, usize, Vec<EdgeInfo>)> {
    assert_eq!(mem_dep.call_seq.len(), mem_dep.mini_store_mem.len());
    let mut res = vec![];
    let length = mem_dep.call_seq.len();
    let mut accurate_store_mem_map: Vec<HashMap<InstrPointerAddress, (TypeSize, InstrId)>> = Default::default();
    let mut accurate_load_mem_map: Vec<HashMap<InstrPointerAddress, (TypeSize, InstrId)>> = Default::default();
    for i in 0..length {
        let mut load_i: HashMap<InstrPointerAddress, (TypeSize, InstrId)> = Default::default();
        let mut store_i: HashMap<InstrPointerAddress, (TypeSize, InstrId)> = Default::default();
        for (ptr, id, size) in &mem_dep.mini_load_mem[i] {
            if load_i.contains_key(ptr) {
                let (original_s, original_id) = load_i.get_mut(ptr).unwrap();
                if *original_s < *size {
                    *original_s = size.clone();
                    *original_id = id.clone();
                }
            } else {
                load_i.insert(ptr.clone(), (size.clone(), id.clone()));
            }
        }
        for (ptr, id, size) in &mem_dep.mini_store_mem[i] {
            if store_i.contains_key(ptr) {
                let (original_s, original_id) = store_i.get_mut(ptr).unwrap();
                if *original_s > *size {
                    *original_s = size.clone();
                    *original_id = id.clone();
                }
            } else {
                store_i.insert(ptr.clone(), (size.clone(), id.clone()));
            }
        }
        accurate_load_mem_map.push(load_i);
        accurate_store_mem_map.push(store_i);
    }
    for i in 0..length {
        let store_set_i: HashSet<&InstrPointerAddress> = accurate_store_mem_map[i].keys().collect();
        for j in i + 1..length {
            let load_set_j: HashSet<&InstrPointerAddress> = accurate_load_mem_map[j].keys().collect();
            let intersect_acc: HashSet<&InstrPointerAddress> = store_set_i.intersection(&load_set_j)
                .cloned().collect();
            if !intersect_acc.is_empty() {
                let mut all_intersect: Vec<EdgeInfo> = Default::default();
                for var in intersect_acc {
                    if accurate_store_mem_map[i][var].0 == accurate_load_mem_map[j][var].0 {
                        let edge = EdgeInfo {
                            source_gep_id: 0,
                            source_instr_id: accurate_store_mem_map[i][var].1,
                            sink_gep_id: 0,
                            sink_instr_id: accurate_load_mem_map[j][var].1,
                        };
                        all_intersect.push(edge);
                    } else {
                        analyzer_print!("miss match: {} {}", accurate_store_mem_map[i][var].0,
                        accurate_load_mem_map[j][var].0);
                    }
                }
                if all_intersect.len() > 0 {
                    res.push((i, j, all_intersect));
                }
            }
        }
    }
    return res;
}

// return: the first vector return the relationship vec based on gep, another is based on accurate
// store/load
pub fn build_graph(mem_dep: &MemDependency)
                   -> (Vec<(usize, usize, Vec<EdgeInfo>)>,
                       Vec<(usize, usize, Vec<EdgeInfo>)>) {
    let mut res_gep = vec![];
    let mut res_acc = vec![];

    let length = mem_dep.call_seq.len();
    let mut gep_store_mem_map: Vec<HashMap<GEPPointerAddress, GEPId>> = Default::default();
    let mut gep_load_mem_map: Vec<HashMap<GEPPointerAddress, GEPId>> = Default::default();
    let mut accurate_store_mem_map: Vec<HashMap<InstrPointerAddress, InstrId>> = Default::default();
    let mut accurate_load_mem_map: Vec<HashMap<InstrPointerAddress, InstrId>> = Default::default();
    let mut store2gep_map: Vec<HashMap<InstrPointerAddress, GEPPointerAddress>> = Default::default();
    let mut load2gep_map: Vec<HashMap<InstrPointerAddress, GEPPointerAddress>> = Default::default();
    for i in 0..length {
        let mut gep_store_i: HashMap<GEPPointerAddress, GEPId> = Default::default();
        let mut gep_load_i: HashMap<GEPPointerAddress, GEPId> = Default::default();
        let mut acc_store_i: HashMap<InstrPointerAddress, InstrId> = Default::default();
        let mut acc_load_i: HashMap<InstrPointerAddress, InstrId> = Default::default();
        let mut store2gep: HashMap<InstrPointerAddress, GEPPointerAddress> = Default::default();
        let mut load2gep: HashMap<InstrPointerAddress, GEPPointerAddress> = Default::default();
        for (acc, gep, acc_id, gep_id) in &mem_dep.store_mem[i] {
            acc_store_i.insert(acc.clone(), acc_id.clone());
            gep_store_i.insert(gep.clone(), gep_id.clone());
            store2gep.insert(acc.clone(), gep.clone());
        }
        for (acc, gep, acc_id, gep_id) in &mem_dep.load_mem[i] {
            acc_load_i.insert(acc.clone(), acc_id.clone());
            gep_load_i.insert(gep.clone(), gep_id.clone());
            load2gep.insert(acc.clone(), gep.clone());
        }
        for (gep, gep_id) in &mem_dep.gep_mem[i] {
            gep_load_i.insert(gep.clone(), gep_id.clone());
        }
        gep_store_mem_map.push(gep_store_i);
        gep_load_mem_map.push(gep_load_i);
        accurate_store_mem_map.push(acc_store_i);
        accurate_load_mem_map.push(acc_load_i);
        store2gep_map.push(store2gep);
        load2gep_map.push(load2gep);
    }
    for i in 0..length {
        let name_i = &mem_dep.call_seq[i];
        let gep_store_set_i: HashSet<&GEPPointerAddress> = gep_store_mem_map[i].keys().collect();
        let acc_store_set_i: HashSet<&InstrPointerAddress> = accurate_store_mem_map[i].keys().collect();
        // j is after i meanwhile i write to a memory which is j read from
        for j in i + 1..length {
            let name_j = &mem_dep.call_seq[j];
            if name_i == name_j {
                continue;
            }

            let gep_load_set_j: HashSet<&GEPPointerAddress> = gep_load_mem_map[j].keys().collect();
            let intersect_gep: HashSet<&GEPPointerAddress> = gep_store_set_i.intersection(&gep_load_set_j)
                .cloned().collect();
            // gep & gep
            if intersect_gep.len() != 0 {
                let mut all_intersect: Vec<EdgeInfo> = Default::default();
                for var in intersect_gep {
                    let edge = EdgeInfo {
                        source_gep_id: gep_store_mem_map[i][var],
                        source_instr_id: gep_store_mem_map[i][var],
                        sink_gep_id: gep_load_mem_map[j][var],
                        sink_instr_id: gep_load_mem_map[j][var],
                    };
                    all_intersect.push(edge);
                }
                res_gep.push((i, j, all_intersect));
            }

            let acc_load_set_j: HashSet<&InstrPointerAddress> = accurate_load_mem_map[j].keys().collect();
            let intersect_acc: HashSet<&InstrPointerAddress> = acc_store_set_i.intersection(&acc_load_set_j)
                .cloned().collect();
            // store_instr & load_instr
            if intersect_acc.len() != 0 {
                let mut all_intersect: Vec<EdgeInfo> = Default::default();
                for var in intersect_acc {
                    let source_gep = store2gep_map[i][var];
                    let sink_gep = load2gep_map[j][var];
                    let edge = EdgeInfo {
                        source_gep_id: gep_store_mem_map[i][&source_gep],
                        source_instr_id: accurate_store_mem_map[i][var],
                        sink_gep_id: gep_load_mem_map[j][&sink_gep],
                        sink_instr_id: accurate_load_mem_map[j][var],
                    };
                    all_intersect.push(edge);
                }
                res_acc.push((i, j, all_intersect));
            }
        }
    }
    return (res_gep, res_acc);
}

// given a c functions dependency, create a corresponding rule dependency for example:
// cfunc_mem_dep.call_seq: [jsElementxxx(xxx), jsElementxxx(), ..]
// new_mem_dep.call_seq: [<Element>.aa(), <Element>.bb = <lala>, ..]
fn mapping_cfunc_to_rule(cfunc_mem_dep: MemDependency, map: &RuleToCFuncMap, rule_seq: &Vec<String>) -> Result<MemDependency, ()> {
    if cfunc_mem_dep.call_seq.len() == 0 || rule_seq.len() == 0 {
        analyzer_print!("either cfunc sequence or rule sequence is empty");
        return Err(());
    }

    let mut new_mem_dep = MemDependency::default();
    let mut cfuncindex_to_ruleindex_map: HashMap<usize, usize> = HashMap::default();
    let mut ruleindex_to_cfuncindex_map: HashMap<usize, usize> = HashMap::default();
    let mut call_seq_index = 0usize;
    let mut total_useful_cnt = 0usize;
    let mut fail_cnt = 0usize;

    // typical longest common sequence (LCS), use dynamic programming
    let mut dp: Vec<Vec<usize>> = vec![vec![0usize; cfunc_mem_dep.call_seq.len() + 1]; rule_seq.len() + 1];
    let mut record = vec![vec![(0usize, 0usize); cfunc_mem_dep.call_seq.len() + 1]; rule_seq.len() + 1];
    for i in 1..rule_seq.len() + 1 {
        for j in 1..cfunc_mem_dep.call_seq.len() + 1 {
            let rule_index = i - 1;
            let cfunc_index = j - 1;
            let mut maxi = 0usize;
            let mut r = (0usize, 0usize);
            if dp[i - 1][j] >= dp[i][j - 1] {
                maxi = dp[i - 1][j];
                r = ((i - 1), j);
            } else {
                maxi = dp[i][j - 1];
                r = (i, (j - 1));
            }
            if map.get(&rule_seq[rule_index]).is_some() &&
                map[&rule_seq[rule_index]] == cfunc_mem_dep.call_seq[cfunc_index] {
                if dp[i - 1][j - 1] + 1 >= maxi {
                    maxi = dp[i - 1][j - 1] + 1;
                    r = ((i - 1), (j - 1));
                }
            }
            dp[i][j] = maxi;
            record[i][j] = r;
        }
    }
    let longest_match_len = dp.last().unwrap().last().unwrap().clone();
    let mut i = rule_seq.len();
    let mut j = cfunc_mem_dep.call_seq.len();
    let mut new_call_seq: Vec<String> = vec![String::new(); longest_match_len];
    let mut cnt = longest_match_len;
    loop {
        if i <= 0 || j <= 0 {
            break;
        }
        let (new_i, new_j) = record[i][j];
        if i - new_i == 1 && j - new_j == 1 {
            // match!
            let rule_position = cnt - 1;
            cfuncindex_to_ruleindex_map.insert(j - 1, rule_position);
            ruleindex_to_cfuncindex_map.insert(rule_position, j - 1);
            new_call_seq[rule_position] = rule_seq[i - 1].clone();
            // println!("{} {}", rule_seq[i - 1], cfunc_mem_dep.call_seq[j - 1]);
            cnt -= 1;
        }
        i = new_i;
        j = new_j;
    }
    assert_eq!(cnt, 0);
    analyzer_print!("match rate: {}/{}", longest_match_len, rule_seq.len());
    new_mem_dep.call_seq = new_call_seq;

    // update access
    for rule_index in 0..new_mem_dep.call_seq.len() {
        let cfunc_index = ruleindex_to_cfuncindex_map.get(&rule_index).unwrap().clone();
        if cfunc_mem_dep.load_mem.len() > cfunc_index {
            new_mem_dep.load_mem.push(cfunc_mem_dep.load_mem[cfunc_index].clone());
        } else {
            new_mem_dep.load_mem.push(vec![]);
        }
        if cfunc_mem_dep.store_mem.len() > cfunc_index {
            new_mem_dep.store_mem.push(cfunc_mem_dep.store_mem[cfunc_index].clone());
        } else {
            new_mem_dep.store_mem.push(vec![]);
        }
        if cfunc_mem_dep.gep_mem.len() > cfunc_index {
            new_mem_dep.gep_mem.push(cfunc_mem_dep.gep_mem[cfunc_index].clone());
        } else {
            new_mem_dep.gep_mem.push(vec![]);
        }
        if cfunc_mem_dep.mini_store_mem.len() > cfunc_index {
            new_mem_dep.mini_store_mem.push(cfunc_mem_dep.mini_store_mem[cfunc_index].clone());
        } else {
            new_mem_dep.mini_store_mem.push(vec![]);
        }
        if cfunc_mem_dep.mini_load_mem.len() > cfunc_index {
            new_mem_dep.mini_load_mem.push(cfunc_mem_dep.mini_load_mem[cfunc_index].clone());
        } else {
            new_mem_dep.mini_load_mem.push(vec![]);
        }
    }


    return Ok(new_mem_dep);
}

fn mapping_to_original_rule(mem_dep: MemDependency, shm_path: &PathBuf) -> Result<MemDependency, ()> {
    if let Ok(map_path) = std::env::var("RULE_TO_CFUNC_MAP") {
        let mut map_fs = match fs::OpenOptions::new()
            .read(true)
            .open(&map_path) {
            Ok(m) => m,
            Err(e) => {
                analyzer_error!("open map error: {:?}", e);
                return Err(());
            }
        };
        let mut buf: Vec<u8> = vec![];
        map_fs.read_to_end(&mut buf);
        drop(map_fs);
        let map: RuleToCFuncMap = serde_json::from_slice(&buf).unwrap();
        let mut rule_seq_path = shm_path.clone().into_os_string().into_string().unwrap() + ".seq";
        let mut rule_seq_fs = match fs::OpenOptions::new()
            .read(true)
            .open(&rule_seq_path) {
            Ok(r) => r,
            Err(e) => {
                analyzer_error!("open rule seq {} error: {:?}", rule_seq_path, e);
                return Err(());
            }
        };
        let rule_seq_buf_reader = BufReader::new(&rule_seq_fs);
        let mut rule_seq: Vec<String> = vec![];
        for line in rule_seq_buf_reader.lines() {
            if line.is_ok() {
                rule_seq.push(line.unwrap());
            }
        }
        drop(rule_seq_fs);
        remove_file(&rule_seq_path).unwrap();
        return mapping_cfunc_to_rule(mem_dep, &map, &rule_seq);
    } else {
        return Ok(mem_dep);
    }
}

pub fn process_one_shm(path: &PathBuf, demangle_map: &mut HashMap<String, String>) -> Result<MemDependency, ()> {
    let mut input_fs = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .expect("file doesn't exist");
    let mut buf: Vec<u8> = vec![];
    input_fs.read_to_end(&mut buf).expect("cannot read buf from input file");
    drop(input_fs);
    let mem_dep: MemDependency = match bincode::deserialize(&buf) {
        Ok(r) => r,
        Err(e) => {
            analyzer_error!("cannot deserialize shm; error message: {:?}", e);
            return Err(());
        }
    };
    let mut new_mem_dep = mem_dep.clone();
    new_mem_dep.call_seq.clear();
    for var in mem_dep.call_seq {
        let mut demangle_var = if demangle_map.contains_key(&var) {
            demangle_map.get(&var).unwrap().clone()
        } else {
            if var.len() == 0 {
                var
            } else {
                let demangle_var = match Command::new("llvm-cxxfilt")
                    .arg(var.clone())
                    .output() {
                    Ok(o) => {
                        let demangle_vec = o.stdout;
                        let demangle_var = String::from_utf8(demangle_vec).unwrap();
                        demangle_map.insert(var, demangle_var.clone());
                        demangle_var
                    }
                    Err(e) => {
                        analyzer_error!("demangle error: name: {}; error message: {:?}", &var, e);
                        "".to_string()
                    }
                };
                demangle_var
            }
        };
        while demangle_var.ends_with("\n") {
            demangle_var.pop();
        }
        new_mem_dep.call_seq.push(demangle_var);
    }
    new_mem_dep.dedup_all_mem();

    if std::env::var("ENABLE_RULE_MAP").is_ok() {
        return mapping_to_original_rule(new_mem_dep, &path);
    } else {
        return Ok(new_mem_dep);
    }
}