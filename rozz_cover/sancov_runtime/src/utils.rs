use std::net::SocketAddr;
use std::collections::{HashMap, BTreeMap};
use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use bincode;
use serde_json;
use memmap::MmapOptions;
use std::sync::{Arc, Mutex, RwLock};
use std::fs::{OpenOptions, File, remove_file};
use std::io::{Write};
use std::slice;
use std::time::{Instant, SystemTime};
use std::ops::Range;
use common::RegisterMsgInfo;
use common::SessionId;

pub const MAXSIZE: usize = 1 << 23;
// for now, its enough for chromium (instr num: 7675079)
pub const HEADER_SIZE: usize = 1 << 10;

/*
|      HEADER        |          COV          |

HEADER: [SHM, SHM+HEADER_SIZE)
COV: [SHM+HEADER_SIZE, SHM+MAXSIZE)
*/
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Header {
    // dsoname -> vec index
    pub map: BTreeMap<String, usize>,
    // (offset, size); the cov will be [offset, offset + size)
    // note that the offset is to the beginning of the COV section, not the
    // beginning of the shm. So I should add a HEADER_SIZE when using the offset
    pub vec: Vec<(usize, usize)>,
}

#[allow(unused)]
impl Header {
    pub fn get_cov_range(&self) -> Range<usize> {
        if self.vec.is_empty() {
            return Range { start: HEADER_SIZE, end: HEADER_SIZE };
        }
        let start = self.vec.first().unwrap().0 + HEADER_SIZE;
        let last_element = self.vec.last().unwrap();
        let end = last_element.0 + last_element.1 + HEADER_SIZE;
        return Range { start, end };
    }

    pub fn get_cov_range_of_dso(&self, target_dso_name: &str) -> Option<Range<usize>> {
        let res = if let Some(index) = self.map.get(target_dso_name) {
            let target = self.vec[index.clone()];
            let start = target.0 + HEADER_SIZE;
            let end = target.0 + target.1 + HEADER_SIZE;
            Some(Range {
                start,
                end,
            })
        } else {
            let mut cnt = 0u8;
            let mut start = 0usize;
            let mut end = 0usize;
            for var in self.map.keys() {
                if var.contains(target_dso_name) {
                    let index = self.map.get(var).unwrap().clone();
                    let target = self.vec[index];
                    start = target.0 + HEADER_SIZE;
                    end = target.0 + target.1 + HEADER_SIZE;
                    cnt += 1;
                }
            };
            if cnt != 1 {
                None
            } else {
                Some(Range {
                    start,
                    end,
                })
            }
        };
        return res;
    }
}

pub unsafe fn get_header(shm: *const u8) -> Header {
    let start_addr = shm;
    let buf = slice::from_raw_parts(start_addr, HEADER_SIZE);

    let header: Header = match bincode::deserialize(buf) {
        Ok(h) => { h }
        Err(_) => {
            let h = Header::default();
            let buf = bincode::serialize(&h).unwrap();
            unsafe {
                let mut p = shm as *mut u8;
                for i in 0..buf.len() {
                    *p = buf[i];
                    p = p.add(1);
                }
            }
            h
        }
    };
    return header;
}

pub unsafe fn get_cov_offset_pointer(shm: *mut u8, offset: usize) -> *mut u8 {
    let start = shm;
    start.add(offset)
}


// return a vec of indexes that need to be update
#[allow(unused)]
pub type UpdateFunc = fn(&RegisterMsgInfo, &Header, &mut Statistics, *mut u8) -> Vec<usize>;

// return true if we should track coverage of the process
#[allow(unused)]
pub type Filter = fn(&Header, &RegisterMsgInfo) -> bool;

#[derive(Debug, Clone)]
pub struct Statistics {
    pub cov: Vec<u8>,
    pub cov_map: BTreeMap<String, Range<usize>>,
    pub cov_range: Range<usize>,
    pub start_time: Instant,
    pub exec_num: u64,
    pub is_active: bool,
}

impl Default for Statistics {
    fn default() -> Self {
        Self {
            cov: vec![0u8; MAXSIZE],
            cov_map: Default::default(),
            cov_range: Range { start: 0, end: 0 },
            start_time: Instant::now(),
            exec_num: 0,
            is_active: false,
        }
    }
}

#[allow(unused)]
impl Statistics {
    pub fn serialize(&self) -> StatisticsForSerialize {
        let mut covered_num = 0usize;
        let total_coverage = self.cov_range.len();
        let exec_num = self.exec_num;
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
        for var in self.cov.iter() {
            if *var != 0 {
                covered_num += 1;
            }
        };
        let cov_map = self.cov_map.clone();
        StatisticsForSerialize {
            covered_num,
            total_coverage,
            exec_num,
            timestamp,
            cov_map,
        }
    }
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct StatisticsForSerialize {
    pub covered_num: usize,
    pub total_coverage: usize,
    pub exec_num: u64,
    pub timestamp: u64,
    pub cov_map: BTreeMap<String, Range<usize>>,
}

#[derive(Debug, Default, Clone)]
pub struct CovCollector {
    pub map: Arc<Mutex<HashMap<SessionId, HashMap<SocketAddr, PathBuf>>>>,
    pub socket2id: Arc<Mutex<HashMap<SocketAddr, RegisterMsgInfo>>>,
    pub statistics: Arc<RwLock<Statistics>>,
    pub output_path: PathBuf,
}

#[allow(unused)]
impl CovCollector {
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
    pub fn on_socket_closed(&mut self, addr: &SocketAddr, filer: Filter, func: UpdateFunc) {
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

        // update the statistics
        let shm_file = if let Ok(shm_file) = OpenOptions::new().read(true).write(true).open(&path) {
            shm_file
        } else {
            println!("cannot open the file: {:?}", &path);
            return;
        };

        let mut mmap = unsafe { MmapOptions::new().map_mut(&shm_file) }.unwrap();
        let shm_ptr = mmap.as_mut_ptr();
        let header = unsafe { get_header(shm_ptr) };

        if !filer(&header, &register_info) {
            // we don't need to track the coverage of this client process
            // remove the shm file and return
            remove_file(path).unwrap();
            return;
        }

        // find the update indexes
        let mut write_locked_statistics = self.statistics.write().unwrap();
        let update_indexes: Vec<usize> = func(&register_info, &header, &mut write_locked_statistics, shm_ptr);
        // drop(write_locked_statistics);

        // update them
        // let mut write_locked_statistics = self.statistics.write().unwrap();
        write_locked_statistics.exec_num += 1;
        // write_locked_statistics.cov_range = header.get_cov_range();
        if update_indexes.len() != 0 {
            for index in update_indexes {
                write_locked_statistics.cov[index] = 1;
            }
            drop(write_locked_statistics);
            println!("{:?} {:?}", &path, &header);
            self.serialize_statistics_to_filesystem(false);
        }

        // remove the shm file and return
        drop(shm_file);
        remove_file(path).unwrap();
    }
    pub fn serialize_statistics_to_filesystem(&self, should_created: bool) -> bool {
        let read_locked_statistics = self.statistics.read().unwrap();
        if !read_locked_statistics.is_active {
            return false;
        }
        let output_statistics = read_locked_statistics.serialize();
        let cov = &read_locked_statistics.cov;

        // write to statistics file
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

        // write to coverage file

        // println!("a");
        let mut cov_path = self.output_path.clone().to_str().unwrap().to_string();

        // println!("b");
        cov_path = cov_path + ".cov";
        let mut file: File = OpenOptions::new().read(true).write(true)
            .create(true).truncate(true).open(&cov_path).expect(
            &format!("cannot open the file: {:?}", cov_path));
        file.write_all(cov).expect(
            &format!("cannot write to the file: {:?}", cov_path)
        );

        // println!("bb");

        return true;
    }
}
