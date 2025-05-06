#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[cfg(feature = "axstd")]
use axstd::println;
use axstd::{color_println, ColorCode};

#[cfg_attr(feature = "axstd", no_mangle)]
fn main() {
    println!("[WithColor]: Hello, Arceos!");
    color_println!(ColorCode::Red, "[WithColor]: Hello, Arceos!");
}
