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

## Examples

```bash
cargo run --example utils_regex
```

## Scripts

| Script              | Description                                          |
| ------------------- | ---------------------------------------------------- |
| `scripts/deploy.sh` | Runs tests, bumps semver version, commits and pushes |

## License

MIT
