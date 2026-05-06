use std::env;

pub mod json;
pub mod utils;
pub mod server;
pub mod commands;
pub mod quickjs;

use crate::utils::parse_port;

const VERSION: &str = "0.1.0";

fn parse_str_flag(args: &[String], flag: &str) -> Option<String> {
    for i in 0..args.len() {
        if args[i] == flag && i + 1 < args.len() { return Some(args[i + 1].clone()); }
    }
    None
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        help();
        return;
    }

    match args[1].as_str() {
        "init" => {
            let name = args.get(2).map(|s| s.as_str()).unwrap_or("glass-app");
            let gh_ver = parse_str_flag(&args, "--glasshouse").unwrap_or("latest".to_string());
            if gh_ver == "latest" {
                commands::init(name);
            } else {
                commands::init_with_version(name, &gh_ver);
            }
        }
        "serve" => {
            let port = parse_port(&args, 2);
            commands::serve(port);
        }
        "dev" => {
            let port = parse_port(&args, 2);
            let open = args.iter().any(|a| a == "--open");
            let lint = args.iter().any(|a| a == "--lint");
            commands::dev(port, open, lint);
        }
        "build" => {
            let dev = args.iter().any(|a| a == "--dev");
            let lint = args.iter().any(|a| a == "--lint");
            commands::build(dev, lint);
        }
        "install" => {
            let source = args.get(2).cloned().unwrap_or_default();
            let force = args.iter().any(|a| a == "--force" || a == "-f");
            if source.is_empty() {
                eprintln!("glas: install requires a source path or package name");
                return;
            }
            commands::install(&source, force);
        }
        "uninstall" => {
            let name = args.get(2).cloned().unwrap_or_default();
            if name.is_empty() {
                eprintln!("glas: uninstall requires a package name");
                return;
            }
            commands::uninstall(&name);
        }
        "list" => commands::list(),
        "info" => {
            let name = args.get(2).cloned().unwrap_or_default();
            if name.is_empty() {
                eprintln!("glas: info requires a package name");
                return;
            }
            commands::info(&name);
        }
        "audit" => {
            let deep = args.iter().any(|a| a == "--deep");
            let fix = args.iter().any(|a| a == "--fix");
            commands::audit(deep, fix);
        }
        "upgrade" => {
            let major = args.iter().any(|a| a == "--major");
            let dry = args.iter().any(|a| a == "--dry-run");
            let name = args.get(2).cloned();
            commands::upgrade(name.as_deref(), major, dry);
        }
        "run" => {
            if args.len() < 3 {
                eprintln!("glas: run requires a script name");
                return;
            }
            let script = &args[2];
            if script == "dev" {
                let port = parse_port(&args, 3);
                commands::serve(port);
            } else {
                commands::run(script);
            }
        }
        "lint" => {
            let flags = commands::parse_lint_flags(&args);
            commands::lint_run(&args, &flags);
        }
        "test" => {
            let flags = commands::parse_test_flags(&args);
            commands::test_run(&args, &flags);
        }
        "glasshouse" => {
            let sub = args.get(2).cloned().unwrap_or_else(|| "list".to_string());
            commands::glasshouse::run(&sub, &args[2..]);
        }
        "help" | "--help" | "-h" => help(),
        "version" | "--version" | "-v" => println!("glas v{}", VERSION),
        other => {
            eprintln!("glas: unknown command '{}'", other);
            println!("Run 'glas help' for available commands.");
        }
    }
}

fn help() {
    println!("glas v{} — Glass House CLI", VERSION);
    println!();
    println!("Usage: glas <command> [options]");
    println!();
    println!("Commands:");
    println!("  init <name>                  Scaffold a new Glass House project");
    println!("  dev [--port <N>] [--open]    Dev server with hot reload and TS compilation");
    println!("    --lint                       Lint on each file change during dev");
    println!("  serve [--port <N>]           Serve production build from dist/");
    println!("  build [--dev] [--lint]       Production build (hyper-compaction via QuickJS)");
    println!("    --dev                        Readable dev build (no compaction)");
    println!("    --lint                       Run lint before build, abort on errors");
    println!("  lint [options]               Lint project source files");
    println!("    --realtime, -r               Watch mode, re-lint on file changes");
    println!("    --fix, -f                    Auto-fix where possible (add 'use strict')");
    println!("    --json                       Output as JSON for CI");
    println!("    --strict                     Treat warnings as errors");
    println!("  test [options]               Run project tests");
    println!("    --filter <pattern>           Run only matching test names");
    println!("    --verbose, -v                Detailed output with timings");
    println!("    --json                       JSON output for CI");
    println!("    --watch                      Re-run tests on file changes");
    println!("  install <source> [-f]        Install a package (path or name@version)");
    println!("  install <name@1.2.3>         Install specific version");
    println!("  uninstall <name>             Remove an installed package");
    println!("  list                         List all installed packages");
    println!("  info <name>                  Show package details");
    println!("  audit [--deep] [--fix]       Audit project (--deep for full, --fix to auto-fix)");
    println!("  upgrade [<name>] [--major]   Upgrade packages (--major allows major bumps)");
    println!("  upgrade --dry-run             Preview upgrades without applying");
    println!("  run <script>                 Run a project script");
    println!("  glasshouse list              List available GlassHouse releases");
    println!("  glasshouse cache [--update]  Show or manage cached framework");
    println!("  help                         Show this help");
    println!("  version                      Show version");
    println!();
    println!("TypeScript Support:");
    println!("  All user code in src/ and packages/ must use TypeScript (.ts/.tsx).");
    println!("  JavaScript (.js) files are only allowed in glasshouse/ (framework).");
    println!("  The built-in TS compiler compiles .ts/.tsx via QuickJS subprocess.");
    println!();
    println!("Flags:");
    println!("  --force, -f    Force reinstall (overwrites existing)");
    println!("  --dev          Development mode (no compaction, readable output)");
    println!("  --port <N>     Set server port (default: 3000)");
    println!();
    println!("Examples:");
    println!("  glas init my-app");
    println!("  glas dev");
    println!("  glas dev --lint");
    println!("  glas build");
    println!("  glas build --lint");
    println!("  glas lint --realtime");
    println!("  glas lint --json");
    println!("  glas test --verbose");
    println!("  glas test --filter \"should render\"");
    println!("  glas serve --port 8080");
    println!("  glas install ./packages/my-pebble");
    println!("  glas install button@^1.0.0");
}
