extern crate common;
extern crate rand;
extern crate memmap;
#[macro_use]
extern crate lazy_static;

mod utils;

use utils::{Header, MAXSIZE, HEADER_SIZE};
use utils::{get_cov_offset_pointer, get_header};
use common::{analyzer_error, analyzer_print, RegisterMsgInfo, Logger, is_black_list};
use common::{get_pid, get_process_name, get_dso_name};

use colored::Colorize;
use std::ptr::{null_mut};
use std::env;
use std::path::Path;
use std::net::TcpStream;
use memmap::{MmapOptions, MmapMut};
use std::fs::{remove_file, create_dir_all, OpenOptions};
use std::process::exit;
use bincode;
use rand::random;
use std::io::{Write, Read};
use std::sync::Mutex;

/// Env FLAGS:
/// SHM_DIR: required, the shm file directory
/// PRINT_ANALYSIS_LOG: print logs if it is set to true
/// REGISTRATION_ADDR: the coverage collection server's tcp address

pub static mut SHM: *mut u8 = null_mut();
pub static mut MMAP: Option<MmapMut> = None;
pub static mut CONNECTOR: Option<TcpStream> = None;
// pub static mut PRINT_LOG_FLAG: bool = false;
static mut LOGGER: Option<Logger> = None;

lazy_static! {
    pub static ref CAN_USE_HEADER_LOCK: Mutex<bool> = Mutex::new(false);
}

#[no_mangle]
pub extern "C" fn __sanitizer_cov_trace_pc_guard(guard: *mut u32) {
    unsafe {
        if SHM.is_null() {
            return;
        }
        let off = get_cov_offset_pointer(SHM, *guard as _);
        *off = (*off).wrapping_add(1);
    }
}

#[no_mangle]
pub extern "C" fn __sanitizer_cov_trace_pc_guard_init(start: *mut u32, end: *mut u32) {
    unsafe {
        if LOGGER.is_none() {
            LOGGER = Some(Logger::new());
        }
    }
    if is_black_list(unsafe { LOGGER.as_mut().unwrap() }) {
        unsafe {
            LOGGER.as_mut().unwrap().printinfo("black list, ignore it");
        }
        return;
    }

    if start == end {
        return;
    }
    let file_name = get_process_name();
    let dso_name = get_dso_name(start as _);
    let pid = get_pid();
    let len = unsafe { end.offset_from(start) as usize };
    // match env::var_os("PRINT_ANALYSIS_LOG") {
    //     Some(flag) => {
    //         if flag.to_str().unwrap().eq("true") {
    //             unsafe { PRINT_LOG_FLAG = true };
    //         }
    //     }
    //     None => {}
    // };
    // let mut tmp: usize = 0;
    // let mut p = start;
    // while p != end {
    //     p = unsafe { p.add(1) };
    //     tmp += 1;
    // }
    unsafe {
        LOGGER.as_mut().unwrap().printinfo(&format!("start! file name: {}, dso name: {}, len: {}, start: {:?}, end: {:?}",
                                                    file_name,
                                                    dso_name, len, start, end));
    }
    // assert_eq!(len, tmp, "{:?} {:?}", start, end);


    // lock for header completeness
    let header_lock = CAN_USE_HEADER_LOCK.lock().unwrap();

    let mut header = if unsafe { SHM.is_null() } {
        let shm_file_path = match env::var_os("SHM_DIR") {
            Some(shm_dir) => {
                let dir_path = Path::new(&shm_dir);
                if !dir_path.exists() {
                    create_dir_all(dir_path).unwrap();
                }
                let rand_u64: u64 = random();
                let shm_file_name = pid.to_string() + "-" + file_name.as_str()
                    + "-rand" + rand_u64.to_string().as_str();
                let shm_file_path = dir_path.join(shm_file_name);
                if shm_file_path.exists() {
                    remove_file(&shm_file_path).unwrap();
                }
                let shm_file = OpenOptions::new().read(true).write(true)
                    .create(true).open(&shm_file_path).expect(
                    &format!("cannot create the file: {:?}", shm_file_path));
                shm_file.set_len(MAXSIZE as u64).unwrap();
                let mut mmap = unsafe { MmapOptions::new().map_mut(&shm_file) }.unwrap();
                unsafe { SHM = mmap.as_mut_ptr() };
                unsafe { MMAP = Some(mmap) };
                shm_file_path
            }
            None => {
                unsafe { LOGGER.as_mut().unwrap().printinfo("no SHM_DIR in env variables; exit!") };
                exit(1);
            }
        };
        match env::var_os("REGISTRATION_ADDR") {
            Some(reg_addr) => {
                let addr = reg_addr.into_string().unwrap();
                let connector = match TcpStream::connect(&addr) {
                    Ok(mut tcp_stream) => {
                        let session_id: u64 = match env::var_os("SESSION_ID") {
                            Some(id) => {
                                id.into_string().unwrap().parse().unwrap()
                            }
                            None => {
                                0u64
                            }
                        };
                        let register_info = RegisterMsgInfo {
                            path: shm_file_path,
                            id: session_id,
                            filename: file_name,
                        };
                        let buf = bincode::serialize(&register_info).unwrap();
                        tcp_stream.write_all(&buf)
                            .expect("cannot write to the other side");
                        unsafe {
                            LOGGER.as_mut().unwrap().printinfo(&format!("connected to {:?}", addr));
                        }
                        Some(tcp_stream)
                    }
                    Err(e) => {
                        unsafe {
                            LOGGER.as_mut().unwrap().printerr(&format!("cannot connect to {:?}, reason: {:?}", addr, e));
                        }
                        None
                    }
                };
                unsafe {
                    CONNECTOR = connector;
                }
            }
            None => {
                unsafe {
                    LOGGER.as_mut().unwrap().printinfo("no REGISTRATION_ADDR in env variables!");
                }
            }
        };
        Header::default()
    } else {
        unsafe { get_header(SHM) }
    };
    if header.map.contains_key(&dso_name) {
        unsafe {
            LOGGER.as_mut().unwrap().printerr(&format!("{} has been inserted into header map. {:?}", dso_name, header));
        }
        return;
    }
    let start_offset = if header.vec.len() > 0 {
        let (offset, size) = header.vec.last().unwrap();
        offset + size
    } else {
        0usize
    };
    let index = header.vec.len();
    header.vec.push((start_offset, len));
    header.map.insert(dso_name, index);
    let buf = bincode::serialize(&header).unwrap();
    unsafe {
        let mut p = SHM;
        for i in 0..buf.len() {
            *p = buf[i];
            p = p.add(1);
        }
    }

    // free the lock
    drop(header_lock);

    if start_offset + len >= MAXSIZE - HEADER_SIZE {
        unsafe {
            LOGGER.as_mut().unwrap().printerr(&format!("on my god, COV ({}, {}) exceeds MAXSIZE {}", start_offset,
                                                       start_offset + len, MAXSIZE - HEADER_SIZE));
        }
        panic!();
    }
    let mut p = start;
    let mut off = 0;
    unsafe {
        while p != end {
            *p = (start_offset + HEADER_SIZE + off) as _;
            off += 1;
            p = p.add(1);
        }
    }
    return;
}
