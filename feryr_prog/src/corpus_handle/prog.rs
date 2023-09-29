use super::{
    super::{ExecError, ERR_LOG_PATTERN, FALSE_LOG_PATTERN, HANG_LOG_PATTERN},
    interface::{InterfaceVal, Node, ITF},
    target::Target,
    RngType,
};
use rand::{rngs::OsRng, Rng};
use serde::{Deserialize, Serialize};
use util::fuzzer_info;
use std::{
    io::Read,
    process::{Command, Stdio},
    time::{Duration, SystemTime},
};
use wait_timeout::ChildExt;

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct Prog {
    pub itf: ITF,
    pub itf_name: String,
    pub itf_type: String,
    pub call_stream: String,
    pub itf_info: InterfaceVal,
    pub size: u64,
}
impl Prog {
    pub fn get_call(target: &Target, period: u32) -> Result<Prog, failure::Error> {
        let mut prog = Prog::default();
        if period % 3 != 0 {
            // go to generation
            prog.generate_call(target).unwrap();
        } else {
            // go to mutation
            prog.muatate_call(target).unwrap();
        }

        Ok(prog)
    }

    pub fn muatate_call(&mut self, _target: &Target) -> Result<(), failure::Error> {
        Ok(())
    }
    pub fn generate_call(&mut self, target: &Target) -> Result<(), failure::Error> {
        let target_node = self.choice_node(target).unwrap();
        let typ_vec = target_node.get_avalible_interface();

        let idx = OsRng::default().gen_range(0..typ_vec.len());
        match typ_vec[idx] {
            ITF::Topic => {
                match self.gen_topic(target, &target_node, &mut target.rng.clone()) {
                    Ok(_) => {}
                    Err(e) => return Err(e),
                }
                self.itf = ITF::Topic;
            }
            ITF::Service => {
                match self.gen_service(target, target_node, &mut target.rng.clone()) {
                    Ok(_) => {}
                    Err(e) => return Err(e),
                }
                self.itf = ITF::Service;
            }
            ITF::Action => {
                match self.gen_action(target, target_node, &mut target.rng.clone()) {
                    Ok(_) => {}
                    Err(e) => return Err(e),
                }
                self.itf = ITF::Action;
            }
            ITF::Param => {
                match self.gen_param(target, target_node, &mut target.rng.clone()) {
                    Ok(_) => {}
                    Err(e) => return Err(e),
                }
                self.itf = ITF::Param;
            }
        }
        self.serialization(&target_node.node_name);
        Ok(())
    }

    pub fn choice_node<'a>(&self, target: &'a Target) -> Result<&'a Node, failure::Error> {
        let mut target_node = &target.nodes[rand::thread_rng().gen_range(0..target.nodes.len())];

        while target_node.get_node_subscribers().len() == 0
            && target_node.get_service_server().len() == 0
            && target_node.get_action_server().len() == 0
            && target_node.get_param().len() == 0
        {
            // if all equal to 0, regenerate node
            target_node = &target.nodes[OsRng::default().gen_range(0..target.nodes.len())];
        }

        Ok(target_node)
    }

    pub fn serialization(&mut self, node_name: &String) {
        let mut call_stream = String::new();
        match self.itf {
            ITF::Topic => {
                call_stream.push_str("ros2 topic pub --once ");
            }
            ITF::Service => {
                call_stream.push_str("ros2 service call ");
            }
            ITF::Action => {
                call_stream.push_str("ros2 action send_goal ");
            }
            ITF::Param => {
                call_stream.push_str("ros2 param set ");
            }
        }
        if self.itf != ITF::Param {
            call_stream.push_str(&self.itf_name);
            call_stream.push_str(" ");
            call_stream.push_str(&self.itf_type);
            call_stream.push_str(" \"{");
            let res = &self.itf_info.get_value();
            // TODO
            let start_idx = 0;
            // match res.find(':') {
            //     Some(idx) => idx + 1,
            //     None => 0,
            // };
            call_stream.push_str(&res[start_idx..].to_string());
            // pop ", "
            call_stream.pop();
            call_stream.pop();
            call_stream.push_str("}\"");
        } else {
            call_stream.push_str(node_name);
            call_stream.push_str(" ");
            let res = &&self.itf_info.get_value();
            call_stream.push_str(res);
            call_stream.retain(|c| c != ':');
            call_stream.pop();
            call_stream.pop();
        }
        self.call_stream = call_stream;
        dbg!(&self.call_stream);
    }

    pub fn gen_topic(
        &mut self,
        _target: &Target,
        target_node: &Node,
        rng: &mut RngType,
    ) -> Result<(), failure::Error> {
        // ros2 topic pub <topic_name> <msg_type> '<args>'
        let topic_subscriber = target_node.get_node_subscribers();
        let subscriber_len = topic_subscriber.len();

        // random choose a topic to execute
        let topic_idx = rand::thread_rng().gen_range(0..subscriber_len);
        self.itf_name = topic_subscriber.keys().nth(topic_idx).unwrap().to_string();
        self.itf_type = topic_subscriber
            .get(&self.itf_name)
            .unwrap()
            .itf_type
            .clone();
        self.itf_info = topic_subscriber.get(&self.itf_name).unwrap().clone();
        match self.itf_info.gen_value(rng) {
            Ok(_) => {}
            Err(e) => {
                println!("gen topic error: {}", e);
            }
        }
        Ok(())
    }

    pub fn gen_service(
        &mut self,
        _target: &Target,
        target_node: &Node,
        rng: &mut RngType,
    ) -> Result<(), failure::Error> {
        // ros2 service call <service_name>  <service_type> <arguments>

        let service_server = target_node.get_service_server();
        let service_len = service_server.len();

        let service_idx = rand::thread_rng().gen_range(0..service_len);
        self.itf_name = service_server.keys().nth(service_idx).unwrap().to_string();
        self.itf_type = service_server.get(&self.itf_name).unwrap().itf_type.clone();
        self.itf_info = service_server.get(&self.itf_name).unwrap().clone();
        match self.itf_info.gen_value(rng) {
            Ok(_) => {}
            Err(e) => {
                println!("gen service error: {}", e);
            }
        }

        Ok(())
    }

    pub fn gen_param(
        &mut self,
        _target: &Target,
        target_node: &Node,
        rng: &mut RngType,
    ) -> Result<(), failure::Error> {
        // ros2 param load <node_name> <parameter_file>
        let _param = "ros2 param set".to_string();
        let param_list = target_node.get_param();
        let param_len = param_list.len();

        let param_idx = rand::thread_rng().gen_range(0..param_len);
        self.itf_name = param_list.keys().nth(param_idx).unwrap().to_string();
        self.itf_type = param_list.get(&self.itf_name).unwrap().itf_type.clone();
        self.itf_info = param_list.get(&self.itf_name).unwrap().clone();
        match self.itf_info.gen_value(rng) {
            Ok(_) => {}
            Err(e) => {
                println!("gen param error: {}", e);
            }
        }

        Ok(())
    }

    pub fn gen_action(
        &mut self,
        _target: &Target,
        target_node: &Node,
        rng: &mut RngType,
    ) -> Result<(), failure::Error> {
        // ros2 action send_goal <action_name> <action_type> <values>
        let action_server = target_node.get_action_server();
        let act_len = action_server.len();

        let service_idx = rand::thread_rng().gen_range(0..act_len);
        self.itf_name = action_server.keys().nth(service_idx).unwrap().to_string();
        self.itf_type = action_server.get(&self.itf_name).unwrap().itf_type.clone();
        self.itf_info = action_server.get(&self.itf_name).unwrap().clone();
        match self.itf_info.gen_value(rng) {
            Ok(_) => {}
            Err(e) => {
                println!("gen action error: {}", e);
            }
        }

        Ok(())
    }

    pub fn exec_input_prog(
        &self,
        shm_dir: &mut String,
        target: &mut Target,
        // handle: &mut RwLockWriteGuard<FuzzManager>,
    ) -> Result<(), failure::Error> {
        // check system condition before execution
        match target.check_crash() {
            Ok(_) => {}
            Err(e) => {
                return Err(e.into());
            }
        }

        match self.exec_one(target, shm_dir) {
            Ok(_) => {
                // check if the result is valid
                Ok(())
            }
            Err(e) => {
                return Err(e.into());
            }
        }
    }

    // check if a input has finish execution
    pub fn exec_one(&self, target: &mut Target, work_dir: &String) -> Result<(), failure::Error> {
        // send input to ros app
        fuzzer_info!("exec_one");
        // clean shm
        target
            .shm_region
            .allow_time_write(&(work_dir.clone() + "/shm"));
        let mut send_input_cmd = Command::new("bash")
            .env("SHM_PATH", work_dir.to_owned() + "/shm")
            .arg("-c")
            .arg(&self.call_stream)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to execute process");
        // get current timestamp
        let start_time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        dbg!(start_time);

        match &send_input_cmd
            .wait_timeout(Duration::from_secs(10))
            .unwrap()
        {
            Some(_) => {
                // child has exited
                fuzzer_info!("execution normal");
                let mut input_res = String::new();
                send_input_cmd
                    .stdout
                    .expect("filed to open stdout")
                    .read_to_string(&mut input_res)
                    .expect("Failed to read stdout");
                let mut input_err = String::new();
                send_input_cmd
                    .stderr
                    .expect("filed to open stdout")
                    .read_to_string(&mut input_err)
                    .expect("Failed to read stdout");
                input_res.push_str(&input_err);

                dbg!(&input_res);
                // check if have bad input or if cli have error
                if ERR_LOG_PATTERN.iter().any(|substring| {
                    input_res.contains(substring)
                        && !FALSE_LOG_PATTERN
                            .iter()
                            .any(|substring| input_res.contains(substring))
                }) {
                    return Err(
                        ExecError::ExecError("ros2 log error: ".to_string() + &input_res).into(),
                    );
                }

                match target.check_crash() {
                    Ok(_) => {}
                    Err(e) => {
                        return Err(e.into());
                    }
                }

                match target.check_timeout_and_interets(
                    start_time,
                    &(work_dir.to_owned() + &"/shm".to_string()),
                    &input_res,
                ) {
                    Ok(true) => {
                        target.corpus.push(self.clone());
                        return Ok(());
                    }
                    Ok(false) => return Ok(()),
                    Err(e) => {
                        return Err(e.into());
                    }
                }
            }
            None => {
                // child hasn't exited yet, most likely to be a system crash or hang, just do a reboot
                fuzzer_info!("execution timeout");
                match target.check_crash() {
                    Ok(_) => {}
                    Err(e) => {
                        return Err(e.into());
                    }
                }
                send_input_cmd.kill().unwrap();
                let mut input_res = String::new();
                send_input_cmd
                    .stdout
                    .expect("filed to open stdout")
                    .read_to_string(&mut input_res)
                    .expect("Failed to read stdout");
                println!("{}", input_res);
                if input_res.is_empty() {
                    return Ok(())

                    // Fix: this may not leading to crash, reduce unwanted false positive: system carshed, return crashed error
                    // return Err(ExecError::ExecError("ros2 is crashed ".into()).into());
                } else if HANG_LOG_PATTERN
                    .iter()
                    .any(|substring| input_res.contains(substring))
                {
                    // check if there is wait for XXX exist
                    return Err(ExecError::ExecError(
                        "ros2 error state, waiting for: ".to_string() + &input_res,
                    )
                    .into());
                } else {
                    // system hang, return hang error
                    match target.update_shm(&(work_dir.to_owned() + &"/shm".to_string())) {
                        Ok(_) => {
                            return Ok(())

                            // Fix: this may not leading to crash, reduce unwanted false positive
                            // return Err(ExecError::InvalidResult {
                            //     reason: "ros2 hang".into(),
                            // }
                            // .into())
                        }
                        Err(e) => return Err(e.into()),
                    };
                }
            }
        }
    }
}
