// installer/uninstall.rs — uninstall script generation + Windows registry

use std::fs;
use std::io;
use std::path::Path;

use crate::platform;

const GLAS_UNINSTALL: &str = "glas-uninstall.ps1";
const REG_UNINSTALL_KEY: &str = r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\GlasCLI";

pub fn write_uninstall_script(install_dir: &str, version: &str) {
    let script_path = Path::new(install_dir).join(GLAS_UNINSTALL);
    let escaped = install_dir.replace('\\', "\\\\");
    let content = format!(
        r#"# Glas CLI Uninstaller v{}
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
        version, escaped, REG_UNINSTALL_KEY
    );
    let _ = fs::write(&script_path, &content);
}

pub fn register_uninstall(install_dir: &str, version: &str) -> io::Result<()> {
    let uninstall_ps1 = Path::new(install_dir).join(GLAS_UNINSTALL);
    let uninstall_cmd = format!("powershell -ExecutionPolicy Bypass -File \"{}\"", uninstall_ps1.display());
    let cmds = [
        ("DisplayName", "Glas CLI"),
        ("UninstallString", &uninstall_cmd),
        ("Publisher", "OniuUI"),
        ("DisplayVersion", version),
        ("InstallLocation", install_dir),
        ("NoModify", "1"),
        ("NoRepair", "1"),
        ("URLInfoAbout", "https://github.com/OniuUI/Glas-CLI"),
    ];
    for (key, value) in &cmds {
        let cmd = format!(r#"reg add "{}" /v {} /d "{}" /f"#, REG_UNINSTALL_KEY, key, value);
        platform::run_shell_status(&cmd)?;
    }
    Ok(())
}

pub fn write_glas_bat(install_dir: &str) {
    let bat_path = Path::new(install_dir).join("glas.bat");
    let content = format!("@echo off\r\nsetlocal\r\n\"{}\\glas.exe\" %*\r\n", install_dir);
    let _ = fs::write(bat_path, content);
}
