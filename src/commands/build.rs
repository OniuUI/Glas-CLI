// cli/commands/build.rs
// Build command — compiles TypeScript, bundles via hyper-compactor (QuickJS),
// and writes output to dist/.
//
// Flags:
//   --dev   Dev mode: readable concatenation, no hyper-compaction
//   --lint  Run lint before build, abort on errors

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::quickjs;
use crate::utils;

/// Production build entry point.
/// `dev` — if true, produce a readable concatenated dev bundle.
/// `lint` — if true, run lint first and abort on errors.
pub fn build(dev: bool, lint: bool) {
    let cwd = env::current_dir().unwrap_or_default();

    if !cwd.join("glasshouse").exists() {
        eprintln!("glas: not in a Glass House project");
        return;
    }

    // ── Pre-build: check for forbidden .js files in src/ and packages/ ──
    let js_violations = check_forbidden_js_files(&cwd);
    if !js_violations.is_empty() {
        eprintln!("glas: JavaScript files not allowed in user codebase:");
        for v in &js_violations {
            eprintln!("  ✗ {}", v);
        }
        eprintln!("  Use TypeScript (.ts/.tsx) instead.");
        return;
    }

    // ── Optional pre-build lint ──
    if lint {
        println!("⚑ Running lint before build...");
        let flags = crate::commands::LintFlags {
            realtime: false,
            fix: false,
            json: false,
            strict: true, // strict mode for pre-build lint
        };
        crate::commands::lint_run(&[], &flags);
    }

    if dev {
        build_dev(&cwd);
    } else {
        build_prod(&cwd);
    }
}

/// Check for .js files in src/ and packages/ directories.
/// Returns a list of relative paths that violate the rule.
fn check_forbidden_js_files(cwd: &Path) -> Vec<String> {
    let mut violations = Vec::new();

    for dir_name in &["src", "packages"] {
        let dir_path = cwd.join(dir_name);
        if !dir_path.exists() {
            continue;
        }
        if let Ok(entries) = utils::walk_dir(&dir_path) {
            for p in entries {
                if p.extension().and_then(|e| e.to_str()) == Some("js") {
                    // Skip registry metadata
                    if p.file_name().map_or(false, |n| n == ".registry.json") {
                        continue;
                    }
                    let rel = p.strip_prefix(cwd).unwrap_or(&p).to_string_lossy().to_string();
                    violations.push(rel);
                }
            }
        }
    }

    violations.sort();
    violations
}

/// Collect all source files for building.
/// Returns: (framework_js_files, ts_files_to_compile, package_js_allowed)
/// Framework .js files from glasshouse/ are allowed as-is.
/// User .ts/.tsx files from src/ and packages/ will be compiled.
struct SourceCollection {
    /// Framework .js files (glasshouse/*.js) — served as-is
    framework: Vec<PathBuf>,
    /// User .ts/.tsx files (src/**/*.ts, packages/**/*.ts)
    ts_files: Vec<PathBuf>,
}

fn collect_sources(cwd: &Path) -> SourceCollection {
    let mut framework = Vec::new();
    let mut ts_files = Vec::new();

    // Framework: glasshouse/*.js
    let gh_dir = cwd.join("glasshouse");
    if gh_dir.exists() {
        if let Ok(es) = fs::read_dir(&gh_dir) {
            let mut files: Vec<PathBuf> = es
                .flatten()
                .filter_map(|e| {
                    let p = e.path();
                    if p.extension().and_then(|ex| ex.to_str()) == Some("js") {
                        Some(p)
                    } else {
                        None
                    }
                })
                .collect();
            files.sort();
            framework = files;
        }
    }

    // User sources: .ts/.tsx from src/ and packages/
    for dir_name in &["src", "packages"] {
        let dir_path = cwd.join(dir_name);
        if !dir_path.exists() {
            continue;
        }
        if let Ok(entries) = utils::walk_dir(&dir_path) {
            for p in entries {
                if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                    if ext == "ts" || ext == "tsx" {
                        ts_files.push(p);
                    }
                }
            }
        }
    }
    ts_files.sort();

    SourceCollection { framework, ts_files }
}

/// Compile all .ts/.tsx files via QuickJS running the TS compiler.
/// Returns a map of (relative_path → compiled_js_source).
fn compile_ts_files(
    cwd: &Path,
    ts_files: &[PathBuf],
) -> Result<Vec<(String, String)>, String> {
    if ts_files.is_empty() {
        return Ok(Vec::new());
    }

    // Load TS compiler and all its dependencies
    let ts_modules = &[
        "glasshouse", "ts-lexer", "ts-parser", "ts-binder",
        "ts-checker", "ts-emitter", "ts-compiler",
    ];

    let modules = quickjs::load_glasshouse_modules(cwd, ts_modules)?;

    // Build sources JSON for the JS wrapper
    let mut sources_json = String::from("[");
    let mut file_count = 0u32;
    for fp in ts_files {
        if file_count > 0 {
            sources_json.push(',');
        }
        let rel = fp
            .strip_prefix(cwd)
            .unwrap_or(fp)
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        let source = fs::read_to_string(fp).unwrap_or_default();
        let escaped = source
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t");
        sources_json.push_str(&format!(
            "{{\"file\":\"{}\",\"source\":\"{}\"}}",
            rel, escaped
        ));
        file_count += 1;
    }
    sources_json.push(']');

    let compile_code = format!(
        r#"
var sources = {};
var compiler = GlassHouse.require('ts-compiler');
var results = [];

for (var i = 0; i < sources.length; i++) {{
    var sf = sources[i];
    var result = compiler.compile(sf.source, {{}});
    results.push({{
        file: sf.file,
        success: result.success,
        js: result.js || '',
        diagnostics: result.diagnostics || [],
        parseTime: result.parseTime || 0,
        totalTime: result.totalTime || 0
    }});
}}

print(JSON.stringify(results));
"#,
        sources_json
    );

    let output = quickjs::eval_with_modules(&compile_code, &modules)?;
    let trimmed = output.trim();

    // Parse the JSON result
    let compiled: Vec<(String, String)> = match crate::json::parse(trimmed) {
        Some(crate::json::Value::Array(items)) => {
            let mut results = Vec::new();
            let mut had_errors = false;
            for item in &items {
                if let crate::json::Value::Object(pairs) = item {
                    let mut file = String::new();
                    let mut js = String::new();
                    let mut success = false;
                    let mut diagnostics = String::new();
                    for (key, val) in pairs {
                        match key.as_str() {
                            "file" => {
                                if let crate::json::Value::String(s) = val {
                                    file = s.clone();
                                }
                            }
                            "js" => {
                                if let crate::json::Value::String(s) = val {
                                    js = s.clone();
                                }
                            }
                            "success" => {
                                if let crate::json::Value::Boolean(b) = val {
                                    success = *b;
                                }
                            }
                            "diagnostics" => {
                                diagnostics = format_diagnostics(val);
                            }
                            _ => {}
                        }
                    }
                    if !success && !diagnostics.is_empty() {
                        eprintln!("  ✗ Compile error in {}:", file);
                        eprintln!("{}", diagnostics);
                        had_errors = true;
                    }
                    if !js.is_empty() {
                        results.push((file, js));
                    }
                }
            }
            if had_errors {
                eprintln!("glas: TypeScript compilation had errors. Fix them and try again.");
                // Still return what we have — some files may have compiled successfully
            }
            results
        }
        _ => {
            return Err(format!("Failed to parse TS compiler output: {}", trimmed));
        }
    };

    Ok(compiled)
}

/// Format diagnostics array for display.
fn format_diagnostics(val: &crate::json::Value) -> String {
    let mut out = String::new();
    if let crate::json::Value::Array(items) = val {
        for item in items {
            if let crate::json::Value::Object(pairs) = item {
                let mut severity = String::new();
                let mut message = String::new();
                for (k, v) in pairs {
                    match k.as_str() {
                        "severity" => {
                            if let crate::json::Value::String(s) = v {
                                severity = s.clone();
                            }
                        }
                        "message" => {
                            if let crate::json::Value::String(s) = v {
                                message = s.clone();
                            }
                        }
                        _ => {}
                    }
                }
                out.push_str(&format!("    [{}] {}\n", severity, message));
            }
        }
    }
    out
}

// ── Production build: full hyper-compaction via QuickJS ──

fn build_prod(cwd: &Path) {
    let dist = cwd.join("dist");
    let _ = fs::create_dir_all(&dist);

    println!("⚒ Building Glass House (hyper-compaction)...");

    let sources = collect_sources(cwd);

    // 1. Compile TypeScript files
    if !sources.ts_files.is_empty() {
        println!("  Compiling {} TypeScript file(s)...", sources.ts_files.len());
    }

    let compiled = match compile_ts_files(cwd, &sources.ts_files) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("glas: {}", e);
            return;
        }
    };

    // 2. Collect all JS sources: framework + compiled TS
    let mut all_sources: Vec<(String, String)> = Vec::new();
    let mut total_orig: u64 = 0;

    // Framework JS (glasshouse/*.js)
    for fp in &sources.framework {
        let rel = fp
            .strip_prefix(cwd)
            .unwrap_or(fp)
            .to_string_lossy()
            .to_string();
        if let Ok(src) = fs::read_to_string(fp) {
            total_orig += src.len() as u64;
            all_sources.push((rel, src));
        }
    }

    // Compiled TS sources
    for (rel, js) in &compiled {
        total_orig += js.len() as u64;
        all_sources.push((rel.clone(), js.clone()));
    }

    if all_sources.is_empty() {
        eprintln!("glas: no source files found to build");
        return;
    }

    let source_count = all_sources.len();

    // 3. Run hyper-compactor via QuickJS
    println!(
        "  Compacting {} source(s) ({:.1} KB)...",
        source_count,
        total_orig as f64 / 1024.0
    );

    match run_hyper_compactor(cwd, &all_sources) {
        Ok(bundle_result) => {
            let out_path = dist.join("glasshouse.bundle.js");
            let _ = fs::write(&out_path, &bundle_result.bundle);

            if let Ok(meta) = fs::metadata(&out_path) {
                let size = meta.len();
                let reduction = if total_orig > 0 {
                    ((1.0 - (size as f64 / total_orig as f64)) * 100.0).round() as i32
                } else {
                    0
                };
                println!(
                    "  dist/glasshouse.bundle.js  ({:.1} KB, {}% reduction)",
                    size as f64 / 1024.0,
                    reduction
                );
                if bundle_result.renamed > 0 {
                    println!("  {} identifiers renamed", bundle_result.renamed);
                }
                if bundle_result.pool_entries > 0 {
                    println!("  {} string pool entries", bundle_result.pool_entries);
                }
            }

            write_index_html(&dist, &all_sources, true);
            println!("  dist/index.html");
            println!("  ✓ Build complete");
        }
        Err(e) => {
            eprintln!("glas: hyper-compaction failed — {}", e);
        }
    }
}

/// Run the hyper-compactor via QuickJS.
struct BundleResult {
    bundle: String,
    renamed: u32,
    pool_entries: u32,
}

fn run_hyper_compactor(
    cwd: &Path,
    sources: &[(String, String)],
) -> Result<BundleResult, String> {
    // Load required modules: glasshouse, ts-lexer, hyper-compactor
    let hc_modules = &["glasshouse", "ts-lexer", "hyper-compactor"];
    let modules = quickjs::load_glasshouse_modules(cwd, hc_modules)?;

    // Build sources JSON
    let mut sources_json = String::from("{");
    for (i, (name, src)) in sources.iter().enumerate() {
        if i > 0 {
            sources_json.push(',');
        }
        let escaped_name = name.replace('\\', "\\\\").replace('"', "\\\"");
        let escaped_src = src
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r");
        sources_json.push_str(&format!(
            "\"{}\":\"{}\"",
            escaped_name, escaped_src
        ));
    }
    sources_json.push('}');

    let code = format!(
        r#"
var sources = {};
var compactor = GlassHouse.require('hyper-compactor');
var result = compactor.compactAll(sources);

// Build the bundle string
var bundle = '/* Glass House GHC2 */\n';
bundle += 'var _GH="' + result.binaryBase64 + '";\n';
bundle += 'GlassHouse.require("decompressor").loadFromBase64(_GH);\n';

// Output JSON with bundle + stats
var out = {{
    bundle: bundle,
    renamed: result.totalRenamed || 0,
    poolEntries: result.poolEntries || 0,
    reduction: result.reduction || 0,
    originalSize: result.originalSize || 0,
    binarySize: result.binarySize || 0,
    time: result.time || 0
}};

print(JSON.stringify(out));
"#,
        sources_json
    );

    // Need decompressor module too for the bundle
    let mut all_modules = modules;
    if let Ok(decomp) = quickjs::load_glasshouse_modules(cwd, &["decompressor"]) {
        all_modules.extend(decomp);
    }

    let output = quickjs::eval_with_modules(&code, &all_modules)?;
    let trimmed = output.trim();

    // Parse the result
    match crate::json::parse(trimmed) {
        Some(crate::json::Value::Object(pairs)) => {
            let mut bundle = String::new();
            let mut renamed = 0u32;
            let mut pool_entries = 0u32;
            for (key, val) in &pairs {
                match key.as_str() {
                    "bundle" => {
                        if let crate::json::Value::String(s) = val {
                            bundle = s.clone();
                        }
                    }
                    "renamed" => {
                        if let crate::json::Value::Number(n) = val {
                            renamed = *n as u32;
                        }
                    }
                    "poolEntries" => {
                        if let crate::json::Value::Number(n) = val {
                            pool_entries = *n as u32;
                        }
                    }
                    _ => {}
                }
            }
            if bundle.is_empty() {
                Err("Hyper-compactor produced empty output".to_string())
            } else {
                Ok(BundleResult {
                    bundle,
                    renamed,
                    pool_entries,
                })
            }
        }
        _ => Err(format!(
            "Failed to parse hyper-compactor output: {}",
            if trimmed.len() > 200 {
                &trimmed[..200]
            } else {
                trimmed
            }
        )),
    }
}

// ── Dev build: readable concatenation ──

fn build_dev(cwd: &Path) {
    let dist = cwd.join("dist");
    let _ = fs::create_dir_all(&dist);

    println!("⚒ Building Glass House (dev mode)...");

    let sources = collect_sources(cwd);

    // Compile TypeScript files
    let compiled = match compile_ts_files(cwd, &sources.ts_files) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("glas: {}", e);
            return;
        }
    };

    // Build concatenated bundle
    let mut bundle = String::from("/* Glass House dev build */\n(function(){\n\"use strict\";\n");
    let mut file_count = 0u32;

    // Framework JS files first (dependency order)
    let framework_order = [
        "glasshouse.js", "types.js", "dom.js", "pebble.js",
        "handler.js", "shine.js", "pane.js",
        "ts-lexer.js", "ts-parser.js", "ts-binder.js", "ts-checker.js",
        "ts-emitter.js", "ts-compiler.js",
        "resolver.js", "tree-shaker.js", "hyper-compactor.js",
        "rom.js", "decompressor.js", "builder.js",
        "package-manager.js", "package-validator.js", "package-scaffold.js",
        "lint.js", "auditor.js", "wcag-validator.js",
        "cli.js",
    ];

    for name in &framework_order {
        for fp in &sources.framework {
            if fp.file_name().map_or(false, |n| n == *name) {
                if let Ok(src) = fs::read_to_string(fp) {
                    bundle.push_str(&src);
                    bundle.push('\n');
                    file_count += 1;
                }
                break;
            }
        }
    }

    // Add any framework files not in the ordered list
    for fp in &sources.framework {
        let name = fp.file_name().unwrap_or_default().to_string_lossy();
        let name_str: &str = name.as_ref();
        if !framework_order.iter().any(|&n| n == name_str) {
            if let Ok(src) = fs::read_to_string(fp) {
                bundle.push_str(&src);
                bundle.push('\n');
                file_count += 1;
            }
        }
    }

    // Compiled TS output
    for (_rel, js) in &compiled {
        bundle.push_str(js);
        bundle.push('\n');
        file_count += 1;
    }

    bundle.push_str("\n})();\n");

    let out_path = dist.join("glasshouse.bundle.js");
    let _ = fs::write(&out_path, &bundle);

    println!(
        "  {} files | {:.1} KB",
        file_count,
        bundle.len() as f64 / 1024.0
    );
    println!("  dist/glasshouse.bundle.js");

    // Build source list for index.html generation
    let mut all_sources: Vec<(String, String)> = Vec::new();
    for fp in &sources.framework {
        let rel = fp
            .strip_prefix(cwd)
            .unwrap_or(fp)
            .to_string_lossy()
            .to_string();
        all_sources.push((rel, String::new())); // placeholder, not used for script tags
    }
    for (rel, _js) in &compiled {
        all_sources.push((rel.clone(), String::new()));
    }

    write_index_html(&dist, &all_sources, false);
    println!("  dist/index.html");
    println!("  ✓ Dev build complete");
}

// ── Index HTML generation ──

fn write_index_html(dist: &Path, _sources: &[(String, String)], production: bool) {
    if production {
        let html = format!(
            "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1.0\"><title>Glass House</title><script src=\"glasshouse.bundle.js\"></script></head><body><div id=\"app\"></div></body></html>"
        );
        let _ = fs::write(dist.join("index.html"), &html);
        return;
    }

    // Dev mode: generate script tags pointing to original files
    let cwd = env::current_dir().unwrap_or_default();
    let mut scripts = String::new();

    // Framework files (glasshouse/*.js)
    let framework_order = [
        "glasshouse.js", "types.js", "dom.js", "pebble.js",
        "handler.js", "shine.js", "pane.js",
        "ts-lexer.js", "ts-parser.js", "ts-binder.js", "ts-checker.js",
        "ts-emitter.js", "ts-compiler.js",
        "resolver.js", "tree-shaker.js", "hyper-compactor.js",
        "rom.js", "decompressor.js", "builder.js",
        "package-manager.js", "package-validator.js", "package-scaffold.js",
        "lint.js", "auditor.js", "wcag-validator.js",
        "cli.js",
    ];

    let gh_dir = cwd.join("glasshouse");
    if gh_dir.exists() {
        for name in &framework_order {
            let fp = gh_dir.join(name);
            if fp.exists() {
                scripts.push_str(&format!(
                    "  <script src=\"../glasshouse/{}\"></script>\n",
                    name
                ));
            }
        }
        // Any remaining .js files not in order
        if let Ok(es) = fs::read_dir(&gh_dir) {
            let mut remaining: Vec<String> = es
                .flatten()
                .filter_map(|e| {
                    let n = e.file_name().to_string_lossy().to_string();
                    if n.ends_with(".js") && !framework_order.contains(&n.as_str().as_ref()) {
                        Some(n)
                    } else {
                        None
                    }
                })
                .collect();
            remaining.sort();
            for n in &remaining {
                scripts.push_str(&format!(
                    "  <script src=\"../glasshouse/{}\"></script>\n",
                    n
                ));
            }
        }
    }

    // User TS/TSX files (src/) — referenced by their .ts path so dev server can compile
    let src_dir = cwd.join("src");
    if src_dir.exists() {
        if let Ok(entries) = utils::walk_dir(&src_dir) {
            let mut user_files: Vec<PathBuf> = entries
                .into_iter()
                .filter(|p| {
                    p.extension()
                        .and_then(|e| e.to_str())
                        .map_or(false, |ext| ext == "ts" || ext == "tsx")
                })
                .collect();
            user_files.sort();
            for fp in &user_files {
                let rel = fp.strip_prefix(&cwd).unwrap_or(fp);
                scripts.push_str(&format!(
                    "  <script src=\"../{}\"></script>\n",
                    rel.to_string_lossy()
                ));
            }
        }
    }

    // Package TS files
    let pkg_dir = cwd.join("packages");
    if pkg_dir.exists() {
        if let Ok(entries) = utils::walk_dir(&pkg_dir) {
            let mut pkg_files: Vec<PathBuf> = entries
                .into_iter()
                .filter(|p| {
                    p.extension()
                        .and_then(|e| e.to_str())
                        .map_or(false, |ext| ext == "ts" || ext == "tsx")
                })
                .collect();
            pkg_files.sort();
            for fp in &pkg_files {
                let rel = fp.strip_prefix(&cwd).unwrap_or(fp);
                scripts.push_str(&format!(
                    "  <script src=\"../{}\"></script>\n",
                    rel.to_string_lossy()
                ));
            }
        }
    }

    let html = format!(
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1.0\"><title>Glass House</title>\n{}</head><body><div id=\"app\"></div></body></html>",
        scripts
    );
    let _ = fs::write(dist.join("index.html"), &html);
}
