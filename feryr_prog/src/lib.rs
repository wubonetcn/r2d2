extern crate pest_derive;
pub mod corpus_handle;
pub mod cover_handle;
pub mod crash_handle;
use failure::Fail;
use iota::iota;
use regex::bytes::Regex;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    str,
};
use util::{FULL_LEN, SHORT_LEN};

lazy_static::lazy_static! {
    pub static ref CHECK_LEN: usize = 200;
    pub static ref RE: Regex = Regex::new(r"@@(\d+)@@").unwrap();
    pub static ref ERR_LOG_PATTERN: Vec<&'static str> = vec!["EOF", "Failed", "no attribute", "not found"];
    pub static ref FALSE_LOG_PATTERN: Vec<&'static str> = vec!["xvfb", "Failed to populate field", "Node not found"];
    pub static ref HANG_LOG_PATTERN: Vec<&'static str> = vec!["Waiting for "];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    CbStart = 1,
    CbEnd = 2,
    RclPub = 3,
    RclSub = 4,
    SrvReq = 5,
    SrvRsp = 6,
    CliReq = 7,
    CliRsp = 8,
    ExeExe = 9,
    ExeRdy = 10,
}
impl EventType {
    fn from(value: u64) -> Option<EventType> {
        match value {
            1 => Some(EventType::CbStart),
            2 => Some(EventType::CbEnd),
            3 => Some(EventType::RclPub),
            4 => Some(EventType::RclSub),
            5 => Some(EventType::SrvReq),
            6 => Some(EventType::SrvRsp),
            7 => Some(EventType::CliReq),
            8 => Some(EventType::CliRsp),
            9 => Some(EventType::ExeExe),
            10 => Some(EventType::ExeRdy),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum CallType {
    TOPIC = 0,
    SERVICE,
    ACTION,
    PARAMETER,
}
iota! {
    const WAIT: u64 = iota;
        , READY
        , RUNNING
}

#[derive(Debug, Clone, Fail)]
enum ExecError {
    #[fail(
        display = "Timeout Detected on Trace! Predictions: {:?}, Violations: {:?}",
        predictions, violations
    )]
    TimeOutErrorWithViolations {
        predictions: Vec<f32>,            // Update the type
        violations: Vec<(u64, u64, f32)>, // Update the type
    },
    #[fail(
        display = "Timeout Detected on Trace! Current Value: {}, Max Value: {}",
        current_value, max_value
    )]
    _TimeOutErrorWithValues { current_value: f64, max_value: f64 },
    #[fail(display = "Timeout Error Detected: {}", _0)]
    TimeOutError(String),

    #[fail(display = "Process Crash Detected: {}", _0)]
    ExecError(String),

    #[fail(display = "Zombie Process Detected: {}", _0)]
    ZombError(String),

    #[fail(display = "Log Crashed Detected: {}", _0)]
    _LogError(String),

    #[fail(display = "Result is not valid: {}", reason)]
    InvalidResult { reason: String },
}

pub fn get_name_short(name: &[u8; SHORT_LEN]) -> String {
    let name = name.to_vec();
    let name = String::from_utf8_lossy(&name)
        .trim_end_matches('\0')
        .to_string();
    let pos = name.find('\0').unwrap_or(name.len());
    let name = name[..pos].to_string();
    if name.contains("ros2cli_daemon") {
        return "ros2cli_daemon".to_string();
    } else if name.contains("ros2cli") {
        return "ros2cli".to_string();
    }
    name
}

pub fn get_name_full(name: &[u8; FULL_LEN]) -> String {
    let name = name.to_vec();
    let name = String::from_utf8_lossy(&name)
        .trim_end_matches('\0')
        .to_string();
    let pos = name.find('\0').unwrap_or(name.len());
    name[..pos].to_string()
}

fn string_hasher(string: &String) -> u64 {
    let mut hasher = DefaultHasher::new();
    string.hash(&mut hasher);
    hasher.finish()
}
