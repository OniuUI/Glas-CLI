use std::env;
use std::fs;
use std::path::Path;

use crate::utils;

pub fn install(source: &str, force: bool) {
    let cwd = env::current_dir().unwrap_or_default();
    let pkgd = cwd.join("packages"); if !pkgd.exists() { let _ = fs::create_dir_all(&pkgd); }
    let name = utils::package_name_from_source(source);
    let target = pkgd.join(&name);
    if target.exists() && !force { eprintln!("glas: '{}' already installed (use -f to force)", name); return; }
    if force && target.exists() { let _ = fs::remove_dir_all(&target); }
    let sp = Path::new(source);
    if sp.is_dir() { match utils::copy_dir_recursive(sp, &target) { Ok(_) => { utils::update_registry(&pkgd, &name, "0.1.0", "local"); println!("✓ Installed {} (local)", name); } Err(e) => { eprintln!("glas: {}", e); } } return; }
    if sp.is_file() { let _ = fs::create_dir_all(&target); let df = target.join(sp.file_name().unwrap_or_default()); if let Err(e) = fs::copy(sp, &df) { eprintln!("glas: {}", e); return; } utils::update_registry(&pkgd, &name, "0.1.0", "local-file"); println!("✓ Installed {} (file)", name); return; }
    if source.starts_with("http://") || source.starts_with("https://") {
        println!("  Fetching {}...", source);
        match utils::fetch_url(source) { Ok(b) => { let _ = fs::create_dir_all(target.clone()); let _ = fs::write(target.join("index.js"), &b); utils::update_registry(&pkgd, &name, "0.1.0", "remote"); println!("✓ Installed {} (remote)", name); } Err(e) => { eprintln!("glas: {}", e); } }
        return;
    }
    if source.contains('@') && !source.starts_with("@") {
        let parts: Vec<&str> = source.splitn(2,'@').collect(); let np = parts[0]; let vp = parts.get(1).unwrap_or(&"latest");
        println!("  Resolving {}@{}...", np, vp); let t2 = target.clone(); let _ = fs::create_dir_all(t2); let _ = fs::write(target.join("index.js"), &format!("// {} v{}\n", np, vp));
        utils::update_registry(&pkgd, np, if *vp == "latest" { "0.1.0" } else { vp }, "registry");
        println!("✓ Staged {}@{}", np, vp); return;
    }
    if source.contains('@') && !source.starts_with("@") {
        let parts: Vec<&str> = source.splitn(2,'@').collect(); let np = parts[0]; let vp = parts.get(1).unwrap_or(&"latest");
        println!("  Resolving {}@{}...", np, vp); let t2 = target.clone(); let _ = fs::create_dir_all(t2); let _ = fs::write(target.join("index.js"), &format!("// {} v{}\n", np, vp));
        utils::update_registry(&pkgd, np, if *vp == "latest" { "0.1.0" } else { vp }, "registry");
        println!("✓ Staged {}@{}", np, vp); return;
    }
    eprintln!("glas: source '{}' not found", source);
}

pub fn uninstall(name: &str) {
    let cwd = env::current_dir().unwrap_or_default();
    let pd = cwd.join("packages").join(name);
    if !pd.exists() { eprintln!("glas: '{}' not installed", name); return; }
    let _ = fs::remove_dir_all(&pd);
    let rp = cwd.join("packages").join(".registry.json");
    if let Ok(raw) = fs::read_to_string(&rp) {
        if let Some(mut reg) = crate::json::parse(&raw) { reg.remove(name); let _ = fs::write(&rp, reg.to_json()); }
    }
    println!("✓ Uninstalled {}", name);
}
