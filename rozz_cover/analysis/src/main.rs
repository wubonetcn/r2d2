extern crate common;
extern crate clap;
extern crate indicatif;
extern crate tokio;
extern crate memmap;

mod utils;

use utils::{Graph, MemDependencyList, MemAnalyzer};
use common::{analyzer_error, analyzer_print, MemDependency};
use common::get_pid;
use serde::{Serialize, Deserialize};
use bincode;
use clap::{Arg, App};
use colored::Colorize;
use std::fs;
use std::io::{Read, Write};
use serde_json;
use std::path::{PathBuf};
use std::collections::{HashSet, HashMap};
use std::iter::FromIterator;
use std::process::Command;
use indicatif::ProgressIterator;
use std::borrow::Borrow;
use std::fs::DirEntry;
use std::io;
use std::time::Duration;
use tokio::net::{TcpStream, TcpListener};
use std::net::SocketAddr;
use tokio::io::AsyncReadExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = App::new("analysis tool")
        .version("1.0")
        .author("Chijin <tlock.chijin@gmail.com>")
        .about("listen to the target port and analyze the shm")
        .arg(Arg::with_name("output")
            .short("o")
            .long("output")
            .value_name("output_path")
            .help("path of dependence json")
            .required(true))
        .arg(Arg::with_name("port")
            .short("p")
            .long("port")
            .value_name("port")
            .help("port to listen")
            .required(true))
        .arg(Arg::with_name("time")
            .short("t")
            .long("time")
            .value_name("time")
            .help("to to serialize to file system")
            .default_value("300"))
        .get_matches();
    let port = matches.value_of("port").unwrap();
    let output_path = matches.value_of("output").unwrap();
    let serialize_time: u64 = matches.value_of("time").unwrap().parse().unwrap();
    let mut analyzer = MemAnalyzer::default();
    analyzer.output_path = PathBuf::from(output_path);

    // background thread for serialize to filesystem at a certain interval
    tokio::spawn({
        let local_analyzer = analyzer.clone();
        async move {
            let mut interval = tokio::time::interval(Duration::from_secs(serialize_time));
            loop {
                interval.tick().await;
                let date = chrono::Local::now();
                let ok = local_analyzer.serialize_to_filesystem(false);
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
            let mut local_analyzer = analyzer.clone();
            let local_addr = addr;
            async move {
                let mut buf = [0u8; 256];
                loop {
                    let res: std::io::Result<usize> = tcp_stream.read(&mut buf).await;
                    if res.is_ok() {
                        local_analyzer.on_socket_connected(&local_addr, &buf);
                        if res.unwrap() == 0 {
                            // socket closed
                            local_analyzer.on_socket_closed(&local_addr);
                            break;
                        }
                    } else {
                        // socket closed
                        local_analyzer.on_socket_closed(&local_addr);
                        break;
                    }
                }
            }
        });
    }
}