use libc::{
    pthread_mutex_init, pthread_mutex_lock, pthread_mutex_t, pthread_mutex_unlock,
    PTHREAD_MUTEX_INITIALIZER,
};
use memmap::*;
use std::fs::OpenOptions;
// import SHM_LEN and SHORT_LEN in mod.rs
use crate::{fuzzer_info, FULL_LEN, SHM_LEN, SHORT_LEN};

// share memory var which is shm_cb_name and type is arc<mut* u8>
#[derive(Debug)]
pub struct SharedMem {
    pub shm_cb_time: shared_cb_times,
    pub shm_nodes: shared_nodes,
    pub shm_callback_infos: shared_callback_infos,
    pub shm_msg: shared_msg_infos,
}
impl SharedMem {
    // new a ShareMem that is mut
    pub fn new() -> Self {
        Self {
            shm_cb_time: shared_cb_times::new(),
            shm_nodes: shared_nodes::new(),
            shm_callback_infos: shared_callback_infos::new(),
            shm_msg: shared_msg_infos::new(),
        }
    }

    // open shared memory vai mmap and return
    pub fn ros_file_mmap(&self, shm_name: &String) -> Result<MmapMut, failure::Error> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(shm_name)
            .unwrap();
        let mmap = unsafe {
            match MmapOptions::new().map_mut(&file) {
                Ok(mmap) => mmap,
                Err(e) => {
                    fuzzer_info!("mmap is empty {}", e);
                    MmapMut::map_anon(1).unwrap()
                }
            }
        };

        Ok(mmap)
    }

    pub fn mmap_load_times(&mut self, shm_path: &String) {
        let mmap_cb_time = self
            .ros_file_mmap(&String::from(shm_path.to_owned() + "/times"))
            .unwrap();
        self.set_shm_cb_time(mmap_cb_time);
    }

    pub fn mmap_load_nodes(&mut self, shm_path: &String) {
        let mmap_nodes = self
            .ros_file_mmap(&String::from(shm_path.to_owned() + "/nodes"))
            .unwrap();
        self.set_shm_nodes(mmap_nodes);
    }

    pub fn mmap_load_callback(&mut self, shm_path: &String) {
        let mmap_pub_infos = self
            .ros_file_mmap(&String::from(shm_path.to_owned() + "/callbacks"))
            .unwrap();
        self.set_shm_callback_infos(mmap_pub_infos);
    }

    pub fn mmap_load_msg(&mut self, shm_path: &String) {
        let mmap_msg_infos = self
            .ros_file_mmap(&String::from(shm_path.to_owned() + "/msg"))
            .unwrap();
        self.set_shm_msg(mmap_msg_infos);
    }

    // load share memory here passing arg ShareMem
    pub fn mmap_load_info(&mut self, shm_path: &String) {
        self.mmap_load_times(shm_path);
        self.mmap_load_nodes(shm_path);
        self.mmap_load_callback(shm_path);
        self.mmap_load_msg(shm_path);
    }

    pub fn get_shm_msg(&self) -> [shared_msg; SHM_LEN] {
        self.shm_msg.msgs
    }

    pub fn set_shm_msg(&mut self, mmap_msg_infos: MmapMut) {
        self.shm_msg.msgs = unsafe {
            std::slice::from_raw_parts(mmap_msg_infos.as_ptr() as *const shared_msg, SHM_LEN)
        }
        .try_into()
        .unwrap();
    }

    pub fn clean_shm_msg(&mut self, work_dir: &String) {
        let mut mmap_cb_msg = self
            .ros_file_mmap(&String::from(work_dir.to_owned() + "/msgs"))
            .unwrap();
        let shm_msg = unsafe { &mut *(mmap_cb_msg.as_mut_ptr() as *mut shared_msg_infos) };
        unsafe {
            pthread_mutex_lock(&mut shm_msg.mutex);
            libc::memset(
                shm_msg as *mut shared_msg_infos as *mut libc::c_void,
                0,
                std::mem::size_of::<shared_msg_infos>(),
            );
            shm_msg.count = 0;
            pthread_mutex_unlock(&mut shm_msg.mutex);
        }
    }

    pub fn get_shm_nodes(&self) -> [nodes; SHM_LEN] {
        self.shm_nodes.nodes
    }
    pub fn set_shm_nodes(&mut self, mmap_nodes: MmapMut) {
        self.shm_nodes.nodes =
            unsafe { std::slice::from_raw_parts(mmap_nodes.as_ptr() as *const nodes, SHM_LEN) }
                .try_into()
                .unwrap();
    }
    pub fn clean_shm_nodes(&mut self, work_dir: &String) {
        let mut mmap_cb_node = self
            .ros_file_mmap(&String::from(work_dir.to_owned() + "/nodes"))
            .unwrap();
        let shm_node = unsafe { &mut *(mmap_cb_node.as_mut_ptr() as *mut shared_nodes) };
        unsafe {
            pthread_mutex_lock(&mut shm_node.mutex);
            libc::memset(
                shm_node as *mut shared_nodes as *mut libc::c_void,
                0,
                std::mem::size_of::<shared_nodes>(),
            );
            shm_node.count = 0;
            pthread_mutex_unlock(&mut shm_node.mutex);
        }
    }

    pub fn get_shm_callback_infos(&self) -> [callback_infos; SHM_LEN] {
        self.shm_callback_infos.callback_infos
    }
    pub fn set_shm_callback_infos(&mut self, mmap_callback_infos: MmapMut) {
        self.shm_callback_infos.callback_infos = unsafe {
            std::slice::from_raw_parts(
                mmap_callback_infos.as_ptr() as *const callback_infos,
                SHM_LEN,
            )
        }
        .try_into()
        .unwrap();
    }
    pub fn clean_shm_callback_infos(&mut self, work_dir: &String) {
        let mut mmap_cb_pub = self
            .ros_file_mmap(&String::from(work_dir.to_owned() + "/callbacks"))
            .unwrap();
        let shm_pub = unsafe { &mut *(mmap_cb_pub.as_mut_ptr() as *mut shared_callback_infos) };
        unsafe {
            pthread_mutex_lock(&mut shm_pub.mutex);
            libc::memset(
                shm_pub as *mut shared_callback_infos as *mut libc::c_void,
                0,
                std::mem::size_of::<shared_callback_infos>(),
            );
            shm_pub.count = 0;
            pthread_mutex_unlock(&mut shm_pub.mutex);
        }
    }

    pub fn get_shm_cb_time(&self) -> [cb_times; SHM_LEN] {
        self.shm_cb_time.cb_times
    }
    pub fn set_shm_cb_time(&mut self, mut mmap_cb_time: MmapMut) {
        self.shm_cb_time.cb_times = unsafe {
            std::slice::from_raw_parts(mmap_cb_time.as_mut_ptr() as *mut cb_times, SHM_LEN)
        }
        .try_into()
        .unwrap();

        // sort cb_times based on time.time
        self.shm_cb_time
            .cb_times
            .sort_by(|a, b| a.time.cmp(&b.time));
    }

    pub fn get_mut_shm_cb_time(
        &mut self,
        shm_path: &String,
    ) -> Result<&mut shared_cb_times, failure::Error> {
        // unmmap self.mmap_cb_time
        let mut mmap_cb_time = self
            .ros_file_mmap(&String::from(shm_path.to_owned() + "/times"))
            .unwrap();
        let shm_cb_time = unsafe { &mut *(mmap_cb_time.as_mut_ptr() as *mut shared_cb_times) };
        Ok(shm_cb_time)
    }

    pub fn allow_time_write(&mut self, work_dir: &String) {
        let mut mmap_cb_time = self
            .ros_file_mmap(&String::from(work_dir.to_owned() + "/times"))
            .unwrap();
        let shm_cb_time = unsafe { &mut *(mmap_cb_time.as_mut_ptr() as *mut shared_cb_times) };

        // let mut mmap_node = self
        //     .ros_file_mmap(&String::from(work_dir.to_owned() + "/nodes"))
        //     .unwrap();
        // let shm_node = unsafe { &mut *(mmap_node.as_mut_ptr() as *mut shared_nodes) };

        // let mut mmap_cb = self
        //     .ros_file_mmap(&String::from(work_dir.to_owned() + "/callbacks"))
        //     .unwrap();
        // let shm_cb = unsafe { &mut *(mmap_cb.as_mut_ptr() as *mut shared_callback_infos) };
        // clean shm_cb_time.cb_time
        unsafe {
            // TODO: may need further lock here
            // pthread_mutex_lock(&mut shm_cb_time.mutex);
            libc::memset(
                shm_cb_time as *mut shared_cb_times as *mut libc::c_void,
                0,
                std::mem::size_of::<shared_cb_times>(),
            );
            shm_cb_time.count = 0;
            // pthread_mutex_unlock(&mut shm_cb_time.mutex);
            // libc::memset(
            //     shm_node as *mut shared_nodes as *mut libc::c_void,
            //     0,
            //     std::mem::size_of::<shared_nodes>(),
            // );
            // shm_node.count = 0;
            // libc::memset(
            //     shm_cb as *mut shared_callback_infos as *mut libc::c_void,
            //     0,
            //     std::mem::size_of::<shared_callback_infos>(),
            // );
            // shm_cb.count = 0;
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct shared_cb_times {
    // array of cb_times
    pub cb_times: [cb_times; SHM_LEN],
    pub count: i32,
    pub mutex: pthread_mutex_t,
}
impl shared_cb_times {
    // new a mutable
    pub fn new() -> Self {
        Self {
            cb_times: [cb_times::new(); SHM_LEN],
            count: 0,
            mutex: PTHREAD_MUTEX_INITIALIZER,
        }
    }

    pub fn set_fd(&mut self, path: &String) -> std::io::Result<()> {
        OpenOptions::new().create(true).write(true).open(path)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[repr(C)]
pub struct cb_times {
    pub cb: u64,
    pub time: u64,
    pub flag: u64,
    pub message_size: u64,
    pub rmw_handle: u64,
}
impl cb_times {
    pub fn new() -> Self {
        cb_times {
            cb: 0,
            time: 0,
            flag: 0,
            message_size: 0,
            rmw_handle: 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct shared_nodes {
    // array of nodes
    pub nodes: [nodes; SHM_LEN],
    pub count: i32,
    pub mutex: pthread_mutex_t,
}
impl shared_nodes {
    pub fn new() -> Self {
        let mut node = shared_nodes {
            nodes: [nodes::new(); SHM_LEN],
            count: 0,
            mutex: PTHREAD_MUTEX_INITIALIZER,
        };
        unsafe {
            pthread_mutex_init(&mut node.mutex, std::ptr::null());
        }
        node
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct nodes {
    pub name: [u8; SHORT_LEN],
    pub name_space: [u8; SHORT_LEN],
    pub handle: u64,
    pub pid: u64,
}
impl nodes {
    pub fn new() -> Self {
        Self {
            name: [0; SHORT_LEN],
            name_space: [0; SHORT_LEN],
            handle: 0,
            pid: 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct shared_callback_infos {
    // array of callback_infos
    pub callback_infos: [callback_infos; SHM_LEN],
    pub count: i32,
    pub mutex: pthread_mutex_t,
}
impl shared_callback_infos {
    pub fn new() -> Self {
        let mut info = shared_callback_infos {
            callback_infos: [callback_infos::new(); SHM_LEN],
            count: 0,
            mutex: PTHREAD_MUTEX_INITIALIZER,
        };
        unsafe {
            pthread_mutex_init(&mut info.mutex, std::ptr::null());
        }
        info
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct callback_infos {
    pub cb_type: u64,
    pub stage: u64,
    pub idx: u64,
    pub pid: u64,
    pub period: u64,
    pub rcl_handle: u64,
    pub rmw_handle: u64,
    pub node_handle: u64,
    pub rclcpp_handle: u64,
    pub rclcpp_handle1: u64,
    pub cb_name: [u8; SHORT_LEN],
    pub function_symbol: [u8; FULL_LEN],
}
impl callback_infos {
    pub fn new() -> Self {
        Self {
            cb_type: 0,
            stage: 0,
            idx: 0,
            pid: 0,
            period: 0,
            rcl_handle: 0,
            rmw_handle: 0,
            node_handle: 0,
            rclcpp_handle: 0,
            rclcpp_handle1: 0,
            cb_name: [0; SHORT_LEN],
            function_symbol: [0; FULL_LEN],
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct shared_msg_infos {
    // array of callback_infos
    pub msgs: [shared_msg; SHM_LEN],
    pub count: i32,
    pub mutex: pthread_mutex_t,
}
impl shared_msg_infos {
    pub fn new() -> Self {
        let mut info: shared_msg_infos = shared_msg_infos {
            msgs: [shared_msg::new(); SHM_LEN],
            count: 0,
            mutex: PTHREAD_MUTEX_INITIALIZER,
        };
        unsafe {
            pthread_mutex_init(&mut info.mutex, std::ptr::null());
        }
        info
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct shared_msg {
    pub subscription: u64,
    pub callback: u64,
    pub size: u64,
    pub send_time: u64,
    pub recv_time: u64,
}
impl shared_msg {
    pub fn new() -> Self {
        Self {
            subscription: 0,
            callback: 0,
            size: 0,
            send_time: 0,
            recv_time: 0,
        }
    }
}
