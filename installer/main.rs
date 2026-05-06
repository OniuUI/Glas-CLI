// installer/main.rs
// Glas CLI Installer — custom Rust installer for glas + GlassHouse framework.
// Compiled with: rustc --edition 2021 installer/main.rs -o glas-installer.exe
//
// Downloads glas.exe and GlassHouse framework from GitHub Releases,
// installs to a directory, adds to PATH, writes uninstall script.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const VERSION: &str = "2.0.0";
const GLAS_EXE: &str = "glas.exe";
const GLAS_BAT: &str = "glas.bat";

const GLAS_RELEASES: &str = "https://github.com/OniuUI/Glas-CLI/releases";
const GH_RELEASES: &str = "https://github.com/OniuUI/GlassHouse/releases";

fn main() {
    let args: Vec<String> = env::args().collect();
    let silent = args.iter().any(|a| a == "--silent");
    let install_dir = parse_arg(&args, "--dir").unwrap_or_else(default_install_dir);

    if !silent { print_banner(); }

    let glas_ver = parse_arg(&args, "--glas").unwrap_or_else(|| "latest".into());
    let gh_ver = parse_arg(&args, "--glasshouse").unwrap_or_else(|| "latest".into());

    if !silent { println!("Install directory: {}", install_dir); }

    fs::create_dir_all(&install_dir).unwrap_or_else(|e| {
        eprintln!("Cannot create install directory: {}", e);
        std::process::exit(1);
    });

    let glas_path = Path::new(&install_dir).join(GLAS_EXE);
    let glas_bat_path = Path::new(&install_dir).join(GLAS_BAT);

    if !silent { println!("Downloading glas {}...", glas_ver); }
    match download_glas(&glas_ver, &glas_path) {
        Ok(_) => { if !silent { println!("  glas installed."); } }
        Err(e) => {
            eprintln!("Failed to download glas: {}", e);
            eprintln!("You can place glas.exe manually in: {}", install_dir);
        }
    }

    write_glas_bat(&glas_bat_path, &install_dir);

    if !silent { println!("Downloading GlassHouse framework {}...", gh_ver); }
    let gh_dir = Path::new(&install_dir).join("glasshouse");
    match download_glasshouse(&gh_ver, &gh_dir) {
        Ok(_) => { if !silent { println!("  GlassHouse framework cached."); } }
        Err(e) => {
            if !silent { eprintln!("  Could not pre-download GlassHouse: {}", e); }
            if !silent { eprintln!("  glas init will download it on first use."); }
        }
    }

    if let Err(e) = add_to_path(&install_dir) {
        eprintln!("Could not add to PATH: {}", e);
        println!("Add this directory to your PATH manually:");
        println!("  {}", install_dir);
    }

    write_uninstall_script(&install_dir);

    if !silent {
        println!();
        println!("Glas CLI installed successfully.");
        println!("Restart your terminal and run: glas init my-app");
    }
}

fn default_install_dir() -> String {
    if cfg!(windows) {
        let local = env::var("LOCALAPPDATA").unwrap_or_else(|_| {
            env::var("USERPROFILE").unwrap_or_else(|_| ".".into())
        });
        format!("{}\\GlassHouse", local)
    } else {
        let home = env::var("HOME").unwrap_or_else(|_| ".".into());
        format!("{}/.glasshouse", home)
    }
}

fn parse_arg(args: &[String], flag: &str) -> Option<String> {
    for i in 0..args.len() {
        if args[i] == flag && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }
    }
    None
}

fn print_banner() {
    println!("Glas CLI Installer v{}", VERSION);
    println!("=========================");
    println!();
}

fn download_glas(version: &str, dest: &Path) -> io::Result<()> {
    let url = if version == "latest" {
        format!("{}/latest/download/{}", GLAS_RELEASES, GLAS_EXE)
    } else {
        format!("{}/download/v{}/{}", GLAS_RELEASES, version, GLAS_EXE)
    };
    download_file(&url, dest)
}

fn download_glasshouse(version: &str, dest_dir: &Path) -> io::Result<()> {
    let url = if version == "latest" {
        format!("{}/latest/download/glasshouse.zip", GH_RELEASES)
    } else {
        format!("{}/download/v{}/glasshouse-v{}.zip", GH_RELEASES, version, version)
    };
    let tmp = std::env::temp_dir().join("glasshouse-tmp.zip");
    download_file(&url, &tmp)?;
    let _ = fs::create_dir_all(dest_dir);
    extract_zip(&tmp, dest_dir)?;
    let _ = fs::remove_file(&tmp);
    Ok(())
}

fn download_file(url: &str, dest: &Path) -> io::Result<()> {
    let cmd = if cfg!(windows) {
        format!(
            "powershell -Command \"[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; Invoke-WebRequest -Uri '{}' -OutFile '{}'\"",
            url, dest.display()
        )
    } else {
        format!("curl -L -o '{}' '{}'", dest.display(), url)
    };
    run_shell_status(&cmd)
}

fn extract_zip(zip_path: &Path, dest_dir: &Path) -> io::Result<()> {
    let cmd = if cfg!(windows) {
        format!(
            "powershell -Command \"Expand-Archive -Path '{}' -DestinationPath '{}' -Force\"",
            zip_path.display(), dest_dir.display()
        )
    } else {
        format!("unzip -o '{}' -d '{}'", zip_path.display(), dest_dir.display())
    };
    run_shell_status(&cmd)
}

fn add_to_path(dir: &str) -> io::Result<()> {
    if cfg!(windows) {
        let current = env::var("PATH").unwrap_or_default();
        if current.to_lowercase().contains(&dir.to_lowercase()) {
            return Ok(());
        }
        let cmd = format!("setx PATH \"{};{}\"", current, dir);
        run_shell_status(&cmd)?;
        let user_path = env::var("PATH").unwrap_or_default();
        if !user_path.to_lowercase().contains(&dir.to_lowercase()) {
            env::set_var("PATH", format!("{};{}", current, dir));
        }
    } else {
        let home = env::var("HOME").unwrap_or_default();
        let profile = Path::new(&home).join(".profile");
        let entry = format!("\nexport PATH=\"$PATH:{}\"", dir);
        if profile.exists() {
            let contents = fs::read_to_string(&profile).unwrap_or_default();
            if !contents.contains(dir) {
                let mut f = fs::OpenOptions::new().append(true).open(&profile)?;
                f.write_all(entry.as_bytes())?;
            }
        } else {
            fs::write(&profile, entry)?;
        }
    }
    Ok(())
}

fn write_glas_bat(bat_path: &Path, install_dir: &str) {
    let content = format!(
        "@echo off\r\nsetlocal\r\n\"{}\\{}\" %*\r\n",
        install_dir, GLAS_EXE
    );
    let _ = fs::write(bat_path, content);
}

fn write_uninstall_script(install_dir: &str) {
    let script_path = Path::new(install_dir).join("uninstall.ps1");
    let content = format!(
        r#"# Glas CLI Uninstaller
$dir = "{}"
Write-Host "Uninstalling Glas CLI from $dir..."
Remove-Item -LiteralPath "$dir" -Recurse -Force -ErrorAction SilentlyContinue
Write-Host "Done. Remove $dir from your PATH manually if needed."
"#,
        install_dir
    );
    let _ = fs::write(script_path, content);

    if cfg!(windows) {
        let script_path_unix = script_path.to_string_lossy().replace('\\', "\\\\");
        let cmd = format!(
            "powershell -Command \"Set-Content -Path '{}' -Value '{}'\"",
            script_path_unix,
            content.replace('\n', "`n").replace('\'', "''")
        );
        let _ = run_shell_status(&cmd);
    }
}

fn run_shell_status(cmd: &str) -> io::Result<()> {
    let status = if cfg!(windows) {
        Command::new("cmd").args(["/C", cmd]).status()
    } else {
        Command::new("sh").args(["-c", cmd]).status()
    };
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(io::Error::new(io::ErrorKind::Other, format!("exit code {}", s.code().unwrap_or(1)))),
        Err(e) => Err(e),
    }
}
