use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
pub struct TopicTrace {
    pub callback: u64,
    pub subscription: u64,
    pub trace: Vec<(u64, u64)>, // duratoion. size
    pub throughput_map: HashMap<(u64, u64), u64>,
    pub std_diva: f64,
    pub mean: f64,
    pub max_time: f64,
    pub min_time: f64,
}

impl TopicTrace {
    pub fn new() -> Self {
        Self {
            callback: 0,
            subscription: 0,
            trace: Vec::new(),
            throughput_map: HashMap::new(),
            std_diva: 0.0,
            mean: 0.0,
            max_time: f64::MIN,
            min_time: f64::MAX,
        }
    }

    pub fn update_trace_info(&mut self, current_trace: &TopicTrace) {
        // based on current_trace.trace, update throughput_map
        for current_trace in current_trace.trace.iter() {
            if self.throughput_map.contains_key(&current_trace) {
                let count = self.throughput_map.get_mut(&current_trace).unwrap();
                *count += 1;
            } else {
                self.throughput_map.insert(current_trace.clone(), 1);
            }
        }
    }

    pub fn get_callback(&self) -> u64 {
        self.callback
    }

    pub fn set_callback(&mut self, callback: u64) {
        self.callback = callback;
    }

    pub fn get_subscription(&self) -> u64 {
        self.subscription
    }

    pub fn set_subscription(&mut self, subscription: u64) {
        self.subscription = subscription;
    }

    pub fn set_trace(&mut self, trace: (u64, u64)) {
        self.trace.push(trace);
    }

    pub fn get_trace(&self) -> &Vec<(u64, u64)> {
        &self.trace
    }
}
