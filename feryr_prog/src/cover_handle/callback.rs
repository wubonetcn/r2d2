use super::{
    super::{get_name_full, get_name_short, EventType},
    CallbackType,
};
use core::panic;
use serde::Serialize;
use std::{
    cmp::{max, min},
    str,
};
use util::{shmem::*, FULL_LEN, SHORT_LEN};

#[derive(Debug, Clone, PartialEq, Default, Eq, Hash, Serialize)]
pub struct CallbackInfo {
    pub id: u64,
    // handle is from rcl layer
    pub rcl_handle: u64,
    // callback is from rclcpp layer
    pub rclcpp_handle: u64,
    // possible callback_id2 from rclcpp layer
    pub rclcpp_handle1: u64,
    // period for timer callback
    pub period: u64,
    // ros middle ware handle
    pub rmw_handle: u64,

    pub pid: u64,

    pub node_handle: u64,

    pub node_name: String,

    pub cb_name: String,

    pub itf_name: String,

    pub cb_type: CallbackType,

    pub idx: u64,

    pub invoke_time: Vec<u64>,

    pub start_time: Vec<u64>,

    pub end_time: Vec<u64>,

    pub duration: u64,
}
impl CallbackInfo {
    pub fn new(cb: &callback_infos, id: &u64) -> CallbackInfo {
        let mut cb_info = CallbackInfo {
            id: 0,
            rcl_handle: cb.rcl_handle,
            rclcpp_handle: cb.rclcpp_handle,
            rclcpp_handle1: cb.rclcpp_handle1,
            period: cb.period,
            rmw_handle: cb.rmw_handle,
            pid: cb.pid,
            node_handle: cb.node_handle,
            // call getname here
            node_name: String::new(),
            cb_name: String::new(),
            itf_name: String::new(),
            cb_type: CallbackType::Other,
            idx: 1,
            invoke_time: Vec::new(),
            start_time: Vec::new(),
            end_time: Vec::new(),
            duration: 0,
        };
        cb_info.set_itf_name(&cb.function_symbol);
        cb_info.set_cb_name(&cb.cb_name);
        cb_info.set_type(cb.cb_type);
        cb_info.set_id(id);
        return cb_info;
    }

    pub fn set_id(&mut self, id: &u64) {
        self.id = *id;
    }

    pub fn update_idx(&mut self) {
        self.idx += 1;
    }

    pub fn set_pid(&mut self, pid: u64) {
        self.pid = pid;
    }

    pub fn set_rmw_handle(&mut self, id: u64) {
        self.rmw_handle = id;
    }

    pub fn set_rclcpp_handle(&mut self, id: u64) {
        self.rclcpp_handle = id;
    }

    pub fn set_rclcpp_handle1(&mut self, id: u64) {
        self.rclcpp_handle1 = id;
    }

    pub fn set_period(&mut self, period: u64) {
        self.period = period;
    }

    pub fn set_rcl_handle(&mut self, handle: u64) {
        self.rcl_handle = handle;
    }

    pub fn set_type(&mut self, cb_type: u64) {
        match cb_type {
            1 => self.cb_type = CallbackType::Subscriber,
            2 => self.cb_type = CallbackType::Publisher,
            3 => self.cb_type = CallbackType::Service,
            4 => self.cb_type = CallbackType::Client,
            5 => self.cb_type = CallbackType::Timer,
            _ => panic!("callback type unrecognized"),
        }
    }

    pub fn set_node_handle(&mut self, handle: u64) {
        self.node_handle = handle;
    }

    pub fn set_node_name(&mut self, node_name: String) {
        self.node_name = node_name;
    }

    pub fn get_node_name(&self) -> &str {
        &self.node_name
    }

    pub fn set_itf_name(&mut self, itf_name: &[u8; FULL_LEN]) {
        let name = get_name_full(itf_name);
        self.itf_name = name;
    }

    pub fn set_cb_name(&mut self, itf_name: &[u8; SHORT_LEN]) {
        let name = get_name_short(itf_name);
        self.cb_name = name;
    }

    pub fn get_node_handle(&self) -> u64 {
        self.node_handle
    }

    // TODO here we need to sort the start_time and end_time
    pub fn get_cb_during(&mut self) {
        // dbg!(&self.start_time, &self.end_time);
        let len = min(self.start_time.len(), self.end_time.len());
        // sort self.start_time and self.end_time
        self.start_time.sort();
        self.end_time.sort();
        for i in 0..len {
            let a = max(self.start_time[i], self.end_time[i]);
            let b = min(self.start_time[i], self.end_time[i]);
            self.duration += a - b;
        }
    }

    pub fn set_time(&mut self, start_time: u64, end_time: u64) {
        self.start_time.push(start_time);
        self.end_time.push(end_time);
    }

    pub fn get_callback_name(&self) -> &str {
        &self.cb_name
    }

    pub fn reset_start_time(&mut self, start_time: Vec<u64>) {
        self.start_time.clear();
        self.start_time = start_time;
    }

    pub fn reset_end_time(&mut self, end_time: Vec<u64>) {
        self.end_time.clear();
        self.end_time = end_time;
    }

    pub fn reset_duration(&mut self, duration: u64) {
        self.duration = duration;
    }

    pub fn update_cb_start_end_time(&mut self, time: &cb_times) {
        match EventType::from(time.flag).unwrap() {
            EventType::CbStart => self.start_time.push(time.time),
            EventType::CbEnd => self.end_time.push(time.time),
            EventType::ExeRdy => self.invoke_time.push(time.time),
            _ => {}
        }
    }
}
