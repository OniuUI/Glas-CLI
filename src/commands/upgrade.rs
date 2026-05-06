use std::env;
use std::fs;

use crate::utils::{extract_json_str, bump_version, check_registry};

pub fn upgrade(name: Option<&str>, major: bool, dry: bool) {
    let cwd = env::current_dir().unwrap_or_default();
    let pd = cwd.join("packages");
    if !pd.exists() { println!("No packages."); return; }
    let mode = if dry { "DRY RUN" } else { "UPGRADING" };
    println!("▲ {}...", mode);
    let reg = env::var("GLAS_REGISTRY").unwrap_or_default();
    if let Ok(es) = fs::read_dir(&pd) { let mut up = 0u32;
        for e in es.flatten() { let p = e.path(); if !p.is_dir() { continue; } let pn = p.file_name().unwrap_or_default().to_string_lossy().to_string(); if pn.starts_with('.') { continue; } if let Some(ref n) = name { if pn != *n { continue; } }
            let pj = p.join("package.json"); if !pj.exists() { continue; }
            if let Ok(r) = fs::read_to_string(&pj) { let cv = extract_json_str(&r,"version").unwrap_or("0.0.0");
                let lv = if !reg.is_empty() { check_registry(&reg, &pn).unwrap_or_else(|| bump_version(cv, major)) } else { bump_version(cv, major) };
                if lv != cv { if dry { println!("  {}: {} → {} (would update)", pn, cv, lv); } else { let u = r.replace(&format!("\"version\":\"{}\"",cv), &format!("\"version\":\"{}\"",lv)); let _ = fs::write(&pj, &u); println!("  ✓ {}: {} → {}", pn, cv, lv); } up += 1; }
            }
        }
        if up == 0 { println!("  All up to date."); } else if dry { println!("  {} would update. Run 'glas upgrade' to apply.", up); } else { println!("  {} updated.", up); }
    }
}
