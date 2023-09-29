use super::super::{cover_handle::callgraph::*, ExecError};
use super::ty::TYPE;
use crate::corpus_handle::{
    interface::*,
    models::OnnxModel,
    prog::Prog,
    ty::{array, character, double, integer, Type, TypeId},
    SHM_PATH,
};
use multimap::MultiMap;
use nix::{
    sys::wait::{waitpid, WaitStatus},
    unistd::Pid,
};
use rand::rngs::SmallRng;
use rand::Rng;
use rand::SeedableRng;
use std::{
    collections::{HashMap, HashSet},
    fs::{File, OpenOptions},
    io::Write,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};
use sysinfo::{ProcessExt, ProcessStatus::Zombie, System, SystemExt};
use util::{fuzzer_info, shmem::*};

// This struture manage the ros targets, include ros boot file and all node information
#[derive(Debug)]
pub struct Target {
    // current process id
    pub pid: u32,
    // where ros launch file locates
    pub launch_file: String,
    // ros nodes in a application
    pub nodes: Vec<Node>,
    pub node_name: HashSet<String>,
    // all type information
    pub tys: Vec<Type>,
    // Type id to type mapping.
    ty_id_mapping: HashMap<TypeId, Type>,
    pub rng: SmallRng,
    pub converted: HashMap<TypeId, Type>,

    // interface list
    pub itfs_types: Vec<String>,
    pub itfs_maps: HashMap<String, String>,

    // detailed interface info
    pub itfs_info: HashMap<String, Vec<InterfaceParam>>,
    pub param_info: HashMap<String, String>,
    pub banned_param: Vec<String>,

    // shmem management
    pub shm_region: SharedMem,
    pub call_graph: CallGraph,
    pub current_corpus: Vec<Prog>,
    pub corpus: Vec<Prog>,
    pub executor_model: Arc<Mutex<OnnxModel>>,
    pub topic_model: Arc<Mutex<OnnxModel>>,
    pub trace_model: Arc<Mutex<OnnxModel>>,
}
impl Target {
    pub fn new(ros_dir_path: String, output_path: String) -> Self {
        let executor_model_path = output_path.clone() + "sys/executor.onnx";
        let topic_model_path = output_path.clone() + "sys/topic.onnx";
        let trace_model_path = output_path.clone() + "sys/event.onnx";
        let mut target = Target {
            pid: 0,
            launch_file: ros_dir_path,
            nodes: Vec::new(),
            node_name: HashSet::new(),
            tys: Vec::new(),
            ty_id_mapping: HashMap::default(),
            rng: SmallRng::from_entropy(),
            converted: HashMap::default(),
            itfs_info: HashMap::default(),
            itfs_maps: HashMap::default(),
            param_info: HashMap::default(),
            banned_param: Vec::new(),
            shm_region: SharedMem::new(),
            call_graph: CallGraph::new(),
            current_corpus: Vec::new(),
            executor_model: Arc::new(Mutex::new(OnnxModel::new(&std::path::Path::new(
                executor_model_path.as_str(),
            )))),
            topic_model: Arc::new(Mutex::new(OnnxModel::new(&std::path::Path::new(
                topic_model_path.as_str(),
            )))),
            trace_model: Arc::new(Mutex::new(OnnxModel::new(&std::path::Path::new(
                trace_model_path.as_str(),
            )))),
            corpus: Vec::new(),
            itfs_types: vec![
                "bool".to_string(),
                "byte".to_string(),
                "char".to_string(),
                "float32".to_string(),
                "float64".to_string(),
                "int8".to_string(),
                "uint8".to_string(),
                "int16".to_string(),
                "uint16".to_string(),
                "int32".to_string(),
                "uint32".to_string(),
                "int64".to_string(),
                "uint64".to_string(),
                "string".to_string(),
                "wstring".to_string(),
            ],
        };

        target.ty_id_mapping = target
            .tys
            .iter()
            .map(|s| (s.id(), s.clone()))
            .collect::<HashMap<_, _>>();
        target
    }

    #[inline(always)]
    pub fn tys(&self) -> &[Type] {
        &self.tys
    }

    #[inline]
    pub fn ty_of(&self, tid: TypeId) -> &Type {
        &self.ty_id_mapping[&tid]
    }

    pub fn add_interes_prog(&mut self, prog: &Prog) {
        self.corpus.push(prog.clone());
    }

    pub fn set_node_param(&mut self, node_name: &String, param_buffer: &String) {
        self.param_info
            .insert(node_name.to_string(), param_buffer.to_string());
    }

    // check if a word exist in itfs_types
    pub fn exist(&self, word: &str) -> bool {
        if self.is_array(word) {
            // substr that remove anything between '[' and ']'
            let substr = &word[..word.find('[').unwrap()];
            self.itfs_types.contains(&substr.to_string())
        } else {
            self.itfs_types.contains(&word.to_string())
        }
    }

    // check if is array type like float[] int[32]
    pub fn is_array(&self, word: &str) -> bool {
        word.contains("[") && word.contains("]")
    }
    pub fn is_const(&self, word: &str) -> bool {
        word.eq("=")
    }

    pub fn clean_prog(&mut self) {
        self.current_corpus.clear();
    }

    pub fn add_prog(&mut self, prog: Prog) {
        self.current_corpus.push(prog);
    }

    pub fn vaild_name(&self, word: &str) -> bool {
        if word.len() > 0 && word.chars().last().unwrap().is_alphabetic() {
            return true;
        }
        return false;
    }

    // for given string, get the number within []
    pub fn get_array_size(&self, word: &str) -> i32 {
        let mut size = 0;
        let mut flag = false;
        for c in word.chars() {
            if c == '[' {
                flag = true;
                continue;
            }
            if c == ']' {
                break;
            }
            if flag && c.is_digit(10) {
                size = size * 10 + c.to_digit(10).unwrap() as i32;
            }
        }

        let mut rng = rand::thread_rng();
        match size {
            0 => rng.gen_range(1..8),
            _ => size,
        }
    }

    pub fn gen_param_ift(&self, param_list: Vec<String>) -> Vec<InterfaceParam> {
        let mut params = Vec::new();

        // new a interfaceParam
        for i in 0..param_list.len() {
            // check if contains '\t'
            if param_list[i].contains('\t') {
                continue;
            }

            if self.exist(&param_list[i])
                && i + 1 < param_list.len()
                && self.vaild_name(&param_list[i + 1])
            {
                if self.is_array(&param_list[i]) {
                    // get array size
                    let size = self.get_array_size(&param_list[i]);
                    let itf_param = InterfaceParam::new(
                        param_list[i].to_string(),
                        param_list[i + 1].to_string(),
                        true,
                        size,
                        false,
                        String::new(),
                    );
                    params.push(itf_param);
                } else {
                    // if is not array type
                    if i + 3 < param_list.len() && self.is_const(&param_list[i + 2]) {
                        let val = param_list[i + 3].to_string();
                        let itf_param = InterfaceParam::new(
                            param_list[i].to_string(),
                            param_list[i + 1].to_string(),
                            false,
                            0,
                            true,
                            val,
                        );
                        params.push(itf_param);
                    } else {
                        let itf_param = InterfaceParam::new(
                            param_list[i].to_string(),
                            param_list[i + 1].to_string(),
                            false,
                            0,
                            false,
                            String::new(),
                        );
                        params.push(itf_param);
                    }
                }
            }
        }
        params
    }

    pub fn get_param_list(&mut self, itf: String) -> Vec<String> {
        // executor cmd: "ros2 interface show itf"
        let get_interface_param_cmd = Command::new("ros2")
            .args(["interface", "show", itf.as_str()])
            .output()
            .expect("failed to get interface information: {}");
        let param_list: String = String::from_utf8(get_interface_param_cmd.stdout).unwrap();

        // remove substr within param_list start '#' end  '\n'
        let mut target_stream = String::new();
        let mut flag = false;
        // remove all comment
        for c in param_list.chars() {
            if c == '#' {
                flag = true;
            }
            if c == '\n' {
                flag = false;
            }
            if !flag {
                target_stream.push(c);
            }
        }

        let param_list = target_stream
            .split(|c| c == ' ' || c == '\n')
            .filter(|s| s.len() > 0)
            .map(|s| s.to_string())
            .collect();
        param_list
    }
    pub fn get_interfaces<'a>(&mut self, work_dir: &str, input_type: &String) {
        // deserialize self.itfs_info from work_dir/../sys/itf_types.json
        let mut path_prefix = work_dir.to_owned() + "/../../sys/";
        if input_type.contains("turtlebot3") {
            path_prefix = path_prefix + "turtlebot3/";
        } else if input_type.contains("moveit2") {
            path_prefix = path_prefix + "moveit2/";
        } else if input_type.contains("nav2") {
            path_prefix = path_prefix + "nav2/";
        } else if input_type.contains("autoware") {
            path_prefix = path_prefix + "autoware/";
        }
        let itf_types_path = path_prefix.clone() + "itf_types.json";
        let itf_types_file = File::open(itf_types_path).unwrap();
        let itf_types = serde_json::from_reader(itf_types_file).unwrap();
        self.itfs_types = itf_types;

        // deserialize self.itfs_info from work_dir/../sys/itf_types.json
        let itf_info_path = path_prefix.clone() + "itf_param.json";
        let itf_info_file = File::open(itf_info_path).unwrap();
        let itf_info = serde_json::from_reader(itf_info_file).unwrap();
        self.itfs_info = itf_info;

        // deserialize self.itfs_maps from work_dir/../sys/itf_maps.json
        let itf_maps_path = path_prefix.clone() + "itf_maps.json";
        let itf_maps_file = File::open(itf_maps_path).unwrap();
        let itf_maps = serde_json::from_reader(itf_maps_file).unwrap();
        self.itfs_maps = itf_maps;

        // assign SHM_PATH
        let shm = String::from(work_dir);
        Self::write_shm_path(shm);
    }

    fn write_shm_path(new_path: String) {
        let mut path = SHM_PATH.lock().unwrap();
        *path = new_path;
    }

    pub fn parse_interface(&mut self, interface: Vec<&str>, shm_path: &String) {
        for itf in interface.clone() {
            if itf.contains("Messages:") || itf.contains("Services:") || itf.contains("Actions:") {
                continue;
            } else {
                // substr of last / to last word
                let itf_name = itf.split("/").last().unwrap();
                self.itfs_types.push(itf_name.to_string());
                self.itfs_maps.insert(itf_name.to_string(), itf.to_string());

                self.itfs_types.push(itf.to_string());
                // remove substr msg/ or stv/
                let itf = itf
                    .replace("msg/", "")
                    .replace("srv/", "")
                    .replace("action/", "");
                self.itfs_types.push(itf.to_string());
            }
        }
        // flag default to Messages
        let mut _flag = InterfaceTpyes::Messages;
        for itf in interface {
            if itf.contains("Messages:") {
                _flag = InterfaceTpyes::Messages;
            } else if itf.contains("Services:") {
                _flag = InterfaceTpyes::Services;
            } else if itf.contains("Actions:") {
                _flag = InterfaceTpyes::Actions;
            } else {
                let param_list = self.get_param_list(itf.to_string());
                let itf_param = self.gen_param_ift(param_list);
                self.itfs_info.insert(itf.to_string(), itf_param);
            }
        }

        // write self.itfs_info to workdir in json format
        let mut itfs_info_file =
            File::create(shm_path.to_owned() + "/itf_param-autowarre.json").unwrap();
        let itf_json = serde_json::to_string(&self.itfs_info).unwrap();
        itfs_info_file.write_all(itf_json.as_bytes()).unwrap();

        // write self.itfs_types, self.itfs_maps to file
        let mut itfs_types_file =
            File::create(shm_path.to_owned() + "/itf_types-autowarre.json").unwrap();
        let itf_types_json = serde_json::to_string(&self.itfs_types).unwrap();
        itfs_types_file
            .write_all(itf_types_json.as_bytes())
            .unwrap();

        let mut itfs_maps_file =
            File::create(shm_path.to_owned() + "/itf_maps-autowarre.json").unwrap();
        let itf_maps_json = serde_json::to_string(&self.itfs_maps).unwrap();
        itfs_maps_file.write_all(itf_maps_json.as_bytes()).unwrap();

        panic!();
    }

    pub fn get_ty_kind(&mut self, name: String) -> String {
        let mut type_name = name;
        let c = type_name.matches("\t").count();
        if c == 1 {
            type_name = type_name[1..type_name.len()].to_string();
        } else if c == 2 {
            type_name = type_name[3..type_name.len()].to_string();
        }
        type_name
    }

    pub fn get_ty_name(&mut self, name: String) -> String {
        name.to_string()
    }

    pub fn checked_const(&mut self, ty_name: &String) -> bool {
        ty_name.chars().nth(0).unwrap().is_uppercase()
    }

    pub fn node_serialize(
        &mut self,
        node_name: String,
        target_stream: Vec<&str>,
        param_map: MultiMap<String, InterfaceVal>,
    ) {
        let mut new_node = Node::new(node_name.to_string());

        // update ros2 param based on previous info
        new_node.set_param(param_map);

        // serailize a node
        if target_stream.len() != 0 {
            new_node.node_name = target_stream[0].to_string();

            let mut node_type: NodeTpyes = NodeTpyes::Subscribers;
            let mut i = 1;
            while 1 <= target_stream.len() {
                // skip parameter when extracting node information
                if i >= target_stream.len() {
                    break;
                }
                if target_stream[i] == "Subscribers:" {
                    node_type = NodeTpyes::Subscribers;
                    i += 1;
                }
                if target_stream[i] == "Publishers:" {
                    node_type = NodeTpyes::Publishers;
                    i += 1;
                }

                if target_stream[i] == "Service" && target_stream[i + 1] == "Servers:" {
                    node_type = NodeTpyes::ServiceServer;
                    i += 2;
                }

                if target_stream[i] == "Service" && target_stream[i + 1] == "Clients:" {
                    node_type = NodeTpyes::ServiceClients;
                    i += 2;
                }

                if target_stream[i] == "Action" && target_stream[i + 1] == "Servers:" {
                    node_type = NodeTpyes::ActionServers;
                    i += 2;
                }

                if target_stream[i] == "Action" && target_stream[i + 1] == "Clients:" {
                    node_type = NodeTpyes::ActionClients;
                    i += 2;
                }

                if i >= target_stream.len() {
                    break;
                }
                if target_stream[i].contains("parameter") || target_stream[i].contains("Parameter")
                {
                    i += 2;
                    continue;
                }

                let len = target_stream[i].len();
                let key = target_stream[i][..len - 1].to_string();
                i += 1;
                let val = target_stream[i].to_string();
                i += 1;
                let mut itf_val: InterfaceVal = InterfaceVal::new(&key, &val);
                itf_val.construct_itf_layers(&self.itfs_types, &self.itfs_maps, &self.itfs_info);
                // insert node information into node structures
                match node_type {
                    NodeTpyes::Subscribers => {
                        new_node.node_subscribers.insert(key, itf_val);
                    }
                    NodeTpyes::Publishers => {
                        new_node.node_publisher.insert(key, itf_val);
                    }
                    NodeTpyes::ServiceServer => {
                        new_node.service_server.insert(key, itf_val);
                    }
                    NodeTpyes::ServiceClients => {
                        new_node.service_client.insert(key, itf_val);
                    }
                    NodeTpyes::ActionServers => {
                        new_node.action_server.insert(key, itf_val);
                    }
                    NodeTpyes::ActionClients => {
                        new_node.action_client.insert(key, itf_val);
                    }
                }
            }
        }
        // dbg!(&new_node.node_subscribers);
        // create a file that can append
        let mut node_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open("./sys/node.json")
            .unwrap();
        // serialize new_node,
        let node_json = serde_json::to_string(&new_node).unwrap();
        writeln!(node_file, "{}", node_json).unwrap();

        self.nodes.push(new_node);
    }

    pub fn get_parameter_info(
        &mut self,
        node_name: &String,
        param_vec: &Vec<&str>,
        shm_path: &String,
    ) -> Result<MultiMap<String, InterfaceVal>, failure::Error> {
        // find the corresponding node
        let mut res_map = MultiMap::new();
        for param in param_vec {
            // contiune if contain substr "qos"
            if param.contains("qos") || param.contains("use_sim_time") {
                continue;
            }
            // use ros2 param describe node_name param to get param information
            let get_param_info_cmd = Command::new("ros2")
                .env("SHM_PATH", shm_path)
                .arg("param")
                .arg("describe")
                .arg(node_name.to_string())
                .arg(param.to_string())
                .output()
                .expect("failed to get param inforamtion!");
            let cmd_output = String::from_utf8_lossy(&get_param_info_cmd.stdout).to_string();
            let cmd_output = cmd_output.split_whitespace().collect::<Vec<&str>>();
            // type check
            // continue if cmd_output is empty
            if cmd_output.is_empty() {
                continue;
            }
            let type_idx = cmd_output.iter().position(|&r| r == "Type:").unwrap();
            let param_type = cmd_output[type_idx + 1].to_string();

            // constant value check
            let constran_idx = cmd_output
                .iter()
                .position(|&r| r == "Constraints:")
                .unwrap();
            for i in constran_idx..cmd_output.len() {
                if cmd_output[i] == "Read"
                    && cmd_output[i + 1] == "only:"
                    && i + 2 < cmd_output.len()
                {
                    if cmd_output[i + 2] == "true" {
                        // update those param that is const
                        self.banned_param.push(param.to_string());
                    }
                }
            }

            // set value for the parameter
            let mut rng = rand::thread_rng();
            match param_type.as_str() {
                "boolean" => {
                    let mut itf_val = InterfaceVal::new(&param.to_string(), &"bool".to_string());
                    itf_val.val.push(ValueType::Op2(integer::BoolType::new(
                        TYPE::Bool as usize,
                        0 as u64,
                        0,
                        1,
                    )));
                    res_map.insert(param.to_string(), itf_val);
                }
                "integer" => {
                    // get min and max val
                    let mut min_val = i64::MAX;
                    let mut max_val = i64::MIN;
                    for i in constran_idx..cmd_output.len() {
                        if cmd_output[i] == "Min" && cmd_output[i + 1] == "value:" {
                            min_val = cmd_output[i + 1].parse::<i64>().unwrap();
                        }
                        if cmd_output[i] == "Max" && cmd_output[i + 1] == "value:" {
                            max_val = cmd_output[i + 1].parse::<i64>().unwrap();
                        }
                    }

                    let mut itf_val = InterfaceVal::new(&param.to_string(), &"int64".to_string());
                    itf_val.val.push(ValueType::Op1(integer::IntType::new(
                        TYPE::Int64 as usize,
                        0 as u64,
                        max_val as u64,
                        min_val as u64,
                        64,
                    )));
                    res_map.insert(param.to_string(), itf_val);
                }
                "double" => {
                    // get min and max val
                    let mut min_val = f64::MAX;
                    let mut max_val = f64::MIN;
                    for i in constran_idx..cmd_output.len() {
                        if cmd_output[i] == "Min" && cmd_output[i + 1] == "value:" {
                            min_val = cmd_output[i + 1].parse::<f64>().unwrap();
                        }
                        if cmd_output[i] == "Max" && cmd_output[i + 1] == "value:" {
                            max_val = cmd_output[i + 1].parse::<f64>().unwrap();
                        }
                    }

                    let mut itf_val = InterfaceVal::new(&param.to_string(), &"float64".to_string());
                    itf_val.val.push(ValueType::Op3(double::DoubleType::new(
                        TYPE::Float64 as usize,
                        0.0,
                        max_val as f64,
                        min_val as f64,
                        64,
                    )));
                    res_map.insert(param.to_string(), itf_val);
                }
                "string" => {
                    let mut itf_val = InterfaceVal::new(&param.to_string(), &"string".to_string());
                    itf_val.val.push(ValueType::Op6(character::StringType::new(
                        TYPE::String as usize,
                        "".to_string(),
                        rng.gen_range(1..32),
                    )));
                    res_map.insert(param.to_string(), itf_val);
                }
                "boolean array" => {
                    let mut itf_val = InterfaceVal::new(&param.to_string(), &"bool[]".to_string());
                    let array_len = rng.gen_range(1..32);
                    let array_type = TYPE::Bool;
                    itf_val.val.push(ValueType::Op5(array::ArrayType::new(
                        TYPE::ARRAY as usize,
                        array_type,
                        array_len as u64,
                    )));
                    res_map.insert(param.to_string(), itf_val);
                }
                "integer array" => {
                    let mut itf_val = InterfaceVal::new(&param.to_string(), &"int64[]".to_string());
                    let array_len = rng.gen_range(1..32);
                    let array_type = TYPE::Int64;
                    itf_val.val.push(ValueType::Op5(array::ArrayType::new(
                        TYPE::ARRAY as usize,
                        array_type,
                        array_len as u64,
                    )));
                    res_map.insert(param.to_string(), itf_val);
                }
                "double array" => {
                    let mut itf_val =
                        InterfaceVal::new(&param.to_string(), &"float64[]".to_string());
                    let array_len = rng.gen_range(1..32);
                    let array_type = TYPE::Float64;
                    itf_val.val.push(ValueType::Op5(array::ArrayType::new(
                        TYPE::ARRAY as usize,
                        array_type,
                        array_len as u64,
                    )));
                    res_map.insert(param.to_string(), itf_val);
                }
                "string array" => {
                    let mut itf_val =
                        InterfaceVal::new(&param.to_string(), &"string[]".to_string());
                    let array_len = rng.gen_range(1..32);
                    let array_type = TYPE::String;
                    itf_val.val.push(ValueType::Op5(array::ArrayType::new(
                        TYPE::ARRAY as usize,
                        array_type,
                        array_len as u64,
                    )));
                    res_map.insert(param.to_string(), itf_val);
                }
                "byte array" => {
                    let mut itf_val = InterfaceVal::new(&param.to_string(), &"byte[]".to_string());
                    let array_len = rng.gen_range(1..32);
                    let array_type = TYPE::Byte;
                    itf_val.val.push(ValueType::Op5(array::ArrayType::new(
                        TYPE::ARRAY as usize,
                        array_type,
                        array_len as u64,
                    )));
                    res_map.insert(param.to_string(), itf_val);
                }
                _ => {
                    panic!("unkonw type: {}", param_type);
                }
            }
        }
        Ok(res_map)
    }

    pub fn update_shm(&mut self, shm_dir: &String) -> Result<(), failure::Error> {
        // load shared memory
        println!(
            "[{}]: update shm region ",
            chrono::Utc::now().format("%Y-%m-%d][%H:%M:%S"),
        );
        self.shm_region.mmap_load_info(&shm_dir);
        self.call_graph.update_callback_info(&mut self.shm_region);
        // if self
        //     .call_graph
        //     .update_callback_time(&mut self.shm_region, &shm_dir)
        //     .unwrap()
        // {
        //     self.add_interes_prog(cur_prog);
        // }
        Ok(())
    }

    pub fn check_timeout_and_interets(
        &mut self,
        start_time: u128,
        shm_dir: &String,
        output: &String,
    ) -> Result<bool, failure::Error> { 

        // get calltrace
        self.call_graph
            .get_call_trace(start_time, &mut self.shm_region, shm_dir, output)
            .unwrap();
        // update and monitor here
        match self.call_graph.monitors(
            shm_dir,
            &mut self.executor_model.clone(),
            &mut self.topic_model.clone(),
            &mut self.trace_model.clone(),
        ) {
            Ok(flag) => {
                return Ok(flag);
            }
            Err(e) => {
                fuzzer_info!("monitor error: {}", e);
                return Err(e);
            }
        }
    }

    pub fn get_child_process(&mut self, pid: &Pid) -> Vec<String> {
        let pid: String = pid.to_string();
        let output = Command::new("pstree")
            .arg("-p")
            .arg("-n")
            .arg("-T")
            .arg(pid)
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to execute pstree command")
            .stdout
            .expect("Failed to capture stdout");

        let grep_output = Command::new("grep")
            .arg("-oP")
            .arg(r#"\(\K\d+\)"#)
            .stdin(Stdio::from(output))
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to execute grep command")
            .stdout
            .expect("Failed to capture stdout");

        let awk_output = Command::new("awk")
            .arg("-F)")
            .arg("{ print $1 }")
            .stdin(Stdio::from(grep_output))
            .output()
            .expect("Failed to execute awk command");

        let child_pids = String::from_utf8_lossy(&awk_output.stdout);
        let pids: Vec<String> = child_pids
            .trim()
            .split('\n')
            .map(|s| s.to_string())
            .collect();
        return pids;
    }

    // kill zombie process on site
    pub fn kill_zombie_procee(&mut self, pid: i32) {
        // run ps -o ppid= -p $pid and get the result
        let output = Command::new("ps")
            .arg("-o")
            .arg("ppid=")
            .arg("-p")
            .arg(pid.to_string())
            .output()
            .expect("Failed to execute ps command");
 
        // run kill -9 $ppid
        Command::new("kill")
            .arg("-9")
            .arg(String::from_utf8_lossy(&output.stdout).trim().to_string())
            .output()
            .expect("Failed to execute kill command");
        
    }

    pub fn check_crash(&mut self) -> Result<(), failure::Error> {
        // get pid of ros2
        let mut sys = System::new_all();
        sys.refresh_all();
        let processes = sys.processes();
        let pid = self.pid;
        
        if let Some(process) = processes.get(&(pid as i32)) {
            fuzzer_info!("=====Process name: {}, with status: {:?}=====", process.name(), process.status());
            // if process.status() is zombie
            if process.status() == Zombie {
                self.kill_zombie_procee(processes.get(&(pid as i32)).unwrap().pid());
                return Err(ExecError::ZombError("Zombie Process".into()).into());
            }

            let pids = self.get_child_process(&Pid::from_raw(pid as i32));
            for pid in pids {
                // convert pid into Pid
                let pid = pid.parse::<i32>().unwrap();
                let cpid = Pid::from_raw(pid);
                match waitpid(cpid, Some(nix::sys::wait::WaitPidFlag::WNOHANG)) {
                    Ok(WaitStatus::Signaled(cpid, signal, _core_dumped)) => { 
                        fuzzer_info!("Process {} was terminated by signal: {}", cpid, signal);
                        return Err(ExecError::ExecError("ros2 is crashed ".into()).into());
                    }
                    Ok(_) => { 
                        return Ok(());
                    }
                    Err(err) => match err.into() {
                        Some(nix::errno::Errno::ECHILD) => { 
                            continue;
                        }
                        _ => {  
                            return Ok(());
                        }
                    },
                }
            }

        } else {
            dbg!("No process with PID {} was found", pid);
        }

        Ok(())
    }
}
