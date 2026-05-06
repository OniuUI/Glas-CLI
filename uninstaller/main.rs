// uninstaller/main.rs — glas-uninstall.exe
// Removes Glas CLI installation: files, PATH, registry entries.
// Compiled with: rustc --edition 2021 uninstaller/main.rs -o ../glas-uninstall.exe -C link-arg=../uninstall.res

use std::env;
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

const REG_UNINSTALL_KEY: &str = r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\GlasCLI";

fn main() {
    let exe_path = env::current_exe().unwrap_or_default();
    let install_dir = exe_path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();

    if install_dir.is_empty() || !Path::new(&install_dir).exists() {
        println!("Could not determine install directory.");
        println!("Remove the GlassHouse directory manually from your PATH.");
        return;
    }

    println!("Uninstalling Glas CLI from {}", install_dir);

    // Remove from PATH
    if cfg!(windows) {
        if let Ok(current) = env::var("PATH") {
            let entries: Vec<&str> = current.split(';').filter(|e| e.trim_end_matches('\\') != install_dir.trim_end_matches('\\')).collect();
            let new_path = entries.join(";");
            let cmd = format!("setx PATH \"{}\"", new_path);
            let _ = run_shell(&cmd);
            println!("  Removed from PATH.");
        }
    } else {
        let home = env::var("HOME").unwrap_or_default();
        let profile = Path::new(&home).join(".profile");
        if profile.exists() {
            if let Ok(c) = fs::read_to_string(&profile) {
                let filtered: String = c.lines().filter(|l| !l.contains(&install_dir)).collect::<Vec<_>>().join("\n");
                let _ = fs::write(&profile, filtered);
            }
        }
    }

    // Remove registry entry
    if cfg!(windows) {
        let cmd = format!("reg delete \"{}\" /f", REG_UNINSTALL_KEY);
        let _ = run_shell(&cmd);
    }

    // Remove files (schedule via temp .bat since we're running from this directory)
    if cfg!(windows) {
        let bat = std::env::temp_dir().join("_glas_cleanup.bat");
        let content = format!(
            "@echo off\r\ntimeout /t 2 /nobreak >nul\r\nrmdir /s /q \"{}\" 2>nul\r\ndel /f \"%~f0\" 2>nul\r\n",
            install_dir
        );
        let _ = fs::write(&bat, &content);
        let _ = Command::new("cmd").args(["/C", "start", "/B"]).arg(&bat).spawn();
    }

    println!("Glas CLI uninstalled. Files will be removed shortly.");
}

fn run_shell(cmd: &str) -> io::Result<()> {
    let status = Command::new("cmd").args(["/C", cmd]).status()?;
    if status.success() { Ok(()) } else { Err(io::Error::new(io::ErrorKind::Other, "cmd failed")) }
}
