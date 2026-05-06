// cli/commands/lint.rs
// Lint command — runs GlassHouse lint checks on project source files via QuickJS.
//
// Flags:
//   --realtime / -r   Watch mode — re-lint on file changes every 2 seconds
//   --fix / -f        Auto-fix where possible (add 'use strict', etc.)
//   --json            Output as JSON for CI
//   --strict          Treat warnings as errors

use std::env;
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use crate::quickjs;
use crate::utils;

/// LintFlags holds the parsed command-line flag state.
pub struct LintFlags {
    pub realtime: bool,
    pub fix: bool,
    pub json: bool,
    pub strict: bool,
}

impl Default for LintFlags {
    fn default() -> Self {
        LintFlags {
            realtime: false,
            fix: false,
            json: false,
            strict: false,
        }
    }
}

/// Parse lint-specific flags from the args slice.
pub fn parse_lint_flags(args: &[String]) -> LintFlags {
    LintFlags {
        realtime: args.iter().any(|a| a == "--realtime" || a == "-r"),
        fix: args.iter().any(|a| a == "--fix" || a == "-f"),
        json: args.iter().any(|a| a == "--json"),
        strict: args.iter().any(|a| a == "--strict"),
    }
}

/// Main lint entry point — called from main.rs dispatch.
pub fn run(_args: &[String], flags: &LintFlags) {
    let cwd = env::current_dir().unwrap_or_default();

    if !cwd.join("glasshouse").exists() {
        eprintln!("glas: not in a Glass House project");
        return;
    }

    if flags.realtime {
        run_watch(&cwd, flags);
    } else {
        run_once(&cwd, flags);
    }
}

/// Single-run lint: collect files, run lint, print results.
fn run_once(cwd: &std::path::Path, flags: &LintFlags) {
    let result = lint_project(cwd, flags);
    print_lint_result(&result, flags);
}

/// Watch mode: re-lint every 2 seconds on file changes.
fn run_watch(cwd: &std::path::Path, flags: &LintFlags) {
    println!("● Lint watch mode — re-linting on changes...");
    println!("  Press Ctrl+C to stop");
    println!();

    let mut _last_result = LintResult::default();
    // Initial lint
    let result = lint_project(cwd, flags);
    print_lint_result(&result, flags);
    _last_result = result;

    // Track file modification times
    let mut last_mtimes: Vec<(PathBuf, u64)> = Vec::new();

    loop {
        thread::sleep(Duration::from_secs(2));

        // Check if any watched files changed
        let current_files = collect_lint_files(cwd);
        let mut changed = false;

        for fp in &current_files {
            if let Ok(m) = fs::metadata(fp) {
                let s = m.len();
                let _modified = m
                    .modified()
                    .map(|t| {
                        t.duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs()
                    })
                    .unwrap_or(0);
                match last_mtimes.iter().find(|(p, _)| p == fp) {
                    Some((_, old_s)) if *old_s != s => changed = true,
                    None => changed = true,
                    _ => {}
                }
            }
        }

        if changed {
            last_mtimes = current_files
                .iter()
                .filter_map(|p| {
                    fs::metadata(p).ok().map(|m| {
                        let _mt = m
                            .modified()
                            .map(|t| {
                                t.duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs()
                            })
                            .unwrap_or(0);
                        (p.clone(), m.len())
                    })
                })
                .collect();

            println!("\n  ⟳ Re-linting...\n");
            let result = lint_project(cwd, flags);
            print_lint_result(&result, flags);
            _last_result = result;
        }
    }
}

/// Collect all files that should be linted: .ts, .tsx files from src/ and packages/.
fn collect_lint_files(cwd: &std::path::Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    for dir_name in &["src", "packages"] {
        let dir_path = cwd.join(dir_name);
        if !dir_path.exists() {
            continue;
        }
        if let Ok(entries) = utils::walk_dir(&dir_path) {
            for p in entries {
                if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                    if ext == "ts" || ext == "tsx" {
                        files.push(p);
                    }
                }
            }
        }
    }

    files.sort();
    files
}

/// Check for forbidden .js files in src/ and packages/.
/// Returns a list of violation entries for each .js file found.
fn check_forbidden_js(cwd: &std::path::Path) -> Vec<LintViolation> {
    let mut violations = Vec::new();

    for dir_name in &["src", "packages"] {
        let dir_path = cwd.join(dir_name);
        if !dir_path.exists() {
            continue;
        }
        if let Ok(entries) = utils::walk_dir(&dir_path) {
            for p in entries {
                if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                    if ext == "js" {
                        // Check if it's inside a package's compiled output or test fixture
                        let _path_str = p.to_string_lossy();
                        // Skip .registry.json and similar meta files
                        if p.file_name()
                            .map_or(false, |n| n == ".registry.json")
                        {
                            continue;
                        }
                        violations.push(LintViolation {
                            file: p
                                .strip_prefix(cwd)
                                .unwrap_or(&p)
                                .to_string_lossy()
                                .to_string(),
                            line: 0,
                            col: 0,
                            severity: "error".to_string(),
                            rule: "NO_JS_FILES".to_string(),
                            message: "JavaScript files not allowed in user codebase. Use TypeScript (.ts/.tsx)."
                                .to_string(),
                        });
                    }
                }
            }
        }
    }

    violations
}

/// Run the actual lint checks by invoking QuickJS with a lint wrapper.
fn lint_project(cwd: &std::path::Path, flags: &LintFlags) -> LintResult {
    let mut result = LintResult::default();

    // 1. Check for forbidden .js files (this is a hard check, no JS needed)
    let js_violations = check_forbidden_js(cwd);
    if !js_violations.is_empty() {
        for v in &js_violations {
            if v.severity == "error" {
                result.errors += 1;
            } else {
                result.warnings += 1;
            }
        }
        result.violations.extend(js_violations);
        // Don't continue — we report these and still lint .ts files
    }

    // 2. Collect .ts/.tsx files to lint
    let files = collect_lint_files(cwd);
    if files.is_empty() {
        return result;
    }

    // 3. Build the JS lint wrapper and run via QuickJS
    let lint_js = build_lint_js_wrapper(cwd, &files, flags);
    match quickjs::eval_with_modules(&lint_js.code, &lint_js.modules) {
        Ok(output) => {
            if let Some(parsed) = parse_lint_json(&output) {
                for v in parsed {
                    if v.severity == "error" {
                        result.errors += 1;
                    } else {
                        result.warnings += 1;
                    }
                    result.violations.push(v);
                }
            }
        }
        Err(e) => {
            result.violations.push(LintViolation {
                file: "(lint runner)".to_string(),
                line: 0,
                col: 0,
                severity: "error".to_string(),
                rule: "RUNNER".to_string(),
                message: format!("Lint runner failed: {}", e),
            });
            result.errors += 1;
        }
    }

    result
}

/// Build the JS wrapper code and load required modules for linting.
struct LintJsWrapper {
    modules: Vec<(String, String)>,
    code: String,
}

fn build_lint_js_wrapper(
    cwd: &std::path::Path,
    files: &[PathBuf],
    flags: &LintFlags,
) -> LintJsWrapper {
    // Load required glasshouse modules
    let module_names = &["glasshouse", "lint"];
    let modules = quickjs::load_glasshouse_modules(cwd, module_names).unwrap_or_default();

    // Build a JSON array of { file, source } objects
    let mut sources_json = String::from("[");
    for (i, fp) in files.iter().enumerate() {
        if i > 0 {
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
    }
    sources_json.push(']');

    // The lint runner code
    let strict_flag = if flags.strict { "true" } else { "false" };
    let code = format!(
        r#"
// Lint runner for GlassHouse CLI
var sources = {};
var strictMode = {};

// Lint each source file
var allViolations = [];

function lintSource(source, filename) {{
    var violations = [];
    var s = source;

    // Strict mode check
    if (s.indexOf("'use strict'") === -1 && s.indexOf('"use strict"') === -1) {{
        violations.push({{
            file: filename, line: 0, col: 0,
            severity: 'warning',
            rule: 'STRICT_MODE',
            message: 'Missing "use strict" directive'
        }});
    }}

    // Security: eval()
    if (/\beval\s*\(/.test(s)) {{
        violations.push({{
            file: filename, line: 0, col: 0,
            severity: 'error',
            rule: 'SECURITY',
            message: 'Uses forbidden eval()'
        }});
    }}

    // Security: new Function()
    if (/\bnew\s+Function\s*\(/.test(s)) {{
        violations.push({{
            file: filename, line: 0, col: 0,
            severity: 'error',
            rule: 'SECURITY',
            message: 'Uses forbidden new Function()'
        }});
    }}

    // Security: document.write()
    if (/document\.write\s*\(/.test(s)) {{
        violations.push({{
            file: filename, line: 0, col: 0,
            severity: 'error',
            rule: 'SECURITY',
            message: 'Uses forbidden document.write()'
        }});
    }}

    // Security: innerHTML assignment
    if (/innerHTML\s*=/.test(s) && !/\/\/\s*ok/.test(s)) {{
        violations.push({{
            file: filename, line: 0, col: 0,
            severity: 'warning',
            rule: 'SECURITY',
            message: 'Direct innerHTML assignment — use dom.safe() or textContent'
        }});
    }}

    // Security: setTimeout/setInterval with string
    if (/setTimeout\s*\(\s*['"][\s\S]*?['"]/.test(s) ||
        /setInterval\s*\(\s*['"][\s\S]*?['"]/.test(s)) {{
        violations.push({{
            file: filename, line: 0, col: 0,
            severity: 'warning',
            rule: 'SECURITY',
            message: 'setTimeout/setInterval with string argument — use function reference'
        }});
    }}

    // Size check: file over 40KB
    if (s.length > 40960) {{
        violations.push({{
            file: filename, line: 0, col: 0,
            severity: 'error',
            rule: 'MAX_SIZE',
            message: 'File size ' + s.length + 'B exceeds 40KB limit'
        }});
    }}

    // Type safety: Pebble/Shine without propTypes
    if (/\bnew\s+Pebble\s*\(/.test(s) || /\bPebble\s*\(\s*\{{/.test(s) ||
        /\bnew\s+Shine\s*\(/.test(s) || /\bShine\s*\(\s*\{{/.test(s)) {{
        if (!/\bpropTypes\b/.test(s)) {{
            violations.push({{
                file: filename, line: 0, col: 0,
                severity: 'error',
                rule: 'TYPE_SAFETY',
                message: 'Pebble/Shine must declare propTypes'
            }});
        }}
        if (/\b\.setState\b/.test(s) && !/\bstateTypes\b/.test(s)) {{
            violations.push({{
                file: filename, line: 0, col: 0,
                severity: 'warning',
                rule: 'TYPE_SAFETY',
                message: 'Uses setState without declaring stateTypes'
            }});
        }}
    }}

    // Handler logic in Pebbles
    if (/\bnew\s+Pebble\s*\(/.test(s) || /\bPebble\s*\(\s*\{{/.test(s)) {{
        if (/\bfetch\s*\(/.test(s)) {{
            violations.push({{
                file: filename, line: 0, col: 0,
                severity: 'error',
                rule: 'HANDLER_LOGIC',
                message: 'fetch() in Pebble — extract to a Handler via this.use()'
            }});
        }}
        if (/\bXMLHttpRequest\b/.test(s)) {{
            violations.push({{
                file: filename, line: 0, col: 0,
                severity: 'error',
                rule: 'HANDLER_LOGIC',
                message: 'XMLHttpRequest in Pebble — extract to a Handler'
            }});
        }}
        if (/\blocalStorage\s*\./.test(s) || /\bsessionStorage\s*\./.test(s)) {{
            violations.push({{
                file: filename, line: 0, col: 0,
                severity: 'error',
                rule: 'HANDLER_LOGIC',
                message: 'Storage access in Pebble — extract to a Handler'
            }});
        }}
        if (/\bdocument\.cookie\b/.test(s)) {{
            violations.push({{
                file: filename, line: 0, col: 0,
                severity: 'error',
                rule: 'HANDLER_LOGIC',
                message: 'document.cookie in Pebble — extract to a Handler'
            }});
        }}
        if (/\bthis\.use\s*\(/.test(s) && !/\bhandlers\s*\:/.test(s) && !/\bhandlers\s*=/.test(s)) {{
            violations.push({{
                file: filename, line: 0, col: 0,
                severity: 'error',
                rule: 'HANDLER_UNDECLARED',
                message: 'Uses this.use() without declaring handlers array'
            }});
        }}
    }}

    // TS-specific: use of 'any' type
    if (/:\s*any\b/.test(s)) {{
        violations.push({{
            file: filename, line: 0, col: 0,
            severity: 'warning',
            rule: 'TYPE_ANY',
            message: 'Use of "any" type — prefer explicit typing'
        }});
    }}

    return violations;
}}

for (var i = 0; i < sources.length; i++) {{
    var v = lintSource(sources[i].source, sources[i].file);
    for (var j = 0; j < v.length; j++) {{
        allViolations.push(v[j]);
    }}
}}

// Output results as JSON to stdout
var out = JSON.stringify(allViolations);
print(out);
"#,
        sources_json, strict_flag
    );

    LintJsWrapper { modules, code }
}

/// Parse the JSON output from the lint runner into a Vec of LintViolations.
fn parse_lint_json(output: &str) -> Option<Vec<LintViolation>> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return Some(Vec::new());
    }

    // Use the hand-written JSON parser
    match crate::json::parse(trimmed) {
        Some(crate::json::Value::Array(items)) => {
            let mut violations = Vec::new();
            for item in &items {
                if let crate::json::Value::Object(pairs) = item {
                    let mut v = LintViolation::default();
                    for (key, val) in pairs {
                        match (key.as_str(), val) {
                            ("file", crate::json::Value::String(s)) => v.file = s.clone(),
                            ("line", crate::json::Value::Number(n)) => v.line = *n as u32,
                            ("col", crate::json::Value::Number(n)) => v.col = *n as u32,
                            ("severity", crate::json::Value::String(s)) => v.severity = s.clone(),
                            ("rule", crate::json::Value::String(s)) => v.rule = s.clone(),
                            ("message", crate::json::Value::String(s)) => v.message = s.clone(),
                            _ => {}
                        }
                    }
                    violations.push(v);
                }
            }
            Some(violations)
        }
        _ => None,
    }
}

/// Print lint results with color-coded output.
fn print_lint_result(result: &LintResult, flags: &LintFlags) {
    if flags.json {
        print_json_result(result);
        return;
    }

    if result.violations.is_empty() {
        println!("✓ No issues found.");
        return;
    }

    let total = result.errors + result.warnings;
    println!("⚑ Lint: {} issue(s) found ({} errors, {} warnings)", total, result.errors, result.warnings);
    println!();

    // Group violations by file
    let mut by_file: std::collections::HashMap<String, Vec<&LintViolation>> =
        std::collections::HashMap::new();
    for v in &result.violations {
        by_file
            .entry(v.file.clone())
            .or_insert_with(Vec::new)
            .push(v);
    }

    let mut file_names: Vec<String> = by_file.keys().cloned().collect();
    file_names.sort();

    for file_name in &file_names {
        let violations = by_file.get(file_name).unwrap();
        println!("  {}", file_name);
        for v in violations {
            let symbol = if v.severity == "error" { "✗" } else { "⚠" };
            let loc = if v.line > 0 {
                format!("{}:{}", v.line, v.col)
            } else {
                "-".to_string()
            };
            println!(
                "    {} {} [{}] {}",
                symbol, loc, v.rule, v.message
            );
        }
        println!();
    }

    let exit_code = if flags.strict {
        result.errors + result.warnings
    } else {
        result.errors
    };
    if exit_code > 0 {
        println!(
            "  Lint {} ({} error{}, {} warning{})",
            if exit_code > 0 { "FAILED" } else { "PASSED" },
            result.errors,
            if result.errors == 1 { "" } else { "s" },
            result.warnings,
            if result.warnings == 1 { "" } else { "s" }
        );
    }
}

/// Print results as a JSON array for CI consumption.
fn print_json_result(result: &LintResult) {
    let mut items = Vec::new();
    for v in &result.violations {
        items.push(format!(
            r#"{{"file":"{}","line":{},"col":{},"severity":"{}","rule":"{}","message":"{}"}}"#,
            v.file.replace('\\', "\\\\").replace('"', "\\\""),
            v.line,
            v.col,
            v.severity,
            v.rule,
            v.message.replace('\\', "\\\\").replace('"', "\\\"")
        ));
    }
    println!("[{}]", items.join(","));
}

// ── Data types ──

#[derive(Debug, Clone, Default)]
struct LintViolation {
    file: String,
    line: u32,
    col: u32,
    severity: String,
    rule: String,
    message: String,
}

#[derive(Debug, Default)]
struct LintResult {
    violations: Vec<LintViolation>,
    errors: u32,
    warnings: u32,
}
