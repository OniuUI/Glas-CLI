use std::fs;
use std::path::{Path, PathBuf};
use std::io;
use std::io::Read;
use std::time::Duration;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::json;

static LAST_CHANGE: AtomicU64 = AtomicU64::new(0);

pub fn mark_changed() { LAST_CHANGE.store(now_millis(), Ordering::SeqCst); }
pub fn now_millis() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64 }
pub fn get_last_change() -> u64 { LAST_CHANGE.load(Ordering::SeqCst) }

pub fn parse_port(args: &[String], flag_pos: usize) -> u16 {
    for i in flag_pos..args.len() { if args[i] == "--port" && i + 1 < args.len() { return args[i + 1].parse().unwrap_or(3000); } }
    3000
}

pub fn format_size(bytes: u64) -> String {
    if bytes < 1024 { format!("{}B", bytes) } else if bytes < 1024 * 1024 { format!("{:.1}KB", bytes as f64 / 1024.0) } else { format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0)) }
}

pub fn walk_dir(root: &Path) -> io::Result<Vec<PathBuf>> { let mut files = Vec::new(); walk_dir_recursive(root, &mut files)?; Ok(files) }
fn walk_dir_recursive(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    if !dir.exists() { return Ok(()); }
    for entry in fs::read_dir(dir)? {
        let entry = entry?; let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name.starts_with('.') || name == "node_modules" { continue; }
            walk_dir_recursive(&path, files)?;
        } else if path.is_file() { files.push(path); }
    }
    Ok(())
}

pub fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?; let ty = entry.file_type()?; let dest = dst.join(entry.file_name());
        if ty.is_dir() { copy_dir_recursive(&entry.path(), &dest)?; } else { fs::copy(entry.path(), &dest)?; }
    }
    Ok(())
}

pub fn fetch_url(url: &str) -> io::Result<String> {
    use std::net::TcpStream;
    use std::io::Write;
    let without_proto = if let Some(r) = url.strip_prefix("http://") { r } else if let Some(r) = url.strip_prefix("https://") { r } else { return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid URL")); };
    let (host, path) = if let Some(idx) = without_proto.find('/') { (&without_proto[..idx], &without_proto[idx..]) } else { (without_proto, "/") };
    let hp: Vec<&str> = host.split(':').collect();
    let hostname = hp[0]; let port: u16 = hp.get(1).and_then(|p| p.parse().ok()).unwrap_or(80);
    let mut stream = TcpStream::connect((hostname, port))?;
    stream.set_read_timeout(Some(Duration::from_secs(15)))?;
    let req = format!("GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: glas/1.0\r\nConnection: close\r\n\r\n", path, host);
    stream.write_all(req.as_bytes())?;
    let mut resp = String::new(); stream.read_to_string(&mut resp)?;
    if let Some(b) = resp.find("\r\n\r\n") { Ok(resp[b + 4..].to_string()) } else { Ok(resp) }
}

pub fn package_name_from_source(source: &str) -> String {
    if source.contains('@') && !source.starts_with("@") { return source.split('@').next().unwrap_or("unnamed").to_string(); }
    Path::new(source).file_name().unwrap_or_default().to_string_lossy().trim_end_matches(".ts").trim_end_matches(".js").trim_end_matches(".peb.ts").to_string()
}

pub fn update_registry(packages_dir: &Path, name: &str, version: &str, source_type: &str) {
    let rp = packages_dir.join(".registry.json");
    let mut reg = if rp.exists() { json::parse(&fs::read_to_string(&rp).unwrap_or_default()).unwrap_or(json::Value::Object(Vec::new())) } else { json::Value::Object(Vec::new()) };
    reg.set(name, &json::Value::Object(vec![("version".into(), json::Value::String(version.into())), ("source".into(), json::Value::String(source_type.into())), ("installed".into(), json::Value::String("2026".into()))]));
    let _ = fs::write(&rp, reg.to_json());
}

pub fn extract_json_str<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let s = format!("\"{}\":\"", key); let start = json.find(&s)? + s.len(); let end = json[start..].find('"')?; Some(&json[start..start + end])
}

pub fn bump_version(current: &str, major: bool) -> String {
    let parts: Vec<&str> = current.split('.').collect();
    if parts.len() >= 3 { let m: u32 = parts[0].parse().unwrap_or(0); let n: u32 = parts[1].parse().unwrap_or(0); let p: u32 = parts[2].parse().unwrap_or(0); if major { format!("{}.0.0", m + 1) } else { format!("{}.{}.{}", m, n, p + 1) } } else { "0.1.0".to_string() }
}

pub fn check_registry(url: &str, name: &str) -> Option<String> {
    let u = format!("{}/{}/latest", url.trim_end_matches('/'), name);
    match fetch_url(&u) { Ok(b) => { let b = b.trim(); if b.len() < 20 && b.chars().filter(|c| c.is_digit(10) || *c == '.').count() > 1 { Some(b.into()) } else { None } } Err(_) => None }
}

pub fn run_shell(cmd: &str) {
    let status = if cfg!(windows) { std::process::Command::new("cmd").args(["/C", cmd]).status() } else { std::process::Command::new("sh").args(["-c", cmd]).status() };
    match status { Ok(s) if s.success() => {}, Ok(s) => { eprintln!("glas: exit code {}", s.code().unwrap_or(1)); }, Err(e) => { eprintln!("glas: {}", e); } }
}

pub fn run_shell_status(cmd: &str) -> io::Result<()> {
    let status = if cfg!(windows) { std::process::Command::new("cmd").args(["/C", cmd]).status() } else { std::process::Command::new("sh").args(["-c", cmd]).status() };
    match status { Ok(s) if s.success() => Ok(()), Ok(s) => Err(io::Error::new(io::ErrorKind::Other, format!("exit code {}", s.code().unwrap_or(1)))), Err(e) => Err(e) }
}

pub fn find_cached_glasshouse() -> Option<String> {
    let dir = if cfg!(windows) {
        let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
        format!("{}\\GlassHouse\\glasshouse", local)
    } else {
        let home = std::env::var("HOME").unwrap_or_default();
        format!("{}/.glasshouse/glasshouse", home)
    };
    let p = Path::new(&dir);
    if p.exists() && p.is_dir() {
        let mut count = 0;
        if let Ok(entries) = fs::read_dir(p) { for _ in entries { count += 1; } }
        if count > 0 { return Some(dir); }
    }
    None
}

pub fn fetch_github_releases(owner: &str, repo: &str) -> io::Result<Vec<(String, String, bool)>> {
    let url = format!("https://api.github.com/repos/{}/{}/releases", owner, repo);
    let tmp = std::env::temp_dir().join(format!("glas-{}-releases.json", repo));

    let status = if cfg!(windows) {
        let script = format!(
            "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12\r\nInvoke-WebRequest -Uri '{}' -OutFile '{}' -Headers @{{'User-Agent'='glas/2.0'}}\r\n",
            url, tmp.display()
        );
        let ps1 = std::env::temp_dir().join("glas-api.ps1");
        fs::write(&ps1, &script)?;
        let s = std::process::Command::new("powershell").args(["-ExecutionPolicy", "Bypass", "-File"]).arg(&ps1).status();
        let _ = fs::remove_file(&ps1);
        s
    } else {
        std::process::Command::new("sh")
            .args(["-c", &format!("curl -L -H 'User-Agent: glas/2.0' -o '{}' '{}'", tmp.display(), url)])
            .status()
    };
    match status {
        Ok(s) if s.success() => {},
        Ok(s) => return Err(io::Error::new(io::ErrorKind::Other, format!("exit code {}", s.code().unwrap_or(1)))),
        Err(e) => return Err(e),
    }

    let raw = fs::read_to_string(&tmp).unwrap_or_default();
    let _ = fs::remove_file(&tmp);

    if raw.is_empty() { return Ok(Vec::new()); }

    let mut results = Vec::new();
    if let Some(json::Value::Array(items)) = json::parse(&raw) {
        for item in items {
            if let json::Value::Object(pairs) = item {
                let mut tag = String::new();
                let mut name = String::new();
                let mut prerelease = false;
                for (k, v) in pairs {
                    match k.as_str() {
                        "tag_name" => { if let json::Value::String(s) = v { tag = s.clone(); } }
                        "name" => { if let json::Value::String(s) = v { name = s.clone(); } }
                        "prerelease" => { if let json::Value::Boolean(b) = v { prerelease = b; } }
                        _ => {}
                    }
                }
                if !tag.is_empty() {
                    results.push((tag, name, prerelease));
                }
            }
        }
    }
    Ok(results)
}

pub fn latest_glasshouse_version() -> String {
    match fetch_github_releases("OniuUI", "GlassHouse") {
        Ok(releases) => {
            for (tag, _, prerelease) in &releases {
                if !prerelease { return tag.clone(); }
            }
            releases.first().map(|r| r.0.clone()).unwrap_or_else(|| "v2.0.0".to_string())
        }
        Err(_) => "v2.0.0".to_string()
    }
}

pub fn fetch_release(url: &str, dest: &Path) -> io::Result<()> {
    if let Some(parent) = dest.parent() { fs::create_dir_all(parent)?; }
    let script = format!(
        "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12\r\nInvoke-WebRequest -Uri '{}' -OutFile '{}'\r\n",
        url, dest.display()
    );
    let ps1 = std::env::temp_dir().join("glas-fetch.ps1");
    fs::write(&ps1, &script)?;
    let status = if cfg!(windows) {
        std::process::Command::new("powershell").args(["-ExecutionPolicy", "Bypass", "-File"]).arg(&ps1).status()
    } else {
        std::process::Command::new("sh").args(["-c", &format!("curl -L -o '{}' '{}'", dest.display(), url)]).status()
    };
    let _ = fs::remove_file(&ps1);
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(io::Error::new(io::ErrorKind::Other, format!("exit code {}", s.code().unwrap_or(1)))),
        Err(e) => Err(e),
    }
}

pub fn extract_zip(zip_path: &Path, dest_dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dest_dir)?;
    let script = format!(
        "Expand-Archive -Path '{}' -DestinationPath '{}' -Force\r\n",
        zip_path.display(), dest_dir.display()
    );
    let ps1 = std::env::temp_dir().join("glas-extract.ps1");
    fs::write(&ps1, &script)?;
    let status = if cfg!(windows) {
        std::process::Command::new("powershell").args(["-ExecutionPolicy", "Bypass", "-File"]).arg(&ps1).status()
    } else {
        std::process::Command::new("sh").args(["-c", &format!("unzip -o '{}' -d '{}'", zip_path.display(), dest_dir.display())]).status()
    };
    let _ = fs::remove_file(&ps1);
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(io::Error::new(io::ErrorKind::Other, format!("exit code {}", s.code().unwrap_or(1)))),
        Err(e) => Err(e),
    }
}
