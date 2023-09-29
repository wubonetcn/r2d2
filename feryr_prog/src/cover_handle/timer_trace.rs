use serde::Serialize;
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, Serialize)]
pub struct TimerTrace {
    pub pairs: Vec<(u64, u64)>, // HashMap<cb, (duration, queue size)>
    pub sched_vec: VecDeque<u64>,
    pub start_vec: VecDeque<u64>,
    pub queue_vec: VecDeque<u64>,
    pub throughput_map: HashMap<(u64, u64), u64>,
    pub std_diva: f64,
    pub mean: f64,
    pub max_time: f64,
    pub min_time: f64,
}
impl TimerTrace {
    pub fn new() -> Self {
        Self {
            pairs: Vec::new(),
            sched_vec: VecDeque::new(),
            start_vec: VecDeque::new(),
            queue_vec: VecDeque::new(),
            throughput_map: HashMap::new(),
            std_diva: 0.0,
            mean: 0.0,
            max_time: f64::MIN,
            min_time: f64::MAX,
        }
    }

    pub fn update_trace_info(&mut self, current_trace: &TimerTrace) {
        // based on current_trace.trace, update throughput_map
        for current_trace in current_trace.pairs.iter() {
            if self.throughput_map.contains_key(&current_trace) {
                let count = self.throughput_map.get_mut(&current_trace).unwrap();
                *count += 1;
            } else {
                self.throughput_map.insert(current_trace.clone(), 1);
            }
        }
    }

    pub fn mean(&mut self, data: &Vec<u64>) -> Option<f64> {
        let sum = data.iter().sum::<u64>() as f64;
        let count = data.len();
        self.mean = sum / count as f64;
        Some(sum / count as f64)
    }

    pub fn std_deviation(&mut self, data: &Vec<u64>) -> Option<f64> {
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
}
