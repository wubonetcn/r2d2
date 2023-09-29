use crate::cover_handle::callback::CallbackInfo;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize)]
pub struct CallTrace {
    pub id: i128,
    pub trace: HashMap<u64, CallbackInfo>,
    pub time_set: HashSet<u64>,
    pub std_diva: f64,
    pub cur_latency: u64,
    pub total_time: u64,
    pub mean: f64,
    pub max_time: u64,
    pub min_time: u64,
    pub corp_cnt: u64,
}

impl CallTrace {
    pub fn new() -> Self {
        Self {
            id: 0,
            trace: HashMap::new(),
            time_set: HashSet::new(),
            total_time: 0,
            std_diva: 0.0,
            mean: 0.0,
            cur_latency: 0,
            max_time: u64::MIN,
            min_time: u64::MAX,
            corp_cnt: 0,
        }
    }

    pub fn event_trace_add_new_callback(&mut self, cb: &CallbackInfo) {
        self.trace.insert(cb.id, cb.to_owned());
    }

    fn mean(&mut self, data: &Vec<u64>) -> Option<f64> {
        let sum = data.iter().sum::<u64>() as f64;
        let count = data.len();
        self.mean = sum / count as f64;
        Some(sum / count as f64)
    }

    fn std_deviation(&mut self, data: &Vec<u64>) -> Option<f64> {
        match (self.mean(data), data.len()) {
            (Some(data_mean), count) if count > 0 => {
                let variance = data
                    .iter()
                    .map(|value| {
                        let diff = data_mean - (*value as f64);

                        diff * diff
                    })
                    .sum::<f64>()
                    / count as f64;

                Some(variance.sqrt())
            }
            _ => None,
        }
    }

    pub fn set_std_diva(&mut self) {
        let vec: Vec<u64> = self.time_set.iter().map(|x| *x).collect();
        self.std_diva = self.std_deviation(&vec).unwrap();
    }

    pub fn gen_id(&mut self) {
        // get the sum of trace callback's id
        let mut sum: i128 = 0;
        for (id, _) in self.trace.iter() {
            // I want to do sum += id, while keeping sum do not overflow
            sum += *id as i128;
        }
        if self.trace.len() != 0 {
            self.id = sum / self.trace.len() as i128;
        }
    }

    pub fn get_duration(&mut self) -> u64 {
        let mut sum = 0;
        for cb in self.trace.iter_mut() {
            sum += cb.1.duration;
        }
        sum
    }

    pub fn is_exist(&self, cb_id: &u64) -> bool {
        self.trace.contains_key(&cb_id)
    }

    pub fn get_callback_by_cb_id(&self, cb_id: &u64) -> Option<&CallbackInfo> {
        self.trace.get(&cb_id)
    }

    pub fn get_callback_in_et_by_cb_id_mut(&mut self, cb_id: &u64) -> Option<&mut CallbackInfo> {
        self.trace.get_mut(&cb_id)
    }

    pub fn update_trace_info(&mut self) {
        let mut to_remove = Vec::new();
        for (key, cb) in &mut self.trace {
            // remove unnedded callback
            if cb.start_time.is_empty() || cb.end_time.is_empty() {
                to_remove.push(key.clone());
                continue;
            }
            cb.get_cb_during();
        }
        for key in to_remove {
            self.trace.remove(&key);
        }
        self.gen_id();
        self.cur_latency = self.get_duration();
    }
}
