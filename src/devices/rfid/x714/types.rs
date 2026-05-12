use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct X714Tag {
    pub epc: Option<String>,
    pub tid: Option<String>,
    pub ant: i32,
    pub rssi: i32,
    pub protected: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum X714Event {
    Connection(bool),
    Reading(bool),
    Tag(X714Tag),
    TagsCleared,
    SetupDone,
    SerialNumber(String),
    Receive(String),
}
