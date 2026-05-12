use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use dashmap::DashMap;
use serde_json::{Map, Number, Value};

pub type TagRecord = Map<String, Value>;
pub type Tag = Arc<Mutex<TagRecord>>;

pub fn make_tag(epc: &str, tid: Option<&str>, rssi: i64, ant: u64) -> HashMap<String, Value> {
    let mut tag = HashMap::new();
    tag.insert("epc".to_string(), Value::String(epc.to_string()));
    tag.insert(
        "tid".to_string(),
        tid.map(|value| Value::String(value.to_string()))
            .unwrap_or(Value::Null),
    );
    tag.insert("rssi".to_string(), Value::Number(Number::from(rssi)));
    tag.insert("ant".to_string(), Value::Number(Number::from(ant)));
    tag
}

#[derive(Debug, Clone)]
pub struct TagListBuilder {
    prefix: Option<Vec<String>>,
}

impl Default for TagListBuilder {
    fn default() -> Self {
        Self { prefix: None }
    }
}

impl TagListBuilder {
    pub fn prefix(mut self, prefix: Vec<&str>) -> Self {
        let cleaned = prefix
            .into_iter()
            .map(normalize_hex)
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>();

        self.prefix = if cleaned.is_empty() {
            None
        } else {
            Some(cleaned)
        };
        self
    }

    pub fn prefix_from_str(mut self, prefix_csv: &str) -> Self {
        let cleaned = prefix_csv
            .split(',')
            .map(normalize_hex)
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>();

        self.prefix = if cleaned.is_empty() {
            None
        } else {
            Some(cleaned)
        };
        self
    }

    pub fn build(self) -> TagList {
        TagList::new(self.prefix)
    }
}

#[derive(Debug)]
pub struct TagList {
    tags: DashMap<String, Tag>,
    epc_to_keys: DashMap<String, Vec<String>>,
    prefix: Option<Vec<String>>,
    chip_map: HashMap<String, String>,
}

impl Default for TagList {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl TagList {
    pub fn builder() -> TagListBuilder {
        TagListBuilder::default()
    }

    pub fn new(prefix: Option<Vec<String>>) -> Self {
        Self {
            tags: DashMap::new(),
            epc_to_keys: DashMap::new(),
            prefix,
            chip_map: default_chip_map(),
        }
    }

    pub fn len(&self) -> usize {
        self.tags.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tags.is_empty()
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.tags.contains_key(&normalize_key(key))
    }

    pub fn add(&self, mut tag: HashMap<String, Value>, device: &str) -> (bool, Option<Tag>) {
        let epc = match self.extract_epc(&tag) {
            Ok(v) => v,
            Err(_) => return (false, None),
        };
        if self.validate_prefix(&epc).is_err() {
            return (false, None);
        }
        let tid = match self.extract_tid(&tag) {
            Ok(v) => v,
            Err(_) => return (false, None),
        };

        let key = make_primary_key(&epc, tid.as_deref());
        tag.insert("epc".to_string(), Value::String(epc.clone()));
        tag.insert(
            "tid".to_string(),
            tid.clone().map(Value::String).unwrap_or(Value::Null),
        );

        let now = Utc::now().to_rfc3339();
        let incoming = hash_map_to_record(tag);

        if let Some(arc_ref) = self.tags.get(&key) {
            let arc = arc_ref.clone();
            drop(arc_ref);

            let old_epc;
            let updated_epc;
            {
                let mut current = arc.lock().unwrap();

                old_epc = current
                    .get("epc")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| epc.clone());

                for (field, value) in incoming {
                    current.insert(field, value);
                }

                current.insert("timestamp".to_string(), Value::String(now));
                current.insert("device".to_string(), Value::String(device.to_string()));

                let next_count = current.get("count").and_then(Value::as_u64).unwrap_or(1) + 1;
                current.insert("count".to_string(), Value::Number(Number::from(next_count)));

                let chip_name = resolve_chip_name(&self.chip_map, current.get("tid"));
                current.insert("chip".to_string(), Value::String(chip_name));

                updated_epc = current
                    .get("epc")
                    .and_then(Value::as_str)
                    .map(normalize_hex)
                    .unwrap_or_else(|| epc.clone());
            }

            if old_epc != updated_epc {
                self.unlink_epc_key(&old_epc, &key);
                self.link_epc_key(&updated_epc, &key);
            }

            return (false, Some(arc));
        }

        let mut record = incoming;

        record.insert("count".to_string(), Value::Number(Number::from(1_u64)));
        record.insert("device".to_string(), Value::String(device.to_string()));
        record.insert("timestamp".to_string(), Value::String(now.clone()));
        record.insert("first_seen".to_string(), Value::String(now));
        record.insert(
            "chip".to_string(),
            Value::String(resolve_chip_name(&self.chip_map, record.get("tid"))),
        );

        let arc = Arc::new(Mutex::new(record));
        self.tags.insert(key.clone(), arc.clone());
        self.link_epc_key(&epc, &key);

        (true, Some(arc))
    }

    pub fn get_all(&self) -> Vec<TagRecord> {
        self.tags
            .iter()
            .map(|entry| entry.lock().unwrap().clone())
            .collect()
    }

    pub fn get_all_sorted(&self) -> Vec<TagRecord> {
        let mut items = self
            .tags
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().lock().unwrap().clone()))
            .collect::<Vec<_>>();
        items.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        items.into_iter().map(|(_, tag)| tag).collect::<Vec<_>>()
    }

    pub fn get_n(&self, n: usize) -> Vec<TagRecord> {
        self.tags
            .iter()
            .take(n)
            .map(|entry| entry.value().lock().unwrap().clone())
            .collect::<Vec<_>>()
    }

    pub fn get_n_sorted(&self, n: usize) -> Vec<TagRecord> {
        self.get_all_sorted()
            .into_iter()
            .take(n)
            .collect::<Vec<_>>()
    }

    pub fn get_by_key(&self, key: &str) -> Option<Tag> {
        self.tags
            .get(&normalize_key(key))
            .map(|entry| entry.clone())
    }

    pub fn get_by_tid(&self, tid: &str) -> Option<Tag> {
        self.get_by_key(&normalize_hex(tid))
    }

    pub fn get_by_epc(&self, epc: &str) -> Option<Tag> {
        let epc = normalize_hex(epc);
        let keys = self.epc_to_keys.get(&epc)?;
        let first = keys.first()?.clone();
        self.get_by_key(&first)
    }

    pub fn get_by_identifier(&self, value: &str, identifier_type: &str) -> Option<Tag> {
        match identifier_type {
            "tid" => self.get_by_tid(value),
            _ => self.get_by_epc(value),
        }
    }

    pub fn get_epcs(&self) -> Vec<String> {
        self.epc_to_keys
            .iter()
            .map(|entry| entry.key().clone())
            .collect::<Vec<_>>()
    }

    pub fn get_n_epcs(&self, limit: usize) -> Vec<String> {
        self.epc_to_keys
            .iter()
            .take(limit)
            .map(|entry| entry.key().clone())
            .collect::<Vec<_>>()
    }

    pub fn get_tids(&self, limit: Option<usize>) -> Vec<String> {
        self.get_all_sorted()
            .into_iter()
            .filter_map(|tag| tag.get("tid").and_then(Value::as_str).map(str::to_string))
            .take(limit.unwrap_or(usize::MAX))
            .collect::<Vec<_>>()
    }

    pub fn get_tids_from_epc(&self, epc: &str) -> Vec<String> {
        let epc = normalize_hex(epc);
        let Some(keys) = self.epc_to_keys.get(&epc) else {
            return Vec::new();
        };

        keys.iter()
            .map(|key| {
                if let Some(stripped) = key.strip_prefix('_') {
                    stripped.to_string()
                } else {
                    key.clone()
                }
            })
            .collect::<Vec<_>>()
    }

    pub fn get_tid_from_epc(&self, epc: &str) -> Option<String> {
        self.get_tids_from_epc(epc).into_iter().next()
    }

    pub fn clear(&self) {
        self.tags.clear();
        self.epc_to_keys.clear();
    }

    pub fn remove_by_key(&self, key: &str) -> Option<TagRecord> {
        let key = normalize_key(key);
        let (_, removed_arc) = self.tags.remove(&key)?;
        let removed = removed_arc.lock().unwrap().clone();

        let epc = removed
            .get("epc")
            .and_then(Value::as_str)
            .map(normalize_hex)
            .unwrap_or_default();

        if !epc.is_empty() {
            self.unlink_epc_key(&epc, &key);
        }

        Some(removed)
    }

    pub fn remove_by_tid(&self, tid: &str) -> Option<TagRecord> {
        self.remove_by_key(&normalize_hex(tid))
    }

    pub fn remove_by_epc(&self, epc: &str) -> Vec<TagRecord> {
        let epc = normalize_hex(epc);
        let keys = self
            .epc_to_keys
            .get(&epc)
            .map(|entry| entry.clone())
            .unwrap_or_default();

        keys.into_iter()
            .filter_map(|key| self.remove_by_key(&key))
            .collect::<Vec<_>>()
    }

    fn extract_epc(&self, tag: &HashMap<String, Value>) -> Result<String, String> {
        let epc = tag
            .get("epc")
            .and_then(Value::as_str)
            .map(normalize_hex)
            .ok_or_else(|| "Tag missing 'epc'".to_string())?;

        if epc.is_empty() {
            return Err("Tag missing 'epc'".to_string());
        }

        if !is_hex(&epc) {
            return Err("Tag 'epc' must be hexadecimal".to_string());
        }

        Ok(epc)
    }

    fn extract_tid(&self, tag: &HashMap<String, Value>) -> Result<Option<String>, String> {
        let Some(value) = tag.get("tid") else {
            return Ok(None);
        };

        if value.is_null() {
            return Ok(None);
        }

        let tid = value
            .as_str()
            .map(normalize_hex)
            .ok_or_else(|| "Tag 'tid' must be a string".to_string())?;

        if tid.is_empty() {
            return Ok(None);
        }

        if !is_hex(&tid) {
            return Err("Tag 'tid' must be hexadecimal".to_string());
        }

        Ok(Some(tid))
    }

    fn validate_prefix(&self, epc: &str) -> Result<(), String> {
        let Some(prefix) = &self.prefix else {
            return Ok(());
        };

        if prefix.iter().any(|item| epc.starts_with(item)) {
            return Ok(());
        }

        Err("Tag filtered by prefix".to_string())
    }

    fn link_epc_key(&self, epc: &str, key: &str) {
        let mut keys = self.epc_to_keys.entry(epc.to_string()).or_default();
        if !keys.iter().any(|current| current == key) {
            keys.push(key.to_string());
        }
    }

    fn unlink_epc_key(&self, epc: &str, key: &str) {
        if let Some(mut keys) = self.epc_to_keys.get_mut(epc) {
            keys.retain(|current| current != key);
            if keys.is_empty() {
                drop(keys);
                self.epc_to_keys.remove(epc);
            }
        }
    }
}

fn hash_map_to_record(map: HashMap<String, Value>) -> TagRecord {
    map.into_iter().collect::<TagRecord>()
}

fn normalize_key(value: &str) -> String {
    let normalized = value.trim().to_lowercase();
    if normalized.starts_with('_') {
        normalized
    } else {
        normalize_hex(&normalized)
    }
}

fn normalize_hex(value: &str) -> String {
    value.trim().to_lowercase()
}

fn make_primary_key(epc: &str, tid: Option<&str>) -> String {
    match tid {
        Some(value) if !value.is_empty() => normalize_hex(value),
        _ => format!("_{}", normalize_hex(epc)),
    }
}

fn is_hex(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|c| c.is_ascii_hexdigit())
}

fn resolve_chip_name(chip_map: &HashMap<String, String>, tid_value: Option<&Value>) -> String {
    let Some(Value::String(tid)) = tid_value else {
        return "Unknown".to_string();
    };

    let mut key = tid.chars().take(8).collect::<String>().to_lowercase();
    if !key.starts_with('e') {
        key.insert(0, 'e');
    }

    chip_map
        .get(&key)
        .cloned()
        .unwrap_or_else(|| "Unknown".to_string())
}

fn default_chip_map() -> HashMap<String, String> {
    let mut chip_map = HashMap::new();
    chip_map.insert("e2801114".to_string(), "Impinj Monza 4i".to_string());
    chip_map.insert("e2801100".to_string(), "Impinj Monza 4D".to_string());
    chip_map.insert("e2801105".to_string(), "Impinj Monza 4QT".to_string());
    chip_map.insert("e2801160".to_string(), "Impinj Monza R6".to_string());
    chip_map.insert("e2801170".to_string(), "Impinj Monza R6-P".to_string());
    chip_map.insert("e2801191".to_string(), "Impinj M730".to_string());
    chip_map.insert("e2806915".to_string(), "Impinj M730".to_string());
    chip_map.insert("e2801190".to_string(), "Impinj M750".to_string());
    chip_map.insert("e28011a0".to_string(), "Impinj M770".to_string());
    chip_map.insert("e28011c0".to_string(), "Impinj M780".to_string());
    chip_map.insert("e28011c1".to_string(), "Impinj M781".to_string());
    chip_map.insert("e28011b0".to_string(), "Impinj M800".to_string());
    chip_map.insert("e2806894".to_string(), "NXP Ucode 8".to_string());
    chip_map.insert("e2806994".to_string(), "NXP Ucode 8m".to_string());
    chip_map.insert("e2806995".to_string(), "NXP Ucode 9".to_string());
    chip_map.insert("e2806a16".to_string(), "NXP Ucode 9XE".to_string());
    chip_map.insert("e2806897".to_string(), "NXP Ucode 9XM".to_string());
    chip_map.insert("e2003412".to_string(), "Alien Higgs 3".to_string());
    chip_map.insert("e2003811".to_string(), "Alien Higgs-EC".to_string());
    chip_map.insert("e2003821".to_string(), "Alien Higgs 9".to_string());
    chip_map.insert("e2803821".to_string(), "Alien Higgs 9".to_string());
    chip_map.insert("e2803813".to_string(), "Alien Higgs 10".to_string());
    chip_map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_returns_new_then_existing() {
        let list = TagList::default();

        let (is_new, tag) = list.add(
            make_tag(
                "E28011606000020000000001",
                Some("E28011052000701234567890"),
                -55,
                1,
            ),
            "reader-a",
        );
        assert!(is_new);
        assert!(tag.is_some());

        let (is_new, tag) = list.add(
            make_tag(
                "E28011606000020000000001",
                Some("E28011052000701234567890"),
                -55,
                1,
            ),
            "reader-a",
        );
        assert!(!is_new);
        let tag = tag.unwrap();
        let record = tag.lock().unwrap();
        assert_eq!(record.get("count").and_then(Value::as_u64), Some(2));
    }

    #[test]
    fn epc_index_supports_more_than_one_tid() {
        let list = TagList::default();

        list.add(
            make_tag(
                "E28011606000020000000002",
                Some("E28011052000701234567891"),
                -55,
                1,
            ),
            "reader-a",
        );
        list.add(
            make_tag(
                "E28011606000020000000002",
                Some("E28011052000701234567892"),
                -55,
                1,
            ),
            "reader-a",
        );

        let tids = list.get_tids_from_epc("E28011606000020000000002");
        assert_eq!(tids.len(), 2);
    }
}
