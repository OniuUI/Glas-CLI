use std::fs;
use std::path::Path;

use crate::utils;

const INDEX_HTML: &str = r#"<!DOCTYPE html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1.0"><title>Glass House</title><script src="glasshouse/glasshouse.js"></script><script src="glasshouse/types.js"></script><script src="glasshouse/dom.js"></script><script src="glasshouse/pebble.js"></script><script src="src/app.js"></script></head><body><div id="app"></div></body></html>"#;
const MAIN_CSS: &str = "*,*::before,*::after{box-sizing:border-box;margin:0;padding:0}html{font-size:16px}body{font-family:system-ui,sans-serif;line-height:1.6;color:#111827;background:#fff;min-height:100vh}#app{min-height:100vh}";
const APP_JS: &str = "(function(){'use strict';GlassHouse.ready(function(){console.log('Glass House ready.');});})();";

const GLASSHOUSE_RELEASES: &str = "https://github.com/OniuUI/GlassHouse/releases";
const DEFAULT_GLASSHOUSE_VERSION: &str = "latest";
const GLASSHOUSE_ASSET: &str = "glasshouse.zip";

pub fn init(name: &str) {
    init_with_version(name, DEFAULT_GLASSHOUSE_VERSION);
}

pub fn init_with_version(name: &str, glasshouse_version: &str) {
    let root = Path::new(name);
    if root.exists() {
        eprintln!("glas: directory '{}' already exists", name);
        return;
    }
    for d in &["glasshouse", "packages", "src", "styles"] {
        let _ = fs::create_dir_all(root.join(d));
    }
    for (p, c) in &[
        ("index.html", INDEX_HTML),
        ("styles/main.css", MAIN_CSS),
        ("src/app.js", APP_JS),
        ("packages/.registry.json", "{}"),
        (".gitignore", "dist/\n"),
    ] {
        let fp = root.join(p);
        if let Some(pr) = fp.parent() {
            let _ = fs::create_dir_all(pr);
        }
        let _ = fs::write(&fp, c);
    }
    let gjson = format!(
        r#"{{"name":"{}","version":"0.1.0","entry":"src/app.js"}}"#,
        name
    );
    let _ = fs::write(root.join("glass.json"), &gjson);

    let gh_dir = root.join("glasshouse");
    if let Err(e) = fetch_glasshouse(glasshouse_version, &gh_dir) {
        eprintln!("glas: warning: could not fetch GlassHouse framework: {}", e);
        eprintln!("glas: run 'glas install glasshouse' to install the framework.");
        eprintln!("glas: or download manually from {}", GLASSHOUSE_RELEASES);
    }

    println!("✓ Created Glass House project '{}'", name);
    println!("  cd {}", name);
    println!("  glas dev");
}

fn fetch_glasshouse(version: &str, dest_dir: &Path) -> Result<(), String> {
    if dest_dir.exists() {
        let mut count = 0;
        if let Ok(entries) = fs::read_dir(dest_dir) {
            for _ in entries { count += 1; }
        }
        if count > 0 {
            return Ok(());
        }
    }

    println!("  Fetching GlassHouse {}...", version);

    let url = if version == "latest" {
        format!(
            "{}/latest/download/{}",
            GLASSHOUSE_RELEASES, GLASSHOUSE_ASSET
        )
    } else {
        format!(
            "{}/download/v{}/glasshouse-v{}.zip",
            GLASSHOUSE_RELEASES, version, version
        )
    };

    let tmp = std::env::temp_dir().join(format!("glasshouse-{}.zip", version));
    utils::fetch_release(&url, &tmp).map_err(|e| format!("download failed: {}", e))?;
    utils::extract_zip(&tmp, dest_dir).map_err(|e| format!("extract failed: {}", e))?;
    let _ = fs::remove_file(&tmp);

    println!("  GlassHouse {} installed.", version);
    Ok(())
}
