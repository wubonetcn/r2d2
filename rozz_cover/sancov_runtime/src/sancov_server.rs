extern crate tokio;
extern crate clap;
extern crate common;

mod utils;

use std::collections::BTreeMap;
use chrono;
use colored::Colorize;
use common::{analyzer_print};
use common::{get_pid};
use common::RegisterMsgInfo;
use utils::{CovCollector, Header};
use utils::get_cov_offset_pointer;
use utils::{UpdateFunc, Filter};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt};

use clap::{Arg, App};
use std::net::SocketAddr;
use std::ops::Range;
use std::path::PathBuf;
use std::time::{Duration};
use crate::utils::Statistics;

fn filer_for_normal_lib(_header: &Header, _register_info: &RegisterMsgInfo) -> bool {
    return true;
}

// if allow_list is empty, it means all name is allowed
fn update_index_with_allow_list(_register_info: &RegisterMsgInfo,
                                header: &Header,
                                statistics: &mut Statistics,
                                shm_ptr: *mut u8, allow_list: &Vec<String>) -> Vec<usize> {
    let mut update_indexes = vec![];

    for (name, range_index) in header.map.iter() {
        if !allow_list.is_empty() {
            let mut allow = false;
            for allow_name in allow_list {
                if name.contains(allow_name) {
                    allow = true;
                    break;
                }
            }
            if !allow {
                continue;
            }
        }


        let (start_index, length) = header.vec[*range_index].clone();

        let my_cov_range = if statistics.cov_map.contains_key(name) {
            statistics.cov_map.get(name).unwrap().clone()
        } else {
            let statistics_cov_start_point = statistics.cov_range.end;
            let statistics_cov_range = Range { start: statistics_cov_start_point, end: statistics_cov_start_point + length };
            statistics.cov_range.end = statistics_cov_start_point + length;
            statistics.cov_map.insert(name.clone(), statistics_cov_range.clone());
            statistics_cov_range
        };

        let mut shm_current_ptr = unsafe { get_cov_offset_pointer(shm_ptr, start_index) }
            as *const u8;

        for index in my_cov_range {
            if unsafe { *shm_current_ptr != 0 } && statistics.cov[index] == 0 {
                update_indexes.push(index);
            }
            shm_current_ptr = unsafe { shm_current_ptr.add(1) };
        }
    }

    return update_indexes;
}

fn update_index_for_normal_lib(register_info: &RegisterMsgInfo,
                               header: &Header,
                               statistics: &mut Statistics,
                               shm_ptr: *mut u8) -> Vec<usize> {
    return update_index_with_allow_list(register_info, header, statistics, shm_ptr, &vec![]);
}

fn filer_for_webkit(_header: &Header, register_info: &RegisterMsgInfo) -> bool {
    // if register_info.filename != "WebKitWebProcess" {
    //     return false;
    // }

    // for var in header.map.keys() {
    //     if var.contains("libwebkit2gtk-") {
    //         return true;
    //     }
    // }
    return true;
}

fn filer_for_firefox(header: &Header, _register_info: &RegisterMsgInfo) -> bool {
    for var in header.map.keys() {
        if var.contains("libxul") {
            return true;
        }
    }
    return false;
}

fn filer_for_chrome(header: &Header, _register_info: &RegisterMsgInfo) -> bool {
    for var in header.map.keys() {
        if var.contains("libblink_modules") || var.contains("libblink_core") || var.contains("libcontent") {
            return true;
        }
    }
    return false;
}

fn update_index_for_webkit(register_info: &RegisterMsgInfo,
                           header: &Header,
                           statistics: &mut Statistics,
                           shm_ptr: *mut u8) -> Vec<usize> {
    return update_index_with_allow_list(register_info, header, statistics, shm_ptr, &vec!["libwebkit2gtk-".to_string()]);
    // let cov_range = header.get_cov_range_of_dso("libwebkit2gtk-").unwrap();
    // let mut update_indexes = vec![];
    // let mut ptr =
    //     unsafe { get_cov_offset_pointer(shm_ptr, cov_range.start) }
    //         as *const u8;
    // for index in cov_range {
    //     if unsafe { *ptr != 0 } && cov[index] == 0 {
    //         update_indexes.push(index);
    //     }
    //     ptr = unsafe { ptr.add(1) };
    // }
    // return update_indexes;
}

fn update_index_for_firefox(register_info: &RegisterMsgInfo,
                            header: &Header,
                            statistics: &mut Statistics,
                            shm_ptr: *mut u8) -> Vec<usize> {
    return update_index_for_normal_lib(register_info, header, statistics, shm_ptr);

    // let cov_range = header.get_cov_range_of_dso("libxul").unwrap();
    //
    // let mut update_indexes = vec![];
    // let mut ptr =
    //     unsafe { get_cov_offset_pointer(shm_ptr, cov_range.start) }
    //         as *const u8;
    // for index in cov_range {
    //     if unsafe { *ptr != 0 } && cov[index] == 0 {
    //         update_indexes.push(index);
    //     }
    //     ptr = unsafe { ptr.add(1) };
    // }
    // return update_indexes;
}

fn update_index_for_chrome(
    register_info: &RegisterMsgInfo,
    header: &Header,
    statistics: &mut Statistics,
    shm_ptr: *mut u8,
) -> Vec<usize> {
    return update_index_for_normal_lib(register_info, header, statistics, shm_ptr);

    // let mut update_indexes = vec![];
    // for (dso_name, dso_index) in &header.map {
    //     let cov_range = header.get_cov_range_of_dso(dso_name).unwrap();
    //     let mut ptr = unsafe { get_cov_offset_pointer(shm_ptr, cov_range.start) };
    //     for index in cov_range {
    //         if unsafe { *ptr != 0 } && cov[index] == 0 {
    //             update_indexes.push(index);
    //         }
    //         ptr = unsafe { ptr.add(1) };
    //     }
    // }

    // if let Some(cov_range) = header.get_cov_range_of_dso("libblink_modules") {
    //     let mut ptr = unsafe { get_cov_offset_pointer(shm_ptr, cov_range.start) } as *const u8;
    //     for index in cov_range {
    //         if unsafe { *ptr != 0 } && cov[index] == 0 {
    //             update_indexes.push(index);
    //         }
    //         ptr = unsafe { ptr.add(1) };
    //     }
    // }
    //
    // if let Some(cov_range_core) = header.get_cov_range_of_dso("libblink_core") {
    //     let mut ptr = unsafe { get_cov_offset_pointer(shm_ptr, cov_range_core.start) } as *const u8;
    //     for index in cov_range_core {
    //         if unsafe { *ptr != 0 } && cov[index] == 0 {
    //             update_indexes.push(index);
    //         }
    //         ptr = unsafe { ptr.add(1) };
    //     }
    // }
    // if let Some(cov_range_libcontent) = header.get_cov_range_of_dso("libcontent") {
    //     let mut ptr = unsafe { get_cov_offset_pointer(shm_ptr, cov_range_libcontent.start) } as *const u8;
    //     for index in cov_range_libcontent {
    //         if unsafe { *ptr != 0 } && cov[index] == 0 {
    //             update_indexes.push(index);
    //         }
    //         ptr = unsafe { ptr.add(1) };
    //     }
    // }
    // return update_indexes;
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = App::new("collect coverage tool")
        .version("1.0")
        .author("Chijin <tlock.chijin@gmail.com>")
        .about("listen to the target port and collect coverage")
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
        .arg(Arg::with_name("strategy_mode")
            .short("m")
            .long("mode")
            .value_name("strategy_mode")
            .help("different strategies for statistics. options: normal_lib/webkit")
            .required(true))
        .get_matches();
    let port = matches.value_of("port").unwrap();
    let output_path = matches.value_of("output_path").unwrap();
    let (filter, update_func): (Filter, UpdateFunc) =
        match matches.value_of("strategy_mode").unwrap() {
            "normal_lib" => (filer_for_normal_lib, update_index_for_normal_lib),
            "webkit" => (filer_for_webkit, update_index_for_webkit),
            "firefox" => (filer_for_firefox, update_index_for_firefox),
            "chrome" => (filer_for_chrome, update_index_for_chrome),
            _ => panic!("not this strategy mode")
        };

    let mut collector = CovCollector::default();
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
                            local_collector.on_socket_closed(&local_addr, filter, update_func);
                            break;
                        }
                    } else {
                        // socket closed or buffer is overflow
                        local_collector.on_socket_closed(&local_addr, filter, update_func);
                        break;
                    }
                }
            }
        });
    }
}