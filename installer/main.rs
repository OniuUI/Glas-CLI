// installer/main.rs — entry point, compiled with rustc --edition 2021 installer/main.rs

mod platform;
mod release;
mod download;
mod extract;
mod ui;
mod install;
mod uninstall;

fn main() { install::run(); }
