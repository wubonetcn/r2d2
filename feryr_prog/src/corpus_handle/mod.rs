use ahash::AHashMap;
use onnxruntime::environment::Environment;
// use std::collections::{HashSet, HashMap};
#[macro_use]
pub mod gen;
pub mod sys;
// pub mod mutation;
pub mod interface;
pub mod models;
pub mod prog;
pub mod target;
pub mod ty;
pub mod value;
use std::sync::Mutex;

lazy_static::lazy_static! {
    pub static ref SHM_PATH: Mutex<String> = Mutex::new(String::from(""));
    static ref ENVIRONMENT: Environment = Environment::builder().build().unwrap();
}

pub const IN_SHM_SZ: usize = 1 << 16;
pub type HashMap<K, V> = AHashMap<K, V>;
pub type RngType = rand::rngs::SmallRng;
pub const IN_MAGIC: u64 = 0xBADC0FFEEBADFACE;
