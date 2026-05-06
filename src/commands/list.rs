use std::env;
use std::fs;

use crate::json;

pub fn list() {
    let cwd = env::current_dir().unwrap_or_default();
    let pd = cwd.join("packages");
    if !pd.exists() { println!("No packages."); return; }
    let mut es: Vec<(String,String,String)> = Vec::new();
    if let Ok(de) = fs::read_dir(&pd) {
        for e in de.flatten() { let p = e.path(); if p.is_dir() { let n = p.file_name().unwrap_or_default().to_string_lossy().to_string(); if n.starts_with('.') { continue; }
            let mut v = "?".into(); let mut t = "?".into();
            let pj = p.join("package.json");
            if let Ok(r) = fs::read_to_string(&pj) { if let Some(j) = json::parse(&r) { v = j.get_str("version").unwrap_or("?").into(); t = j.get_str("type").unwrap_or("?").into(); } }
            es.push((n,v,t));
        }}
    }
    if es.is_empty() { println!("No packages."); return; }
    es.sort_by(|a,b| a.0.cmp(&b.0));
    println!("{:<30} {:<12} {:<12}","NAME","VERSION","TYPE");
    println!("{}","-".repeat(56));
    for (n,v,t) in &es { println!("{:<30} {:<12} {:<12}",n,v,t); }
    println!("  {} packages", es.len());
}

pub fn info(name: &str) {
    let cwd = env::current_dir().unwrap_or_default();
    let pd = cwd.join("packages").join(name);
    let pj = pd.join("package.json");
    if !pd.exists() { eprintln!("glas: '{}' not installed", name); return; }
    println!("Package: {}", name);
    if let Ok(r) = fs::read_to_string(&pj) { if let Some(j) = json::parse(&r) {
        println!("  Version:     {}", j.get_str("version").unwrap_or("?"));
        println!("  Type:        {}", j.get_str("type").unwrap_or("?"));
        println!("  Description: {}", j.get_str("description").unwrap_or("-"));
    }}
    println!("  Files:");
    if let Ok(es) = fs::read_dir(&pd) { for e in es.flatten() { let fnm = e.file_name().to_string_lossy().to_string(); if fnm == "package.json" { continue; } let s = e.metadata().map(|m| m.len()).unwrap_or(0); println!("    {} ({})", fnm, crate::utils::format_size(s)); } }
}
