# ghpascon-rust

A personal Rust utility library.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
ghpascon-rust = { git = "https://github.com/ghpascon/ghpascon-rust" }
```

## Modules

### `utils::regex`

Utilities for regex-based validation.

#### `regex_hex(value: &str, len: Option<usize>) -> bool`

Validates whether a string is a valid hexadecimal value.

| Parameter | Type            | Description                                       |
| --------- | --------------- | ------------------------------------------------- |
| `value`   | `&str`          | The string to validate                            |
| `len`     | `Option<usize>` | Expected length. Pass `None` to skip length check |

**Returns** `true` if the string contains only hex characters (`0-9`, `a-f`, `A-F`) and matches the expected length (if provided).

```rust
use ghpascon_rust::utils::regex::regex_hex;

regex_hex("1a2b3c", None);    // true
regex_hex("1a2b3c", Some(6)); // true
regex_hex("1a2b3c", Some(5)); // false — wrong length
regex_hex("1a2b3g", None);    // false — invalid char
```

---

### `utils::tag_list`

Thread-safe, high-performance container for RFID tags backed by `DashMap`.
The internal primary key follows this rule:

- if `tid` exists, `tid` is the key (unique);
- if `tid` is missing, key is `_{epc}`.

The structure also maintains a fast EPC index:

- `epc_to_keys: DashMap<String, Vec<String>>`

This means one EPC can reference more than one key/TID while keeping most operations in O(1).
There is no numeric `id` in the public record; the internal key is the stable identifier.

Every tag snapshot is a dynamic `serde_json::Map<String, Value>` (`TagRecord`).
`add` and `get_by_*` return a shared `Tag` (`Arc<Mutex<TagRecord>>`) so you can mutate the stored record in place.

#### Key types

| Type        | Description                                                   |
| ----------- | ------------------------------------------------------------- |
| `TagList`   | Main container. Built via `TagList::builder()`.               |
| `TagRecord` | `Map<String, Value>` — dynamic tag record.                    |
| `Tag`       | `Arc<Mutex<TagRecord>>` — shared, mutable reference to a tag. |
| `make_tag`  | Helper to create a `HashMap<String, Value>` input.            |

#### Known fields in every `TagRecord`

`epc` · `tid` · `rssi` · `ant` · `device` · `count` · `chip` · `timestamp` · `first_seen`

#### Builder options

```rust
let list = TagList::builder()
    .prefix(vec!["E28011", "E28069"]) // EPC prefix allow-list (case-insensitive)
    // or: .prefix_from_str("E28011,E28069")
    .build();
```

#### Validation rules

| Field  | Rule                                      |
| ------ | ----------------------------------------- |
| `epc`  | Hex-only                                  |
| `tid`  | Hex-only (when `Some`)                    |
| `rssi` | Positive values are automatically negated |

#### Methods

| Method                                          | Description                                                                                  |
| ----------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `add(tag_map, device) -> (bool, Option<Tag>)`   | Add or update a tag. Returns `(is_new, Some(tag))` or `(false, None)` when filtered/invalid. |
| `get_all() -> Vec<TagRecord>`                   | Snapshot of all tags (arbitrary order)                                                       |
| `get_n(n) -> Vec<TagRecord>`                    | First `n` tags in iteration order (snapshot)                                                 |
| `get_all_sorted() -> Vec<TagRecord>`            | All tags sorted by internal key (snapshot)                                                   |
| `get_n_sorted(n) -> Vec<TagRecord>`             | First `n` tags by internal key order (snapshot)                                              |
| `get_by_key(key) -> Option<Tag>`                | Lookup by internal key (`tid` or `_epc`)                                                     |
| `get_by_epc(epc) -> Option<Tag>`                | Lookup by EPC (case-insensitive)                                                             |
| `get_by_tid(tid) -> Option<Tag>`                | Lookup by TID (case-insensitive)                                                             |
| `get_by_identifier(value, type) -> Option<Tag>` | Lookup by `"tid"` or `"epc"`                                                                 |
| `get_epcs() -> Vec<String>`                     | All EPC index keys                                                                           |
| `get_n_epcs(limit) -> Vec<String>`              | First `n` EPC index keys                                                                     |
| `get_tids(limit) -> Vec<String>`                | All TIDs (excludes tags with no TID)                                                         |
| `get_tid_from_epc(epc)`                         | Cross-lookup: EPC → TID                                                                      |
| `get_tids_from_epc(epc)`                        | EPC index list (`Vec<String>`)                                                               |
| `remove_by_key(key)`                            | Remove by internal key                                                                       |
| `remove_by_epc(epc)`                            | Remove all tags with this EPC                                                                |
| `remove_by_tid(tid)`                            | Remove all tags with this TID                                                                |
| `clear()`                                       | Remove all tags and reset indices                                                            |
| `len() / is_empty() / contains_key(key)`        | Introspection helpers                                                                        |

```rust
use ghpascon_rust::utils::tag_list::{Tag, TagList, make_tag};
use serde_json::json;

let list = TagList::builder().prefix_from_str("E28011").build();

let tag = make_tag("E28011606000020000000000", Some("E28011052000701234567890"), -60, 1);

// add returns (is_new: bool, Option<Tag>) — None when filtered or invalid
let (is_new, tag) = list.add(tag, "reader-01");
if let Some(tag) = tag {
    println!("new? {}", is_new);

    // Mutate in place — changes persist inside the list (Python-like reference)
    tag.lock().unwrap().insert("zone".to_string(), json!("warehouse-A"));

    println!("{}", serde_json::to_string_pretty(&*tag.lock().unwrap()).unwrap());
}

// get_by_* also returns Tag (shared reference)
if let Some(stored) = list.get_by_tid("E28011052000701234567890") {
    stored.lock().unwrap().insert("status".to_string(), json!("processed"));
}

// collection methods return snapshots (Vec<TagRecord>)
let first_two = list.get_n(2);
println!("{}", serde_json::to_string_pretty(&first_two).unwrap());

let epcs = list.get_epcs();
println!("{}", serde_json::to_string_pretty(&epcs).unwrap());

let first_two_epcs = list.get_n_epcs(2);
println!("{}", serde_json::to_string_pretty(&first_two_epcs).unwrap());
```

---

### `utils::path`

#### `get_working_dir() -> Result<PathBuf, std::io::Error>`

Returns the directory containing the running executable.

---

### `utils::logger_manager`

Non-blocking JSON logger with daily file rotation, automatic retention cleanup, and coloured console output. See the module docs for full usage.

---

### `devices::rfid::x714`

X714 RFID reader with automatic reconnection, inspired by the Python implementation.
Built on `Arc<X714Shared>` so `X714: Clone` is cheap – all clones share the same live connection state.

Transport status:

| Transport | Status                                                           |
| --------- | ---------------------------------------------------------------- |
| TCP       | Full: reconnection loop + receive task + monitor + 10 s ping     |
| Serial    | Full: VID/PID auto-detect, `tokio-serial`, reconnection loop     |
| BLE       | Stub: logs "not implemented"; replace body once `btleplug` added |

#### Architecture

All mutable runtime state lives in `Arc<X714Shared>`:
`is_connected` · `is_reading` · `serial_number` · `writer` · `running`

`connect()` runs the reconnection loop **forever**. Spawn it as a background task:

```rust
let bg = reader.clone();
tokio::spawn(async move { bg.connect().await; });
```

On every successful connection the reader automatically calls `config_reader()` + optionally
`start_inventory()` (mirrors Python's `on_connected()`). Reconnection happens automatically
without user code.

#### Key points

- `ConnectionType`: `Serial`, `Tcp`, `Ble`.
- `X714Config` / `X714::from_map(HashMap<String, Value>)` — build from dynamic params.
- Default event sink: `utils::dummy_event::dummy_event`. Override with `with_event_handler(...)`.
- `parse_line(frame)` / `on_receive(data)` parse reader lines into typed `X714Event` values and
  update internal state automatically.

#### Main API

| Method                                           | Description                                                |
| ------------------------------------------------ | ---------------------------------------------------------- |
| `X714::new(config)`                              | Create from `X714Config`                                   |
| `X714::from_map(params)`                         | Create from `HashMap<String, Value>`                       |
| `X714::default()`                                | Default Serial config                                      |
| `with_event_handler(h)` / `set_event_handler(h)` | Replace event sink                                         |
| `connect().await`                                | Run reconnection loop forever (spawn as background task)   |
| `close().await`                                  | Stop the reconnection loop and release resources           |
| `write(cmd).await`                               | Send a command over the current transport                  |
| `is_connected() / is_reading()`                  | Runtime state accessors                                    |
| `serial_number()`                                | Returns `Option<String>` set after `#name:` frame received |
| `parse_line(frame)`                              | Parse + dispatch events, returns `Vec<X714Event>`          |
| `on_receive(data)`                               | Parse one raw line (no return value)                       |
| `config_commands()`                              | Build `Vec<String>` with all setup commands                |
| `start_inventory().await`                        | Send `#READ:ON` and update state                           |
| `stop_inventory().await`                         | Send `#READ:OFF` and update state                          |
| `clear_tags().await`                             | Send `#CLEAR`                                              |
| `config_reader().await`                          | Send all config commands                                   |
| `get_reader_info().await`                        | Poll `#get_info` until serial number is received           |
| `write_epc(...).await`                           | Write new EPC to a tag                                     |
| `write_gpo(...).await`                           | Control GPO pin (static or pulsed)                         |
| `to_map()`                                       | Export config back to map                                  |
| `connect_instruction()`                          | Human-readable connection string                           |

```rust
use std::collections::HashMap;
use std::time::Duration;

use ghpascon_rust::devices::rfid::x714::X714;
use serde_json::{Number, Value};

#[tokio::main]
async fn main() {
    let mut params = HashMap::new();
    params.insert("name".to_string(), Value::String("dock-x714".to_string()));
    params.insert("connection_type".to_string(), Value::String("TCP".to_string()));
    params.insert("ip".to_string(), Value::String("192.168.1.50".to_string()));
    params.insert("tcp_port".to_string(), Value::Number(Number::from(23)));

    let reader = X714::from_map(params).expect("valid config");
    println!("{}", reader.connect_instruction());

    // connect() runs forever – always spawn it as a background task
    let bg = reader.clone();
    tokio::spawn(async move { bg.connect().await; });

    tokio::time::sleep(Duration::from_secs(15)).await;
    println!("connected={}, reading={}", reader.is_connected(), reader.is_reading());
    reader.close().await;
}
```

---

### `devices::rfid::r700`

Impinj R700 IOT RFID reader driver over HTTPS REST API with automatic reconnection.
Same `Arc<R700Shared>` architecture as X714 — `R700: Clone` is cheap.

The R700 exposes a REST API over HTTPS (self-signed certificate).
Tags are delivered as an NDJSON stream via `GET /data/stream`.

#### Architecture

| State field     | Type                    | Description                             |
| --------------- | ----------------------- | --------------------------------------- |
| `is_connected`  | `AtomicBool`            | Set after full setup sequence succeeds  |
| `is_reading`    | `AtomicBool`            | Set when `inventoryStatus == "running"` |
| `serial_number` | `Mutex<Option<String>>` | From `GET /system`                      |
| `running`       | `AtomicBool`            | Set to `false` by `close()`             |

#### Reconnection loop (mirrors Python `connect()`)

1. `PUT /system/rfid/interface` → `{"rfidInterface":"rest"}`
2. `GET /system/image` → firmware version check (if configured)
3. `GET /status` → `POST /profiles/stop` (if not idle)
4. `GET /system` → extract `serialNumber`, dispatch `SerialNumber` event
5. `POST /profiles/inventory/start` → full reading config
6. Dispatch `Connection(true)` event
7. Stream `GET /data/stream` (NDJSON): parse `tagInventoryEvent` + `inventoryStatusEvent`
8. On disconnect: dispatch `Connection(false)`, sleep `reconnection_time`, retry

#### R700Config fields

| Field                          | Default           | Description                                     |
| ------------------------------ | ----------------- | ----------------------------------------------- |
| `name`                         | `"r700"`          | Device name (appears in event dispatch)         |
| `ip`                           | `"192.168.1.100"` | Reader IP address                               |
| `username`                     | `"root"`          | Basic Auth username                             |
| `password`                     | `"impinj"`        | Basic Auth password                             |
| `start_reading`                | `true`            | Start inventory on connect                      |
| `firmware_version`             | `None`            | Required firmware prefix (skip check if `None`) |
| `session`                      | `1`               | RFID session (0–3)                              |
| `read_power`                   | `3000`            | Transmit power in cdbm                          |
| `read_rssi`                    | `-80`             | Minimum RSSI in dBm                             |
| `search_mode`                  | `"dual-target"`   | Inventory search mode                           |
| `rf_mode`                      | `4`               | RF mode index                                   |
| `gpi_start`                    | `false`           | Use GPI triggers                                |
| `protected_inventory_active`   | `false`           | Enable protected inventory                      |
| `protected_inventory_password` | `"12345678"`      | Pin hex for protected inventory                 |
| `reconnection_time`            | `2`               | Seconds between reconnection attempts           |
| `active_ant`                   | `[1]`             | Active antenna ports                            |

#### R700Event variants

| Variant                | Payload         | Description                                |
| ---------------------- | --------------- | ------------------------------------------ |
| `Connection(bool)`     | `Value::Bool`   | `true` = connected, `false` = disconnected |
| `Reading(bool)`        | `Value::Bool`   | Inventory start/stop                       |
| `Tag(R700Tag)`         | `Value::Object` | New tag reading                            |
| `SerialNumber(String)` | `Value::String` | Reader serial number                       |

#### R700Tag fields

`epc: Option<String>` · `tid: Option<String>` · `ant: i32` · `rssi: i32` · `protected: bool`

#### Main API

| Method                                                 | Description                                         |
| ------------------------------------------------------ | --------------------------------------------------- |
| `R700::new(config)`                                    | Create from `R700Config`                            |
| `R700::from_map(params)`                               | Create from `HashMap<String, Value>`                |
| `with_event_handler(h)` / `set_event_handler(&mut, h)` | Replace event sink                                  |
| `connect().await`                                      | Run reconnection loop forever (spawn as background) |
| `close().await`                                        | Stop loop and clear runtime state                   |
| `start_inventory().await`                              | `POST /profiles/inventory/start`                    |
| `stop_inventory().await`                               | `POST /profiles/stop`                               |
| `write_gpo(pin, state, control, time_ms).await`        | Control GPO pin                                     |
| `write_epc(target_id, target_val, new_epc, pw).await`  | Write new EPC (3 blockWrite commands)               |
| `protected_inventory(&mut, active, pw).await`          | Enable/disable protected inventory                  |
| `is_connected() / is_reading()`                        | Runtime state accessors                             |
| `serial_number()`                                      | `Option<String>`                                    |
| `to_map()`                                             | Export config to map                                |
| `connect_instruction()`                                | Human-readable connection string                    |

```rust
use std::collections::HashMap;
use ghpascon_rust::devices::rfid::r700::R700;
use serde_json::{Number, Value};

#[tokio::main]
async fn main() {
    let mut params = HashMap::new();
    params.insert("name".to_string(), Value::String("dock-r700".to_string()));
    params.insert("ip".to_string(), Value::String("192.168.1.101".to_string()));
    params.insert("start_reading".to_string(), Value::Bool(true));
    params.insert(
        "active_ant".to_string(),
        Value::Array(vec![Value::Number(Number::from(1))]),
    );

    let reader = R700::from_map(params).expect("valid config");
    println!("{}", reader.connect_instruction());

    // connect() runs forever – always spawn it as a background task
    let bg = reader.clone();
    tokio::spawn(async move { bg.connect().await; });

    tokio::signal::ctrl_c().await.ok();
    reader.stop_inventory().await.ok();
    reader.close().await;
}
```

---

### `devices::device_manager`

Manages multiple devices (X714, R700, ACUPAD, SerialDevice, TcpDevice, SatoPrinter, SatoWs4Printer)
from `.json` config files. Inspired by the Python `DeviceManager`.

#### JSON config format

The `"reader"` field determines the device type. All other fields are optional — each device's
defaults are applied automatically. The filename (without `.json`) becomes the device `name`.

| Type            | `"reader"` field            |
| --------------- | --------------------------- |
| X714            | `"X714"`                    |
| Impinj R700     | `"R700_IOT"`                |
| ACUPAD          | `"ACUPAD"`                  |
| Generic Serial  | `"SERIAL"`                  |
| Generic TCP     | `"TCP"`                     |
| SATO printer    | `"SATO"`                    |
| SATO WS4        | `"SATO_WS4"`                |

```json
{ "reader": "X714", "connection_type": "TCP", "ip": "192.168.1.50" }
{ "reader": "R700_IOT", "ip": "192.168.1.101", "active_ant": [1, 2] }
{ "reader": "ACUPAD" }
{ "reader": "SERIAL", "port": "/dev/ttyUSB0" }
{ "reader": "TCP", "host": "192.168.1.200", "port": 9000 }
{ "reader": "SATO", "ip": "192.168.1.100" }
{ "reader": "SATO_WS4", "ip": "192.168.1.102" }
```

#### DeviceInfo

| Field                 | Type             | Description                              |
| --------------------- | ---------------- | ---------------------------------------- |
| `name`                | `String`         | Device name (from filename)              |
| `device_type`         | `String`         | e.g. `"X714"`, `"R700_IOT"`, `"ACUPAD"` |
| `is_connected`        | `bool`           | Current connection state                 |
| `is_reading`          | `bool`           | Current reading/inventory state          |
| `serial_number`       | `Option<String>` | Serial number (if available)             |
| `connect_instruction` | `String`         | Human-readable connection string         |

#### DeviceManager API

| Method                                                   | Description                                              |
| -------------------------------------------------------- | -------------------------------------------------------- |
| `DeviceManager::new(path)`                               | Create manager pointing to the config directory          |
| `with_event_handler(h)` / `set_event_handler(h)`         | Set shared event handler                                 |
| `assign_event_handler()`                                 | Distribute handler to all loaded devices                 |
| `load_devices()`                                         | Read JSONs and populate `devices` (calls assign_handler) |
| `connect_devices(force).await`                           | Spawn background connection tasks; `force` restarts      |
| `cancel_connect_tasks().await`                           | Cancel active connection tasks                           |
| `disconnect_devices().await`                             | Close all devices and clear the list                     |
| `get_device_names() -> Vec<String>`                      | Names of all devices                                     |
| `get_device(name) -> Option<&Device>`                    | Reference to a device by name                            |
| `get_device_info(name: Option<&str>) -> Vec<DeviceInfo>` | State snapshot for one or all devices                    |
| `any_device_reading() -> bool`                           | `true` if any device is connected and reading            |
| `get_serial_number(name) -> Option<String>`              | Serial number of a device (if connected)                 |
| `start_inventory(name).await`                            | Start inventory on a device                              |
| `stop_inventory(name).await`                             | Stop inventory on a device                               |
| `start_inventory_all().await -> HashMap<String, bool>`   | Start on all connected devices                           |
| `stop_inventory_all().await -> HashMap<String, bool>`    | Stop on all connected devices                            |
| `write_epc(name, tid, val, epc, pw).await`               | Write EPC to a tag                                       |
| `write_gpo(name, pin, state, ctrl, ms).await`            | Control a GPO pin                                        |
| `len() / is_empty()`                                     | Device count                                             |

```rust
use std::sync::{Arc, Mutex};
use ghpascon_rust::device_manager::{DeviceManager, SharedEventHandler};
use ghpascon_rust::utils::tag_list::{TagList, make_tag};
use serde_json::Value;

#[tokio::main]
async fn main() {
    let tags = Arc::new(TagList::builder().build());
    let tags_clone = Arc::clone(&tags);

    let handler: SharedEventHandler = Arc::new(Mutex::new(Box::new(
        move |name: &str, event_type: &str, data: Option<Value>| {
            if event_type == "tag" {
                if let Some(obj) = data.as_ref().and_then(|v| v.as_object()) {
                    let epc = obj.get("epc").and_then(|v| v.as_str()).unwrap_or_default();
                    let tid = obj.get("tid").and_then(|v| v.as_str());
                    let rssi = obj.get("rssi").and_then(|v| v.as_i64()).unwrap_or(0);
                    let ant  = obj.get("ant").and_then(|v| v.as_u64()).unwrap_or(0);
                    tags_clone.add(make_tag(epc, tid, rssi, ant), name);
                }
            }
        },
    )));

    let mut manager = DeviceManager::new("examples/devices/configs")
        .with_event_handler(handler);

    manager.connect_devices(false).await;
    println!("Devices: {:?}", manager.get_device_names());

    tokio::signal::ctrl_c().await.ok();
    manager.cancel_connect_tasks().await;
    manager.disconnect_devices().await;
}
```

---

### `devices::rfid::acupad`

ACUPAD RFID reader over serial (USB CDC). Same `Arc<AcupadShared>` architecture — `Acupad: Clone`
is cheap.

#### Main API

| Method                                           | Description                                              |
| ------------------------------------------------ | -------------------------------------------------------- |
| `Acupad::new(config)`                            | Create from `AcupadConfig`                               |
| `Acupad::from_map(params)`                       | Create from `HashMap<String, Value>`                     |
| `Acupad::default()`                              | Default config (VID=260, PID=24656, 115200 baud)         |
| `with_event_handler(h)` / `set_event_handler(h)` | Replace event sink                                       |
| `connect().await`                                | Run reconnection loop forever (spawn as background task) |
| `close().await`                                  | Stop loop and release resources                          |
| `write(cmd).await`                               | Send a command over serial                               |
| `is_connected() / is_reading()`                  | Runtime state accessors                                  |
| `serial_number()`                                | Returns `Option<String>`                                 |
| `parse_line(frame)`                              | Parse + dispatch events, returns `Vec<AcupadEvent>`      |
| `start_inventory().await`                        | Send `#READ:ON`                                          |
| `stop_inventory().await`                         | Send `#READ:OFF`                                         |
| `write_epc(...).await`                           | Write new EPC to a tag                                   |
| `connect_instruction()`                          | Human-readable connection string                         |

---

### `devices::generic::serial`

Generic serial device for sending and receiving raw data over a serial port.
Uses `Arc<SerialDeviceShared>` — `SerialDevice: Clone` is cheap.

#### Main API

| Method                                           | Description                                              |
| ------------------------------------------------ | -------------------------------------------------------- |
| `SerialDevice::new(config)`                      | Create from `SerialDeviceConfig`                         |
| `SerialDevice::from_map(params)`                 | Create from `HashMap<String, Value>` (infallible)        |
| `SerialDevice::default()`                        | Default config (auto-detect port, 115200 baud)           |
| `with_event_handler(h)` / `set_event_handler(h)` | Replace event sink                                       |
| `connect().await`                                | Run reconnection loop forever (spawn as background task) |
| `close().await`                                  | Stop loop and release resources                          |
| `write(data).await`                              | Send raw bytes over serial                               |
| `is_connected()`                                 | Runtime state accessor                                   |
| `connect_instruction()`                          | Human-readable connection string                         |

---

### `devices::generic::tcp`

Generic TCP device for sending and receiving raw data over a TCP socket.
Uses `Arc<TcpDeviceShared>` — `TcpDevice: Clone` is cheap.

#### Main API

| Method                                           | Description                                              |
| ------------------------------------------------ | -------------------------------------------------------- |
| `TcpDevice::new(config)`                         | Create from `TcpDeviceConfig`                            |
| `TcpDevice::from_map(params)`                    | Create from `HashMap<String, Value>` (infallible)        |
| `TcpDevice::default()`                           | Default config (127.0.0.1:9000)                          |
| `with_event_handler(h)` / `set_event_handler(h)` | Replace event sink                                       |
| `connect().await`                                | Run reconnection loop forever (spawn as background task) |
| `close().await`                                  | Stop loop and release resources                          |
| `write(data).await`                              | Send raw bytes over TCP                                  |
| `is_connected()`                                 | Runtime state accessor                                   |
| `connect_instruction()`                          | Human-readable connection string                         |

---

### `devices::printer::sato`

SATO thermal printer driver over TCP (ZPL). Supports queued and sequential printing.
Uses `Arc<SatoShared>` — `SatoPrinter: Clone` is cheap.

`SatoWs4Printer` is a newtype wrapper over `SatoPrinter` with a different default IP
(`192.168.1.102`) suited for the SATO WS4 model.

#### Main API

| Method                              | Description                                              |
| ----------------------------------- | -------------------------------------------------------- |
| `SatoPrinter::new(config)`          | Create from `SatoConfig`                                 |
| `SatoPrinter::from_map(params)`     | Create from `HashMap<String, Value>` (infallible)        |
| `SatoPrinter::default()`            | Default config (192.168.1.100:9100)                      |
| `SatoWs4Printer::default()`         | Default config (192.168.1.102:9100)                      |
| `connect().await`                   | Run reconnection loop forever (spawn as background task) |
| `close().await`                     | Stop loop and release resources                          |
| `print(zpl).await`                  | Print ZPL bytes; returns `Result<print_id, error>`       |
| `add_to_print_queue(labels).await`  | Enqueue a list of ZPL strings                            |
| `process_queue().await`             | Print all queued labels one by one                       |
| `is_connected()`                    | Runtime state accessor                                   |
| `connect_instruction()`             | Human-readable connection string                         |

#### ZPL utilities

`devices::printer::sato::zpl_utils::generate_zpl_with_params(template, params)` — performs
`{key}` → value substitution in a ZPL template string.

## Examples

```bash
# Utils
cargo run --example utils_regex
cargo run --example example_logger
cargo run --example utils_delayed_function
cargo run --example example_tag_list
cargo run --example taglist_performance

# X714 RFID reader
cargo run --example x714_basic
cargo run --example x714_custom_event
cargo run --example x714_from_map

# Impinj R700
cargo run --example r700_basic -- 192.168.1.101
cargo run --example r700_custom_event -- 192.168.1.101
cargo run --example r700_gpi -- 192.168.1.101
cargo run --example r700_protected_inventory -- 192.168.1.101
cargo run --example r700_write_epc -- 192.168.1.101
cargo run --example r700_gpo -- 192.168.1.101

# ACUPAD RFID reader
cargo run --example acupad_basic
cargo run --example acupad_custom_event
cargo run --example acupad_write_epc

# Generic devices
cargo run --example serial_device_basic
cargo run --example tcp_device_basic -- 127.0.0.1 9000

# SATO printers
cargo run --example sato_basic -- 192.168.1.100
cargo run --example sato_print_single -- 192.168.1.100
cargo run --example sato_print_list -- 192.168.1.100
cargo run --example sato_sequential -- 192.168.1.100
cargo run --example sato_ws4

# DeviceManager
cargo run --example device_manager_example
```

## Device config examples

Files in `examples/devices/configs/` show the minimal format for each type:

| File               | Type           | Transport                    |
| ------------------ | -------------- | ---------------------------- |
| `dock_x714.json`   | X714           | TCP                          |
| `serial_x714.json` | X714           | Serial (VID/PID auto-detect) |
| `dock_r700.json`   | R700 IOT       | HTTPS REST                   |

## Scripts

| Script              | Description                    |
| ------------------- | ------------------------------ |
| `scripts/deploy.sh` | Runs tests, commits and pushes |

## Dependencies

| Crate          | Purpose                                             |
| -------------- | --------------------------------------------------- |
| `regex`        | Hex validation                                      |
| `dashmap`      | Concurrent hash maps (TagList)                      |
| `tokio`        | Async runtime                                       |
| `sha2`         | SHA-256 hashing                                     |
| `hex`          | Hex encoding/decoding                               |
| `chrono`       | Timestamps (serde feature enabled)                  |
| `serde`        | Serialisation/deserialisation                       |
| `serde_json`   | JSON output                                         |
| `serialport`   | Serial port enumeration (X714/ACUPAD VID/PID detect)|
| `tokio-serial` | Async serial I/O (X714, ACUPAD, SerialDevice)       |
| `reqwest`      | HTTPS REST client with stream support (R700)        |
| `uuid`         | Print job IDs (SatoPrinter)                         |

## License

MIT
