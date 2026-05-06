// cli/commands/audit.rs
// Audit command — checks project structure, security, types, and TypeScript compliance.

use std::env;
use std::fs;

use crate::utils::{walk_dir, format_size};

pub fn audit(deep: bool, fix: bool) {
    if fix {
        println!("⚑ Audit --fix\n");
    } else if deep {
        println!("⚑ Deep Audit\n");
    } else {
        println!("⚑ Quick Audit (use --deep for full)\n");
    }

    let cwd = env::current_dir().unwrap_or_default();
    let mut issues = 0u32;
    let mut warnings = 0u32;

    // ── Structure ──
    println!("  [Structure]");
    for d in &["glasshouse", "packages", "src", "styles"] {
        if !cwd.join(d).exists() {
            println!("    ✗ Missing: {}", d);
            issues += 1;
        }
    }
    if !cwd.join("index.html").exists() {
        println!("    ✗ Missing: index.html");
        issues += 1;
    }
    if issues == 0 {
        println!("    ✓ OK");
    }

    // ── TypeScript compliance ──
    println!("  [TypeScript]");
    check_ts_compliance(&cwd, &mut issues, &mut warnings);

    // ── Security scan ──
    println!("  [Security]");
    if let Ok(entries) = walk_dir(&cwd) {
        for fp in &entries {
            let ext = fp.extension().and_then(|e| e.to_str()).unwrap_or("");
            let is_source = ext == "js" || ext == "ts" || ext == "tsx";

            if is_source {
                if let Ok(c) = fs::read_to_string(fp) {
                    if c.contains("eval(") {
                        println!("    ✗ {}: eval()", fp.display());
                        issues += 1;
                    }
                    if c.contains("document.write(") {
                        println!("    ✗ {}: document.write()", fp.display());
                        issues += 1;
                    }
                    if c.contains("innerHTML") && c.contains("=") {
                        println!("    ⚠ {}: innerHTML=", fp.display());
                        warnings += 1;
                    }
                    if !c.contains("'use strict'") && !c.contains("\"use strict\"") {
                        println!("    ⚠ {}: missing strict mode", fp.display());
                        warnings += 1;
                    }
                    if c.contains("new Function(") {
                        println!("    ✗ {}: new Function()", fp.display());
                        issues += 1;
                    }
                    // TS-specific: 'as any' is a code smell
                    if ext == "ts" || ext == "tsx" {
                        if c.contains(" as any") || c.contains(": any") {
                            println!("    ⚠ {}: uses 'any' type", fp.display());
                            warnings += 1;
                        }
                    }
                }
            }
        }
    }
    if issues == 0 && warnings == 0 {
        println!("    ✓ No issues");
    }

    // ── Deep audit: extended checks ──
    if deep {
        println!("  [Type Safety]");
        let mut pebbles = 0u32;
        let mut with_types = 0u32;
        if let Ok(entries) = walk_dir(&cwd) {
            for fp in &entries {
                let ext = fp.extension().and_then(|e| e.to_str()).unwrap_or("");
                if ext == "js" || ext == "ts" || ext == "tsx" {
                    if let Ok(c) = fs::read_to_string(fp) {
                        if c.contains("new Pebble(") || c.contains("Pebble({") {
                            pebbles += 1;
                            if c.contains("propTypes") {
                                with_types += 1;
                            }
                        }
                    }
                }
            }
        }
        if pebbles > 0 {
            let pct = with_types * 100 / pebbles;
            println!(
                "    {} pebbles, {} with propTypes ({}%)",
                pebbles, with_types, pct
            );
            if pct < 100 {
                issues += 1;
            }
        } else {
            println!("    No pebbles found");
        }

        println!("  [Handlers]");
        let mut hc = 0u32;
        if let Ok(entries) = walk_dir(&cwd) {
            for fp in &entries {
                let ext = fp.extension().and_then(|e| e.to_str()).unwrap_or("");
                if ext == "js" || ext == "ts" || ext == "tsx" {
                    if let Ok(c) = fs::read_to_string(fp) {
                        if c.contains("GlassHouse.handler(")
                            || c.contains("Cobblestone.handler(")
                            || c.contains("handler(")
                        {
                            hc += 1;
                        }
                    }
                }
            }
        }
        println!("    {} handlers", hc);

        // Packages
        println!("  [Packages]");
        let pp = cwd.join("packages");
        if pp.exists() {
            if let Ok(entries) = fs::read_dir(&pp) {
                let mut pkg_count = 0u32;
                for e in entries.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        let n = p
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        if n.starts_with('.') {
                            continue;
                        }
                        pkg_count += 1;
                        if !p.join("package.json").exists() {
                            println!("    ⚠ '{}' missing package.json", n);
                            warnings += 1;
                        }
                        // Check for TS files in package
                        let has_ts = if let Ok(pkg_entries) = walk_dir(&p) {
                            pkg_entries.iter().any(|fp| {
                                fp.extension()
                                    .and_then(|e| e.to_str())
                                    .map_or(false, |ex| ex == "ts" || ex == "tsx")
                            })
                        } else {
                            false
                        };
                        let has_js = if let Ok(pkg_entries) = walk_dir(&p) {
                            pkg_entries.iter().any(|fp| {
                                fp.extension()
                                    .and_then(|e| e.to_str())
                                    .map_or(false, |ex| ex == "js")
                            })
                        } else {
                            false
                        };
                        if has_js && !has_ts {
                            println!(
                                "    ⚠ '{}' uses JavaScript — consider migrating to TypeScript (.ts/.tsx)",
                                n
                            );
                            warnings += 1;
                        }
                    }
                }
                if pkg_count == 0 {
                    println!("    No packages installed");
                }
            }
        }

        // Size audit
        println!("  [Size]");
        let mut total_size = 0u64;
        let max_block = 40960u64;
        if let Ok(entries) = walk_dir(&cwd) {
            for fp in &entries {
                let ext = fp.extension().and_then(|e| e.to_str()).unwrap_or("");
                if ext == "js" || ext == "ts" || ext == "tsx" {
                    if let Ok(m) = fs::metadata(fp) {
                        total_size += m.len();
                        if m.len() > max_block {
                            println!("    ✗ {} exceeds {}B", fp.display(), max_block);
                            issues += 1;
                        }
                    }
                }
            }
        }
        println!("    Total: {}", format_size(total_size));

        // TS-specific deep checks
        println!("  [TypeScript Compilation]");
        let ts_files = count_ts_files(&cwd);
        if ts_files > 0 {
            println!(
                "    {} TypeScript file(s) in src/ and packages/",
                ts_files
            );
            // Check if quickjs is available for compilation
            match crate::quickjs::check_available() {
                Ok(()) => println!("    ✓ QuickJS runtime available"),
                Err(e) => {
                    println!("    ⚠ QuickJS not available: {}", e);
                    warnings += 1;
                }
            }
        } else {
            println!("    No TypeScript files found (consider using .ts/.tsx)");
        }
    }

    // ── Fix mode: add 'use strict' to source files ──
    if fix {
        println!("\n  [Fix]");
        if let Ok(entries) = walk_dir(&cwd) {
            for fp in &entries {
                let ext = fp.extension().and_then(|e| e.to_str()).unwrap_or("");
                if ext == "js" || ext == "ts" || ext == "tsx" {
                    if let Ok(c) = fs::read_to_string(fp) {
                        if !c.contains("'use strict'") && !c.contains("\"use strict\"") {
                            let fixed = format!("'use strict';\n{}", c);
                            let _ = fs::write(fp, &fixed);
                            println!(
                                "    ↻ Added strict mode to {}",
                                fp.display()
                            );
                        }
                    }
                }
            }
        }
    }

    // ── Summary ──
    if issues == 0 && warnings == 0 {
        println!("\n  ✓ Clean audit.");
    } else {
        println!(
            "\n  {} error(s), {} warning(s)",
            issues, warnings
        );
        if issues > 0 {
            println!("  Audit FAILED");
        } else {
            println!("  Audit PASSED (warnings)");
        }
    }
}

/// Check TypeScript compliance: no .js files in src/ or packages/.
fn check_ts_compliance(cwd: &std::path::Path, issues: &mut u32, _warnings: &mut u32) {
    let mut found_js = false;

    for dir_name in &["src", "packages"] {
        let dir_path = cwd.join(dir_name);
        if !dir_path.exists() {
            continue;
        }
        if let Ok(entries) = walk_dir(&dir_path) {
            for fp in &entries {
                if fp.extension().and_then(|e| e.to_str()) == Some("js") {
                    // Skip .registry.json
                    if fp.file_name().map_or(false, |n| n == ".registry.json") {
                        continue;
                    }
                    let rel = fp
                        .strip_prefix(cwd)
                        .unwrap_or(fp)
                        .to_string_lossy()
                        .to_string();
                    println!(
                        "    ✗ {}: .js files not allowed — use TypeScript (.ts/.tsx)",
                        rel
                    );
                    found_js = true;
                    *issues += 1;
                }
            }
        }
    }

    if !found_js {
        // Count TS files
        let ts_count = count_ts_files(cwd);
        if ts_count > 0 {
            println!("    ✓ {} TypeScript file(s) found, no forbidden .js files", ts_count);
        } else {
            println!("    ✓ No forbidden .js files found");
        }
    }
}

/// Count TypeScript files in src/ and packages/.
fn count_ts_files(cwd: &std::path::Path) -> u32 {
    let mut count = 0u32;

    for dir_name in &["src", "packages"] {
        let dir_path = cwd.join(dir_name);
        if !dir_path.exists() {
            continue;
        }
        if let Ok(entries) = walk_dir(&dir_path) {
            for fp in &entries {
                if let Some(ext) = fp.extension().and_then(|e| e.to_str()) {
                    if ext == "ts" || ext == "tsx" {
                        count += 1;
                    }
                }
            }
        }
    }

    count
}
