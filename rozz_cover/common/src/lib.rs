use serde::{Serialize, Deserialize};
use std::process;
use std::env;
use std::os::raw::c_void;
use std::borrow::BorrowMut;
use std::ffi;
use std::ptr::{null, null_mut};
use std::path::PathBuf;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use colored::Colorize;
use std::io::{Write, Read};

pub type SessionId = u64;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RegisterMsgInfo {
    pub id: SessionId,
    pub path: PathBuf,
    pub filename: String,
}

pub type InstrPointerAddress = u64;
pub type GEPPointerAddress = u64;
pub type InstrId = u32;
pub type GEPId = u32;
pub type TypeSize = u32;

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct MemDependency {
    pub call_seq: Vec<String>,
    pub load_mem: Vec<Vec<(InstrPointerAddress, GEPPointerAddress, InstrId, GEPId)>>,
    pub store_mem: Vec<Vec<(InstrPointerAddress, GEPPointerAddress, InstrId, GEPId)>>,
    pub gep_mem: Vec<Vec<(GEPPointerAddress, GEPId)>>,
    pub mini_load_mem: Vec<Vec<(InstrPointerAddress, InstrId, TypeSize)>>,
    pub mini_store_mem: Vec<Vec<(InstrPointerAddress, InstrId, TypeSize)>>,
}

#[allow(dead_code)]
impl MemDependency {
    pub fn add_new_func(&mut self, func_name: String) {
        self.call_seq.push(func_name);
        self.store_mem.push(Default::default());
        self.load_mem.push(Default::default());
        self.gep_mem.push(Default::default());
        self.mini_load_mem.push(Default::default());
        self.mini_store_mem.push(Default::default());
    }
    pub fn dedup_mem(&mut self, i: usize) {
        if let Some(l) = self.load_mem.get_mut(i) {
            l.sort_by(|(ptr1, gep1, _, _), (ptr2, gep2, _, _)| {
                (ptr1, gep1).cmp(&(ptr2, gep2))
            });
            l.dedup_by(|(ptr1, gep1, _, _), (ptr2, gep2, _, _)| {
                (ptr1, gep1).eq(&(ptr2, gep2))
            });
        }
        if let Some(s) = self.store_mem.get_mut(i) {
            s.sort_by(|(ptr1, gep1, _, _), (ptr2, gep2, _, _)| {
                (ptr1, gep1).cmp(&(ptr2, gep2))
            });
            s.dedup_by(|(ptr1, gep1, _, _), (ptr2, gep2, _, _)| {
                (ptr1, gep1).eq(&(ptr2, gep2))
            });
        }
        if let Some(ms) = self.gep_mem.get_mut(i) {
            ms.sort_by(|(gep1, _), (gep2, _)| {
                gep1.cmp(gep2)
            });
            ms.dedup_by(|(gep1, _), (gep2, _)| {
                gep1.eq(&gep2)
            });
        }
        if let Some(mini_ls) = self.mini_load_mem.get_mut(i) {
            mini_ls.sort_by(|(ptr1, _, size1), (ptr2, _, size2)| {
                (ptr1, size1).cmp(&(ptr2, size2))
            });
            mini_ls.dedup_by(|(ptr1, _, size1), (ptr2, _, size2)| {
                (ptr1, size1).eq(&(ptr2, size2))
            });
        }
        if let Some(mini_ss) = self.mini_store_mem.get_mut(i) {
            mini_ss.sort_by(|(ptr1, _, size1), (ptr2, _, size2)| {
                (ptr1, size1).cmp(&(ptr2, size2))
            });
            mini_ss.dedup_by(|(ptr1, _, size1), (ptr2, _, size2)| {
                (ptr1, size1).eq(&(ptr2, size2))
            });
        }
    }
    pub fn dedup_all_mem(&mut self) {
        // assert_eq!(self.call_seq.len(), self.load_mem.len());
        // assert_eq!(self.call_seq.len(), self.store_mem.len());
        // assert_eq!(self.call_seq.len(), self.gep_mem.len());
        for i in 0..self.call_seq.len() {
            self.dedup_mem(i);
        }
    }
}

#[allow(dead_code)]
pub fn get_pid() -> u64 {
    process::id() as u64
}

#[allow(dead_code)]
pub fn get_tid() -> u64 {
    return unsafe { libc::pthread_self() };
    // (unsafe { libc::syscall(libc::SYS_gettid) }) as u64
}

#[allow(dead_code)]
pub fn get_process_name() -> String {
    String::from(env::current_exe().unwrap()
        .iter().last().unwrap().to_str().unwrap())
}

#[allow(dead_code)]
pub fn get_dso_name(any_pointer: *const c_void) -> String {
    let mut info: libc::Dl_info = libc::Dl_info {
        dli_fname: null(),
        dli_fbase: null_mut(),
        dli_saddr: null_mut(),
        dli_sname: null(),
    };
    let return_code = unsafe {
        let addr = any_pointer as _;
        libc::dladdr(addr,
                     info.borrow_mut())
    };
    let name = if return_code == 0 {
        get_process_name()
    } else {
        unsafe { ffi::CStr::from_ptr(info.dli_fname) }
            .to_str().unwrap().to_string()
    };
    return extract_filename_from_path(name);
}

#[allow(dead_code)]
pub fn extract_filename_from_path(path: String) -> String {
    let x: Vec<&str> = path.split("/").collect();
    x.last().unwrap().to_string()
}


#[macro_export]
macro_rules! analyzer_print {
    ($($arg:tt)*) => ({
        let pid = get_pid();
        print!("{} {}", format!("[{}]", pid).green().bold(), "[analyzer info]: ".green().bold());
        println!($($arg)*);
    });
}

#[macro_export]
macro_rules! analyzer_error {
    ($($arg:tt)*) => ({
        let pid = get_pid();
        print!("{} {}", format!("[{}]", pid).red().bold(), "[analyzer error]: ".red().bold());
        println!($($arg)*);
    });
}

#[derive(Debug, Default)]
pub struct Logger {
    log: Option<File>,
    mute: bool,
}

impl Logger {
    pub fn new() -> Self {
        let mut log = Logger::default();
        log.mute = false;
        match env::var_os("PRINT_ANALYSIS_LOG") {
            Some(l) => {
                if l == "true" || l == "std" {
                    log.mute = false;
                    log.log = None;
                } else if l == "mute" {
                    log.mute = true;
                    log.log = None;
                } else {
                    match OpenOptions::new().read(true).write(true)
                        .create(true).append(true).open(&l) {
                        Ok(f) => {
                            log.mute = false;
                            log.log = Some(f);
                        }
                        Err(e) => {
                            analyzer_print!("cannot open {:?}: {:?}", l, e);
                            log.mute = true;
                        }
                    }
                }
            }
            None => {
                log.mute = true;
            }
        };
        return log;
    }
    pub fn printinfo(&mut self, msg: &str) {
        if self.mute {
            return;
        }
        if let Some(f) = &mut self.log {
            let pid = get_pid();
            let write_res = writeln!(f, "[{}]analyzer info: {}", pid, msg);
        } else {
            analyzer_print!("{}", msg);
        }
    }
    pub fn printerr(&mut self, msg: &str) {
        if self.mute {
            return;
        }
        if let Some(f) = &mut self.log {
            let pid = get_pid();
            let write_res = writeln!(f, "[{}]analyzer info: {}", pid, msg);
        } else {
            analyzer_error!("{}", msg);
        }
    }
}


pub fn is_black_list(LOGGER: &mut Logger) -> bool {
    let mut args: Vec<String> = vec![];
    if let Ok(mut f) = OpenOptions::new().read(true).open("/proc/self/cmdline")
    {
        let mut args_str = String::new();
        f.read_to_string(&mut args_str).unwrap();
        for var in args_str.split("\0") {
            args.push(var.to_owned());
        }
    } else {
        for arg in env::args() {
            args.push(arg.clone());
        }
    }

    if args.len() == 0 {
        return true;
    }
    LOGGER.printinfo(&format!("{:?}", args));
    if args[0] == "/proc/self/exe" {
        for arg in args {
            if arg.contains("--type=zygote") || arg.contains("--type=renderer") {
                return false;
            }
        }
        return true;
    }
    // if args[0].contains("firefox") {
    //     for arg in args {
    //         if arg.contains("contentproc") {
    //             return false;
    //         }
    //     }
    //     return true;
    // }
    if args[0].contains("chrome") {
        for arg in args {
            if arg.contains("--type=zygote") || arg.contains("--type=renderer") {
                return false;
            }
        }
        return true;
    }
    return false;
}