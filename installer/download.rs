// installer/download.rs — download artifacts from GitHub Releases

use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

use crate::release::{GH_RELEASES, GLAS_RELEASES};

const GLAS_EXE: &str = "glas.exe";
const QJS_VERSION: &str = "v0.8.0";

pub fn download_glas(version: &str, dest: &Path) -> io::Result<()> {
    let url = if version == "latest" {
        format!("{}/download/{}/{}", GLAS_RELEASES, crate::release::latest_glas_version(), GLAS_EXE)
    } else {
        format!("{}/download/v{}/{}", GLAS_RELEASES, version, GLAS_EXE)
    };
    download_file(&url, dest)
}

pub fn download_glasshouse(version: &str, dest_dir: &Path) -> io::Result<()> {
    let asset = format!("glasshouse.zip");
    let tag = if version == "latest" { crate::release::latest_gh_version() } else { format!("v{}", version) };
    let url = format!("{}/download/{}/{}", GH_RELEASES, tag, asset);
    let tmp = std::env::temp_dir().join("glasshouse-tmp.zip");
    download_file(&url, &tmp)?;
    let _ = fs::create_dir_all(dest_dir);
    crate::extract::extract_zip(&tmp, dest_dir)?;
    let _ = fs::remove_file(&tmp);
    Ok(())
}

pub fn download_qjs(dest: &Path) -> io::Result<()> {
    let platform = if cfg!(windows) { "qjs-windows-x86_64.exe" } else if cfg!(target_os = "macos") { "qjs-darwin" } else { "qjs-linux-x86_64" };
    let url = format!("https://github.com/quickjs-ng/quickjs/releases/download/{}/{}", QJS_VERSION, platform);
    download_file(&url, dest)
}

pub fn download_file(url: &str, dest: &Path) -> io::Result<()> {
    if let Some(parent) = dest.parent() { let _ = fs::create_dir_all(parent); }
    let script = format!(
        "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12\r\nInvoke-WebRequest -Uri '{}' -OutFile '{}'\r\n",
        url, dest.display()
    );
    let ps1 = std::env::temp_dir().join("glas-installer-fetch.ps1");
    fs::write(&ps1, &script)?;
    let status = if cfg!(windows) {
        Command::new("powershell").args(["-ExecutionPolicy", "Bypass", "-File"]).arg(&ps1).status()
    } else {
        Command::new("sh").args(["-c", &format!("curl -L -o '{}' '{}'", dest.display(), url)]).status()
    };
    let _ = fs::remove_file(&ps1);
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(io::Error::new(io::ErrorKind::Other, format!("exit code {}", s.code().unwrap_or(1)))),
        Err(e) => Err(e),
    }
}
