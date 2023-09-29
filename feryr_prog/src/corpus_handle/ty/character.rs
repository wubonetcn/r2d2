use crate::corpus_handle::{
    ty::{Deserialize, Serialize},
    SHM_PATH,
};
use rand::Rng;
use std::process::Command;
// here we want to have a struct that store a integer type and value
//     as_kind!(as_int, checked_as_int, IntType);
#[derive(Copy, Default, Clone, Debug, Deserialize, Serialize)]
pub struct CharType {
    tyid: usize,
    val: char,
    max_val: char,
    min_val: char,
}
impl CharType {
    pub fn new(tyid: usize, val: char, max_val: char, min_val: char) -> Self {
        Self {
            tyid,
            val,
            max_val,
            min_val,
        }
    }
    pub fn get_tyid(&self) -> usize {
        self.tyid
    }
    pub fn get_val(&self) -> String {
        self.val.to_string()
    }
    pub fn get_max_val(&self) -> char {
        self.max_val
    }
    pub fn get_min_val(&self) -> char {
        self.min_val
    }

    pub fn gen_char(&mut self) -> char {
        let mut rng = rand::thread_rng();
        self.val = rng.gen::<char>();
        self.val
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct StringType {
    tyid: usize,
    val: String,
    len: i32,
}
impl StringType {
    pub fn new(tyid: usize, val: String, len: i32) -> Self {
        Self { tyid, val, len }
    }
    pub fn get_tyid(&self) -> usize {
        self.tyid
    }
    pub fn get_val(&self) -> String {
        self.val.to_string()
    }
    pub fn get_len(&self) -> i32 {
        self.len
    }
    pub fn gen_string(&mut self) -> Result<(), failure::Error> {
        if self.len != 0 {
            let mut rng = rand::thread_rng();
            while self.val.len() < self.len as usize {
                let ch = rng.gen::<char>();
                if ch != '\0' {
                    self.val.push(ch);
                }
            }

            // for _ in 0..self.len {
            //     self.val.push(rng.gen::<char>());
            // }
        } else {
            //  env::var("SHM_PATH").expect("$USER is not set");

            let shm_path = Self::read_shm_path();
            let node_list = Command::new("ros2")
                .env("SHM_PATH", shm_path)
                .args(["node", "list", "--no-daemon"])
                .output()
                .expect("failed to get node list: {}");

            let node_list = String::from_utf8(node_list.stdout).unwrap();
            let node_list: Vec<&str> = node_list.split_whitespace().collect();
            let idx = match node_list.len() {
                0 => return Err(failure::err_msg("no node found")),
                _ => rand::thread_rng().gen_range(0..node_list.len()),
            };
            self.val = node_list[idx].to_string();
        }
        Ok(())
    }
    fn read_shm_path() -> String {
        let path_guard = SHM_PATH.lock().unwrap();
        let path = path_guard.clone();
        drop(path_guard); // Ensure path_guard is dropped before unlocking the mutex
        path
    }
}
