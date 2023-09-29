use csv;
use defs::*;
use feryr::*;
use feryr_prog::corpus_handle::prog::Prog;
use std::{
    cmp::min,
    env, fs,
    io::{Error, ErrorKind},
    sync::{
        atomic::{Ordering::*, *},
        Arc, RwLock,
    },
    thread, time,
};
use util::fuzzer_info;

fn main() -> Result<(), failure::Error> {
    ctrlc::set_handler(move || {
        // kill process
        RUNNING.store(false, Ordering::SeqCst);
        fuzzer_info!("Receive Ctrl C, wait...");
        quit_fuzzer();
        std::process::exit(0);
    })
    .expect("Error setting Ctrl-C handler");

    parse_args();

    println!(
        "
            ███████╗███████╗██████╗ ██╗   ██╗██████╗ 
            ██╔════╝██╔════╝██╔══██╗╚██╗ ██╔╝██╔══██╗
            █████╗  █████╗  ██████╔╝ ╚████╔╝ ██████╔╝
            ██╔══╝  ██╔══╝  ██╔══██╗  ╚██╔╝  ██╔══██╗
            ██║     ███████╗██║  ██║   ██║   ██║  ██║
            ╚═╝     ╚══════╝╚═╝  ╚═╝   ╚═╝   ╚═╝  ╚═╝"
    );

    // parse input args
    let ros_dir_path: String;
    let config_file_path: String;
    let input_type: String;
    let input_args: String;
    let output_path: String;
    let args: Vec<String> = env::args().collect();
    match args.len() {
        11 => {
            ros_dir_path = args[2].to_string();
            config_file_path = args[4].to_string();
            input_type = args[6].to_string();
            input_args = args[8].to_string();
            output_path = args[10].to_string();
        }
        _ => {
            usage_help();
            return Err(Error::new(ErrorKind::Other, "wrong arguments").into());
        }
    }
    // init Manager to perserve runtime status
    fuzzer_info!("init fuzz manager");
    let fuzz_manager = feryr::FuzzManager::new(
        ros_dir_path,
        config_file_path,
        output_path,
        input_type,
        input_args,
    );

    let fuzz_manager = Arc::new(RwLock::new(fuzz_manager));
    let status_manager = Arc::clone(&fuzz_manager);

    match fuzz_manager.write() {
        Ok(mut handle) => {
            // init ros evn
            handle.init_ros_env();
            fuzzer_info!("work dir is {}", &handle.workdir);
            // generate ros target
            fuzzer_info!("generate targets");
            handle.gen_targets().unwrap();
        }
        Err(e) => {
            println!("Error: {:?}", e);
        }
    }

    // TODO bug here, this code section can not execute on time
    thread::spawn(move || -> Result<(), failure::Error> {
        fuzzer_info!("Let's roll!");
        let mut total_branch = 0;
        loop {
            thread::sleep(time::Duration::from_millis(10000));
            match status_manager.read() {
                Ok(handle) => {
                    dbg!("fuck here");
                    handle.output_log(&mut total_branch).unwrap();
                }
                Err(_e) => {
                    RUNNING.store(false, SeqCst);
                }
            }
        }
    });

    fuzzer_info!("{:?} is running", thread::current().id());

    // start fuzz loop
    fuzz_manager.write().unwrap().reboot().unwrap();
    fuzz_loop(fuzz_manager);

    Ok(())
}

// fuzzing loop start here
pub fn fuzz_loop(fuzz_manager: Arc<RwLock<FuzzManager>>) {
    // set generation rules
    const GENERATE_PERIOD: u32 = 50;

    let mut idx = 0;
    // start fuzzing loop
    while RUNNING.load(Ordering::SeqCst) {
        if idx == 30 {
            break;
        }
        // idx += 1;

        // write to manager
        match fuzz_manager.write() {
            Ok(mut handle) => {
                // generate input for node
                fuzzer_info!("generating prog ");
                let current_prog = match Prog::get_call(&handle.ros_launch, GENERATE_PERIOD) {
                    Ok(prog) => prog,
                    Err(e) => {
                        fuzzer_info!("failed to generate prog: {}", e);
                        handle.reboot().unwrap();
                        continue;
                    }
                };

                handle.last_exec = handle.last_exec + 1;
                handle.total_exec = handle.total_exec + 1;

                // execute seeds
                let work_dir = &mut handle.workdir.clone();
                // let pid = &mut handle.fuzzing_inst.id();
                handle.ros_launch.add_prog(current_prog.clone());
                handle.ros_launch.pid = handle.fuzzing_inst.id().clone();
                if let Err(e) = current_prog.exec_input_prog(work_dir, &mut handle.ros_launch) {
                    fuzzer_info!("getting crash: {}", e);
                    if !format!("{}", e).contains("ros2 log error")
                        && !format!("{}", e).contains("ros2 waiting for")
                    {
                        // generate a random string
                        handle.save_crash(&format!("{}", e));
                    }

                    // TODO adding repro here
                    handle.repro();

                    // reboot
                    handle.reboot().unwrap();
                    handle.ros_launch.clean_prog();
                } else {
                    // normal exec, handle coverage
                    let _file_name = format!("{}/{}", work_dir, "corpus.db");
                }
            }
            Err(_e) => {
                fuzzer_info!("failed to write to manager!");
                RUNNING.store(false, SeqCst);
            }
        }
    }

    // create a directory name csv in work_dir
    let work_dir = fuzz_manager.read().unwrap().workdir.clone() + &"/csv".to_owned();
    fs::create_dir_all(work_dir.clone()).unwrap();
    let mut idx = 0;
    for trace in fuzz_manager
        .read()
        .unwrap()
        .ros_launch
        .call_graph
        .event_trace
        .iter()
    {
        if trace.1.trace.len() == 0 {
            continue;
        }
        let mut wtr = csv::Writer::from_path(
            work_dir.clone() + &"/data".to_owned() + &idx.to_string() + &".csv".to_owned(),
        )
        .unwrap();
        idx += 1;
        wtr.write_record(&["trace_id", "cb_id", "start", "end", "cb_name"])
            .unwrap();
        for cb in trace.1.trace.iter() {
            let len = min(cb.1.start_time.len(), cb.1.end_time.len());
            for i in 0..len {
                // print as following order: trace.id, callback.id, callback.start_time, callback.end_time
                if cb.1.cb_name == "" {
                    println!(
                        "{}, {}, {}, {}",
                        trace.0, cb.0, cb.1.start_time[i], cb.1.end_time[i]
                    );
                    wtr.write_record(&[
                        trace.0.to_string(),
                        cb.0.to_string(),
                        cb.1.start_time[i].to_string(),
                        cb.1.end_time[i].to_string(),
                        " ".to_string(),
                    ])
                    .unwrap();
                } else {
                    println!(
                        "{}, {}, {}, {}",
                        trace.0, cb.1.cb_name, cb.1.start_time[i], cb.1.end_time[i]
                    );
                    wtr.write_record(&[
                        trace.0.to_string(),
                        cb.0.to_string(),
                        cb.1.start_time[i].to_string(),
                        cb.1.end_time[i].to_string(),
                        cb.1.cb_name.to_string(),
                    ])
                    .unwrap();
                }
            }
        }
        wtr.flush().unwrap();
    }
    println!("{:?} is ended", thread::current());
}
