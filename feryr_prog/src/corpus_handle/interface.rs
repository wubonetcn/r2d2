use super::{ty::TYPE, RngType};
use crate::corpus_handle::ty::{array, character, double, integer};
use multimap::MultiMap;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum ITF {
    Topic,
    Service,
    Action,
    Param,
}
impl Default for ITF {
    fn default() -> Self {
        ITF::Topic
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum InterfaceTpyes {
    Messages,
    Services,
    Actions,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum NodeTpyes {
    Subscribers,
    Publishers,
    ServiceServer,
    ServiceClients,
    ActionServers,
    ActionClients,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceParam {
    // the pair is like: (type paran_name)
    pub arg_type: String,
    pub arg_name: String,
    pub is_array: bool,
    pub max_array_size: i32,
    pub is_const: bool,
    pub const_val: String,
}
impl InterfaceParam {
    pub fn new(
        arg_type: String,
        arg_name: String,
        is_array: bool,
        max_array_size: i32,
        is_const: bool,
        const_val: String,
    ) -> Self {
        InterfaceParam {
            arg_name,
            arg_type,
            is_array,
            max_array_size,
            is_const,
            const_val,
        }
    }
    pub fn is_meta_type(&self) -> bool {
        let meta_type = match self.arg_type.contains('[') {
            true => self.arg_type[0..self.arg_type.find('[').unwrap()].to_string(),
            false => self.arg_type.clone(),
        };
        TYPE::from_str(meta_type.as_str()) != TYPE::COMPLEX
    }

    pub fn get_array_inner_type(&self) -> TYPE {
        let mut inner_type = self.arg_type.clone();
        // substr from first char to the first [
        inner_type = inner_type[0..inner_type.find('[').unwrap()].to_string();
        TYPE::from_str(inner_type.as_str())
    }

    pub fn update_arg_type(&mut self, arg_type: String) {
        self.arg_type = arg_type;
    }

    pub fn update_array_len(&mut self, len: i32) {
        self.max_array_size = len;
    }
}

// This structure manage one particular ros node information: subscribers/publisher/service/action, etc
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub node_name: String,
    pub node_subscribers: MultiMap<String, InterfaceVal>,
    pub node_publisher: MultiMap<String, InterfaceVal>,
    pub service_server: MultiMap<String, InterfaceVal>,
    pub service_client: MultiMap<String, InterfaceVal>,
    pub action_server: MultiMap<String, InterfaceVal>,
    pub action_client: MultiMap<String, InterfaceVal>,
    pub param: MultiMap<String, InterfaceVal>,
}

impl Node {
    pub fn new(node_name: String) -> Self {
        Node {
            node_name,
            node_subscribers: MultiMap::new(),
            node_publisher: MultiMap::new(),
            service_server: MultiMap::new(),
            service_client: MultiMap::new(),
            action_server: MultiMap::new(),
            action_client: MultiMap::new(),
            param: MultiMap::new(),
        }
    }

    pub fn get_node_subscribers(&self) -> &MultiMap<String, InterfaceVal> {
        &self.node_subscribers
    }

    pub fn get_node_publisher(&self) -> &MultiMap<String, InterfaceVal> {
        &self.node_publisher
    }

    pub fn get_service_server(&self) -> &MultiMap<String, InterfaceVal> {
        &self.service_server
    }

    pub fn get_service_client(&self) -> &MultiMap<String, InterfaceVal> {
        &self.service_client
    }

    pub fn get_action_server(&self) -> &MultiMap<String, InterfaceVal> {
        &self.action_server
    }

    pub fn get_action_client(&self) -> &MultiMap<String, InterfaceVal> {
        &self.action_client
    }

    pub fn get_param(&self) -> &MultiMap<String, InterfaceVal> {
        &self.param
    }

    pub fn add_subscriber(&mut self, topic_name: String, topic_val: InterfaceVal) {
        self.node_subscribers.insert(topic_name, topic_val);
    }

    pub fn add_publisher(&mut self, topic_name: String, topic_val: InterfaceVal) {
        self.node_publisher.insert(topic_name, topic_val);
    }

    pub fn add_service_server(&mut self, service_name: String, service_val: InterfaceVal) {
        self.service_server.insert(service_name, service_val);
    }

    pub fn add_service_client(&mut self, service_name: String, service_val: InterfaceVal) {
        self.service_client.insert(service_name, service_val);
    }

    pub fn add_action_server(&mut self, action_name: String, action_val: InterfaceVal) {
        self.action_server.insert(action_name, action_val);
    }

    pub fn add_action_client(&mut self, action_name: String, action_val: InterfaceVal) {
        self.action_client.insert(action_name, action_val);
    }

    pub fn add_param(&mut self, param_name: String, param_val: InterfaceVal) {
        self.param.insert(param_name, param_val);
    }

    pub fn set_param(&mut self, param: MultiMap<String, InterfaceVal>) {
        self.param = param;
    }

    pub fn get_node_name(&self) -> &String {
        &self.node_name
    }

    pub fn get_avalible_interface(&self) -> Vec<ITF> {
        let mut vec: Vec<ITF> = vec![];
        if self.get_service_server().len() != 0 {
            vec.push(ITF::Service);
        }
        if self.get_node_subscribers().len() != 0 {
            vec.push(ITF::Topic);
        }
        if self.get_action_server().len() != 0 {
            vec.push(ITF::Action);
        }
        if self.get_param().len() != 0 {
            vec.push(ITF::Param);
        }

        vec
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValueType {
    Op(InterfaceVal),
    Op1(integer::IntType),
    Op2(integer::BoolType),
    Op3(double::DoubleType),
    Op4(character::CharType),
    Op5(array::ArrayType),
    Op6(character::StringType),
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct InterfaceVal {
    pub itf_name: String,
    pub itf_type: String,
    pub val: Vec<ValueType>,
}
impl InterfaceVal {
    pub fn new(itf_name: &String, itf_type: &String) -> InterfaceVal {
        let itf_val = InterfaceVal {
            itf_name: itf_name.to_string(),
            itf_type: itf_type.to_string(),
            val: Vec::new(),
        };
        itf_val
    }

    pub fn construct_itf_layers(
        &mut self,
        type_info: &Vec<String>,
        type_maps: &HashMap<String, String>,
        itf_info: &HashMap<String, Vec<InterfaceParam>>,
    ) {
        // match self.itf_type with itf_info
        let itf_type = &self.itf_type;
        let begin_idx = match itf_type.rfind('/') {
            Some(idx) => idx + 1,
            None => 0,
        };
        let end_idx = match itf_type.find('[') {
            Some(idx) => idx,
            None => itf_type.len(),
        };
        // let type_short = &itf_type[begin_idx..end_idx];
        let type_long = type_maps.get(&itf_type[begin_idx..end_idx]).unwrap();
        let args = itf_info.get(type_long).unwrap().to_owned();
        for arg in args {
            if arg.is_meta_type() {
                let mut itf = InterfaceVal::new(&arg.arg_name, &arg.arg_type);
                let val = match TYPE::from_str(arg.arg_type.as_str()) {
                    TYPE::String => {
                        let mut rng = rand::thread_rng();
                        if itf.itf_name == "node" {
                            ValueType::Op6(character::StringType::new(
                                TYPE::String as usize,
                                "".to_string(),
                                0,
                            ))
                        } else {
                            ValueType::Op6(character::StringType::new(
                                TYPE::String as usize,
                                "".to_string(),
                                rng.gen_range(1..32),
                            ))
                        }
                    }
                    TYPE::Float32 => ValueType::Op3(double::DoubleType::new(
                        TYPE::Float32 as usize,
                        0.0,
                        f32::MAX as f64,
                        f32::MIN as f64,
                        32,
                    )),
                    TYPE::Float64 => ValueType::Op3(double::DoubleType::new(
                        TYPE::Float64 as usize,
                        0.0,
                        f64::MAX as f64,
                        f64::MIN as f64,
                        64,
                    )),
                    TYPE::Byte => ValueType::Op4(character::CharType::new(
                        TYPE::Char as usize,
                        0 as char,
                        u8::MAX as char,
                        0 as char,
                    )),
                    TYPE::Char => ValueType::Op4(character::CharType::new(
                        TYPE::Char as usize,
                        0 as char,
                        u8::MAX as char,
                        0 as char,
                    )),
                    TYPE::Int8 => ValueType::Op1(integer::IntType::new(
                        TYPE::Int8 as usize,
                        0 as u64,
                        i8::MAX as u64,
                        i8::MIN as u64,
                        -8,
                    )),
                    TYPE::Int16 => ValueType::Op1(integer::IntType::new(
                        TYPE::Int16 as usize,
                        0 as u64,
                        i16::MAX as u64,
                        i16::MIN as u64,
                        -16,
                    )),
                    TYPE::Int32 => ValueType::Op1(integer::IntType::new(
                        TYPE::Int32 as usize,
                        0 as u64,
                        i32::MAX as u64,
                        i32::MIN as u64,
                        -32,
                    )),
                    TYPE::Int64 => ValueType::Op1(integer::IntType::new(
                        TYPE::Int64 as usize,
                        0 as u64,
                        i64::MAX as u64,
                        i64::MIN as u64,
                        -64,
                    )),
                    TYPE::UInt8 => ValueType::Op1(integer::IntType::new(
                        TYPE::UInt8 as usize,
                        0 as u64,
                        u8::MAX as u64,
                        u8::MIN as u64,
                        8,
                    )),
                    TYPE::UInt16 => ValueType::Op1(integer::IntType::new(
                        TYPE::UInt16 as usize,
                        0 as u64,
                        u16::MAX as u64,
                        u16::MIN as u64,
                        16,
                    )),
                    TYPE::UInt32 => ValueType::Op1(integer::IntType::new(
                        TYPE::UInt32 as usize,
                        0 as u64,
                        u32::MAX as u64,
                        u32::MIN as u64,
                        32,
                    )),
                    TYPE::UInt64 => ValueType::Op1(integer::IntType::new(
                        TYPE::UInt64 as usize,
                        0 as u64,
                        u64::MAX as u64,
                        u64::MIN as u64,
                        64,
                    )),
                    TYPE::Bool => {
                        ValueType::Op2(integer::BoolType::new(TYPE::Bool as usize, 0 as u64, 0, 1))
                    }
                    _ => {
                        if arg.is_array {
                            let array_len = arg.max_array_size;
                            let array_type = arg.get_array_inner_type();
                            ValueType::Op5(array::ArrayType::new(
                                TYPE::ARRAY as usize,
                                array_type,
                                array_len as u64,
                            ))
                        } else {
                            panic!("unsupport type: {}", arg.arg_type);
                        }
                    }
                };
                itf.val.push(val);
                self.val.push(ValueType::Op(itf));
            } else {
                if arg.is_array {
                    let mut rng = rand::thread_rng();
                    let array_len = rng.gen_range(0..arg.max_array_size);
                    for _ in 0..array_len {
                        let mut itf = InterfaceVal::new(&arg.arg_name, &arg.arg_type);
                        itf.construct_itf_layers(type_info, type_maps, itf_info);
                        self.val.push(ValueType::Op(itf));
                    }
                } else {
                    let mut itf = InterfaceVal::new(&arg.arg_name, &arg.arg_type);
                    itf.construct_itf_layers(type_info, type_maps, itf_info);
                    self.val.push(ValueType::Op(itf));
                }
            }
        }
    }

    pub fn gen_value(&mut self, rng: &mut RngType) -> Result<(), failure::Error> {
        // generate value base on self
        for val in self.val.iter_mut() {
            // check if val is ValueType::Op(Interfaceval)
            match val {
                ValueType::Op(itf) => match itf.gen_value(rng) {
                    Ok(_) => (),
                    Err(e) => {
                        return Err(e);
                    }
                },
                ValueType::Op1(int) => {
                    int.gen_integer();
                }
                ValueType::Op2(bool) => {
                    bool.gen_bool();
                }
                ValueType::Op3(double) => {
                    double.gen_double();
                }
                ValueType::Op4(char) => {
                    char.gen_char();
                }
                ValueType::Op5(array) => match array.gen_array() {
                    Ok(_) => (),
                    Err(e) => {
                        return Err(e);
                    }
                },
                ValueType::Op6(string) => match string.gen_string() {
                    Ok(_) => (),
                    Err(e) => {
                        return Err(e);
                    }
                },
            }
        }
        Ok(())
    }

    pub fn get_value(&self) -> String {
        // recursively generate string with the following pattern {self.name: self.val}
        // format a string
        let mut res = String::new();

        for val in self.val.iter() {
            match val {
                ValueType::Op(itf) => {
                    if TYPE::from_str(itf.itf_type.as_str()) != TYPE::COMPLEX {
                        res.push_str(&itf.get_value());
                    } else {
                        res.push_str(&itf.itf_name);
                        res.push_str(": ");
                        res.push_str("{ ");
                        let sub_res = itf.get_value().to_string();
                        res.push_str(&sub_res);
                        let end_idx = match res.rfind(',') {
                            Some(idx) => idx,
                            None => 0,
                        };
                        res = res[0..end_idx].to_string();
                        res.push_str("}, ");
                    }
                }
                ValueType::Op1(int) => {
                    if self.itf_name == "sec" {
                        res.push_str(&self.itf_name);
                        res.push_str(": ");
                        res.push_str(&integer::IntType::gen_i16_string());
                        res.push_str(", ");
                    } else {
                        res.push_str(&self.itf_name);
                        res.push_str(": ");
                        res.push_str(&int.get_val());
                        res.push_str(", ");
                    }
                }
                ValueType::Op2(bool) => {
                    res.push_str(&self.itf_name);
                    res.push_str(": ");
                    res.push_str(&bool.get_val());
                    res.push_str(", ");
                }
                ValueType::Op3(double) => {
                    res.push_str(&self.itf_name);
                    res.push_str(": ");
                    res.push_str(&double.get_val());
                    res.push_str(", ");
                }
                ValueType::Op4(chara) => {
                    res.push_str(&self.itf_name);
                    res.push_str(": ");
                    res.push_str(&chara.get_val());
                    res.push_str(", ");
                }

                ValueType::Op5(array) => {
                    // res.push_str(&self.itf_name);
                    // res.push_str(": ");
                    res.push_str(&array.get_val().to_string());
                    res.push_str(", ");
                }
                ValueType::Op6(string) => {
                    res.push_str(&self.itf_name);
                    res.push_str(": {'");
                    res.push_str(&string.get_val());
                    res.push_str("'}, ");
                }
            }
        }
        res
    }
}
