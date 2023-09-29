use crate::{corpus_handle::models::OnnxModel, get_name_full, RE};

use super::{
    super::{get_name_short, string_hasher, EventType, ExecError, CHECK_LEN},
    callback::*,
    event_trace::CallTrace,
    node::*,
    timer_trace::TimerTrace,
    topic_trace::TopicTrace,
};
use std::{
    collections::{HashMap, HashSet},
    fs::OpenOptions,
    io::Write,
    str::{self, FromStr},
    sync::{Arc, Mutex},
};
use util::shmem::*;

#[derive(Debug)]
pub struct CallGraph {
    pub nodes: HashMap<u64, NodeInfo>,
    // using rcl_handle as identifier
    pub callbacks: HashMap<u64, CallbackInfo>,

    pub current_times: Vec<cb_times>,
    pub current_msgs: Vec<shared_msg>,

    pub event_trace: HashMap<i128, CallTrace>,
    pub current_event_trace: CallTrace,

    pub timer_trace: HashMap<u64, TimerTrace>, // callback id, all invoke-start pairs
    pub current_timer_trace: HashMap<u64, TimerTrace>,

    pub topic_trace: HashMap<u64, TopicTrace>,
    pub current_topic_trace: HashMap<u64, TopicTrace>,
}
impl CallGraph {
    pub fn new() -> CallGraph {
        CallGraph {
            nodes: HashMap::new(),
            callbacks: HashMap::new(),
            current_times: Vec::new(),
            current_msgs: Vec::new(),

            event_trace: HashMap::new(),
            current_event_trace: CallTrace::new(),
            timer_trace: HashMap::new(),
            current_timer_trace: HashMap::new(),
            topic_trace: HashMap::new(),
            current_topic_trace: HashMap::new(),
        }
    }

    pub fn update_callback_info(&mut self, shmem_region: &mut SharedMem) {
        // update node from shmem_region
        for node in shmem_region.get_shm_nodes() {
            if node.handle == 0 {
                continue;
            }
            let id = string_hasher(&get_name_short(&node.name));
            if !self.is_node_exist(&id) {
                // if dont exist, then add new node
                let node_info = NodeInfo::new(id, &node);
                self.nodes.insert(node_info.id, node_info);
            } else {
                // if node is exist, then update its information
                let node_info = self.get_node_mut(&id).unwrap();
                node_info.set_handle(node.handle);
                node_info.set_pid(node.pid);
            }
        }
        // update callbacks from shmem_region
        for cb in shmem_region.get_shm_callback_infos() {
            if cb.rcl_handle == 0 || cb.node_handle == 0 || cb.cb_type == 0 {
                continue;
            }
            match self.get_node_via_handle_mut(&cb.node_handle) {
                Some(node) => {
                    let name = &node.node_name.clone();
                    self.reset_cb_val(name, &cb);
                }
                None => {
                    self.reset_cb_val(&"ros2cli".to_string(), &cb);
                }
            }
        }
    }

    pub fn reset_cb_val(&mut self, node_name: &String, cb: &callback_infos) {
        let id = string_hasher(
            &(node_name.clone()
                + &get_name_short(&cb.cb_name)
                + &get_name_full(&cb.function_symbol)),
        ) + cb.cb_type;
        if !self.is_cb_exist(&id) {
            // if dont exist, then add new callback
            let mut callback = CallbackInfo::new(&cb, &id);
            callback.set_node_name(node_name.clone());
            self.callbacks.insert(callback.id, callback);
        } else {
            // if callback is exist, then update its information
            let callback = self.get_callback_mut(&id).unwrap();
            // I dont know why the fuck ros may have two identical callback for a single node
            if callback.pid == cb.pid {
                let callback = CallbackInfo::new(&cb, &id);
                self.callbacks.insert(callback.id, callback);
            } else {
                callback.set_rcl_handle(cb.rcl_handle);
                callback.set_rclcpp_handle(cb.rclcpp_handle);
                callback.set_rclcpp_handle1(cb.rclcpp_handle1);
                callback.set_rmw_handle(cb.rmw_handle);
                callback.set_period(cb.period);
                callback.set_node_handle(cb.node_handle);
                callback.set_pid(cb.pid);
            }
        }
    }

    pub fn get_start_time(&self, output: &String) -> u64 {
        match RE.captures_iter(output.as_bytes()).last() {
            Some(m) => u64::from_str(str::from_utf8(&m[1]).unwrap()).unwrap(),
            None => 0,
        }
    }

    pub fn trim_times(
        &mut self,
        shmem_region: &mut SharedMem,
        start_time: u128,
    ) -> Result<(), failure::Error> {
        self.current_times = Vec::new();
        let mut start_push = false;
        for time in shmem_region.get_shm_cb_time().iter().rev() {
            if time.cb == 0 || time.flag == 0 || (time.time as u128) < start_time {
                continue;
            } else {
                if start_push == true && (time.time as u128) >= start_time {
                    self.current_times.push(*time);
                } else {
                    match EventType::from(time.flag).unwrap() {
                        EventType::CbEnd
                        | EventType::RclSub
                        | EventType::SrvReq
                        | EventType::CliRsp
                        | EventType::ExeExe
                        | EventType::ExeRdy => {
                            start_push = true;
                            self.current_times.push(*time);
                        }
                        EventType::CbStart
                        | EventType::RclPub
                        | EventType::SrvRsp
                        | EventType::CliReq => {
                            continue;
                        }
                    }
                }
            }
        }

        self.current_msgs = Vec::new();
        for time in shmem_region.get_shm_msg().iter().rev() {
            if time.callback == 0
                || time.subscription == 0
                || time.send_time == 0
                || time.recv_time == 0
                || time.size > 2048000000
                || time.recv_time < time.send_time
            {
                continue;
            } else {
                if (time.send_time as u128) >= start_time {
                    self.current_msgs.push(*time);
                }
            }
        }

        Ok(())
    }

    // display callbacks after the start time
    pub fn display_cur_cbs(&mut self) {
        let mut cb_set: HashSet<CallbackInfo> = HashSet::new();
        for time in self.current_times.iter().rev() {
            // get corresponding callback
            let cb_id = match self.get_callback_id_via_time(time) {
                Some(cb_id) => cb_id,
                None => {
                    // TODO: here we may missing some callback, there are callback indeed not know where the hell they coming from
                    continue;
                }
            };
            let callback = self.get_callback_by_cb_id(&cb_id).unwrap();
            // add callback to cb_set
            cb_set.insert(callback.clone());
        }
        dbg!(cb_set);
    }

    pub fn display_cur_msg(&mut self) {
        let mut time_set: HashSet<CallbackInfo> = HashSet::new();
        for time in self.current_msgs.iter().rev() {
            let id = time.callback;
            let callback = self.get_callback_by_handle(&id).unwrap();
            time_set.insert(callback.clone());
        }

        dbg!(time_set);
    }

    // display callbacks after the start time
    pub fn display_cbs(&mut self, shmem_region: &mut SharedMem) {
        // dbg!(shmem_region.get_shm_cb_time());
        let mut cb_set: HashSet<CallbackInfo> = HashSet::new();
        for time in shmem_region.get_shm_cb_time().iter().rev() {
            // get corresponding callback
            let cb_id = match self.get_callback_id_via_time(time) {
                Some(cb_id) => cb_id,
                None => {
                    // TODO: here we may missing some callback, there are callback indeed not know where the hell they coming from
                    continue;
                }
            };
            let callback = self.get_callback_by_cb_id(&cb_id).unwrap();
            // add callback to cb_set
            cb_set.insert(callback.clone());
        }
        dbg!(cb_set);
    }

    pub fn display_msg(&mut self, shmem_region: &mut SharedMem) {
        dbg!(shmem_region.get_shm_msg());
        let mut time_set: HashSet<CallbackInfo> = HashSet::new();
        for time in shmem_region.get_shm_msg().iter().rev() {
            let id = time.callback;
            let callback = self.get_callback_by_handle(&id).unwrap();
            time_set.insert(callback.clone());
        }

        dbg!(time_set);
    }

    pub fn get_event_trace(&mut self, cb_id: u64, time: &cb_times) {
        match self
            .current_event_trace
            .get_callback_in_et_by_cb_id_mut(&cb_id)
        {
            Some(callback) => {
                callback.update_cb_start_end_time(time);
            }
            None => {
                // if callback do not exist in event trace
                let callback = self.callbacks.get_mut(&cb_id).unwrap();
                callback.update_cb_start_end_time(time);
                self.current_event_trace
                    .event_trace_add_new_callback(callback);
            }
        }
    }

    // pub fn update_timer_trace(
    //     &mut self,
    //     queue_size: &mut u64,
    //     invoke_queue: &mut HashMap<u64, VecDeque<(u64, u64)>>,
    //     cb_id: u64,
    //     time: &cb_times,
    // ) {
    //     let invoke_cb_reg_vec = match invoke_queue.get_mut(&cb_id) {
    //         Some(invoke_cb_reg_vec) => invoke_cb_reg_vec,
    //         None => {
    //             return;
    //         }
    //     };
    //     let pre_time = match invoke_cb_reg_vec.front() {
    //         Some(time) => time,
    //         None => {
    //             return;
    //         }
    //     };
    //     let duration = time.time - pre_time.0;
    //     invoke_cb_reg_vec.pop_front();

    //     match self.current_timer_trace.get_mut(&cb_id) {
    //         Some(duration_vec) => {
    //             duration_vec.push((duration, *queue_size));
    //         }
    //         None => {
    //             self.current_timer_trace
    //                 .insert(cb_id, vec![(duration, *queue_size)]);
    //         }
    //     };
    //     *queue_size -= 1;
    // }

    pub fn get_timer_trace(&mut self, cb_id: u64, event_type: EventType, time: &cb_times) {
        match self.current_timer_trace.get_mut(&cb_id) {
            Some(duration_vec) => match event_type {
                EventType::ExeRdy => {
                    duration_vec.sched_vec.push_back(time.time);
                }
                EventType::ExeExe => {
                    dbg!(&time);
                    duration_vec.queue_vec.push_back(time.message_size);
                }
                EventType::CbStart => {
                    if duration_vec.sched_vec.len() == 0 || duration_vec.queue_vec.len() == 0 {
                        return;
                    }
                    duration_vec.start_vec.push_back(time.time);
                    let sched_time = duration_vec.sched_vec.pop_front().unwrap();
                    let queue_size = duration_vec.queue_vec.pop_front().unwrap();
                    // dbg!(sched_time, queue_size);
                    duration_vec
                        .pairs
                        .push((time.time - sched_time, queue_size));
                }
                _ => {}
            },
            None => {
                let mut duration_vec = TimerTrace::new();
                match event_type {
                    EventType::ExeRdy => {
                        duration_vec.sched_vec.push_back(time.time);
                    }
                    EventType::ExeExe => {
                        dbg!(&time, cb_id);
                        duration_vec.queue_vec.push_back(time.message_size);
                    }
                    EventType::CbStart => {
                        if duration_vec.sched_vec.len() == 0 || duration_vec.queue_vec.len() == 0 {
                            return;
                        }
                        duration_vec.start_vec.push_back(time.time);
                        let sched_time = duration_vec.sched_vec.pop_front().unwrap();
                        let queue_size = duration_vec.queue_vec.pop_front().unwrap();
                        dbg!(sched_time, queue_size);
                        duration_vec
                            .pairs
                            .push((time.time - sched_time, queue_size));
                    }
                    _ => {}
                }
                self.current_timer_trace.insert(cb_id, duration_vec);
            }
        }
    }

    // update current calltrace based on shared memory
    pub fn get_call_trace(
        &mut self,
        start_time: u128,
        shmem_region: &mut SharedMem,
        shm_dir: &String,
        _output: &String,
    ) -> Result<(), failure::Error> {
        // update callback info
        shmem_region.mmap_load_info(&(shm_dir));
        self.update_callback_info(shmem_region);

        // get trimmed time events
        self.trim_times(shmem_region, start_time).unwrap();

        // NOTICE: this is debug only
        // self.display_cur_cbs();
        // self.display_cur_msg();
        self.display_cbs(shmem_region);
        // self.display_msg(shmem_region);

        // record important event
        self.current_event_trace = CallTrace::new();
        self.current_timer_trace = HashMap::new();
        self.current_topic_trace = HashMap::new();

        // update on msg
        for msg in self.current_msgs.clone().iter().rev() {
            let id = msg.callback ^ msg.subscription;
            if !self.current_topic_trace.contains_key(&id) {
                let mut topic = TopicTrace::new();
                topic.set_callback(msg.callback);
                topic.set_subscription(msg.subscription);
                topic.set_trace((msg.size, msg.recv_time - msg.send_time));
                self.current_topic_trace.insert(id, topic);
            } else {
                let topic = self.current_topic_trace.get_mut(&id).unwrap();
                topic.set_trace((msg.size, msg.recv_time - msg.send_time));
            }
        }

        let set: HashSet<cb_times> = self.current_times.clone().into_iter().collect();
        let mut vec: Vec<_> = set.into_iter().collect();
        vec.sort_by(|a, b| a.time.cmp(&b.time));
        self.current_times = vec;
        dbg!(&self.current_times);

        // update on executor and callback
        for time in self.current_times.clone().iter() {
            // get corresponding callback
            let cb_id = match self.get_callback_id_via_time(time) {
                Some(cb_id) => cb_id,
                None => {
                    // TODO: here may missing some callback, there are callback indeed not know where the hell they coming from
                    continue;
                }
            };

            let event_type = EventType::from(time.flag).unwrap();
            if event_type == EventType::CbStart
                || event_type == EventType::CbEnd
                || event_type == EventType::ExeRdy
            {
                // update event trace
                self.get_event_trace(cb_id, time);
            }

            if event_type == EventType::CbStart
                || event_type == EventType::ExeExe
                || event_type == EventType::ExeRdy
            {
                // update timer trace
                self.get_timer_trace(cb_id, event_type, time);
            }
        }
        for pair in &self.current_timer_trace.clone() {
            // if pair is empty
            if pair.1.pairs.len() == 0 {
                // remove the pari from self.current_timer_trace
                self.current_timer_trace.remove(pair.0);
            }
        }

        dbg!(&self.current_topic_trace);
        dbg!(&self.current_event_trace);
        dbg!(&self.current_timer_trace);
        Ok(())
    }

    pub fn event_monitor(
        &mut self,
        trace_model: &mut Arc<Mutex<OnnxModel>>,
    ) -> Result<bool, failure::Error> {
        // caculte overall latency
        // let input_data = {
        //     let mut trace_model_mutex = trace_model.lock().unwrap();
        //     trace_model_mutex.process_call_trace(&self.current_event_trace)
        // };

        // // Make predictions using the model
        // let predictions = {
        //     let mut trace_model_mutex = trace_model.lock().unwrap();
        //     trace_model_mutex.predict_hash_trace(&input_data)?
        // };

        // // Check for violations based on the predictions
        // let violations = {
        //     let mut trace_model_mutex = trace_model.lock().unwrap();
        //     trace_model_mutex.check_call_trace_violations(
        //         &self.current_event_trace,
        //         &predictions,
        //         3.0,
        //     )
        // };

        // Handle violations (e.g., log them, raise an error, etc.)
        // if !violations.is_empty() {
            
            // return Err(failure::Error::from(
            //     ExecError::TimeOutErrorWithViolations {
            //         predictions: predictions.clone(),
            //         violations: violations.clone(),
            //     },
            // ));
        // }

        let global_trace = match self.event_trace.get_mut(&self.current_event_trace.id) {
            Some(trace) => trace,
            None => {
                self.event_trace.insert(
                    self.current_event_trace.id,
                    self.current_event_trace.to_owned(),
                );
                return Ok(false);
            }
        };
        if self.current_event_trace.cur_latency > global_trace.max_time {
            global_trace.max_time = self.current_event_trace.cur_latency;
            global_trace.trace = self.current_event_trace.trace.to_owned();
            return Ok(true);
        } else {
            global_trace
                .time_set
                .insert(self.current_event_trace.cur_latency);
        }

        dbg!(&self.event_trace);
        return Ok(false);
    }

    pub fn timer_monitor(
        &mut self,
        executor_model: &mut Arc<Mutex<OnnxModel>>,
    ) -> Result<bool, failure::Error> {
        if self.current_timer_trace.len() == 0 {
            return Ok(false);
        }
        for (edge_id, edge_vec) in self.current_timer_trace.clone().iter_mut() {
            let global_trace = match self.timer_trace.get_mut(&edge_id) {
                Some(trace) => trace,
                None => {
                    // if not find the trace, insert it
                    self.timer_trace.insert(edge_id.clone(), edge_vec.clone());
                    self.timer_trace.get_mut(&edge_id).unwrap()
                }
            };

            // Process the input data
            // let input_data = edge_vec.pairs.clone();

            // // Make predictions using the model
            // let predictions = executor_model.lock().unwrap().predict_timer(&input_data)?;

            // // Check for violations based on the predictions
            // let violations = executor_model.lock().unwrap().check_timer_violation(
            //     &mut self.current_timer_trace,
            //     &predictions,
            //     3.0,
            // );

            // // Handle violations (e.g., log them, raise an error, etc.)
            // if !violations.is_empty() {
            //     // TODO: skip timeout check for the monment
            //     return Err(failure::Error::from(
            //         ExecError::TimeOutErrorWithViolations {
            //             predictions: predictions.clone(),
            //             violations: violations.clone(),
            //         },
            //     ));
            // }
            // update global topic trace
            let mut local_throughput: Vec<(f64, u64)> = Vec::new();
            for (duration, size) in edge_vec.pairs.iter() {
                local_throughput.push(((size / duration) as f64, *size));
            }

            let local_min = local_throughput
                .iter()
                .map(|&(time, _)| time)
                .min_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap();
            let local_max = local_throughput
                .iter()
                .map(|&(time, _)| time)
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap();
            if local_min < global_trace.min_time {
                global_trace.min_time = local_min.to_owned();
                global_trace.update_trace_info(edge_vec);
                return Ok(true);
            }
            if local_max > global_trace.max_time {
                global_trace.max_time = local_max.to_owned();
                global_trace.update_trace_info(edge_vec);
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn topic_monitor(
        &mut self,
        topic_model: &mut Arc<Mutex<OnnxModel>>,
    ) -> Result<bool, failure::Error> {
        if self.current_topic_trace.len() == 0 {
            return Ok(false);
        }
        for (edge_id, edge_vec) in self.current_topic_trace.clone().iter_mut() {
            let global_trace = match self.topic_trace.get_mut(&edge_id) {
                Some(trace) => trace,
                None => {
                    // if not find the trace, insert it
                    self.topic_trace.insert(edge_id.clone(), edge_vec.clone());
                    self.topic_trace.get_mut(&edge_id).unwrap()
                }
            };

            // Process the input data
            // let input_data = edge_vec.trace.clone();

            // // Make predictions using the model
            // let predictions = topic_model.lock().unwrap().predict_topic(&input_data)?;

            // // Check for violations based on the predictions
            // let threshold = 3.0; // Replace with an appropriate threshold for your use case
            // let violations = topic_model.lock().unwrap().check_topic_violation(
            //     &mut self.current_topic_trace,
            //     &predictions,
            //     threshold,
            // );

            // if !violations.is_empty() {
            //     // TODO: skip timeout check for the monment
            //     return Err(failure::Error::from(
            //         ExecError::TimeOutErrorWithViolations {
            //             predictions: predictions.clone(),
            //             violations: violations.clone(),
            //         },
            //     ));
            // }

            // update global topic trace
            let mut local_throughput: Vec<(f64, u64)> = Vec::new();
            for (duration, size) in edge_vec.trace.iter() {
                local_throughput.push(((size / duration) as f64, *size));
            }

            let local_min = local_throughput
                .iter()
                .map(|&(time, _)| time)
                .min_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap();
            let local_max = local_throughput
                .iter()
                .map(|&(time, _)| time)
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap();
            if local_min < global_trace.min_time {
                global_trace.min_time = local_min.to_owned();
                global_trace.update_trace_info(edge_vec);
                return Ok(true);
            }
            if local_max > global_trace.max_time {
                global_trace.max_time = local_max.to_owned();
                global_trace.update_trace_info(edge_vec);
                return Ok(true);
            }
        }
        return Ok(false);
    }

    // monitor violation, and update global infor for guidance
    pub fn monitors(
        &mut self,
        shm_dir: &String,
        executor_model: &mut Arc<Mutex<OnnxModel>>,
        topic_model: &mut Arc<Mutex<OnnxModel>>,
        trace_model: &mut Arc<Mutex<OnnxModel>>,
    ) -> Result<bool, failure::Error> {
        let mut monitor_flag = false;
        match self.event_monitor(trace_model) {
            Ok(flag) => {
                monitor_flag = monitor_flag || flag;
            }
            Err(e) => return Err(e),
        }
        match self.timer_monitor(executor_model) {
            Ok(flag) => {
                monitor_flag = monitor_flag || flag;
            }
            Err(e) => return Err(e),
        }
        match self.topic_monitor(topic_model) {
            Ok(flag) => {
                monitor_flag = monitor_flag || flag;
            }
            Err(e) => return Err(e),
        }
        // write all current into file
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(shm_dir.to_owned() + "event_monitor.json")?;
        file.write_all(
            serde_json::to_string(&self.current_event_trace)
                .unwrap()
                .as_bytes(),
        )
        .unwrap();
        file.write_all(b"\n").unwrap();
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(shm_dir.to_owned() + "timer_monitor.json")?;
        file.write_all(
            serde_json::to_string(&self.current_timer_trace)
                .unwrap()
                .as_bytes(),
        )
        .unwrap();
        file.write_all(b"\n").unwrap();
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(shm_dir.to_owned() + "topic_monitor.json")?;
        file.write_all(
            serde_json::to_string(&self.current_topic_trace)
                .unwrap()
                .as_bytes(),
        )
        .unwrap();
        file.write_all(b"\n").unwrap();
        Ok(monitor_flag)
    }

    pub fn add_trace(&mut self, call_trace: &CallTrace) {
        self.event_trace
            .insert(call_trace.id, call_trace.to_owned());
    }

    pub fn is_interest_traces(&mut self, trace: CallTrace) -> Result<bool, failure::Error> {
        let mut flag = false;
        if self.event_trace.contains_key(&trace.id) {
            let global_trace = self.event_trace.get_mut(&trace.id).unwrap();
            for i in 0..global_trace.trace.len() {
                // get the i th element in global_trace.trace
                let global_cb = global_trace.trace.iter_mut().nth(i).unwrap().1;
                let local_cb = trace.trace.iter().nth(i).unwrap().1;
                if global_cb.duration < local_cb.duration {
                    flag = true;
                    //  global_cb.end_time and global_cb.start_time and global_cb.duration set to local_cb's
                    global_cb.reset_duration(local_cb.duration);
                    global_cb.reset_start_time(local_cb.start_time.clone());
                    global_cb.reset_end_time(local_cb.end_time.clone());
                }
            }
            if global_trace.time_set.len() > *CHECK_LEN {
                let upper_bound: f64 = global_trace.mean + (2.0 * global_trace.std_diva);
                let lower_bound: f64 = global_trace.mean - (2.0 * global_trace.std_diva);
                // not compliant to std divation
                if (trace.cur_latency as f64) > upper_bound
                    || (trace.cur_latency as f64) < lower_bound
                {
                    return Err(failure::Error::from(ExecError::TimeOutError(
                        "Timeout Detected!".to_string(),
                    )));
                } else {
                    return Ok(flag);
                }
            } else {
                return Ok(flag);
            }
        } else {
            self.event_trace.insert(trace.id, trace);
            return Ok(true);
        }
    }

    pub fn add_node(&mut self, node: NodeInfo) {
        self.nodes.insert(node.handle, node);
    }

    pub fn get_node_via_handle_mut(&mut self, handle: &u64) -> Option<&NodeInfo> {
        for node in self.nodes.iter_mut() {
            if &node.1.handle == handle {
                return Some(node.1);
            }
        }
        return None;
    }
    pub fn get_node_mut(&mut self, id: &u64) -> Option<&mut NodeInfo> {
        self.nodes.get_mut(id)
    }

    pub fn is_cb_exist(&self, id: &u64) -> bool {
        self.callbacks.contains_key(id)
    }

    pub fn get_callback_mut(&mut self, id: &u64) -> Option<&mut CallbackInfo> {
        self.callbacks.get_mut(id)
    }

    pub fn get_callback_id_via_time(&self, time: &cb_times) -> Option<u64> {
        let handle = time.cb;
        let rmw_handle = time.rmw_handle;
        for cb in self.callbacks.iter() {
            if handle == cb.1.rcl_handle
                || handle == cb.1.rclcpp_handle
                || handle == cb.1.rclcpp_handle1
                || rmw_handle == cb.1.rmw_handle
            {
                let id = &cb.1.node_handle.clone();
                // iter self.node in a for loop
                let mut flag = 0;
                for node in &self.nodes {
                    if node.1.handle == *id {
                        flag = 1;
                        break;
                    }
                }
                if 0 == flag {
                    // skip ros2cli
                    // dbg!("ros2cli");
                }

                return Some(cb.1.id);
            }
        }

        return None;
    }

    pub fn get_callback_mut_via_handle(
        &mut self,
        time: &mut cb_times,
    ) -> Option<&mut CallbackInfo> {
        let handle = time.cb;
        let rmw_handle = time.rmw_handle;
        for cb in self.callbacks.iter_mut() {
            if handle == cb.1.rcl_handle
                || handle == cb.1.rclcpp_handle
                || handle == cb.1.rclcpp_handle1
                || rmw_handle == cb.1.rmw_handle
            {
                let id = &cb.1.node_handle.clone();
                // iter self.node in a for loop
                let mut flag = 0;
                for node in &self.nodes {
                    if node.1.handle == *id {
                        flag = 1;
                        break;
                    }
                }
                if 0 == flag {
                    dbg!("ros2cli");
                }

                return Some(cb.1);
            }
        }

        return None;
    }

    pub fn get_callback_by_cb_id_mut(&mut self, id: &u64) -> Option<&mut CallbackInfo> {
        self.callbacks.get_mut(id)
    }

    pub fn get_callback_by_handle(&self, handle: &u64) -> Option<&CallbackInfo> {
        for cb in self.callbacks.iter() {
            if handle == &cb.1.rcl_handle
                || handle == &cb.1.rclcpp_handle
                || handle == &cb.1.rclcpp_handle1
            {
                return Some(cb.1);
            }
        }
        return None;
    }

    pub fn get_callback_by_cb_id(&self, id: &u64) -> Option<&CallbackInfo> {
        self.callbacks.get(id)
    }

    pub fn get_node_name(&self, handle: u64) -> Option<&str> {
        match self.nodes.get(&handle) {
            Some(node) => Some(&node.node_name),
            None => None,
        }
    }

    pub fn is_node_exist(&self, id: &u64) -> bool {
        self.nodes.contains_key(id)
    }
}
