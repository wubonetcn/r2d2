use super::super::get_name_short;
use util::shmem::*;

#[derive(Debug, PartialEq, Clone)]
pub struct NodeInfo {
    pub id: u64,

    pub cpid: u64,

    pub handle: u64,

    pub node_name: String,

    pub node_namespace: String,

    pub edges: Vec<EdgeInfo>,

    pub callbacks: Vec<u64>,
}
impl NodeInfo {
    pub fn new(id: u64, node: &nodes) -> NodeInfo {
        let node_info = NodeInfo {
            id: id,
            cpid: node.pid,
            handle: node.handle,
            node_name: get_name_short(&node.name),
            node_namespace: get_name_short(&node.name_space),
            edges: Vec::new(),
            callbacks: Vec::new(),
        };
        return node_info;
    }

    pub fn set_pid(&mut self, pid: u64) {
        self.cpid = pid;
    }

    pub fn set_handle(&mut self, handle: u64) {
        self.handle = handle;
    }

    pub fn add_callback(&mut self, cb_id: u64) {
        self.callbacks.push(cb_id);
    }

    pub fn add_edge(&mut self, edge: EdgeInfo) {
        self.edges.push(edge);
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct EdgeInfo {
    pub start_cb: u64,

    pub end_cb: u64,

    pub end_node: u64,

    pub name: String,
}
impl EdgeInfo {
    pub fn new(start_cb: u64, end_cb: u64, end_node_handle: u64, name: String) -> EdgeInfo {
        let edge_info = EdgeInfo {
            start_cb: start_cb,
            end_cb: end_cb,
            end_node: end_node_handle,
            name: name,
        };
        return edge_info;
    }

    pub fn connect_to_node(&mut self, start_cb: u64, end_cb: u64, end_node_handle: u64) {
        self.start_cb = start_cb;
        self.end_cb = end_cb;
        self.end_node = end_node_handle;
    }
}
