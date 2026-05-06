// installer/ui.rs — Console TUI rendering

use std::io::{self, Write};

const C_RESET: &str = "\x1b[0m";
const C_DIM: &str = "\x1b[2m";
const C_GREEN: &str = "\x1b[32m";
const C_RED: &str = "\x1b[31m";
const C_BLUE: &str = "\x1b[34m";
const C_CYAN: &str = "\x1b[36m";

pub const CHECK: &str = "✓";
pub const CROSS: &str = "✗";
pub const DOT: &str = "●";

fn color(s: &str, code: &str) -> String { format!("{}{}{}", code, s, C_RESET) }
pub fn green(s: &str) -> String { color(s, C_GREEN) }
pub fn red(s: &str) -> String { color(s, C_RED) }
pub fn blue(s: &str) -> String { color(s, C_BLUE) }
pub fn cyan(s: &str) -> String { color(s, C_CYAN) }
pub fn dim(s: &str) -> String { color(s, C_DIM) }

pub fn clear() { print!("\x1b[2J\x1b[H"); }

pub fn box_line(text: &str, width: usize) -> String {
    let inner = width.saturating_sub(4);
    let pad = inner.saturating_sub(text.len());
    let left = pad / 2;
    let right = pad - left;
    format!("║{}{}{}║", " ".repeat(left), text, " ".repeat(right))
}

pub fn box_top(width: usize) -> String { let inner = width.saturating_sub(2); format!("╔{}╗", "═".repeat(inner)) }
pub fn box_bottom(width: usize) -> String { let inner = width.saturating_sub(2); format!("╚{}╝", "═".repeat(inner)) }
pub fn divider(width: usize) -> String { "─".repeat(width) }

pub fn draw_header(version: &str) {
    let w = 60;
    println!("{}", box_top(w));
    println!("{}", box_line(&format!("Glas CLI Installer v{}", version), w));
    println!("{}", box_line("Zero-Dependency GlassHouse Tooling", w));
    println!("{}", box_line("github.com/OniuUI/Glas-CLI", w));
    println!("{}", box_bottom(w));
    println!();
}

pub fn prompt_str(prompt: &str, default: &str) -> String {
    print!("  {} [{}]: ", dim(prompt), cyan(default));
    let _ = io::stdout().flush();
    let mut line = String::new();
    io::stdin().read_line(&mut line).unwrap_or_default();
    let trimmed = line.trim().to_string();
    if trimmed.is_empty() { default.to_string() } else { trimmed }
}

pub fn spin_run(label: &str, f: impl FnOnce() -> io::Result<()>) {
    print!("  {} {} ...", blue(DOT), dim(label));
    let _ = io::stdout().flush();
    let result = f();
    print!("\r\x1b[K");
    match result {
        Ok(()) => println!("  {} {}", green(CHECK), label),
        Err(e) => println!("  {} {} — {}", red(CROSS), label, e),
    }
}
