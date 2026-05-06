// installer/extract.rs — zip extraction via platform-native tools

use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

pub fn extract_zip(zip_path: &Path, dest_dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dest_dir)?;
    let script = format!(
        "Expand-Archive -Path '{}' -DestinationPath '{}' -Force\r\n",
        zip_path.display(), dest_dir.display()
    );
    let ps1 = std::env::temp_dir().join("glas-installer-extract.ps1");
    fs::write(&ps1, &script)?;
    let status = if cfg!(windows) {
        Command::new("powershell").args(["-ExecutionPolicy", "Bypass", "-File"]).arg(&ps1).status()
    } else {
        Command::new("sh").args(["-c", &format!("unzip -o '{}' -d '{}'", zip_path.display(), dest_dir.display())]).status()
    };
    let _ = fs::remove_file(&ps1);
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(io::Error::new(io::ErrorKind::Other, format!("exit code {}", s.code().unwrap_or(1)))),
        Err(e) => Err(e),
    }
}
