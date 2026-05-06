// installer/main.rs
// Glas CLI Installer — custom Rust installer with console UI.
// Compiled with: rustc --edition 2021 installer/main.rs -o glas-installer.exe
//
// Downloads glas.exe + GlassHouse framework from GitHub Releases,
// installs to user directory, adds to PATH, registers uninstaller.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

const VERSION: &str = "2.0.0";
const GLAS_EXE: &str = "glas.exe";
const GLAS_BAT: &str = "glas.bat";
const GLAS_UNINSTALL: &str = "glas-uninstall.ps1";

const GLAS_RELEASES: &str = "https://github.com/OniuUI/Glas-CLI/releases";
const GH_RELEASES: &str = "https://github.com/OniuUI/GlassHouse/releases";
const REG_UNINSTALL_KEY: &str = r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\GlasCLI";

// ── ANSI / UI constants ──

const C_RESET: &str = "\x1b[0m";
const C_DIM: &str = "\x1b[2m";
const C_GREEN: &str = "\x1b[32m";
const C_RED: &str = "\x1b[31m";
const C_BLUE: &str = "\x1b[34m";
const C_CYAN: &str = "\x1b[36m";

const CHECK: &str = "✓";
const CROSS: &str = "✗";
const DOT:   &str = "●";

fn color(s: &str, code: &str) -> String { format!("{}{}{}", code, s, C_RESET) }
fn green(s: &str) -> String { color(s, C_GREEN) }
fn red(s: &str) -> String { color(s, C_RED) }
fn blue(s: &str) -> String { color(s, C_BLUE) }
fn cyan(s: &str) -> String { color(s, C_CYAN) }
fn dim(s: &str) -> String { color(s, C_DIM) }

fn clear() { print!("\x1b[2J\x1b[H"); }

fn box_line(text: &str, width: usize) -> String {
    let inner = width.saturating_sub(4);
    let pad = inner.saturating_sub(text.len());
    let left = pad / 2;
    let right = pad - left;
    format!("║{}{}{}║", " ".repeat(left), text, " ".repeat(right))
}

fn box_top(width: usize) -> String {
    let inner = width.saturating_sub(2);
    format!("╔{}╗", "═".repeat(inner))
}

fn box_bottom(width: usize) -> String {
    let inner = width.saturating_sub(2);
    format!("╚{}╝", "═".repeat(inner))
}

fn divider(width: usize) -> String { "─".repeat(width) }

fn draw_header(width: usize) {
    let w = width.min(60);
    println!("{}", box_top(w));
    println!("{}", box_line(&format!("Glas CLI Installer v{}", VERSION), w));
    println!("{}", box_line("Zero-Dependency GlassHouse Tooling", w));
    println!("{}", box_line("github.com/OniuUI/Glas-CLI", w));
    println!("{}", box_bottom(w));
    println!();
}

fn prompt_str(prompt: &str, default: &str) -> String {
    print!("  {} [{}]: ", dim(prompt), cyan(default));
    let _ = io::stdout().flush();
    let mut line = String::new();
    io::stdin().read_line(&mut line).unwrap_or_default();
    let trimmed = line.trim().to_string();
    if trimmed.is_empty() { default.to_string() } else { trimmed }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let silent = args.iter().any(|a| a == "--silent");

    if silent {
        run_silent(&args);
        return;
    }

    // ── Enable ANSI on Windows ──
    #[cfg(windows)]
    {
        let _ = Command::new("cmd").args(["/C", "echo"]).status();
    }

    clear();
    draw_header(60);

    let install_dir = parse_arg(&args, "--dir").unwrap_or_else(default_install_dir);

    println!("  {} {}", dim("Install location:"), cyan(&install_dir));
    let dir = prompt_str("Enter to accept, or type new path", &install_dir);
    let install_dir = if dir == install_dir { install_dir } else { dir };
    let install_path = Path::new(&install_dir);

    if install_path.join(GLAS_EXE).exists() && !args.iter().any(|a| a == "--force" || a == "-f") {
        println!();
        println!("  {} Glas already installed at {}", red(CROSS), install_dir);
        println!("  {} Use --force to reinstall", dim("→"));
        println!("  {} Run {} to remove", dim("→"), cyan(&format!("{}\\{}", install_dir, GLAS_UNINSTALL)));
        return;
    }

    println!();
    println!("{}", divider(54));
    println!();

    let glas_ver = parse_arg(&args, "--glas").unwrap_or_else(|| "latest".into());
    let gh_ver = parse_arg(&args, "--glasshouse").unwrap_or_else(|| "latest".into());

    fs::create_dir_all(&install_dir).unwrap_or_else(|e| {
        eprintln!("{} Cannot create directory: {}", red(CROSS), e);
        std::process::exit(1);
    });

    // ── Step 1: Download glas.exe ──
    spin_run("Downloading glas", || {
        let glas_path = install_path.join(GLAS_EXE);
        download_glas(&glas_ver, &glas_path)
    }, "gls");

    // ── Step 2: Download GlassHouse ──
    spin_run("Downloading GlassHouse framework", || {
        let gh_dir = install_path.join("glasshouse");
        download_glasshouse(&gh_ver, &gh_dir)
    }, "gh");

    // ── Step 3: PATH ──
    spin_run("Adding to PATH", || { add_to_path(&install_dir) }, "path");

    // ── Step 4: Uninstall script ──
    write_glas_bat(&install_path.join(GLAS_BAT), &install_dir);
    write_uninstall_script(&install_dir);

    // ── Step 5: Registry ──
    if cfg!(windows) {
        spin_run("Registering with Add/Remove Programs", || {
            register_uninstall(&install_dir)
        }, "reg");
    }

    println!("{}", divider(54));
    println!();
    println!("  {} Glas CLI v{} installed.", green(CHECK), VERSION);
    println!();
    println!("  {} {}", dim("Next:"), cyan("glas init my-app"));
    println!("  {} {}", dim("Uninstall:"), dim(&format!("{}\\{}", install_dir, GLAS_UNINSTALL)));
    println!();

    let _ = io::stdout().flush();
}

fn run_silent(args: &[String]) {
    let install_dir = parse_arg(args, "--dir").unwrap_or_else(default_install_dir);
    let install_path = Path::new(&install_dir);
    let glas_ver = parse_arg(args, "--glas").unwrap_or_else(|| "latest".into());
    let gh_ver = parse_arg(args, "--glasshouse").unwrap_or_else(|| "latest".into());

    fs::create_dir_all(&install_dir).unwrap_or_else(|e| {
        eprintln!("glas-installer: cannot create directory: {}", e);
        std::process::exit(1);
    });

    let glas_path = install_path.join(GLAS_EXE);
    if let Err(e) = download_glas(&glas_ver, &glas_path) {
        eprintln!("glas-installer: failed to download glas: {}", e);
    }

    let gh_dir = install_path.join("glasshouse");
    if let Err(e) = download_glasshouse(&gh_ver, &gh_dir) {
        eprintln!("glas-installer: warning: could not download GlassHouse: {}", e);
    }

    let _ = add_to_path(&install_dir);
    write_glas_bat(&install_path.join(GLAS_BAT), &install_dir);
    write_uninstall_script(&install_dir);

    if cfg!(windows) { let _ = register_uninstall(&install_dir); }

    println!("glas-installer: installed to {}", install_dir);
}

fn spin_run(label: &str, f: impl FnOnce() -> io::Result<()>, _step_id: &str) {
    print!("  {} {} ...", blue(DOT), dim(label));
    let _ = io::stdout().flush();

    let result = f();

    // Clear the spinner line
    print!("\r\x1b[K");
    match result {
        Ok(()) => println!("  {} {}", green(CHECK), label),
        Err(e) => println!("  {} {} — {}", red(CROSS), label, e),
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
    if let Some(parent) = dest.parent() { let _ = fs::create_dir_all(parent); }
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
    fs::create_dir_all(dest_dir)?;
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
    let script_path = Path::new(install_dir).join(GLAS_UNINSTALL);
    let escaped = install_dir.replace('\\', "\\\\");
    let content = format!(
        r#"# Glas CLI Uninstaller
Write-Host "Uninstalling Glas CLI..."

$dir = "{}"
$currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($currentPath -like "*$dir*") {{
    $newPath = ($currentPath -split ';' | Where-Object {{ $_ -ne $dir -and $_ -ne "$dir\" }} ) -join ';'
    [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
    Write-Host "  Removed from PATH."
}}

if (Test-Path -LiteralPath "$dir") {{
    Remove-Item -LiteralPath "$dir" -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "  Removed files."
}}

reg delete "{}" /f 2>$null

Write-Host "Glas CLI uninstalled."
"#,
        escaped, REG_UNINSTALL_KEY
    );
    let _ = fs::write(&script_path, &content);
}

fn register_uninstall(install_dir: &str) -> io::Result<()> {
    let uninstall_ps1 = Path::new(install_dir).join(GLAS_UNINSTALL);
    let uninstall_cmd = format!(
        "powershell -ExecutionPolicy Bypass -File \"{}\"",
        uninstall_ps1.display()
    );

    let cmds = [
        ("DisplayName", "Glas CLI"),
        ("UninstallString", &uninstall_cmd),
        ("Publisher", "OniuUI"),
        ("DisplayVersion", VERSION),
        ("InstallLocation", install_dir),
        ("NoModify", "1"),
        ("NoRepair", "1"),
        ("URLInfoAbout", "https://github.com/OniuUI/Glas-CLI"),
    ];

    for (key, value) in &cmds {
        let cmd = format!(
            r#"reg add "{}" /v {} /d "{}" /f"#,
            REG_UNINSTALL_KEY, key, value
        );
        run_shell_status(&cmd)?;
    }
    Ok(())
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
