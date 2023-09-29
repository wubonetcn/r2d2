#[derive(Debug, Clone)]
pub struct ServiceTrace {
    pub trace: Vec<(u64, u64)>, // HashMap<edge, (duration, size)>
    pub throughput_trace: Vec<(f64, u64)>,
    pub std_diva: f64,
    pub mean: f64,
    pub max_time: f64,
    pub min_time: f64,
}
impl ServiceTrace {
    pub fn new() -> Self {
        Self {
            trace: Vec::new(),
            throughput_trace: Vec::new(),
            std_diva: 0.0,
            mean: 0.0,
            max_time: f64::MIN,
            min_time: f64::MAX,
        }
    }

    pub fn update_trace_info(&mut self) {
        for (duration, size) in self.trace.iter() {
            if *size == 0 {
                continue;
            } else {
                self.throughput_trace
                    .push(((*duration as f64 / *size as f64), *size));
            }
        }
        self.mean = self
            .throughput_trace
            .iter()
            .map(|(time, _)| time)
            .sum::<f64>() as f64
            / self.throughput_trace.len() as f64;
        // sum::<u64>() as f64 / self.throughput_trace.len() as f64;
        if self.throughput_trace.len() != 0 {
            self.max_time = self
                .trace
                .iter()
                .map(|&(time, _)| time)
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap() as f64;
            self.min_time = self
                .trace
                .iter()
                .map(|&(time, _)| time)
                .min_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap() as f64;
        }
    }
}
