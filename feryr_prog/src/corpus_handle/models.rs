use super::ENVIRONMENT;
use crate::cover_handle::{
    event_trace::CallTrace, timer_trace::TimerTrace, topic_trace::TopicTrace,
};
use ndarray::{Array, Array2};
use onnxruntime::{session::Session, tensor::*};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
#[derive(Debug)]
pub struct OnnxModel {
    pub session: Session<'static>,
}
unsafe impl Send for OnnxModel {}
unsafe impl Sync for OnnxModel {}
impl OnnxModel {
    pub fn new(model_path: &Path) -> Self {
        // Convert the reference to Path to an owned PathBuf
        let model_path = PathBuf::from(model_path);
        // Use the global static Environment to create a session
        let session = ENVIRONMENT
            .new_session_builder()
            .unwrap()
            .with_model_from_file(model_path.to_owned())
            .unwrap();

        // Return the OnnxModel instance
        OnnxModel { session }
    }

    pub fn process_hash_trace(
        &mut self,
        current_timer_trace: &HashMap<u64, Vec<(u64, u64)>>,
    ) -> Vec<Vec<f32>> {
        let mut data: Vec<Vec<f32>> = vec![];

        for (_id, info) in current_timer_trace {
            for (x, _y) in info {
                data.push(vec![*x as f32]);
            }
        }

        data
    }

    pub fn predict_timer(
        &mut self,
        input_data: &Vec<(u64, u64)>,
    ) -> Result<Vec<f32>, failure::Error> {
        // Check if input_data is empty
        if input_data.is_empty() {
            // Return an empty vector and continue execution
            return Ok(Vec::new());
        }

        let input_shape: (usize, usize) = (input_data.len(), 2);
        let mut input_vec: Vec<f32> = Vec::new();
        for (duration, size) in input_data {
            input_vec.push(*duration as f32);
            input_vec.push(*size as f32);
        }

        let input_array: Array2<f32> = Array::from_shape_vec(input_shape, input_vec)?;

        let outputs = self.session.run(vec![input_array.into_dyn()])?;

        let output_tensor: &OrtOwnedTensor<f32, _> = outputs.get(0).unwrap();
        let output_slice = output_tensor.as_slice().unwrap();
        let output_data: Vec<f32> = output_slice.to_vec();

        Ok(output_data)
    }

    pub fn check_timer_violation(
        &mut self,
        input_data: &mut HashMap<u64, TimerTrace>,
        predictions: &[f32],
        threshold: f32,
    ) -> Vec<(u64, u64, f32)> {
        let mut violations = vec![];

        let mut index = 0;
        for (id, topic_info) in input_data {
            for (duration, size) in &topic_info.pairs {
                let actual_duration = (*duration / *size) as f32;
                let predicted_duration = predictions[index];

                if (actual_duration - predicted_duration).abs() > threshold {
                    violations.push((*id, predicted_duration as u64, actual_duration));
                }
                index += 1;
            }
        }
        violations
    }

    pub fn predict_topic(
        &mut self,
        input_data: &Vec<(u64, u64)>,
    ) -> Result<Vec<f32>, failure::Error> {
        // Check if input_data is empty
        if input_data.is_empty() {
            // Return an empty vector and continue execution
            return Ok(Vec::new());
        }

        let input_shape: (usize, usize) = (input_data.len(), 2);
        let mut input_vec: Vec<f32> = Vec::new();
        for (duration, size) in input_data {
            input_vec.push(*duration as f32);
            input_vec.push(*size as f32);
        }

        let input_array: Array2<f32> = Array::from_shape_vec(input_shape, input_vec)?;

        let outputs = self.session.run(vec![input_array.into_dyn()])?;

        let output_tensor: &OrtOwnedTensor<f32, _> = outputs.get(0).unwrap();
        let output_slice = output_tensor.as_slice().unwrap();
        let output_data: Vec<f32> = output_slice.to_vec();

        Ok(output_data)
    }

    pub fn check_topic_violation(
        &mut self,
        input_data: &mut HashMap<u64, TopicTrace>,
        predictions: &[f32],
        threshold: f32,
    ) -> Vec<(u64, u64, f32)> {
        let mut violations = vec![];

        let mut index = 0;
        for (id, topic_info) in input_data {
            for (duration, size) in &topic_info.trace {
                let actual_duration = (*duration / *size) as f32;
                let predicted_duration = predictions[index];

                if (actual_duration - predicted_duration).abs() > threshold {
                    violations.push((*id, predicted_duration as u64, actual_duration));
                }
                index += 1;
            }
        }
        violations
    }

    pub fn predict_hash_trace(
        &mut self,
        input_data: &[Vec<f32>],
    ) -> Result<Vec<f32>, failure::Error> {
        // Check if input_data is empty
        if input_data.is_empty() {
            // Return an empty vector and continue execution
            return Ok(Vec::new());
        }

        let input_shape: (usize, usize) = (input_data.len(), input_data[0].len());

        let mut input_vec: Vec<f32> = Vec::new();
        for row in input_data {
            for val in row {
                input_vec.push(*val);
            }
        }

        let input_array: Array2<f32> = Array::from_shape_vec(input_shape, input_vec)?;

        let outputs = self.session.run(vec![input_array.into_dyn()])?;

        let output_tensor: &OrtOwnedTensor<f32, _> = outputs.get(0).unwrap();
        let output_slice = output_tensor.as_slice().unwrap();
        let output_data: Vec<f32> = output_slice.to_vec();

        Ok(output_data)
    }

    pub fn check_timer_violations(
        &mut self,
        current_timer_trace: &HashMap<u64, Vec<(u64, u64)>>,
        predictions: &[f32],
        threshold: f32,
    ) -> Vec<(u64, u64, f32)> {
        let mut violations = vec![];

        let mut index = 0;
        for (_id, info) in current_timer_trace {
            for (_x, y) in info {
                let actual_duration = *y as f32 / 1_000_000f32;
                let predicted_duration = predictions[index];

                if (actual_duration - predicted_duration).abs() > threshold {
                    violations.push((*y, index as u64, predicted_duration));
                }

                index += 1;
            }
        }

        violations
    }

    pub fn process_call_trace(&mut self, current_event_trace: &CallTrace) -> Vec<Vec<f32>> {
        let mut data: Vec<Vec<f32>> = vec![];

        for (_id, callback_info) in &current_event_trace.trace {
            let cur_latency = callback_info.duration as f32;
            data.push(vec![cur_latency]);
        }

        data
    }

    pub fn check_call_trace_violations(
        &mut self,
        current_event_trace: &CallTrace,
        predictions: &[f32],
        threshold: f32,
    ) -> Vec<(u64, u64, f32)> {
        let mut violations = vec![];

        let mut index = 0;
        for (_id, callback_info) in &current_event_trace.trace {
            let actual_duration = callback_info.duration as f32 / 1_000_000f32;
            let predicted_duration = predictions[index];

            if (actual_duration - predicted_duration).abs() > threshold {
                violations.push((callback_info.duration, index as u64, predicted_duration));
            }

            index += 1;
        }

        violations
    }
}
