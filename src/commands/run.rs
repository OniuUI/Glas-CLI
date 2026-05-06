use std::env;
use std::fs;

use crate::json;
use crate::utils;

pub fn run(script: &str) {
    let cwd = env::current_dir().unwrap_or_default();
    let gj = cwd.join("glass.json");
    if let Ok(r) = fs::read_to_string(&gj) { if let Some(c) = json::parse(&r) { if let Some(ss) = c.get("scripts") { if let Some(cmd) = ss.get_str(script) { println!("▶ Running '{}': {}", script, cmd); utils::run_shell(cmd); return; } } } }
    eprintln!("glas: script '{}' not found in glass.json", script);
}
