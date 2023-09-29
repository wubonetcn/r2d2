// // use chrono::Utc;
// use feryr_prog::corpus_handle::{
//     // corpus::CorpusWrapper,
//     // prog::Prog,
//     // serialization::serialize,
// };
use rand::{distributions::Alphanumeric, Rng};
use serde::Serialize;
use std::{error::Error, fmt::Display, fs::OpenOptions, io::Write, path::Path};
// use thiserror::Error;

// #[derive(Debug, Error)]
// pub enum ExecError {
//     #[error("exec asan error")]
//     ExecAsan,
//     #[error("killed(maybe cause by timeout)")]
//     TimeOut,
//     #[error("unexpected executor exit status: {0}")]
//     UnexpectedExitStatus(i32),
// }
#[derive(Debug, Clone)]
pub enum LoadError {
    TargetNotSupported,
    Parse(String),
}
impl Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::TargetNotSupported => write!(f, "target not supported"),
            LoadError::Parse(e) => write!(f, "parse: {}", e),
        }
    }
}

impl Error for LoadError {}

pub type JsonValue = simd_json::OwnedValue;
use simd_json::prelude::*;

pub fn get<'a>(val: &'a JsonValue, key: &str) -> Result<&'a JsonValue, LoadError> {
    val.get(key)
        .ok_or_else(|| LoadError::Parse(format!("missing '{}', json:\n{:#}", key, val)))
}

pub fn get_random_string(len: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

pub fn dump_to_file<T: Serialize, P: AsRef<Path>>(
    obj: T,
    file_path: P,
) -> Result<(), failure::Error> {
    let mut f = OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(file_path)?;
    serde_json::to_writer(&mut f, &obj)?;
    f.write_all(b"\n")?;
    Ok(())
}
