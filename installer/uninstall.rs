// installer/uninstall.rs — uninstaller .exe bundling + Windows registry

use std::fs;
use std::io;
use std::path::Path;

const UNINSTALL_EXE: &str = "glas-uninstall.exe";
const REG_KEY: &str = r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\GlasCLI";

pub fn write_uninstall_exe(install_dir: &str) {
    let src = std::env::current_exe().unwrap_or_default();
    let src_dir = src.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let uninstall_src = src_dir.join(UNINSTALL_EXE);
    if uninstall_src.exists() {
        let dest = Path::new(install_dir).join(UNINSTALL_EXE);
        let _ = fs::copy(&uninstall_src, &dest);
    }
}

pub fn register_uninstall(install_dir: &str, version: &str) -> io::Result<()> {
    let exe = Path::new(install_dir).join(UNINSTALL_EXE);
    let cmd = format!("\"{}\"", exe.display());
    let entries = [
        ("DisplayName", "Glas CLI"),
        ("UninstallString", &cmd),
        ("Publisher", "OniuUI"),
        ("DisplayVersion", version),
        ("InstallLocation", install_dir),
        ("NoModify", "1"),
        ("NoRepair", "1"),
        ("URLInfoAbout", "https://github.com/OniuUI/Glas-CLI"),
    ];
    for (key, value) in &entries {
        let c = format!(r#"reg add "{}" /v {} /d "{}" /f"#, REG_KEY, key, value);
        crate::platform::run_shell_status(&c)?;
    }
    Ok(())
}
