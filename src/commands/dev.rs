// cli/commands/dev.rs
// Dev server — serves project files with hot reload, TypeScript compilation on-the-fly,
// and optional lint on file changes.
//
// Flags:
//   --port <N>   Set server port (default: 3000)
//   --open       Open browser on start
//   --lint       Run lint on each file change during dev

use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::quickjs;
use crate::server;
use crate::utils::{self, get_last_change, mark_changed};

/// Shared state for the dev server.
struct DevState {
    /// Map of file path → compiled JS (for .ts/.tsx files)
    compiled_cache: HashMap<PathBuf, String>,
    /// List of compilation/lint errors to report via ping endpoint
    errors: Vec<String>,
    /// TS compiler modules loaded once
    ts_modules: Option<Vec<(String, String)>>,
}

impl DevState {
    fn new() -> Self {
        DevState {
            compiled_cache: HashMap::new(),
            errors: Vec::new(),
            ts_modules: None,
        }
    }
}

pub fn dev(port: u16, open_browser: bool, lint: bool) {
    let cwd = env::current_dir().unwrap_or_default();

    if !cwd.join("glasshouse").exists() {
        eprintln!("glas: not in a Glass House project");
        return;
    }

    let addr = format!("0.0.0.0:{}", port);
    let listener = match TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("glas: {}", e);
            return;
        }
    };

    mark_changed();

    // Shared mutable state
    let state = Arc::new(Mutex::new(DevState::new()));

    // Pre-load TS compiler modules once
    {
        let ts_module_names = &[
            "glasshouse", "ts-lexer", "ts-parser", "ts-binder",
            "ts-checker", "ts-emitter", "ts-compiler",
        ];
        if let Ok(modules) = quickjs::load_glasshouse_modules(&cwd, ts_module_names) {
            if let Ok(mut s) = state.lock() {
                s.ts_modules = Some(modules);
            }
        }
    }

    // Watch for file changes
    let cwd_watcher = cwd.clone();
    let state_watcher = Arc::clone(&state);
    let lint_enabled = lint;
    thread::spawn(move || {
        let mut last_sizes: HashMap<PathBuf, u64> = HashMap::new();
        loop {
            thread::sleep(Duration::from_secs(2));

            let mut changed = false;
            let watch_dirs = ["glasshouse", "src", "packages"];

            for dir_name in &watch_dirs {
                let dir_path = cwd_watcher.join(dir_name);
                if !dir_path.exists() {
                    continue;
                }
                if let Ok(entries) = utils::walk_dir(&dir_path) {
                    for p in &entries {
                        // Skip non-source files
                        if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                            if ext != "js" && ext != "ts" && ext != "tsx" && ext != "css" && ext != "html" {
                                continue;
                            }
                        } else {
                            continue;
                        }

                        if let Ok(m) = fs::metadata(p) {
                            let current_size = m.len();
                            match last_sizes.get(p) {
                                Some(old_size) if *old_size != current_size => {
                                    changed = true;
                                    // Invalidate cache for this file
                                    if let Ok(mut s) = state_watcher.lock() {
                                        s.compiled_cache.remove(p);
                                    }
                                }
                                None => {
                                    changed = true;
                                }
                                _ => {}
                            }
                            last_sizes.insert(p.clone(), current_size);
                        }
                    }
                }
            }

            if changed {
                mark_changed();

                // Re-compile TS files if they changed
                if let Ok(mut s) = state_watcher.lock() {
                    s.errors.clear();
                }

                // Run lint if enabled
                if lint_enabled {
                    let flags = crate::commands::LintFlags {
                        realtime: false,
                        fix: false,
                        json: true,
                        strict: false,
                    };
                    // We run lint synchronously to collect errors for the ping endpoint
                    run_dev_lint_cycle(&cwd_watcher, &state_watcher, &flags);
                }
            }
        }
    });

    // Generate dev index
    let index = generate_dev_index_ts(&cwd);

    let url = format!("http://localhost:{}", port);
    println!("● Dev server at {}", url);
    if lint {
        println!("  Lint on changes: enabled");
    }
    println!("  Watching for changes... (hot reload + TS compilation)");
    println!("  Press Ctrl+C to stop");

    if open_browser {
        #[cfg(windows)]
        {
            let _ = std::process::Command::new("cmd")
                .args(["/C", "start", &url])
                .spawn();
        }
        #[cfg(not(windows))]
        {
            let _ = std::process::Command::new("open").arg(&url).spawn();
        }
    }

    for stream in listener.incoming() {
        match stream {
            Ok(mut s) => {
                let ix = index.clone();
                let c = cwd.clone();
                let st = Arc::clone(&state);
                thread::spawn(move || {
                    handle_dev(&mut s, &c, &ix, &st);
                });
            }
            Err(_) => {}
        }
    }
}

/// Run a lint cycle and store errors for the ping endpoint.
fn run_dev_lint_cycle(cwd: &std::path::Path, state: &Arc<Mutex<DevState>>, _flags: &crate::commands::LintFlags) {
    // Quick lint: check for forbidden .js files and basic patterns
    let mut errors: Vec<String> = Vec::new();

    // Check for .js files in src/ and packages/
    for dir_name in &["src", "packages"] {
        let dir_path = cwd.join(dir_name);
        if !dir_path.exists() {
            continue;
        }
        if let Ok(entries) = utils::walk_dir(&dir_path) {
            for p in entries {
                if p.extension().and_then(|e| e.to_str()) == Some("js") {
                    if p.file_name().map_or(false, |n| n == ".registry.json") {
                        continue;
                    }
                    let rel = p.strip_prefix(cwd).unwrap_or(&p).to_string_lossy().to_string();
                    errors.push(format!(
                        "✗ {}: JavaScript files not allowed in user codebase. Use TypeScript (.ts/.tsx).",
                        rel
                    ));
                }
            }
        }
    }

    if let Ok(mut s) = state.lock() {
        s.errors = errors;
    }
}

/// Handle a single dev server connection.
fn handle_dev(
    stream: &mut TcpStream,
    cwd: &std::path::Path,
    index: &str,
    state: &Arc<Mutex<DevState>>,
) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut rl = String::new();
    if reader.read_line(&mut rl).is_err() {
        return;
    }

    let parts: Vec<&str> = rl.trim().split_whitespace().collect();
    if parts.len() < 2 {
        server::send_404(stream);
        return;
    }

    let method = parts[0];
    let rp = parts[1];

    // ── /__glas_ping — hot reload heartbeat + errors ──
    if rp == "/__glas_ping" && method == "GET" {
        let ts = get_last_change().to_string();

        // Collect any errors for the browser
        let error_json = {
            if let Ok(s) = state.lock() {
                if s.errors.is_empty() {
                    "[]".to_string()
                } else {
                    let mut items = Vec::new();
                    for e in &s.errors {
                        let escaped = e.replace('\\', "\\\\").replace('"', "\\\"");
                        items.push(format!("\"{}\"", escaped));
                    }
                    format!("[{}]", items.join(","))
                }
            } else {
                "[]".to_string()
            }
        };

        let body = format!(
            r#"{{"timestamp":"{}","errors":{}}}"#,
            ts, error_json
        );
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(resp.as_bytes());
        return;
    }

    // ── / or /index.html ──
    if rp == "/" || rp == "/index.html" {
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            index.len(),
            index
        );
        let _ = stream.write_all(resp.as_bytes());
        return;
    }

    // ── Serve a file — with TS compilation on-the-fly ──
    let path = rp.trim_start_matches('/');
    let file_path = cwd.join(path);

    // Reject .js files in src/ and packages/ (TS/TSX redirect)
    if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
        if ext == "js" {
            // Check if the file is in src/ or packages/
            let path_str = file_path.to_string_lossy();
            let src_prefix = cwd.join("src").to_string_lossy().to_string();
            let pkg_prefix = cwd.join("packages").to_string_lossy().to_string();

            if path_str.starts_with(&src_prefix) || path_str.starts_with(&pkg_prefix) {
                // Try .ts redirect first
                let ts_path = file_path.with_extension("ts");
                if ts_path.exists() {
                    serve_compiled_ts(stream, cwd, &ts_path, state);
                    return;
                }
                // Try .tsx redirect
                let tsx_path = file_path.with_extension("tsx");
                if tsx_path.exists() {
                    serve_compiled_ts(stream, cwd, &tsx_path, state);
                    return;
                }
                // Neither .ts nor .tsx exists — 403
                let body = format!(
                    "JavaScript files not allowed in user codebase. Use TypeScript (.ts/.tsx).\n\nFile: {}\n\nConvert this file to .ts or .tsx.",
                    path
                );
                let resp = format!(
                    "HTTP/1.1 403 Forbidden\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(resp.as_bytes());
                return;
            }
        }
    }

    // TS/TSX compilation on-the-fly
    if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
        if ext == "ts" || ext == "tsx" {
            serve_compiled_ts(stream, cwd, &file_path, state);
            return;
        }
    }

    // All other files: serve as-is
    server::serve_file(stream, cwd, rp);
}

/// Compile a .ts/.tsx file on-the-fly and serve the resulting JS.
fn serve_compiled_ts(
    stream: &mut TcpStream,
    cwd: &std::path::Path,
    file_path: &std::path::Path,
    state: &Arc<Mutex<DevState>>,
) {
    // Check cache first
    {
        if let Ok(s) = state.lock() {
            if let Some(cached) = s.compiled_cache.get(file_path) {
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/javascript; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    cached.len(),
                    cached
                );
                let _ = stream.write_all(resp.as_bytes());
                return;
            }
        }
    }

    // Read source
    let source = match fs::read_to_string(file_path) {
        Ok(s) => s,
        Err(_) => {
            server::send_404(stream);
            return;
        }
    };

    // Compile via QuickJS
    let compiled_js = compile_ts_on_the_fly(cwd, file_path, &source, state);

    match compiled_js {
        Ok(js) => {
            // Cache the result
            if let Ok(mut s) = state.lock() {
                s.compiled_cache.insert(file_path.to_path_buf(), js.clone());
            }

            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/javascript; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                js.len(),
                js
            );
            let _ = stream.write_all(resp.as_bytes());
        }
        Err(err_msg) => {
            // Return the error as JS that logs to console
            let error_js = format!(
                "console.error('GlassHouse TS Compile Error in {}:');\nconsole.error({});\n",
                file_path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .replace('\\', "\\\\")
                    .replace('\'', "\\'"),
                escape_js_string(&err_msg)
            );
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/javascript; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                error_js.len(),
                error_js
            );
            let _ = stream.write_all(resp.as_bytes());

            // Store the error for the ping endpoint
            if let Ok(mut s) = state.lock() {
                let rel = file_path.strip_prefix(cwd).unwrap_or(file_path).to_string_lossy().to_string();
                s.errors.push(format!("✗ {}: {}", rel, err_msg));
            }
        }
    }
}

/// Escape a string for embedding in a JS string literal.
fn escape_js_string(s: &str) -> String {
    let mut out = String::from("'");
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('\'');
    out
}

/// Compile a single TypeScript file via QuickJS.
fn compile_ts_on_the_fly(
    cwd: &std::path::Path,
    _file_path: &std::path::Path,
    source: &str,
    state: &Arc<Mutex<DevState>>,
) -> Result<String, String> {
    // Get pre-loaded TS modules
    let modules = {
        if let Ok(s) = state.lock() {
            s.ts_modules.clone()
        } else {
            None
        }
    };

    let modules = match modules {
        Some(m) => m,
        None => {
            // Try loading now
            let ts_module_names = &[
                "glasshouse", "ts-lexer", "ts-parser", "ts-binder",
                "ts-checker", "ts-emitter", "ts-compiler",
            ];
            quickjs::load_glasshouse_modules(cwd, ts_module_names)?
        }
    };

    let escaped_source = source
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");

    let code = format!(
        r#"
var source = "{}";
var compiler = GlassHouse.require('ts-compiler');
var result = compiler.compile(source, {{}});

var out = {{
    success: result.success,
    js: result.js || '',
    diagnostics: result.diagnostics || []
}};

print(JSON.stringify(out));
"#,
        escaped_source
    );

    let output = quickjs::eval_with_modules(&code, &modules)?;
    let trimmed = output.trim();

    match crate::json::parse(trimmed) {
        Some(crate::json::Value::Object(pairs)) => {
            let mut js = String::new();
            let mut success = false;
            let mut diag_msgs = Vec::new();

            for (key, val) in &pairs {
                match key.as_str() {
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
                        if let crate::json::Value::Array(items) = val {
                            for item in items {
                                if let crate::json::Value::Object(dpairs) = item {
                                    let mut msg = String::new();
                                    for (dk, dv) in dpairs {
                                        if dk == "message" {
                                            if let crate::json::Value::String(s) = dv {
                                                msg = s.clone();
                                            }
                                        }
                                    }
                                    if !msg.is_empty() {
                                        diag_msgs.push(msg);
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            if !success || !diag_msgs.is_empty() {
                let err = diag_msgs.join("; ");
                if !js.is_empty() {
                    // Warnings only, JS was produced — return it with console.warn
                    let warn_js = format!(
                        "console.warn('TS warnings:');\n{}",
                        js
                    );
                    return Ok(warn_js);
                }
                return Err(err);
            }

            if js.is_empty() {
                return Err("TypeScript compilation produced empty output".to_string());
            }

            Ok(js)
        }
        _ => Err(format!(
            "Failed to parse TS compiler output: {}",
            if trimmed.len() > 200 {
                &trimmed[..200]
            } else {
                trimmed
            }
        )),
    }
}

/// Generate the dev index.html — includes framework JS, user TS/TSX, and hot-reload ping with error display.
fn generate_dev_index_ts(cwd: &std::path::Path) -> String {
    let mut scripts = String::new();

    // Framework JS files (glasshouse/*.js) — dependency order
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
                    "  <script src=\"/glasshouse/{}\"></script>\n",
                    name
                ));
            }
        }
        // Any remaining .js files
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
                    "  <script src=\"/glasshouse/{}\"></script>\n",
                    n
                ));
            }
        }
    }

    // User TS/TSX files — src/
    let src_dir = cwd.join("src");
    if src_dir.exists() {
        if let Ok(entries) = utils::walk_dir(&src_dir) {
            let mut src_files: Vec<PathBuf> = entries
                .into_iter()
                .filter(|p| {
                    p.extension()
                        .and_then(|e| e.to_str())
                        .map_or(false, |ext| ext == "ts" || ext == "tsx")
                })
                .collect();
            src_files.sort();
            for fp in &src_files {
                let rel = fp.strip_prefix(cwd).unwrap_or(fp);
                scripts.push_str(&format!(
                    "  <script src=\"/{}\"></script>\n",
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
                let rel = fp.strip_prefix(cwd).unwrap_or(fp);
                scripts.push_str(&format!(
                    "  <script src=\"/{}\"></script>\n",
                    rel.to_string_lossy()
                ));
            }
        }
    }

    // Hot-reload script with error display
    let reload_script = r#"
<script>
var _g_timestamp = 0;
var _g_errors = [];
function _g_ping() {
    fetch('/__glas_ping')
        .then(function(r) { return r.json(); })
        .then(function(data) {
            if (_g_timestamp && data.timestamp !== _g_timestamp) {
                location.reload();
            }
            _g_timestamp = data.timestamp;
            _g_errors = data.errors || [];
            if (_g_errors.length > 0) {
                // Show errors in a dev overlay
                _g_showErrors(_g_errors);
            } else {
                _g_hideErrors();
            }
            setTimeout(_g_ping, 2000);
        })
        .catch(function() {
            setTimeout(_g_ping, 2000);
        });
}
function _g_showErrors(errors) {
    var el = document.getElementById('_glas_errors');
    if (!el) {
        el = document.createElement('div');
        el.id = '_glas_errors';
        el.style.cssText = 'position:fixed;bottom:0;left:0;right:0;background:#dc2626;color:#fff;padding:12px 16px;font-family:monospace;font-size:12px;z-index:99999;max-height:200px;overflow-y:auto;';
        document.body.appendChild(el);
    }
    el.innerHTML = '<strong>GLAS ERRORS</strong><br>' +
        errors.map(function(e) { return e.replace(/</g,'&lt;').replace(/>/g,'&gt;'); }).join('<br>');
    el.style.display = 'block';
}
function _g_hideErrors() {
    var el = document.getElementById('_glas_errors');
    if (el) el.style.display = 'none';
}
_g_ping();
</script>
"#;

    format!(
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1.0\"><title>Glass House (dev)</title>\n{}{}</head><body><div id=\"app\"></div></body></html>",
        scripts, reload_script
    )
}
