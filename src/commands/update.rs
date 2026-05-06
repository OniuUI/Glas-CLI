// commands/update.rs
// glas update — self-update the CLI binary and GlassHouse framework.
//
//   glas update               Check + install latest glas.exe
//   glas update --check        Show available updates, don't install
//   glas update --glasshouse    Update cached GlassHouse framework to latest

use std::env;
use std::fs;
use std::path::Path;

use crate::utils;

const GLAS_RELEASES: &str = "https://github.com/OniuUI/Glas-CLI/releases";

pub fn run(check: bool, gh: bool) {
    if gh {
        update_glasshouse();
        return;
    }

    let current = format!("v{}", crate::VERSION);

    match get_latest_cli_version() {
        Ok(latest) => {
            if latest == current {
                println!("glas is up to date ({}).", current);
                return;
            }

            if check {
                println!("glas {} → {} (update available)", current, latest);
                return;
            }

            println!("glas {} → {} — updating...", current, latest);
            match download_and_replace(&latest) {
                Ok(()) => println!("✓ Updated to {}", latest),
                Err(e) => eprintln!("glas: update failed: {}", e),
            }
        }
        Err(e) => eprintln!("glas: failed to check for updates: {}", e),
    }
}

fn get_latest_cli_version() -> Result<String, String> {
    let releases = utils::fetch_github_releases("OniuUI", "Glas-CLI")
        .map_err(|e| format!("{}", e))?;
    for (tag, _, prerelease) in &releases {
        if !prerelease {
            return Ok(tag.clone());
        }
    }
    releases
        .first()
        .map(|r| r.0.clone())
        .ok_or_else(|| "no releases found".to_string())
}

fn download_and_replace(version: &str) -> Result<(), String> {
    let exe_path = env::current_exe().map_err(|e| format!("cannot find current exe: {}", e))?;
    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| "cannot determine install directory".to_string())?;

    let new_exe = exe_dir.join("glas_new.exe");
    let backup_exe = exe_dir.join("glas_old.exe");

    let url = format!(
        "{}/download/{}/glas.exe",
        GLAS_RELEASES, version
    );

    utils::fetch_release(&url, &new_exe)
        .map_err(|e| format!("download failed: {}", e))?;

    // Rename current → backup, new → current
    let _ = fs::remove_file(&backup_exe);
    let _ = fs::rename(&exe_path, &backup_exe);
    let _ = fs::rename(&new_exe, &exe_path);

    // On Windows: schedule cleanup of the old binary via a temp .bat
    if cfg!(windows) {
        let bat = exe_dir.join("_glas_update.bat");
        let bat_content = format!(
            "@echo off\r\ntimeout /t 1 /nobreak >nul\r\ndel /f \"{}\" 2>nul\r\ndel /f \"%~f0\" 2>nul\r\n",
            backup_exe.display()
        );
        let _ = fs::write(&bat, &bat_content);
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "/B"])
            .arg(&bat)
            .spawn();
    } else {
        let _ = fs::remove_file(&backup_exe);
    }

    Ok(())
}

fn update_glasshouse() {
    if let Some(cache_dir) = utils::find_cached_glasshouse() {
        let gh_dir = Path::new(&cache_dir);
        println!("Updating cached GlassHouse...");
        let _ = fs::remove_dir_all(gh_dir);
        let tmp = std::env::temp_dir().join("glasshouse-update.zip");

        if let Err(e) = utils::fetch_release(
            &format!("https://github.com/OniuUI/GlassHouse/releases/download/{}/glasshouse.zip",
                utils::latest_glasshouse_version()
            ), &tmp) {
            eprintln!("glas: download failed: {}", e);
            return;
        }
        if let Err(e) = utils::extract_zip(&tmp, gh_dir) {
            eprintln!("glas: extract failed: {}", e);
            let _ = fs::remove_file(&tmp);
            return;
        }
        let _ = fs::remove_file(&tmp);
        println!("✓ GlassHouse cache updated.");
    } else {
        println!("No cached GlassHouse found.");
        println!("Run 'glas glasshouse cache --update' to download it.");
    }
}
