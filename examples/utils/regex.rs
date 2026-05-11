// Run with: cargo run --example utils_regex
//
// This example shows how `regex_hex` would be used
// after adding ghpascon-rust as a dependency:

use ghpascon_rust::utils::regex::regex_hex;

fn main() {
    println!("{}", regex_hex("1a2b3c")); // true
    println!("{}", regex_hex("1A2B3C")); // true
    println!("{}", regex_hex("1a2b3c")); // true
    println!("{}", regex_hex("1a2b3g")); // false  (invalid char)
}
