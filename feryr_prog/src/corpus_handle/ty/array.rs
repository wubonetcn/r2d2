// use rand::Rng;
// use std::process::Command;

use serde::Deserialize;
use serde::Serialize;

pub use super::character::*;
pub use super::double::*;
pub use super::integer::*;
use super::TYPE;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArrayType {
    tyid: usize,
    inner_type: TYPE,
    len: u64,
    int_array: Vec<IntType>,
    float_array: Vec<DoubleType>,
    char_array: Vec<CharType>,
    bool_array: Vec<BoolType>,
    string_array: Vec<StringType>,
}
impl ArrayType {
    pub fn new(tyid: usize, inner_type: TYPE, len: u64) -> Self {
        Self {
            tyid,
            inner_type,
            len,
            int_array: Vec::new(),
            float_array: Vec::new(),
            char_array: Vec::new(),
            bool_array: Vec::new(),
            string_array: Vec::new(),
        }
    }
    pub fn get_tyid(&self) -> usize {
        self.tyid
    }

    pub fn get_val(&self) -> String {
        let mut res = String::new();
        match self.inner_type {
            TYPE::String => {
                for i in 0..self.string_array.len() {
                    if self.string_array.len() == 0 {
                        break;
                    }
                    res.push_str(&self.string_array[i as usize].get_val());
                    res.push_str(", ");
                }
            }
            TYPE::Char | TYPE::Byte => {
                for i in 0..self.char_array.len() {
                    res.push_str(&self.char_array[i as usize].get_val().to_string());
                    res.push_str(" ");
                }
            }
            TYPE::Float32 => {
                for i in 0..self.float_array.len() {
                    res.push_str(&self.float_array[i as usize].get_val().to_string());
                    res.push_str(", ");
                }
            }
            TYPE::Float64 => {
                for i in 0..self.float_array.len() {
                    res.push_str(&self.float_array[i as usize].get_val().to_string());
                    res.push_str(", ");
                }
            }
            TYPE::Int8 => {
                for i in 0..self.int_array.len() {
                    res.push_str(&self.int_array[i as usize].get_val().to_string());
                    res.push_str(", ");
                }
            }
            TYPE::Int16 => {
                for i in 0..self.int_array.len() {
                    res.push_str(&self.int_array[i as usize].get_val().to_string());
                    res.push_str(", ");
                }
            }
            TYPE::Int32 => {
                for i in 0..self.int_array.len() {
                    res.push_str(&self.int_array[i as usize].get_val().to_string());
                    res.push_str(", ");
                }
            }
            TYPE::Int64 => {
                for i in 0..self.int_array.len() {
                    res.push_str(&self.int_array[i as usize].get_val().to_string());
                    res.push_str(", ");
                }
            }
            TYPE::UInt8 => {
                for i in 0..self.int_array.len() {
                    res.push_str(&self.int_array[i as usize].get_val().to_string());
                    res.push_str(", ");
                }
            }
            TYPE::UInt16 => {
                for i in 0..self.int_array.len() {
                    res.push_str(&self.int_array[i as usize].get_val().to_string());
                    res.push_str(", ");
                }
            }
            TYPE::UInt32 => {
                for i in 0..self.int_array.len() {
                    res.push_str(&self.int_array[i as usize].get_val().to_string());
                    res.push_str(", ");
                }
            }
            TYPE::UInt64 => {
                for i in 0..self.int_array.len() {
                    res.push_str(&self.int_array[i as usize].get_val().to_string());
                    res.push_str(", ");
                }
            }
            TYPE::Bool => {
                for i in 0..self.bool_array.len() {
                    res.push_str(&self.bool_array[i as usize].get_val().to_string());
                    res.push_str(", ");
                }
            }
            _ => {
                panic!("wrong array type");
            }
        }
        res.pop();
        res.pop();

        // res.push_str("}");
        res
    }

    pub fn gen_array(&mut self) -> Result<(), failure::Error> {
        // generate based on pad
        match self.inner_type {
            TYPE::String => {
                for _ in 0..self.len {
                    let mut val = StringType::new(0, "".to_string(), 0);
                    match val.gen_string() {
                        Ok(_) => {}
                        Err(e) => {
                            return Err(e);
                        }
                    }
                    self.string_array.push(val);
                }
            }
            TYPE::Char | TYPE::Byte => {
                for _ in 0..self.len {
                    let mut val = CharType::new(0, 0 as char, char::MAX, 0 as char);
                    val.gen_char();
                    self.char_array.push(val);
                }
            }
            TYPE::Float32 => {
                for _ in 0..self.len {
                    let mut val =
                        DoubleType::new(0, 0 as f64, f32::MAX as f64, f32::MIN as f64, 32);
                    val.gen_double();
                    self.float_array.push(val);
                }
            }
            TYPE::Float64 => {
                for _ in 0..self.len {
                    let mut val =
                        DoubleType::new(0, 0 as f64, f64::MAX as f64, f64::MIN as f64, 64);
                    val.gen_double();
                    self.float_array.push(val);
                }
            }
            TYPE::Int8 => {
                for _ in 0..self.len {
                    let mut val = IntType::new(0, 0 as u64, i8::MAX as u64, i8::MIN as u64, 8);
                    val.gen_integer();
                    self.int_array.push(val);
                }
            }
            TYPE::Int16 => {
                for _ in 0..self.len {
                    let mut val = IntType::new(0, 0 as u64, i16::MAX as u64, i16::MIN as u64, 16);
                    val.gen_integer();
                    self.int_array.push(val);
                }
            }
            TYPE::Int32 => {
                for _ in 0..self.len {
                    let mut val = IntType::new(0, 0 as u64, i32::MAX as u64, i32::MIN as u64, 32);
                    val.gen_integer();
                    self.int_array.push(val);
                }
            }
            TYPE::Int64 => {
                for _ in 0..self.len {
                    let mut val = IntType::new(0, 0 as u64, i64::MAX as u64, i64::MIN as u64, 64);
                    val.gen_integer();
                    self.int_array.push(val);
                }
            }
            TYPE::UInt8 => {
                for _ in 0..self.len {
                    let mut val = IntType::new(0, 0 as u64, u8::MAX as u64, u8::MIN as u64, 8);
                    val.gen_integer();
                    self.int_array.push(val);
                }
            }
            TYPE::UInt16 => {
                for _ in 0..self.len {
                    let mut val = IntType::new(0, 0 as u64, u16::MAX as u64, u16::MIN as u64, 16);
                    val.gen_integer();
                    self.int_array.push(val);
                }
            }
            TYPE::UInt32 => {
                for _ in 0..self.len {
                    let mut val = IntType::new(0, 0 as u64, u32::MAX as u64, u32::MIN as u64, 32);
                    val.gen_integer();
                    self.int_array.push(val);
                }
            }
            TYPE::UInt64 => {
                for _ in 0..self.len {
                    let mut val = IntType::new(0, 0 as u64, u64::MAX, u64::MIN, 64);
                    val.gen_integer();
                    self.int_array.push(val);
                }
            }
            TYPE::Bool => {
                for _ in 0..self.len {
                    let val = BoolType::new(0, 0 as u64, 1 as u64, 0 as u64);
                    val.gen_bool();
                    self.bool_array.push(val);
                }
            }
            _ => {
                panic!("Not supported type");
            }
        }
        Ok(())
    }
}
