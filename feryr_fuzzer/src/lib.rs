pub mod defs;
use chrono::{DateTime, Utc};
use clap::{App, Arg};
use feryr_prog::{
    corpus_handle::{
        interface::Node,
        sys::{dump_to_file, get_random_string},
        target::Target,
    },
    cover_handle::cover::*,
};

use fs_extra::dir::CopyOptions; 
use nix::unistd::{setpgid, Pid};
use rand::{distributions::Alphanumeric, Rng};
use serde::ser::Serialize;
use serde_json::Serializer;
use std::os::unix::process::CommandExt;
use std::{
    path::Path,
    collections::{HashMap, HashSet},
    env, fs,
    fs::{create_dir_all, File},
    io,
    io::{BufRead, BufReader, Write},
    process::{exit, Child, Command, Stdio},
    time::Instant,
};
use util::fuzzer_info;

#[derive(Debug)]
pub struct FuzzManager {
    pub uptime: DateTime<Utc>,
    pub total_exec: usize,
    pub last_exec: usize,
    pub coverage: Cover,
    pub id: String,
    pub config_path: String,
    pub input_type: String,
    pub input_args: String,
    pub workdir: String,
    pub ros_launch: Target,
    pub fuzzing_inst: Child,
}

impl FuzzManager {
    pub fn new(
        ros_dir_path: String,
        config_file_path: String,
        output_path: String,
        input_type: String,
        input_args: String,
    ) -> Self {
        FuzzManager {
            // presist data
            uptime: chrono::offset::Utc::now(),
            // corpus: CorpusWrapper::default(),
            total_exec: 0,
            last_exec: 0,
            coverage: Cover::new(),
            id: String::new(),
            config_path: config_file_path,
            input_type: input_type,
            input_args: input_args,
            workdir: output_path.clone(),
            ros_launch: Target::new(ros_dir_path, output_path),
            fuzzing_inst: Command::new("ls")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
            // cpu_num: 1,
        }
    }

// save crash
    pub fn save_crash(&mut self, error_message: &str) {
        
        fuzzer_info!("saveing crash"); 
 
        let err_des = error_message.split_once("...").unwrap().0;
        dbg!(err_des);
        // set crash path
        let crash_path =  format!("{}/{}/{}", self.workdir, "crash", err_des);
        let mut crash_idx = 0;
        // check if crasg_path exist
        if !Path::new(&crash_path).exists() { 
            // if not exist
            create_dir_all(format!("{}/{}/{}", self.workdir, "crash", err_des)).unwrap();
        } else {
            // is exist
            // read all file from dir
            let paths = fs::read_dir(format!("{}/{}/{}", self.workdir, "crash", err_des)).unwrap();
            for path in paths {
                // get file name
                let file_name = path.unwrap().file_name().into_string().unwrap();
                // get the last char of file name
                let file_idx = file_name.chars().last().unwrap().to_digit(10).unwrap();
                // compare with crash_idx
                crash_idx = file_idx + 1;
                break;
            }
        }

        // serialize current_prog
        let mut input_file = File::create(format!(
            "{}/{}/{}/{}-{}",
            self.workdir, "crash", err_des, "input", crash_idx
        ))
        .unwrap();
        for prog in self.ros_launch.current_corpus.iter() {
            let mut buf = Vec::new();
            prog.serialize(&mut Serializer::new(&mut buf)).unwrap();
            // append buf into input_file
            input_file.write_all(&buf).unwrap();
        }
        self.ros_launch.current_corpus.clear();

        // copy work_dir/shm, workd_dir/instance_err and workd_dir/instance_out to crash/random_string
        let mut options = CopyOptions::new();
        options.overwrite = true;
        options.copy_inside = true;
        fs_extra::dir::copy(
            format!("{}/{}", self.workdir, "shm"),
            format!("{}/{}/{}/{}-{}", self.workdir, "crash", err_des, "shm", crash_idx),
            &options,
        )
        .unwrap();

        fs::copy(
            format!("{}/{}", self.workdir, "instance_err"),
            format!(
                "{}/{}/{}/{}-{}",
                self.workdir, "crash", err_des, "instance_err", crash_idx
            ),
        )
        .unwrap();

        fs::copy(
            format!("{}/{}", self.workdir, "instance_out"),
            format!(
                "{}/{}/{}/{}-{}",
                self.workdir, "crash", err_des, "instance_out", crash_idx
            ),
        )
        .unwrap();
        // Create a file called "description" in the "crash/random_string" directory
        let mut description_file = File::create(format!(
            "{}/{}/{}/{}",
            self.workdir, "crash", err_des, "description"
        ))
        .unwrap();

        // Write the error message to the "description" file
        writeln!(description_file, "{}", err_des).unwrap();
    }

    pub fn try_repro(&mut self) -> bool {
        // try to reproduce crash
        // if reproduce, return true
        // if not reproduce, return false
        false
    }

    // pub fn gen_targets(work_dir: &String, ros_launch: &mut Target) -> Result<(), failure::Error> {
    pub fn gen_targets(&mut self) -> Result<(), failure::Error> {
        // get all interface list
        fuzzer_info!("generate interfaces info");
        // get all node list
        self.ros_launch.get_interfaces(
            &(self.workdir.to_owned() + &"/shm".to_string()),
            &self.input_type,
        );

        //     let get_interface_info = Command::new("ros2")
        //         .env("SHM_PATH", self.workdir.to_owned() + &"/shm".to_owned())
        //         .args(["interface", "list"])
        //         .output()
        //         .expect("failed to get node info for node: {}");
        // // vector of interface names
        //     let interface_list = String::from_utf8(get_interface_info.stdout).unwrap();
        //     let interface_list: Vec<&str> = interface_list.split_whitespace().collect();
        //     self.ros_launch.parse_interface(interface_list, &self.workdir);
        //     panic!();
        // get ros2 param information
        self.boot().unwrap();

        //sleep for 2 sec
        std::thread::sleep(std::time::Duration::from_secs(2));
        let node_list = Command::new("ros2")
            // .env("SHM_PATH", self.workdir.to_owned() + "/shm")
            .args(["node", "list", "--no-daemon"])
            .output()
            .expect("failed to get node list: {}");
        let node_list = String::from_utf8(node_list.stdout).unwrap();
        let node_list: HashSet<&str> = node_list.split_whitespace().collect();
        self.ros_launch.node_name = node_list.into_iter().map(|s| s.to_string()).collect();
        fuzzer_info!("get all node list {:?}", &self.ros_launch.node_name,);

        // read from dir: sys/node.json
        let file = File::open("./sys/node.json").unwrap();
        let reader = BufReader::new(file);

        let mut all_node: Vec<Node> = Vec::new();

        for line in reader.lines() {
            let node: Node = serde_json::from_str(&line.unwrap()).unwrap();
            all_node.push(node);
        }

        let node_map: HashMap<String, Node> = all_node
            .into_iter()
            .map(|node| (node.node_name.clone(), node))
            .collect();

        // get target info for each node
        for node_name in self.ros_launch.node_name.clone() {
            if !node_name.contains("/") || node_name.contains("spawn_entity") {
                continue;
            }
            fuzzer_info!("get info for node: {} ", node_name,);
            if node_map.contains_key(&node_name) {
                self.ros_launch
                    .nodes
                    .push(node_map.get(&node_name).unwrap().clone());
            } else {
                // get parameter information
                let get_param_list = Command::new("ros2")
                    .env("SHM_PATH", self.workdir.to_owned() + "/shm")
                    .args(["param", "list", &node_name])
                    .output()
                    .expect("failed to get node param for node: {}");
                let param_list = String::from_utf8(get_param_list.stdout).unwrap();
                let param_list: Vec<&str> = param_list.split_whitespace().collect();

                let res_map = self
                    .ros_launch
                    .get_parameter_info(
                        &node_name.to_string(),
                        &param_list,
                        &(self.workdir.to_string() + "/shm"),
                    )
                    .unwrap();

                // get interface information
                let get_node_info = Command::new("ros2")
                    .env("SHM_PATH", self.workdir.to_owned() + "/shm")
                    .args(["node", "info", &node_name])
                    .output()
                    .expect("failed to get node info for node: {}");

                self.ros_launch.node_serialize(
                    node_name.to_string(),
                    String::from_utf8(get_node_info.stdout)
                        .unwrap()
                        .split_whitespace()
                        .collect(),
                    res_map,
                );
            }
        }
        fuzzer_info!("construct callback graph ");
        self.ros_launch
            .shm_region
            .mmap_load_info(&(self.workdir.to_owned() + &"/shm".to_string()));
        self.ros_launch
            .call_graph
            .update_callback_info(&mut self.ros_launch.shm_region);
        Ok(())
    }

    pub fn init_ros_env(&mut self) -> bool {
        // create all log files
        self.id = get_random_string(10);
        self.workdir = self.workdir.to_string() + "fuzz-loop-" + self.id.as_str();
        let shm_dir: String = format!("{}/{}", &self.workdir, "shm");
        create_dir_all(&shm_dir).unwrap();

        // init ros env varibles
        let init_cmd = Command::new("bash")
            .arg("-c")
            .arg(format!("source {}", &self.config_path.to_string()))
            .output()
            .unwrap_or_else(|e| {
                eprintln!("failed to init ros env: {}", e);
                exit(1)
            });
        init_cmd.status.success()
    }

    pub fn boot(&mut self) -> Result<(), failure::Error> {
        let std_out = File::create(self.workdir.to_owned() + &"/instance_out").unwrap();
        let std_err = File::create(self.workdir.to_owned() + &"/instance_err").unwrap();

        match self.input_args.is_empty() {
            true => {
                unsafe {
                    self.fuzzing_inst = Command::new("xvfb-run")
                        .env("SHM_PATH", self.workdir.to_owned() + "/shm")
                        .args(&["-s", "-screen 0 1400x900x24"])
                        .args(&[
                            "ros2",
                            "launch",
                            "-d",
                            &self.input_type,
                            &self.ros_launch.launch_file,
                        ])
                        .stdout(Stdio::from(std_out))
                        .stderr(Stdio::from(std_err))
                        .pre_exec(|| {
                            // This will run in the child process before exec,
                            // and sets the child process to a new process group
                            setpgid(Pid::from_raw(0), Pid::from_raw(0)).expect("Failed to setpgid");
                            Ok(())
                        })
                        .spawn()
                        .unwrap_or_else(|e| {
                            eprintln!("failed to spawn ros nodes: {}", e);
                            exit(1)
                        });
                }
            }
            false => {
                let cmd_args: Vec<&str> = self.input_args.split_whitespace().collect();
                self.fuzzing_inst = Command::new("xvfb-run")
                    .env("SHM_PATH", self.workdir.to_owned() + "/shm")
                    .args(&["-s", "-screen 0 1400x900x24"])
                    .arg("ros2")
                    .arg("launch")
                    .arg("-d")
                    .arg(&self.input_type)
                    .arg(&self.ros_launch.launch_file)
                    .args(&cmd_args)
                    .stdout(Stdio::from(std_out))
                    .stderr(Stdio::from(std_err))
                    .spawn()
                    .unwrap_or_else(|e| {
                        eprintln!("failed to spawn ros nodes: {}", e);
                        exit(1)
                    });
            }
        }

        // check boot
        fuzzer_info!("waiting ros app to boot: {}", self.fuzzing_inst.id());

        let start_time = Instant::now();
        while !self.check_boot() {
            if start_time.elapsed().as_secs() > 10 {
                fuzzer_info!("ros app boot timeout");
                return Err(failure::err_msg("ros app boot timeout"));
            }
        }
        Ok(())
    }

    pub fn is_file_not_empty(&mut self, file_path: &str) -> bool {
        if let Ok(metadata) = fs::metadata(file_path) { 
            metadata.len() > 0
        } else {
            false // Error occurred while accessing the file metadata
        }
    }

    pub fn check_boot(&mut self) -> bool {
        let shm_path = self.workdir.to_string() + &"/shm".to_string();
        let msg_path = shm_path.clone() + "/msg";
        let callbacks_path = shm_path.clone() + &"/callbacks".to_string();
        let nodes_path = shm_path.clone() + &"/nodes".to_string();
        let times_path = shm_path.clone() + &"/times".to_string();
        if self.is_file_not_empty(&msg_path)
            && self.is_file_not_empty(&callbacks_path)
            && self.is_file_not_empty(&nodes_path)
            && self.is_file_not_empty(&times_path)
        {
            return true;
        }
        false
    }

    pub fn get_child_process(&mut self, pid: &Pid) -> Vec<String> {
        let pid = pid.to_string();
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

    // kill a process with given node name
    pub fn kill_ros_app(&mut self) -> Result<(), failure::Error> {
        // get pid
        let pids = self.get_child_process(&Pid::from_raw(self.fuzzing_inst.id() as i32));

        for pid in pids {
            Command::new("kill")
                .arg(pid.as_str())
                .spawn()
                .expect(&format!("Failed to kill process with PID {}", pid));
        }
        Ok(())
    }

    pub fn clean_shm_file(&mut self) -> io::Result<()> {
        let dir_path = format!("{}/{}", self.workdir, "/shm");
        // read all file from dir_path
        // let mut files = fs::read_dir(dir_path)?;
        // remove all files using sudo rm file in absolute path
        for entry in fs::read_dir(dir_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                fs::remove_file(&path)?;
            } else if path.is_dir() {
                fs::remove_dir_all(&path)?;
            }
        }
        Ok(())
    }

    pub fn repro(&mut self) {
        // TODO!
        // match handle.check_timeout() {
        //     _ => {
        //         // unknown error detected, save to corpus if can repro
        //         println!(
        //             "[{}]: Unknown Crash Detected",
        //             chrono::Utc::now().format("%Y-%m-%d][%H:%M:%S")
        //         );
        //         if handle.try_repro() == true {
        //             handle.save_crash();
        //         }
        //     }
        // }
    }

    pub fn reboot(&mut self) -> Result<(), failure::Error> {
        self.kill_ros_app().unwrap();
        // clean all file
        self.clean_shm_file().unwrap();
        match self.boot() {
            Ok(_) => {}
            Err(_e) => {
                dbg!("boot failed");
                self.reboot().unwrap();
            }
        }
        self.ros_launch.pid = self.fuzzing_inst.id();
        Ok(())
    }
 
    pub fn output_log(&self, _total_branch: &mut usize) -> Result<(), failure::Error> {
        // update coverage info
        // if total_branch.clone() == 0 {
        //     self.coverage.last_branch = self.coverage.branch;
        // } else {
        //     self.coverage.last_branch = self.coverage.branch - total_branch.clone();
        // }
        // *total_branch = self.coverage.branch.clone();

        let now_time = chrono::Utc::now().format("[%Y-%m-%d][%H:%M:%S]");

        let log_msg = format!(
            "{}: total_exec: {}, total branches: {}, last branches: {}, crash: {}",
            &now_time,
            &self.total_exec,
            &self.coverage.branch,
            // &self.last_exec,
            &self.coverage.last_branch,
            0 // &handle.corpus.len_exceptions()
        );
        println!("{}", &log_msg);
        let cover_path: String = format!("{}/{}", &self.workdir, "cover");
        let log_path: String = format!("{}/{}", &self.workdir, "logs");
        dump_to_file(
            now_time.to_string() + &self.coverage.branch.to_string(),
            &cover_path,
        )?;
        dump_to_file(&log_msg, &log_path)?;
        // self.last_exec = 0;

        Ok(())
    }
}

pub fn usage_help() {
    println!(
        "Usage: ./fuzzer -c config_file_path -r ros_dir -i input_type -a input_args -o output_dir"
    );
}

pub fn quit_fuzzer() {
    println!("{}", "quit fuzzer!!!");

    // kill all ros2 process
}

pub fn parse_args() {
    let _matches = App::new(clap::crate_name!())
        .version(clap::crate_version!())
        .author(clap::crate_authors!())
        .about("Robotic Operating System Fuzzer")
        .arg(Arg::with_name(""))
        .arg(
            Arg::with_name("set ros path")
                .short("r")
                .long("root")
                .required(true)
                .takes_value(true)
                .value_name("ROS_PATH"),
        )
        .arg(
            Arg::with_name("configuration file directory")
                .short("c")
                .long("config")
                .required(true)
                .takes_value(true)
                .value_name("CONFIG_FILE"),
        )
        .arg(
            Arg::with_name("input type")
                .short("i")
                .long("in")
                .required(true)
                .takes_value(true)
                .value_name("INPUT"),
        )
        .arg(
            Arg::with_name("output data directory")
                .short("o")
                .long("out")
                .required(true)
                .takes_value(true)
                .value_name("OUT"),
        )
        .arg(
            Arg::with_name("Auxiliary argument")
                .short("a")
                .long("args")
                .required(true)
                .takes_value(true)
                .value_name("ARGS"),
        )
        .arg(Arg::with_name("debug config").short("d").long("debug"))
        .get_matches();
}
