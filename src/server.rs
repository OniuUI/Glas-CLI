use std::fs;
use std::io::Write;
use std::net::TcpStream;
use std::path::Path;

pub fn serve_file(stream: &mut TcpStream, root: &Path, req_path: &str) {
    let path = req_path.trim_start_matches('/');
    let file_path = root.join(path);
    match fs::read(&file_path) {
        Ok(contents) => {
            let mime = mime_type(&file_path);
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                mime, contents.len()
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.write_all(&contents);
        }
        Err(_) => { send_404(stream); }
    }
}

pub fn send_404(stream: &mut TcpStream) {
    let body = "<h1>404 Not Found</h1><p>Glass House</p>";
    let response = format!("HTTP/1.1 404 Not Found\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
    let _ = stream.write_all(response.as_bytes());
}

pub fn mime_type(path: &Path) -> &str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("ts") => "application/typescript",
        _ => "application/octet-stream",
    }
}

pub fn find_free_port(start: u16) -> u16 {
    for p in start..start + 100 {
        if std::net::TcpListener::bind(format!("0.0.0.0:{}", p)).is_ok() { return p; }
    }
    start
}

pub fn generate_serve_index() -> String {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Glass House</title>
  <script src="glasshouse.bundle.js"></script>
</head>
<body><div id="app"></div></body>
</html>"#.to_string()
}

pub fn generate_dev_index(cwd: &Path) -> String {
    let mut pkg_scripts = String::new();
    if let Ok(entries) = fs::read_dir(cwd.join("packages")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name.starts_with('.') { continue; }
                if let Ok(pkg_entries) = fs::read_dir(&path) {
                    for pf in pkg_entries.flatten() {
                        let p = pf.path();
                        if p.extension().map_or(false, |e| e == "js") {
                            let rel = p.strip_prefix(cwd).unwrap_or(&p);
                            pkg_scripts.push_str(&format!("  <script src=\"{}\"></script>\n", rel.display()));
                        }
                    }
                }
            }
        }
    }
    let reload = r#"<script>var _g=0;function _gp(){fetch('/__glas_ping').then(function(r){return r.text()}).then(function(v){if(_g&&v!==_g){location.reload()}_g=v;setTimeout(_gp,2000)}).catch(function(){setTimeout(_gp,2000)})}_gp()</script>"#;
    let dirs = ["glasshouse","packages","src"];
    let mut scripts = String::new();
    for d in &dirs {
        let dp = cwd.join(d);
        if !dp.exists() { continue; }
        if let Ok(es) = fs::read_dir(&dp) {
            let mut fs: Vec<_> = es.flatten().filter_map(|e|{let p=e.path();if p.extension().map_or(false,|ex|ex=="js"){Some(p)}else{None}}).collect();
            fs.sort();
            for f in &fs { let r = f.strip_prefix(cwd).unwrap_or(f); scripts.push_str(&format!("  <script src=\"{}\"></script>\n",r.display())); }
        }
    }
    format!("<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1.0\"><title>Glass House (dev)</title>\n{}{}</head><body><div id=\"app\"></div></body></html>", scripts, reload)
}

pub fn generate_build_page(cwd: &Path) -> String {
    let mut scripts = String::new();
    let mut sources_json = String::from("{");

    let dirs = ["glasshouse", "packages", "src"];
    for d in &dirs {
        let dp = cwd.join(d);
        if !dp.exists() { continue; }
        if let Ok(es) = fs::read_dir(&dp) {
            let mut files: Vec<_> = es.flatten()
                .filter_map(|e| {
                    let p = e.path();
                    if p.extension().map_or(false, |ex| ex == "js") {
                        if let Ok(content) = fs::read_to_string(&p) {
                            let rel = p.strip_prefix(cwd).unwrap_or(&p).to_string_lossy().to_string();
                            let escaped = content.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "");
                            Some(format!("\"{}\":\"{}\"", rel, escaped))
                        } else { None }
                    } else { None }
                })
                .collect();
            files.sort();
            for f in &files {
                if !sources_json.ends_with('{') { sources_json.push(','); }
                sources_json.push_str(f);
            }
        }
    }
    sources_json.push('}');

    let dirs2 = ["glasshouse", "packages", "src"];
    for d in &dirs2 {
        let dp = cwd.join(d);
        if !dp.exists() { continue; }
        if let Ok(es) = fs::read_dir(&dp) {
            let mut fs: Vec<_> = es.flatten()
                .filter_map(|e| { let p = e.path(); if p.extension().map_or(false, |ex| ex == "js") { Some(p) } else { None } })
                .collect();
            fs.sort();
            for f in &fs {
                let rel = f.strip_prefix(cwd).unwrap_or(&f);
                scripts.push_str(&format!("  <script src=\"{}\"></script>\n", rel.display()));
            }
        }
    }

    format!(r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>Build</title>{}</head><body><div id="status">Building...</div><script>
(function(){{'use strict';var s=document.getElementById('status');var SOURCES={{{}}};
setTimeout(function(){{try{{
s.textContent='Compacting '+Object.keys(SOURCES).length+' sources...';
var cp=GlassHouse.require('hyper-compactor');
var result=cp.compactAll(SOURCES);
s.textContent='Packaging...';
var b='/* Glass House GHC2 */\n';b+='var _GH="'+result.binaryBase64+'";\n';b+='GlassHouse.require("decompressor").loadFromBase64(_GH);\n';
s.textContent='Sending '+(b.length/1024).toFixed(1)+' KB...';
fetch('/__glas_build',{{method:'POST',headers:{{'Content-Type':'application/javascript'}},body:b}}).then(function(){{s.textContent='✓ Build complete ('+result.reduction+'% reduction,'+result.totalRenamed+' ids,'+Object.keys(SOURCES).length+' sources). Close this tab.'}}).catch(function(e){{s.textContent='✗ Failed: '+e.message}})
}}catch(e){{s.textContent='✗ Error: '+e.message}}}},2000)}})();
</script></body></html>"#, scripts, sources_json)
}
