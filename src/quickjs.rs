// cli/quickjs.rs
// Zero-dependency QuickJS runner — invokes qjs.exe as subprocess
// qjs.exe is compiled from https://bellard.org/quickjs/ quickjs.c
//
// All JS execution (lint, compile, test, hyper-compaction) goes through this module.
// Provides browser API shims so that GlassHouse modules (written for browsers)
// can operate in QuickJS's ES2023 environment.

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

/// Find the qjs executable relative to the glas binary location.
/// Checks: same dir as glas.exe, `quickjs/qjs.exe` subdir, `qjs` (no ext), then PATH.
fn find_qjs() -> PathBuf {
    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let candidates = [
                exe_dir.join("qjs.exe"),
                exe_dir.join("quickjs").join("qjs.exe"),
                exe_dir.join("qjs"),
            ];
            for c in &candidates {
                if c.exists() {
                    return c.clone();
                }
            }
        }
    }
    // Fallback: rely on PATH
    if cfg!(windows) {
        PathBuf::from("qjs.exe")
    } else {
        PathBuf::from("qjs")
    }
}

/// Generate a timestamp-based temp filename for a JS runner script.
fn temp_runner_path() -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    env::temp_dir().join(format!("glas_qjs_{}.js", ts))
}

/// Build a self-contained JavaScript runner that:
/// 1. Sets up browser API shims (performance, Blob, btoa, window, etc.)
/// 2. Provides a minimal GlassHouse global (extended by glasshouse.js)
/// 3. Loads each requested GlassHouse module source
/// 4. Executes the user code
/// 5. Calls `std.exit(0)` on completion
///
/// The runner uses QuickJS's `--std` flag so `std` module is available.
pub fn build_runner_script(modules: &[(String, String)], user_code: &str) -> String {
    let mut s = String::with_capacity(65536);

    // ── Preamble: QuickJS std import + browser API shims ──
    s.push_str(r###"
'use strict';
// std, os, and bjson are available as globals when --std is used

// --- Browser API Shims for QuickJS ---
var global = globalThis;

// window global (GlassHouse assigns to window.GlassHouse)
var window = globalThis;

// performance.now() — used by glasshouse.js and ts-compiler.js
var performance = {
    now: function() { return Date.now(); }
};

// Blob constructor — used by lint.js for size measurement
var Blob = function(arr, opts) {
    if (arr && arr.length > 0) {
        this._data = typeof arr[0] === 'string' ? arr[0] : String(arr[0]);
    } else {
        this._data = '';
    }
};
Object.defineProperty(Blob.prototype, 'size', {
    get: function() {
        return typeof this._data === 'string' ? this._data.length : 0;
    },
    enumerable: true,
    configurable: true
});

// btoa — used by hyper-compactor.js for base64 encoding
var btoa = function(str) {
    var chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
    var result = '';
    var bytes = [];
    for (var i = 0; i < str.length; i++) {
        var code = str.charCodeAt(i);
        if (code < 128) {
            bytes.push(code);
        } else if (code < 2048) {
            bytes.push(192 | (code >> 6));
            bytes.push(128 | (code & 63));
        } else {
            bytes.push(224 | (code >> 12));
            bytes.push(128 | ((code >> 6) & 63));
            bytes.push(128 | (code & 63));
        }
    }
    for (var i = 0; i < bytes.length; i += 3) {
        var b1 = bytes[i];
        var b2 = i + 1 < bytes.length ? bytes[i + 1] : -1;
        var b3 = i + 2 < bytes.length ? bytes[i + 2] : -1;
        result += chars.charAt(b1 >> 2);
        result += chars.charAt(((b1 & 3) << 4) | (b2 >= 0 ? (b2 >> 4) : 0));
        result += b2 >= 0 ? chars.charAt(((b2 & 15) << 2) | (b3 >= 0 ? (b3 >> 6) : 0)) : '=';
        result += b3 >= 0 ? chars.charAt(b3 & 63) : '=';
    }
    return result;
};

// TextEncoder — used by some modules
var TextEncoder = function() {};
TextEncoder.prototype.encode = function(str) {
    var arr = [];
    for (var i = 0; i < str.length; i++) {
        var c = str.charCodeAt(i);
        if (c < 128) {
            arr.push(c);
        } else if (c < 2048) {
            arr.push(192 | (c >> 6));
            arr.push(128 | (c & 63));
        } else {
            arr.push(224 | (c >> 12));
            arr.push(128 | ((c >> 6) & 63));
            arr.push(128 | (c & 63));
        }
    }
    return new Uint8Array(arr);
};

// setInterval / clearInterval — no-ops (we don't need stale collection in CLI)
var setInterval = function() { return 0; };
var clearInterval = function() {};

// _GlassHouse_init flag — the glasshouse.js IIFE will bootstrap GlassHouse
var _GlassHouse_init = true;

// Minimal GlassHouse stub (will be fully replaced by glasshouse.js IIFE)
// glasshouse.js does: window.GlassHouse = { ... }; Object.freeze(GlassHouse);
// We just need the global to exist so that define() calls in other files work.
// The glasshouse.js IIFE will mutate/overwrite this object.
var GlassHouse = {
    MAX_BLOCK_SIZE: 40960,
    MAX_TOTAL_SIZE: 262144,
    _blocks: Object.create(null),
    _loaded: Object.create(null),

    define: function(name, deps, factory) {
        this._blocks[name] = { deps: deps, factory: factory };
    },

    require: function(name) {
        if (this._loaded[name]) return this._loaded[name];
        var block = this._blocks[name];
        if (!block) throw new Error('Block not defined: ' + name);
        var resolved = [];
        for (var i = 0; i < block.deps.length; i++) {
            resolved.push(this.require(block.deps[i]));
        }
        var instance = block.factory.apply(null, resolved);
        this._loaded[name] = instance;
        return instance;
    },

    handler: function(name, config) {
        this._blocks[name] = { deps: config.deps || [], factory: config.factory || function() { return config; } };
    },

    pebble: function(name, config) {
        this._blocks[name] = { deps: [], factory: function() { return config; } };
        this._loaded[name] = config;
    },

    shine: function(name, config) {
        this._blocks[name] = { deps: [], factory: function() { return config; } };
        this._loaded[name] = config;
    },

    ready: function(cb) { if (typeof cb === 'function') cb(); },
    stats: {},
    listBlocks: function() { return Object.keys(this._blocks); }
};

"###);

    // ── Load GlassHouse module sources ──
    for (name, source) in modules {
        s.push_str(&format!("\n// ====== Module: {} ======\n", name));
        s.push_str(source);
        s.push('\n');
    }

    // ── User code ──
    s.push_str("\n// ====== User Code ======\n");
    s.push_str(user_code);
    s.push('\n');

    // ── Epilogue: exit cleanly ──
    s.push_str("\nstd.exit(0);\n");

    s
}

/// Run JavaScript code through QuickJS with no GlassHouse modules.
/// Returns stdout on success, error string on failure.
pub fn eval(code: &str) -> Result<String, String> {
    let modules: Vec<(String, String)> = Vec::new();
    eval_with_modules(code, &modules)
}

/// Run JavaScript code with specified GlassHouse modules loaded.
/// `modules` is a list of (module_name, source_code) pairs.
/// The modules are loaded in order before the user code executes.
/// Returns stdout on success, error string on failure.
pub fn eval_with_modules(code: &str, modules: &[(String, String)]) -> Result<String, String> {
    let runner = build_runner_script(modules, code);
    let tmp = temp_runner_path();

    fs::write(&tmp, &runner)
        .map_err(|e| format!("Failed to write temp script: {}", e))?;

    let qjs = find_qjs();
    let output = Command::new(&qjs)
        .arg("--std")
        .arg(tmp.to_str().unwrap_or("glas_qjs_temp.js"))
        .output()
        .map_err(|e| {
            let _ = fs::remove_file(&tmp);
            format!("Failed to execute qjs ({}): {}", qjs.display(), e)
        })?;

    // Clean up temp file
    let _ = fs::remove_file(&tmp);

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok(stdout)
    } else {
        let err_msg = if !stderr.is_empty() {
            stderr.trim().to_string()
        } else {
            stdout.trim().to_string()
        };
        Err(format!("QuickJS error: {}", err_msg))
    }
}

/// Run a JavaScript file directly through QuickJS.
/// The file should be self-contained (no module loading needed beyond what's in the file).
/// Returns stdout on success, error string on failure.
pub fn eval_file(path: &str) -> Result<String, String> {
    let qjs = find_qjs();

    let output = Command::new(&qjs)
        .arg("--std")
        .arg(path)
        .output()
        .map_err(|e| format!("Failed to execute qjs ({}): {}", qjs.display(), e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok(stdout)
    } else {
        let err_msg = if !stderr.is_empty() {
            stderr.trim().to_string()
        } else {
            stdout.trim().to_string()
        };
        Err(format!("QuickJS error: {}", err_msg))
    }
}

/// Run JavaScript code with stdin data piped in.
/// The code can read from stdin via `std.in.readAsString()` when using `--std`.
/// Returns stdout on success, error string on failure.
pub fn eval_with_input(code: &str, stdin_data: &str) -> Result<String, String> {
    let qjs = find_qjs();

    let mut child = Command::new(&qjs)
        .arg("--std")
        .arg("-e")
        .arg(code)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn qjs ({}): {}", qjs.display(), e))?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(stdin_data.as_bytes());
        // stdin is dropped here, closing the pipe
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Failed to wait on qjs: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok(stdout)
    } else {
        let err_msg = if !stderr.is_empty() {
            stderr.trim().to_string()
        } else {
            stdout.trim().to_string()
        };
        Err(format!("QuickJS error: {}", err_msg))
    }
}

/// Read GlassHouse module source files from `glasshouse/` directory in the project.
/// Returns a Vec of (module_name, source_code) in the requested order.
/// Each module is found by scanning glasshouse/ subdirectories for {name}.js.
pub fn load_glasshouse_modules(
    cwd: &Path,
    module_names: &[&str],
) -> Result<Vec<(String, String)>, String> {
    let glasshouse_dir = cwd.join("glasshouse");
    if !glasshouse_dir.exists() {
        return Err("glasshouse/ directory not found".to_string());
    }

    let mut modules = Vec::with_capacity(module_names.len());
    for name in module_names {
        let filename = format!("{}.js", name);
        let mut found = None;

        if let Ok(entries) = fs::read_dir(&glasshouse_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let candidate = path.join(&filename);
                    if candidate.exists() {
                        found = Some(candidate);
                        break;
                    }
                }
            }
        }

        match found {
            Some(file_path) => {
                let source = fs::read_to_string(&file_path).map_err(|e| {
                    format!("Failed to read {}: {}", file_path.display(), e)
                })?;
                modules.push((name.to_string(), source));
            }
            None => {
                return Err(format!("Module '{}' not found in glasshouse/", name));
            }
        }
    }

    Ok(modules)
}

/// Check if qjs executable exists and is runnable.
/// Returns Ok(()) if qjs can be found and executes, Err with details otherwise.
pub fn check_available() -> Result<(), String> {
    let qjs = find_qjs();
    match Command::new(&qjs).arg("--version").output() {
        Ok(o) => {
            if o.status.success() {
                Ok(())
            } else {
                Err(format!(
                    "qjs executable found but returned error ({}). Install QuickJS.",
                    qjs.display()
                ))
            }
        }
        Err(e) => Err(format!(
            "qjs executable not found ({}): {}. Install QuickJS from https://bellard.org/quickjs/",
            qjs.display(),
            e
        )),
    }
}
