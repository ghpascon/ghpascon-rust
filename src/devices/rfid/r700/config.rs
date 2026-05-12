use std::collections::HashMap;

use serde_json::{Map, Number, Value, json};

pub type ParamMap = HashMap<String, Value>;

// ─── Antenna ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AntennaConfig {
    pub antenna_port: u8,
    pub estimated_tag_population: u32,
    pub fast_id: bool,
    pub inventory_search_mode: String,
    pub inventory_session: u8,
    pub receive_sensitivity_dbm: i32,
    pub rf_mode: u32,
    pub transmit_power_cdbm: i32,
    pub protected_mode_pin_hex: Option<String>,
}

impl Default for AntennaConfig {
    fn default() -> Self {
        Self {
            antenna_port: 1,
            estimated_tag_population: 16,
            fast_id: true,
            inventory_search_mode: "dual-target".to_string(),
            inventory_session: 1,
            receive_sensitivity_dbm: -80,
            rf_mode: 4,
            transmit_power_cdbm: 3000,
            protected_mode_pin_hex: None,
        }
    }
}

impl AntennaConfig {
    pub fn to_json(&self) -> Value {
        let mut obj = Map::new();
        obj.insert("antennaPort".to_string(), json!(self.antenna_port));
        obj.insert(
            "estimatedTagPopulation".to_string(),
            json!(self.estimated_tag_population),
        );
        obj.insert(
            "fastId".to_string(),
            json!(if self.fast_id { "enabled" } else { "disabled" }),
        );
        obj.insert(
            "inventorySearchMode".to_string(),
            json!(self.inventory_search_mode),
        );
        obj.insert(
            "inventorySession".to_string(),
            json!(self.inventory_session),
        );
        obj.insert(
            "receiveSensitivityDbm".to_string(),
            json!(self.receive_sensitivity_dbm),
        );
        obj.insert("rfMode".to_string(), json!(self.rf_mode));
        obj.insert(
            "transmitPowerCdbm".to_string(),
            json!(self.transmit_power_cdbm),
        );
        if let Some(pin) = &self.protected_mode_pin_hex {
            obj.insert("protectedModePinHex".to_string(), json!(pin));
        }
        Value::Object(obj)
    }
}

// ─── Main config ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct R700Config {
    pub name: String,
    pub ip: String,
    pub username: String,
    pub password: String,
    pub start_reading: bool,
    pub firmware_version: Option<String>,
    pub session: u8,
    pub read_power: i32,
    pub read_rssi: i32,
    pub search_mode: String,
    pub rf_mode: u32,
    pub gpi_start: bool,
    pub protected_inventory_active: bool,
    pub protected_inventory_password: String,
    pub reconnection_time: u64,
    pub active_ant: Vec<u8>,
}

impl Default for R700Config {
    fn default() -> Self {
        Self {
            name: "R700".to_string(),
            ip: "192.168.1.101".to_string(),
            username: "root".to_string(),
            password: "impinj".to_string(),
            start_reading: false,
            firmware_version: None,
            session: 1,
            read_power: 3000,
            read_rssi: -80,
            search_mode: "dual-target".to_string(),
            rf_mode: 4,
            gpi_start: false,
            protected_inventory_active: false,
            protected_inventory_password: "12345678".to_string(),
            reconnection_time: 2,
            active_ant: vec![1],
        }
    }
}

impl R700Config {
    /// Build the JSON body sent to `POST /profiles/inventory/start`.
    pub fn build_reading_config(&self) -> Value {
        let antennas: Vec<Value> = self
            .active_ant
            .iter()
            .map(|&port| {
                let mut ant = AntennaConfig {
                    antenna_port: port,
                    inventory_session: self.session,
                    receive_sensitivity_dbm: self.read_rssi,
                    transmit_power_cdbm: self.read_power,
                    inventory_search_mode: self.search_mode.clone(),
                    rf_mode: self.rf_mode,
                    ..AntennaConfig::default()
                };
                if self.protected_inventory_active {
                    ant.protected_mode_pin_hex = Some(self.protected_inventory_password.clone());
                }
                ant.to_json()
            })
            .collect();

        let mut cfg = Map::new();
        cfg.insert("antennaConfigs".to_string(), Value::Array(antennas));

        if self.gpi_start {
            cfg.insert(
                "startTriggers".to_string(),
                json!([{"gpiTransitionEvent": {"gpi": 1, "transition": "high-to-low"}}]),
            );
            cfg.insert(
                "stopTriggers".to_string(),
                json!([{"gpiTransitionEvent": {"gpi": 1, "transition": "low-to-high"}}]),
            );
        }

        cfg.insert(
            "eventConfig".to_string(),
            json!({
                "common": {"hostname": "disabled"},
                "tagInventory": {
                    "epc": "disabled",
                    "epcHex": "enabled",
                    "xpcHex": "disabled",
                    "tid": "disabled",
                    "tidHex": "enabled",
                    "antennaPort": "enabled",
                    "transmitPowerCdbm": "disabled",
                    "peakRssiCdbm": "enabled",
                    "frequency": "disabled",
                    "pc": "disabled",
                    "lastSeenTime": "disabled",
                    "phaseAngle": "disabled",
                    "tagReporting": {
                        "reportingIntervalSeconds": 1,
                        "tagCacheSize": 2048,
                        "antennaIdentifier": "antennaPort",
                        "tagIdentifier": "epc"
                    }
                }
            }),
        );

        Value::Object(cfg)
    }

    pub fn base_url(&self) -> String {
        format!("https://{}/api/v1", self.ip)
    }

    // ── from_map / to_map ─────────────────────────────────────────────────────

    pub fn from_map(params: ParamMap) -> Result<Self, String> {
        let mut cfg = Self::default();

        if let Some(Value::String(v)) = params.get("name") {
            cfg.name = v.clone();
        }
        if let Some(Value::String(v)) = params.get("ip") {
            cfg.ip = v.clone();
        }
        if let Some(Value::String(v)) = params.get("username") {
            cfg.username = v.clone();
        }
        if let Some(Value::String(v)) = params.get("password") {
            cfg.password = v.clone();
        }
        if let Some(Value::Bool(v)) = params.get("start_reading") {
            cfg.start_reading = *v;
        }
        if let Some(Value::String(v)) = params.get("firmware_version") {
            cfg.firmware_version = Some(v.clone());
        }
        if let Some(v) = params.get("session").and_then(|v| v.as_u64()) {
            cfg.session = (v.clamp(0, 3)) as u8;
        }
        if let Some(v) = params.get("read_power").and_then(|v| v.as_i64()) {
            let mut p = v as i32;
            if p < 1000 {
                p *= 100;
            }
            if !(1000..=3300).contains(&p) {
                p = 3000;
            }
            cfg.read_power = p;
        }
        if let Some(v) = params.get("read_rssi").and_then(|v| v.as_i64()) {
            cfg.read_rssi = v as i32;
        }
        if let Some(Value::String(v)) = params.get("search_mode") {
            if matches!(v.as_str(), "single-target" | "dual-target") {
                cfg.search_mode = v.clone();
            }
        }
        if let Some(v) = params.get("rf_mode").and_then(|v| v.as_u64()) {
            cfg.rf_mode = v as u32;
        }
        if let Some(Value::Bool(v)) = params.get("gpi_start") {
            cfg.gpi_start = *v;
            if *v {
                cfg.start_reading = false;
            }
        }
        if let Some(Value::Bool(v)) = params.get("protected_inventory_active") {
            cfg.protected_inventory_active = *v;
        }
        if let Some(Value::String(v)) = params.get("protected_inventory_password") {
            cfg.protected_inventory_password = v.clone();
        }
        if let Some(v) = params.get("reconnection_time").and_then(|v| v.as_u64()) {
            cfg.reconnection_time = v;
        }
        if let Some(Value::Array(arr)) = params.get("active_ant") {
            cfg.active_ant = arr
                .iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect();
            if cfg.active_ant.is_empty() {
                cfg.active_ant = vec![1];
            }
        }

        Ok(cfg)
    }

    pub fn to_map(&self) -> ParamMap {
        let mut map = HashMap::new();
        map.insert("name".to_string(), Value::String(self.name.clone()));
        map.insert("ip".to_string(), Value::String(self.ip.clone()));
        map.insert("username".to_string(), Value::String(self.username.clone()));
        map.insert("password".to_string(), Value::String(self.password.clone()));
        map.insert("start_reading".to_string(), Value::Bool(self.start_reading));
        map.insert(
            "firmware_version".to_string(),
            self.firmware_version
                .clone()
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        map.insert(
            "session".to_string(),
            Value::Number(Number::from(self.session)),
        );
        map.insert(
            "read_power".to_string(),
            Value::Number(Number::from(self.read_power)),
        );
        map.insert(
            "read_rssi".to_string(),
            Value::Number(Number::from(self.read_rssi)),
        );
        map.insert(
            "search_mode".to_string(),
            Value::String(self.search_mode.clone()),
        );
        map.insert(
            "rf_mode".to_string(),
            Value::Number(Number::from(self.rf_mode)),
        );
        map.insert("gpi_start".to_string(), Value::Bool(self.gpi_start));
        map.insert(
            "protected_inventory_active".to_string(),
            Value::Bool(self.protected_inventory_active),
        );
        map.insert(
            "protected_inventory_password".to_string(),
            Value::String(self.protected_inventory_password.clone()),
        );
        map.insert(
            "reconnection_time".to_string(),
            Value::Number(Number::from(self.reconnection_time)),
        );
        map.insert(
            "active_ant".to_string(),
            Value::Array(
                self.active_ant
                    .iter()
                    .map(|&a| Value::Number(Number::from(a)))
                    .collect(),
            ),
        );
        map
    }
}
