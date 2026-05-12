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

Every tag is stored and returned as a dynamic `serde_json::Map<String, Value>` (`TagRecord`).
After `add`, callers receive `{ is_new, tag }` and can add custom fields on the returned map.

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

## Examples

```bash
cargo run --example utils_regex
cargo run --example example_logger
cargo run --example utils_delayed_function
cargo run --example example_tag_list
```

## Scripts

| Script              | Description                    |
| ------------------- | ------------------------------ |
| `scripts/deploy.sh` | Runs tests, commits and pushes |

## Dependencies

| Crate        | Purpose                            |
| ------------ | ---------------------------------- |
| `regex`      | Hex validation                     |
| `dashmap`    | Concurrent hash maps (TagList)     |
| `tokio`      | Async runtime (LoggerManager)      |
| `sha2`       | SHA-256 hashing                    |
| `hex`        | Hex encoding/decoding              |
| `chrono`     | Timestamps (serde feature enabled) |
| `serde`      | Serialisation/deserialisation      |
| `serde_json` | JSON output                        |

## License

MIT
