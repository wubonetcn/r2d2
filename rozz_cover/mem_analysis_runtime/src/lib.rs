#[macro_use]
extern crate lazy_static;

extern crate common;
extern crate rand;
extern crate memmap;

use common::{analyzer_error, analyzer_print, RegisterMsgInfo, Logger, is_black_list};

use std::ptr::{null_mut};
use std::env;
use std::ffi;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::hint::spin_loop;

use memmap::{MmapOptions, MmapMut};
use bincode;
use std::fs::{create_dir_all, remove_file, OpenOptions, copy, File};
use std::process::exit;
use std::sync::{Mutex, RwLock};
use common::{MemDependency, get_tid, get_pid, get_process_name};
use colored::Colorize;
use rand::random;
use std::net::TcpStream;
use std::io::{Write, Read};

const MAXSIZE: usize = 1 << 26;
static mut SHM: *mut u8 = null_mut();
static mut MMAP: Option<MmapMut> = None;
static mut VISIT_TID: u64 = 0;
static mut RECORD_TYPE_NAME: bool = false;
pub static mut CONNECTOR: Option<TcpStream> = None;
static CAN_WRITE_SHM_LOCK: AtomicBool = AtomicBool::new(true);
static mut MINI_COLLECTION: bool = true;
static mut IS_BLACKLIST_PROCESS: bool = false;
static mut LOGGER: Option<Logger> = None;

// someday I will refactor it for lock-free
lazy_static! {
    static ref MEM_DEP: Mutex<MemDependency> = Default::default();
    static ref CURRENT_FUNC: RwLock<Option<String>> = Default::default();
}
#[allow(unused)]
#[no_mangle]
pub extern "C" fn __mem_analysis_pointer_load(ptr: usize,
                                              original_gep: usize,
                                              load_id: u32, gep_id: u32, size: u32) {
    if unsafe { SHM.is_null() } {
        return;
    }
    if original_gep == 0 || ptr == 0 {
        return;
    }
    if unsafe { VISIT_TID } == 0 {
        return;
    }
    let current_tid = get_tid();
    if unsafe { VISIT_TID } != current_tid {
        return;
    }
    let ptr_u64 = ptr as u64;
    let original_gep_u64 = original_gep as u64;
    let mut mem_dep_lock = MEM_DEP.lock().unwrap();
    if unsafe { MINI_COLLECTION } {
        let last = mem_dep_lock.mini_load_mem.last_mut().unwrap();
        last.push((ptr_u64, load_id, size));
    } else {
        let last = mem_dep_lock.load_mem.last_mut().unwrap();
        last.push((ptr_u64, original_gep_u64, load_id, gep_id));
    }
}

#[allow(unused)]
#[no_mangle]
pub extern "C" fn __mem_analysis_pointer_store(ptr: usize,
                                               original_gep: usize,
                                               store_id: u32, gep_id: u32, size: u32) {
    if unsafe { SHM.is_null() } {
        return;
    }
    if original_gep == 0 || ptr == 0 {
        return;
    }
    if unsafe { VISIT_TID } == 0 {
        return;
    }
    let current_tid = get_tid();
    if unsafe { VISIT_TID } != current_tid {
        return;
    }
    let ptr_u64 = ptr as u64;
    let original_gep_u64 = original_gep as u64;
    let mut mem_dep_lock = MEM_DEP.lock().unwrap();
    if unsafe { MINI_COLLECTION } {
        let last = mem_dep_lock.mini_store_mem.last_mut().unwrap();
        last.push((ptr_u64, store_id, size));
    } else {
        let last = mem_dep_lock.store_mem.last_mut().unwrap();
        last.push((ptr_u64, original_gep_u64, store_id, gep_id));
    }
}

#[allow(unused)]
#[no_mangle]
pub extern "C" fn __mem_analysis_pointer_gep(address: *const libc::c_char,
                                             gep_id: u32) {
    if unsafe { SHM.is_null() } {
        return;
    }
    if address.is_null() {
        return;
    }
    if unsafe { VISIT_TID } == 0 {
        return;
    }
    let current_tid = get_tid();
    if unsafe { VISIT_TID } != current_tid {
        return;
    }
    let address_u64 = address as u64;
    let mut mem_dep_lock = MEM_DEP.lock().unwrap();
    let last = mem_dep_lock.gep_mem.last_mut().unwrap();
    last.push((address_u64, gep_id));
}

#[allow(unused)]
#[no_mangle]
pub extern "C" fn __mem_analysis_func_entry(func_name_cstr: *const libc::c_char) {
    if unsafe { IS_BLACKLIST_PROCESS } {
        return;
    }
    if unsafe{SHM.is_null()} && env::var_os("MEM_ANALYSIS_SHM_DIR").is_none() {
        return;
    }

    let func_name = String::from(unsafe { ffi::CStr::from_ptr(func_name_cstr) }
        .to_str().unwrap());
    let func_name_clone = func_name.clone();

    if CURRENT_FUNC.read().unwrap().is_some() {
        unsafe { LOGGER.as_mut().unwrap().printerr(&format!("re-entry: {:?}, {:?}", CURRENT_FUNC.read().unwrap(), func_name)); }
        // analyzer_error!("re-entry: {:?}, {:?}", CURRENT_FUNC.read().unwrap(), func_name);
        return;
    }

    if unsafe { SHM.is_null() } {
        let res = initialization();
        if !res {
            // cannot be initialize
            return;
        }
    }

    // modify the VISIT_TID
    unsafe { VISIT_TID = get_tid() };

    // modify the CURRENT_FUNC
    let mut func_write_lock = CURRENT_FUNC.write().unwrap();
    *func_write_lock = Some(func_name);
    drop(func_write_lock);

    // add func_name to MEM_DEP
    let mut mem_dep_lock = MEM_DEP.lock().unwrap();
    mem_dep_lock.add_new_func(func_name_clone);
}

#[allow(unused)]
#[no_mangle]
pub extern "C" fn __mem_analysis_func_return(func_name_cstr: *const libc::c_char) {
    if unsafe { IS_BLACKLIST_PROCESS } {
        return;
    }
    if unsafe { SHM.is_null() } {
        return;
    }
    let current_name_opt = CURRENT_FUNC.read().unwrap();
    if current_name_opt.is_none() {
        return;
    }
    let current_name = current_name_opt.as_ref().unwrap();
    let func_name = String::from(unsafe { ffi::CStr::from_ptr(func_name_cstr) }
        .to_str().unwrap());
    if !current_name.eq(&func_name) {
        return;
    }
    let current_tid = get_tid();
    if unsafe { VISIT_TID } != current_tid {
        return;
    }
    serialize_dependency_to_shm(true);
    let mut current_func = CURRENT_FUNC.write().unwrap();
    *current_func = None;
    drop(current_func);
    unsafe { VISIT_TID = 0 };
}

fn initialization() -> bool {
    unsafe { LOGGER = Some(Logger::new()) };
    if is_black_list(unsafe { LOGGER.as_mut().unwrap() }) {
        unsafe {
            IS_BLACKLIST_PROCESS = true;
        }
        return false;
    }
    if !unsafe { SHM.is_null() } {
        return true;
    }

    // initialize SHM file
    let shm_dir_str = env::var_os("MEM_ANALYSIS_SHM_DIR").unwrap();
    let dir_path = Path::new(&shm_dir_str);
    let file_name = get_process_name();
    let pid = get_pid();
    if !dir_path.exists() {
        create_dir_all(dir_path).unwrap();
    };
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
    let mmap = unsafe { MmapOptions::new().map_mut(&shm_file) }.unwrap();
    unsafe { SHM = mmap.as_ptr() as *mut u8 };
    unsafe { MMAP = Some(mmap) };
    unsafe {
        RECORD_TYPE_NAME = match env::var_os("RECORD_TYPE_NAME") {
            Some(r) => {
                if r == "true" {
                    true
                } else {
                    false
                }
            }
            None => false
        };
    }
    serialize_dependency_to_shm(false);
    connect_to_server(&shm_file_path);

    if env::var_os("ENABLE_RULE_MAP").is_some() {
        if let Some(rule_seq_path_osstr) = env::var_os("RULE_SEQ_PATH") {
            let rule_seq_path = PathBuf::from(rule_seq_path_osstr);
            cp_rule_seq_map(&rule_seq_path, &shm_file_path);
        }
    }
    if env::var_os("DISABLE_MINI_COLLECTION").is_some() {
        unsafe {
            MINI_COLLECTION = false;
        }
    }
    return true;
}

fn cp_rule_seq_map(rule_seq_path: &PathBuf, shm_file_path: &PathBuf) {
    let mut target_file_path = shm_file_path.clone().into_os_string().into_string().unwrap() + ".seq";
    let copy_res = copy(rule_seq_path.clone(), target_file_path.clone());
    if copy_res.is_err() {
        unsafe { LOGGER.as_mut().unwrap().printinfo(&format!("cannot copy: {:?} {:?} {:?}", rule_seq_path, target_file_path, copy_res)) };
        // analyzer_print!("cannot copy: {:?} {:?} {:?}", rule_seq_path, target_file_path, copy_res);
    }
}

fn connect_to_server(shm_file_path: &PathBuf) {
    match env::var_os("MEM_REGISTRATION_ADDR") {
        Some(reg_addr) => {
            let addr = reg_addr.into_string().unwrap();
            let file_name = get_process_name();
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
                        path: shm_file_path.clone(),
                        id: session_id,
                        filename: file_name,
                    };
                    let buf = bincode::serialize(&register_info).unwrap();
                    tcp_stream.write_all(&buf)
                        .expect("cannot write to the other side");
                    unsafe { LOGGER.as_mut().unwrap().printinfo(&format!("connected to {:?}", addr)); }
                    // analyzer_print!("connected to {:?}", addr);
                    Some(tcp_stream)
                }
                Err(e) => {
                    unsafe { LOGGER.as_mut().unwrap().printerr(&format!("cannot connect to {:?}, reason: {:?}", addr, e)); }
                    // analyzer_error!("cannot connect to {:?}, reason: {:?}", addr, e);
                    None
                }
            };
            unsafe {
                CONNECTOR = connector;
            }
        }
        None => {
            unsafe { LOGGER.as_mut().unwrap().printerr("no REGISTRATION_ADDR in env variables!"); }
            // analyzer_error!("no REGISTRATION_ADDR in env variables!");
        }
    };
}

fn serialize_dependency_to_shm(need_dedup_last: bool) {
    set_shm_lock();
    let mut mem = MEM_DEP.lock().unwrap();
    if need_dedup_last {
        let last_index = mem.call_seq.len() - 1;
        mem.dedup_mem(last_index);
    }

    let buf = bincode::serialize(&*mem).unwrap();
    if buf.len() > MAXSIZE {
        unsafe { LOGGER.as_mut().unwrap().printerr(&format!("on my god, buffer ({} bytes) exceeds MAXSIZE {}", buf.len(), MAXSIZE)); }
        // analyzer_error!("on my god, buffer ({} bytes) exceeds MAXSIZE {}", buf.len(), MAXSIZE);
        exit(0);
    }
    unsafe {
        let mut p = SHM;
        for i in 0..buf.len() {
            *p = buf[i];
            p = p.add(1);
        }
    }
    unset_shm_lock();
}

fn set_shm_lock() {
    while !CAN_WRITE_SHM_LOCK.load(Ordering::SeqCst) {
        spin_loop();
    }
    CAN_WRITE_SHM_LOCK.store(false, Ordering::SeqCst);
}

fn unset_shm_lock() {
    CAN_WRITE_SHM_LOCK.store(true, Ordering::SeqCst);
}

