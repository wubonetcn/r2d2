use serde::Serialize;

pub mod callback;
pub mod callgraph;
pub mod cover;
pub mod event_trace;
pub mod node;
pub mod service_trace;
pub mod timer_trace;
pub mod topic_trace;

//  struct to store callback info
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum CallbackType {
    Subscriber,
    Publisher,
    Service,
    Client,
    Timer,
    Other,
}
impl Default for CallbackType {
    fn default() -> Self {
        CallbackType::Other
    }
}
