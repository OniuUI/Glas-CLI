// commands/glasshouse.rs
// glas glasshouse — manage GlassHouse framework installation.
//
//   glas glasshouse list         List available GitHub releases
//   glas glasshouse cache        Show cache status
//   glas glasshouse cache --update  Refresh cache
//   glas glasshouse cache --clear   Remove cache
//   glas glasshouse install      Download latest framework

use std::fs;
use std::path::Path;

use crate::utils;

const GH_RELEASES: &str = "https://github.com/OniuUI/GlassHouse/releases";

pub fn run(subcommand: &str, args: &[String]) {
    match subcommand {
        "list" => cmd_list(),
        "cache" => {
            let update = args.iter().any(|a| a == "--update");
            let clear = args.iter().any(|a| a == "--clear");
            cmd_cache(update, clear);
        }
        "install" => cmd_install(),
        _ => {
            eprintln!("glas: unknown glasshouse subcommand '{}'", subcommand);
            print_help();
        }
    }
}

fn cmd_list() {
    println!("Fetching releases from GitHub...");
    match utils::fetch_github_releases("OniuUI", "GlassHouse") {
        Ok(releases) => {
            if releases.is_empty() {
                println!("No releases found.");
                return;
            }
            println!();
            for (tag, name, prerelease) in &releases {
                let badge = if *prerelease { "(pre-release)" } else { "(stable)" };
                let latest = if !prerelease && tag == &releases.iter().find(|r| !r.2).map(|r| &r.0).unwrap_or(&"".to_string()).clone() {
                    " ← latest"
                } else { "" };
                println!("  {}  {}  {}{}", tag, name, badge, latest);
            }
            println!();
            println!("Install: glas init my-app --glasshouse <version>");
        }
        Err(e) => eprintln!("glas: failed to fetch releases: {}", e),
    }
}

fn cmd_cache(update: bool, clear: bool) {
    if clear {
        if let Some(cache_dir) = utils::find_cached_glasshouse() {
            let parent = Path::new(&cache_dir).parent().unwrap_or(Path::new("."));
            if let Err(e) = fs::remove_dir_all(parent) {
                eprintln!("glas: failed to clear cache: {}", e);
            } else {
                println!("✓ Cache cleared.");
            }
        } else {
            println!("No cache found.");
        }
        return;
    }

    match utils::find_cached_glasshouse() {
        Some(cache_dir) => {
            println!("Cache: {}", cache_dir);
            if update {
                println!("  Refreshing...");
                let gh_dir = Path::new(&cache_dir);
                let _ = fs::remove_dir_all(gh_dir);
                let tmp = std::env::temp_dir().join("glasshouse-cache.zip");
                let url = format!("{}/latest/download/glasshouse.zip", GH_RELEASES);
                if let Err(e) = utils::fetch_release(&url, &tmp) {
                    eprintln!("glas: download failed: {}", e);
                } else if let Err(e) = utils::extract_zip(&tmp, gh_dir) {
                    eprintln!("glas: extract failed: {}", e);
                } else {
                    let _ = fs::remove_file(&tmp);
                    println!("✓ Cache updated.");
                }
            }
        }
        None => {
            println!("No cache found.");
            println!("Run 'glas glasshouse cache --update' to fetch the framework.");
            println!("Or use 'glas-installer.exe' to set up a full installation.");
        }
    }
}

fn cmd_install() {
    if let Some(cache_dir) = utils::find_cached_glasshouse() {
        println!("GlassHouse is already cached at:");
        println!("  {}", cache_dir);
        println!("Run 'glas glasshouse cache --update' to refresh.");
        return;
    }

    println!("Downloading latest GlassHouse framework...");
    let gh_dir = std::env::current_dir().unwrap_or_default().join("glasshouse");
    let tmp = std::env::temp_dir().join("glasshouse-install.zip");
    let url = format!("{}/latest/download/glasshouse.zip", GH_RELEASES);

    if let Err(e) = utils::fetch_release(&url, &tmp) {
        eprintln!("glas: download failed: {}", e);
        return;
    }
    if let Err(e) = utils::extract_zip(&tmp, &gh_dir) {
        eprintln!("glas: extract failed: {}", e);
        let _ = fs::remove_file(&tmp);
        return;
    }
    let _ = fs::remove_file(&tmp);
    println!("✓ GlassHouse installed to glasshouse/");
}

fn print_help() {
    println!("glas glasshouse <command>");
    println!();
    println!("Commands:");
    println!("  list             List available GlassHouse releases");
    println!("  cache            Show cache status");
    println!("  cache --update   Refresh cached framework");
    println!("  cache --clear    Remove cached framework");
    println!("  install          Download latest GlassHouse to current project");
}
