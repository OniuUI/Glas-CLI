// installer/install.rs — main install + silent install flow

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

use crate::download;
use crate::release;
use crate::ui;
use crate::platform;
use crate::uninstall;

const VERSION: &str = "0.1.0";
const GLAS_EXE: &str = "glas.exe";
const QJS_EXE: &str = "qjs.exe";

pub fn run() {
    let args: Vec<String> = env::args().collect();
    let silent = args.iter().any(|a| a == "--silent");

    if silent { install_silent(&args); return; }

    interactive_install(&args);
}

fn interactive_install(args: &[String]) {
    #[cfg(windows)] { let _ = std::process::Command::new("cmd").args(["/C", "echo"]).status(); }

    ui::clear();
    ui::draw_header(VERSION);

    let mut install_dir = release::parse_arg(args, "--dir").unwrap_or_else(platform::default_install_dir);
    println!("  {} {}", ui::dim("Install location:"), ui::cyan(&install_dir));
    let dir = ui::prompt_str("Enter to accept, or type new path", &install_dir);
    if dir != install_dir { install_dir = dir; }

    let install_path = Path::new(&install_dir);

    if install_path.join(GLAS_EXE).exists() && !args.iter().any(|a| a == "--force" || a == "-f") {
        println!();
        println!("  {} Glas already installed at {}", ui::red(ui::CROSS), install_dir);
        println!("  {} Use --force to reinstall", ui::dim("→"));
        return;
    }

    println!();
    println!("{}", ui::divider(54));
    println!();

    fs::create_dir_all(&install_dir).unwrap_or_else(|e| {
        eprintln!("{} Cannot create directory: {}", ui::red(ui::CROSS), e);
        std::process::exit(1);
    });

    let glas_ver = release::parse_arg(args, "--glas").unwrap_or_else(|| "latest".into());
    let gh_ver = release::parse_arg(args, "--glasshouse").unwrap_or_else(|| "latest".into());

    ui::spin_run("Downloading QuickJS runtime", || {
        download::download_qjs(&install_path.join(QJS_EXE))
    });

    ui::spin_run("Downloading glas", || {
        download::download_glas(&glas_ver, &install_path.join(GLAS_EXE))
    });

    ui::spin_run("Downloading GlassHouse framework", || {
        download::download_glasshouse(&gh_ver, &install_path.join("glasshouse"))
    });

    ui::spin_run("Adding to PATH", || { platform::add_to_path(&install_dir) });

    uninstall::write_glas_bat(&install_dir);
    uninstall::write_uninstall_script(&install_dir, VERSION);

    if cfg!(windows) {
        ui::spin_run("Registering with Add/Remove Programs", || {
            uninstall::register_uninstall(&install_dir, VERSION)
        });
    }

    println!("{}", ui::divider(54));
    println!();
    println!("  {} Glas CLI v{} installed.", ui::green(ui::CHECK), VERSION);
    println!();
    println!("  {} {}", ui::dim("Next:"), ui::cyan("glas init my-app"));
    println!();
    let _ = io::stdout().flush();
}

fn install_silent(args: &[String]) {
    let install_dir = release::parse_arg(args, "--dir").unwrap_or_else(platform::default_install_dir);
    let install_path = Path::new(&install_dir);
    let glas_ver = release::parse_arg(args, "--glas").unwrap_or_else(|| "latest".into());
    let gh_ver = release::parse_arg(args, "--glasshouse").unwrap_or_else(|| "latest".into());

    fs::create_dir_all(&install_dir).unwrap_or_else(|e| {
        eprintln!("glas-installer: cannot create directory: {}", e);
        std::process::exit(1);
    });

    if let Err(e) = download::download_qjs(&install_path.join(QJS_EXE)) {
        eprintln!("glas-installer: failed to download qjs: {}", e);
    }
    if let Err(e) = download::download_glas(&glas_ver, &install_path.join(GLAS_EXE)) {
        eprintln!("glas-installer: failed to download glas: {}", e);
    }
    if let Err(e) = download::download_glasshouse(&gh_ver, &install_path.join("glasshouse")) {
        eprintln!("glas-installer: warning: could not download GlassHouse: {}", e);
    }

    let _ = platform::add_to_path(&install_dir);
    uninstall::write_glas_bat(&install_dir);
    uninstall::write_uninstall_script(&install_dir, VERSION);
    if cfg!(windows) { let _ = uninstall::register_uninstall(&install_dir, VERSION); }

    println!("glas-installer: installed to {}", install_dir);
}
