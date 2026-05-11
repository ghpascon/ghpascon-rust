// Run with: cargo run --example utils_regex
//
// This example shows how `regex_hex` would be used
// after adding ghpascon-rust as a dependency:

use ghpascon_rust::utils::regex::regex_hex;

fn main() {
    println!("{}", regex_hex("1a2b3c", None)); // true
    println!("{}", regex_hex("1a2b3c", Some(6))); // true
    println!("{}", regex_hex("1a2b3c", Some(5))); // false  (wrong length)
    println!("{}", regex_hex("1a2b3g", None)); // false  (invalid char)
}
