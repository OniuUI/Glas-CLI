// installer/platform.rs — OS-specific helpers

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

pub fn default_install_dir() -> String {
    if cfg!(windows) {
        let local = env::var("LOCALAPPDATA").unwrap_or_else(|_| env::var("USERPROFILE").unwrap_or_else(|_| ".".into()));
        format!("{}\\GlassHouse", local)
    } else {
        let home = env::var("HOME").unwrap_or_else(|_| ".".into());
        format!("{}/.glasshouse", home)
    }
}

pub fn add_to_path(dir: &str) -> io::Result<()> {
    if cfg!(windows) {
        let current = env::var("PATH").unwrap_or_default();
        if current.to_lowercase().contains(&dir.to_lowercase()) { return Ok(()); }
        let new_path = format!("{};{}", current, dir);
        let cmd = format!("setx PATH \"{}\"", new_path);
        run_shell_status(&cmd)?;
        env::set_var("PATH", &new_path);
    } else {
        let home = env::var("HOME").unwrap_or_default();
        let profile = Path::new(&home).join(".profile");
        let entry = format!("\nexport PATH=\"$PATH:{}\"\n", dir);
        if profile.exists() {
            let contents = fs::read_to_string(&profile).unwrap_or_default();
            if !contents.contains(dir) {
                let mut f = fs::OpenOptions::new().append(true).open(&profile)?;
                f.write_all(entry.as_bytes())?;
            }
        } else { fs::write(&profile, entry)?; }
    }
    Ok(())
}

pub fn run_shell_status(cmd: &str) -> io::Result<()> {
    let status = if cfg!(windows) { Command::new("cmd").args(["/C", cmd]).status() } else { Command::new("sh").args(["-c", cmd]).status() };
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(io::Error::new(io::ErrorKind::Other, format!("exit code {}", s.code().unwrap_or(1)))),
        Err(e) => Err(e),
    }
}
