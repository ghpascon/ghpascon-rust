use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AcupadTag {
    pub epc: Option<String>,
    pub tid: Option<String>,
    pub ant: i32,
    pub rssi: i32,
    pub protected: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AcupadEvent {
    Connection(bool),
    Reading(bool),
    Tag(AcupadTag),
    TagsCleared,
    SetupDone,
    SerialNumber(String),
    Receive(String),
}
