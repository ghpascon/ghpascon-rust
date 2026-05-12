use serde_json::Value;

/// A tag received from the R700 data stream.
#[derive(Debug, Clone)]
pub struct R700Tag {
    pub epc: Option<String>,
    pub tid: Option<String>,
    pub ant: i32,
    /// RSSI in dBm (already divided by 100 from the raw cdbm value).
    pub rssi: i32,
    pub protected: bool,
}

/// Events dispatched by the R700 reader.
#[derive(Debug, Clone)]
pub enum R700Event {
    /// Connection state changed (`true` = connected).
    Connection(bool),
    /// Inventory (reading) state changed (`true` = running).
    Reading(bool),
    /// A tag was detected.
    Tag(R700Tag),
    /// Reader serial number received from `/api/v1/system`.
    SerialNumber(String),
}

impl R700Tag {
    /// Build an `R700Tag` from the raw `tagInventoryEvent` JSON object.
    pub fn from_json(obj: &Value) -> Self {
        let epc = obj
            .get("epcHex")
            .and_then(|v| v.as_str())
            .map(str::to_lowercase);
        let tid = obj
            .get("tidHex")
            .and_then(|v| v.as_str())
            .map(str::to_lowercase);
        let ant = obj.get("antennaPort").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let rssi_cdbm = obj
            .get("peakRssiCdbm")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let rssi = (rssi_cdbm / 100) as i32;

        Self {
            epc: epc.filter(|s| !s.is_empty()),
            tid: tid.filter(|s| !s.is_empty()),
            ant,
            rssi,
            protected: false,
        }
    }
}
