// installer/release.rs — GitHub release version resolution (minimal, no JSON parser)

use std::fs;
use std::process::Command;

pub const GH_RELEASES: &str = "https://github.com/OniuUI/GlassHouse/releases";
pub const GLAS_RELEASES: &str = "https://github.com/OniuUI/Glas-CLI/releases";

pub fn latest_glas_version() -> String {
    fetch_latest_tag("OniuUI", "Glas-CLI")
}

pub fn latest_gh_version() -> String {
    fetch_latest_tag("OniuUI", "GlassHouse")
}

fn fetch_latest_tag(owner: &str, repo: &str) -> String {
    let url = format!("https://api.github.com/repos/{}/{}/releases", owner, repo);
    let tmp = std::env::temp_dir().join(format!("glas-installer-{}.json", repo));
    let script = format!(
        "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12\r\nInvoke-WebRequest -Uri '{}' -OutFile '{}' -Headers @{{'User-Agent'='glas-installer/0.1'}}\r\n",
        url, tmp.display()
    );
    let ps1 = std::env::temp_dir().join("glas-installer-api.ps1");
    let _ = fs::write(&ps1, &script);
    let _status = if cfg!(windows) {
        Command::new("powershell").args(["-ExecutionPolicy", "Bypass", "-File"]).arg(&ps1).status()
    } else {
        Command::new("sh").args(["-c", &format!("curl -s -L -H 'User-Agent: glas-installer/0.1' -o '{}' '{}'", tmp.display(), url)]).status()
    };
    let _ = fs::remove_file(&ps1);

    let raw = fs::read_to_string(&tmp).unwrap_or_default();
    let _ = fs::remove_file(&tmp);

    // Simple extraction: find first stable (non-prerelease) tag_name
    // Search for "prerelease":false then backtrack to "tag_name":"..."
    let mut best = String::new();
    let mut search_from = 0usize;
    while let Some(idx) = raw[search_from..].find("\"tag_name\"") {
        let start = search_from + idx + 12; // skip "tag_name":"
        if let Some(end) = raw[start..].find('"') {
            let tag = &raw[start..start + end];
            // Check if this release is a prerelease
            let prerelease_search = &raw[start..];
            let is_prerelease = prerelease_search.find("\"prerelease\":true").is_some();
            if !is_prerelease && tag.starts_with('v') {
                return tag.to_string();
            }
            if best.is_empty() && tag.starts_with('v') { best = tag.to_string(); }
            search_from = start + end;
        } else { break; }
    }
    if !best.is_empty() { best } else { "v0.1.0".to_string() }
}

pub fn parse_arg(args: &[String], flag: &str) -> Option<String> {
    for i in 0..args.len() { if args[i] == flag && i + 1 < args.len() { return Some(args[i + 1].clone()); } }
    None
}
