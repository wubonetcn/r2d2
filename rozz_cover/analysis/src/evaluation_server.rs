extern crate common;
extern crate clap;
extern crate indicatif;
extern crate tokio;
extern crate memmap;
extern crate chrono;

mod utils;

use common::{get_pid, MemDependency};
use common::{analyzer_error, analyzer_print};
use serde::{Serialize, Deserialize};
use std::time::Instant;
use std::collections::{HashSet, HashMap};
use std::sync::{Arc, Mutex, RwLock};
use std::net::SocketAddr;
use common::{RegisterMsgInfo, SessionId};
use std::path::PathBuf;
use std::fs::{File, OpenOptions, remove_file};
use std::io::{Write, Read};
use colored::Colorize;
use clap::{App, Arg};
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::AsyncReadExt;

#[derive(Debug, Clone)]
pub struct Statistics {
    pub compactness: Vec<f64>,
    pub visited_cfunc_set: HashSet<String>,
    pub start_time: Instant,
    pub exec_num: u64,
    pub is_active: bool,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct StatisticsForSerialize {
    visited_cfunc_num: u64,
    avg_compactness: f64,
    exec_num: u64,
}

impl Default for Statistics {
    fn default() -> Self {
        Self {
            compactness: vec![],
            exec_num: 0,
            visited_cfunc_set: HashSet::default(),
            is_active: false,
            start_time: Instant::now(),
        }
    }
}

#[allow(unused)]
impl Statistics {
    pub fn serialize(&self) -> StatisticsForSerialize {
        let mut sum = 0f64;
        for var in &self.compactness {
            sum += var;
        }
        let avg_compactness = sum / (self.compactness.len() as f64);
        let visited_cfunc_num = self.visited_cfunc_set.len() as u64;
        StatisticsForSerialize {
            avg_compactness,
            visited_cfunc_num,
            exec_num: self.exec_num,
        }
    }
}


#[derive(Debug, Default, Clone)]
pub struct MetricCollector {
    pub map: Arc<Mutex<HashMap<SessionId, HashMap<SocketAddr, PathBuf>>>>,
    pub socket2id: Arc<Mutex<HashMap<SocketAddr, RegisterMsgInfo>>>,
    pub demangle_map: Arc<Mutex<HashMap<String, String>>>,
    pub statistics: Arc<RwLock<Statistics>>,
    pub output_path: PathBuf,
}

#[allow(unused)]
// it should reuse the same logic of sancov_server, but I don't have much time to refactor it
// so there are lots of duplicated code, making the code (as well as me) ugly
impl MetricCollector {
    pub fn on_socket_connected(&mut self, addr: &SocketAddr, read_buf: &[u8]) {
        let register: RegisterMsgInfo = bincode::deserialize(read_buf)
            .expect(format!("cannot deserialize register info: {:?}", read_buf).as_str());
        self.socket2id.lock().unwrap().insert(addr.clone(), register.clone());
        let mut locked_map = self.map.lock().unwrap();
        if !locked_map.contains_key(&register.id) {
            locked_map.insert(register.id, Default::default());
        }
        locked_map.
            get_mut(&register.id).unwrap().insert(addr.clone(), register.path);
        drop(locked_map);
        let read_locked_statistics = self.statistics.read().unwrap();
        let is_active = read_locked_statistics.is_active;
        drop(read_locked_statistics);
        if !is_active {
            let mut write_locked_statistics = self.statistics.write().unwrap();
            write_locked_statistics.is_active = true;
            write_locked_statistics.start_time = Instant::now();
            drop(write_locked_statistics);
            self.serialize_statistics_to_filesystem(true);
        }
    }
    pub fn serialize_statistics_to_filesystem(&self, should_created: bool) -> bool {
        let read_locked_statistics = self.statistics.read().unwrap();
        if !read_locked_statistics.is_active {
            return false;
        }
        let output_statistics = read_locked_statistics.serialize();
        drop(read_locked_statistics);
        let mut vec = serde_json::to_vec(&output_statistics).unwrap();
        vec.push(b'\n');
        let mut file: File = if should_created || !self.output_path.exists() {
            OpenOptions::new().read(true).write(true)
                .create(true).truncate(true).open(&self.output_path).expect(
                &format!("cannot open the file: {:?}", self.output_path))
        } else {
            OpenOptions::new().read(true).write(true)
                .append(true).open(&self.output_path).expect(
                &format!("cannot create the file: {:?}", self.output_path))
        };
        file.write_all(&vec).expect(
            &format!("cannot write to the file: {:?}", self.output_path)
        );
        return true;
    }

    pub fn on_socket_closed(&mut self, addr: &SocketAddr) {
        let mut locked_socket2id = self.socket2id.lock().unwrap();
        let register_info = locked_socket2id
            .get(addr)
            .expect(
                format!("the socket doesn't exist on socket2id: {:?}", addr.clone()).as_str()
            ).clone();
        locked_socket2id.remove(addr).unwrap();
        let mut locked_map = self.map.lock().unwrap();
        let hashmap = locked_map
            .get_mut(&register_info.id).expect(
            format!("the socket doesn't exist on map: {:?}", addr.clone()).as_str()
        );

        let path = hashmap.get(addr).expect(
            format!("the addr doesn't exist on hashmap: {:?}", addr.clone()).as_str()
        ).clone();
        hashmap.remove(addr).unwrap();
        // unlock
        drop(locked_socket2id);
        drop(locked_map);

        let mut input_fs = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .expect("file doesn't exist");
        let mut buf: Vec<u8> = vec![];
        input_fs.read_to_end(&mut buf).expect("cannot read buf from input file");
        let mem_dep: MemDependency = match bincode::deserialize(&buf) {
            Ok(m) => m,
            Err(e) => {
                analyzer_error!("cannot deserialize: {:?}", &register_info);
                remove_file(&register_info.path).unwrap();
                return;
            }
        };
        let mut locked_statistics = self.statistics.write().unwrap();
        locked_statistics.exec_num += 1;
        for cfunc in &mem_dep.call_seq {
            if !locked_statistics.visited_cfunc_set.contains(cfunc) {
                locked_statistics.visited_cfunc_set.insert(cfunc.clone());
            }
        }
        let mut accumulation = 0u64;
        let call_num = mem_dep.call_seq.len();
        for i in 0..call_num {
            let mut store_set: HashSet<u64> = HashSet::default();
            for (ptr, _, _) in &mem_dep.mini_store_mem[i] {
                store_set.insert(ptr.clone());
            }
            for j in i + 1..call_num {
                let mut load_set: HashSet<u64> = HashSet::default();
                for (ptr, _, _) in &mem_dep.mini_load_mem[j] {
                    load_set.insert(ptr.clone());
                }
                let intersection: HashSet<u64> =
                    store_set.intersection(&load_set).cloned().collect();
                if intersection.len() != 0 {
                    accumulation += 1;
                }
            }
        }
        let total = call_num * (call_num - 1) / 2;
        locked_statistics.compactness.push(accumulation as f64 / total as f64);

        locked_statistics.is_active = true;
        remove_file(&register_info.path).unwrap();
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = App::new("tool for collecting memory-related metric")
        .version("1.0")
        .author("Chijin <tlock.chijin@gmail.com>")
        .about("listen to the target port and collect metric")
        .arg(Arg::with_name("port")
            .short("p")
            .long("port")
            .value_name("port")
            .help("port to listen")
            .required(true))
        .arg(Arg::with_name("output_path")
            .short("o")
            .long("output")
            .value_name("output_path")
            .help("the path that the output should serialize")
            .required(true))
        .get_matches();
    let port = matches.value_of("port").unwrap();
    let output_path = matches.value_of("output_path").unwrap();
    let mut collector = MetricCollector::default();
    collector.output_path = PathBuf::from(output_path);
    // spawn a task for serialize to file system every 60 sec
    tokio::spawn({
        let local_collector = collector.clone();
        async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                let date = chrono::Local::now();
                let ok = local_collector.serialize_statistics_to_filesystem(false);
                if ok {
                    analyzer_print!("[{}]: automatically serialize to file system",
                    date.format("%Y-%m-%d][%H:%M:%S"));
                }
            }
        }
    });
    let listen_addr = "127.0.0.1:".to_owned() + port;
    analyzer_print!("start listen to: {}",&listen_addr);

    let listener: TcpListener = TcpListener::bind(&listen_addr).await?;
    loop {
        let (mut tcp_stream, addr): (TcpStream, SocketAddr) = listener.accept().await?;
        tokio::spawn({
            let mut local_collector = collector.clone();
            let local_addr = addr;
            async move {
                let mut buf = [0u8; 256];
                loop {
                    let res: std::io::Result<usize> = tcp_stream.read(&mut buf).await;
                    if res.is_ok() {
                        local_collector.on_socket_connected(&local_addr, &buf);
                        if res.unwrap() == 0 {
                            // socket closed
                            local_collector.on_socket_closed(&local_addr);
                            break;
                        }
                    } else {
                        // socket closed or buffer is overflow
                        local_collector.on_socket_closed(&local_addr);
                        break;
                    }
                }
            }
        });
    }
}