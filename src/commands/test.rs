// cli/commands/test.rs
// Test command — runs GlassHouse test files through QuickJS.
//
// Options:
//   --filter <pattern>   Run only tests matching the pattern
//   --verbose / -v       Detailed output with test names and timings
//   --json               JSON output for CI
//   --watch              Re-run tests on file changes

use std::env;
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use crate::quickjs;
use crate::utils;

/// TestFlags holds the parsed command-line flag state.
pub struct TestFlags {
    pub filter: Option<String>,
    pub verbose: bool,
    pub json: bool,
    pub watch: bool,
}

impl Default for TestFlags {
    fn default() -> Self {
        TestFlags {
            filter: None,
            verbose: false,
            json: false,
            watch: false,
        }
    }
}

/// Parse test-specific flags from the args slice.
pub fn parse_test_flags(args: &[String]) -> TestFlags {
    let mut filter = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--filter" && i + 1 < args.len() {
            filter = Some(args[i + 1].clone());
            i += 1;
        }
        i += 1;
    }

    TestFlags {
        filter,
        verbose: args.iter().any(|a| a == "--verbose" || a == "-v"),
        json: args.iter().any(|a| a == "--json"),
        watch: args.iter().any(|a| a == "--watch"),
    }
}

/// Main test entry point — called from main.rs dispatch.
pub fn run(_args: &[String], flags: &TestFlags) {
    let cwd = env::current_dir().unwrap_or_default();

    if !cwd.join("glasshouse").exists() {
        eprintln!("glas: not in a Glass House project");
        return;
    }

    let tests_dir = cwd.join("tests");
    if !tests_dir.exists() {
        if flags.json {
            println!(r#"{{"pass":0,"fail":0,"skip":0,"tests":[]}}"#);
        } else {
            println!("No tests/ directory found. Create tests/*.js or tests/*.ts files.");
        }
        return;
    }

    if flags.watch {
        run_watch(&cwd, &tests_dir, flags);
    } else {
        run_once(&cwd, &tests_dir, flags);
    }
}

/// Single-run: collect test files, run them, report results.
fn run_once(cwd: &std::path::Path, tests_dir: &std::path::Path, flags: &TestFlags) {
    let test_files = collect_test_files(tests_dir);
    if test_files.is_empty() {
        if flags.json {
            println!(r#"{{"pass":0,"fail":0,"skip":0,"tests":[]}}"#);
        } else {
            println!("No test files found in tests/");
        }
        return;
    }

    let result = run_tests(cwd, &test_files, flags);
    print_test_results(&result, flags);
}

/// Watch mode: re-run tests every 2 seconds on file changes.
fn run_watch(cwd: &std::path::Path, tests_dir: &std::path::Path, flags: &TestFlags) {
    println!("● Test watch mode — re-running on changes...");
    println!("  Press Ctrl+C to stop");
    println!();

    let mut last_mtimes: Vec<(PathBuf, u64)> = Vec::new();

    // Initial run
    let test_files = collect_test_files(tests_dir);
    let result = run_tests(cwd, &test_files, flags);
    print_test_results(&result, flags);

    // Also watch src/ and glasshouse/ for changes
    loop {
        thread::sleep(Duration::from_secs(2));

        let all_files = collect_watch_files(cwd, tests_dir);
        let mut changed = false;

        for fp in &all_files {
            if let Ok(m) = fs::metadata(fp) {
                let s = m.len();
                match last_mtimes.iter().find(|(p, _)| p == fp) {
                    Some((_, old_s)) if *old_s != s => {
                        changed = true;
                        break;
                    }
                    None => {
                        changed = true;
                        break;
                    }
                    _ => {}
                }
            }
        }

        if changed {
            last_mtimes = all_files
                .iter()
                .filter_map(|p| {
                    fs::metadata(p).ok().map(|m| (p.clone(), m.len()))
                })
                .collect();

            println!("\n  ⟳ Re-running tests...\n");
            let test_files = collect_test_files(tests_dir);
            let result = run_tests(cwd, &test_files, flags);
            print_test_results(&result, flags);
        }
    }
}

/// Collect test files: .js and .ts files from tests/ directory.
fn collect_test_files(tests_dir: &std::path::Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = utils::walk_dir(tests_dir) {
        for p in entries {
            if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                if ext == "js" || ext == "ts" || ext == "tsx" {
                    files.push(p);
                }
            }
        }
    }
    files.sort();
    files
}

/// Collect all files to watch: tests/, src/, packages/, glasshouse/.
fn collect_watch_files(cwd: &std::path::Path, tests_dir: &std::path::Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for dir in &["tests", "src", "packages", "glasshouse"] {
        let dp = if *dir == "tests" {
            tests_dir.to_path_buf()
        } else {
            cwd.join(dir)
        };
        if dp.exists() {
            if let Ok(entries) = utils::walk_dir(&dp) {
                for p in entries {
                    if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                        if ext == "js" || ext == "ts" || ext == "tsx" {
                            files.push(p);
                        }
                    }
                }
            }
        }
    }
    files
}

/// Run all test files through QuickJS.
fn run_tests(
    cwd: &std::path::Path,
    test_files: &[PathBuf],
    flags: &TestFlags,
) -> TestResult {
    if test_files.is_empty() {
        return TestResult::default();
    }

    // Load required modules: glasshouse.js and any test-runner.js if it exists
    let mut module_names = vec!["glasshouse"];
    if cwd.join("glasshouse").join("test-runner.js").exists() {
        module_names.push("test-runner");
    }

    let _modules = quickjs::load_glasshouse_modules(cwd, &module_names).unwrap_or_default();

    // Build the test wrapper JS
    let wrapper = build_test_js_wrapper(cwd, test_files, flags);

    match quickjs::eval_with_modules(&wrapper.code, &wrapper.modules) {
        Ok(output) => parse_test_output(&output, flags),
        Err(e) => {
            let mut result = TestResult::default();
            result.fail += 1;
            result.failures.push(TestFailure {
                name: "(runner)".to_string(),
                error: e,
                duration_ms: 0.0,
            });
            result
        }
    }
}

/// Build JS wrapper code for running tests.
struct TestJsWrapper {
    modules: Vec<(String, String)>,
    code: String,
}

fn build_test_js_wrapper(
    cwd: &std::path::Path,
    test_files: &[PathBuf],
    flags: &TestFlags,
) -> TestJsWrapper {
    // The modules were already loaded in run_tests; here we may need additional
    // modules for test infrastructure. Pass through what we have.
    let mut module_names = vec!["glasshouse"];
    let has_test_runner = cwd.join("glasshouse").join("test-runner.js").exists();
    if has_test_runner {
        module_names.push("test-runner");
    }

    let modules = quickjs::load_glasshouse_modules(cwd, &module_names).unwrap_or_default();

    // Build an array of { file, source } for test files
    let mut test_sources_json = String::from("[");
    for (i, fp) in test_files.iter().enumerate() {
        if i > 0 {
            test_sources_json.push(',');
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
        test_sources_json.push_str(&format!(
            "{{\"file\":\"{}\",\"source\":\"{}\"}}",
            rel, escaped
        ));
    }
    test_sources_json.push(']');

    let filter_expr = match &flags.filter {
        Some(f) => format!(r#""{}""#, f.replace('\\', "\\\\").replace('"', "\\\"")),
        None => "null".to_string(),
    };
    let verbose_flag = if flags.verbose { "true" } else { "false" };
    let has_runner = if has_test_runner { "true" } else { "false" };

    let code = format!(
        r#"
// Test runner for GlassHouse CLI
var testFiles = {};
var filterPattern = {};
var verbose = {};
var hasTestRunner = {};

// Simple test harness
var results = {{
    pass: 0,
    fail: 0,
    skip: 0,
    tests: [],
    startTime: Date.now()
}};

function assert(condition, message, testName) {{
    if (!condition) {{
        throw new Error(message || 'Assertion failed');
    }}
}}

function assertEqual(actual, expected, message, testName) {{
    if (actual !== expected) {{
        throw new Error(
            (message || 'Assertion failed') +
            ': expected ' + JSON.stringify(expected) +
            ' but got ' + JSON.stringify(actual)
        );
    }}
}}

function assertDeepEqual(actual, expected, message, testName) {{
    var a = JSON.stringify(actual);
    var b = JSON.stringify(expected);
    if (a !== b) {{
        throw new Error(
            (message || 'Deep assertion failed') +
            ': expected ' + b +
            ' but got ' + a
        );
    }}
}}

// Make test API available globally so test files can use them
var test = function(name, fn) {{
    if (filterPattern && name.indexOf(filterPattern) === -1) {{
        results.skip++;
        results.tests.push({{
            name: name,
            status: 'skip',
            duration: 0,
            error: null
        }});
        return;
    }}

    var t0 = Date.now();
    try {{
        fn();
        var dt = Date.now() - t0;
        results.pass++;
        results.tests.push({{
            name: name,
            status: 'pass',
            duration: dt,
            error: null
        }});
        if (verbose) print('  \u2713 ' + name + ' (' + dt + 'ms)');
    }} catch (e) {{
        var dt = Date.now() - t0;
        results.fail++;
        results.tests.push({{
            name: name,
            status: 'fail',
            duration: dt,
            error: e.message || String(e)
        }});
        if (verbose) print('  \u2717 ' + name + ' (' + dt + 'ms)');
        print('    Error: ' + (e.message || String(e)));
    }}
}};

// Run each test file
for (var i = 0; i < testFiles.length; i++) {{
    var tf = testFiles[i];
    if (verbose) print('\n  File: ' + tf.file);

    try {{
        // Evaluate the test file source in a scoped context
        var testFn = new Function(
            'test', 'assert', 'assertEqual', 'assertDeepEqual',
            'GlassHouse', 'console',
            tf.source
        );
        testFn(test, assert, assertEqual, assertDeepEqual, GlassHouse, console);
    }} catch (e) {{
        results.fail++;
        results.tests.push({{
            name: tf.file + ' (load error)',
            status: 'fail',
            duration: 0,
            error: e.message || String(e)
        }});
        if (verbose) print('  \u2717 ' + tf.file + ' — load error: ' + (e.message || String(e)));
    }}
}}

results.totalTime = Date.now() - results.startTime;

// Output results as JSON
print('__GLAS_TEST_RESULTS__');
print(JSON.stringify(results));
print('__GLAS_END__');
"#,
        test_sources_json, filter_expr, verbose_flag, has_runner
    );

    TestJsWrapper { modules, code }
}

/// Parse the test output from QuickJS. It outputs markers around the JSON result.
fn parse_test_output(output: &str, _flags: &TestFlags) -> TestResult {
    let mut result = TestResult::default();

    // Look for the JSON result between markers
    if let Some(start_idx) = output.find("__GLAS_TEST_RESULTS__") {
        let after_start = &output[start_idx + "__GLAS_TEST_RESULTS__".len()..];
        if let Some(end_idx) = after_start.find("__GLAS_END__") {
            let json_str = after_start[..end_idx].trim();
            if let Some(val) = crate::json::parse(json_str) {
                if let crate::json::Value::Object(pairs) = &val {
                    for (key, val) in pairs {
                        match key.as_str() {
                            "pass" => {
                                if let crate::json::Value::Number(n) = val {
                                    result.pass = *n as u32;
                                }
                            }
                            "fail" => {
                                if let crate::json::Value::Number(n) = val {
                                    result.fail = *n as u32;
                                }
                            }
                            "skip" => {
                                if let crate::json::Value::Number(n) = val {
                                    result.skip = *n as u32;
                                }
                            }
                            "totalTime" => {
                                if let crate::json::Value::Number(n) = val {
                                    result.total_time_ms = *n;
                                }
                            }
                            "tests" => {
                                if let crate::json::Value::Array(items) = val {
                                    for item in items {
                                        if let crate::json::Value::Object(tpairs) = item {
                                            let mut name = String::new();
                                            let mut status = String::new();
                                            let mut error = String::new();
                                            let mut duration = 0.0;
                                            for (tk, tv) in tpairs {
                                                match tk.as_str() {
                                                    "name" => {
                                                        if let crate::json::Value::String(s) = tv {
                                                            name = s.clone();
                                                        }
                                                    }
                                                    "status" => {
                                                        if let crate::json::Value::String(s) = tv {
                                                            status = s.clone();
                                                        }
                                                    }
                                                    "error" => {
                                                        if let crate::json::Value::String(s) = tv {
                                                            error = s.clone();
                                                        }
                                                    }
                                                    "duration" => {
                                                        if let crate::json::Value::Number(n) = tv {
                                                            duration = *n;
                                                        }
                                                    }
                                                    _ => {}
                                                }
                                            }
                                            result.tests.push(TestEntry {
                                                name,
                                                status,
                                                error,
                                                duration_ms: duration,
                                            });
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    // If parsing failed, check for raw error output
    if result.tests.is_empty() && result.fail == 0 && result.pass == 0 {
        result.fail = 1;
        result.failures.push(TestFailure {
            name: "(runner output)".to_string(),
            error: output.trim().to_string(),
            duration_ms: 0.0,
        });
    }

    // Collect failures from test entries
    for t in &result.tests {
        if t.status == "fail" {
            result.failures.push(TestFailure {
                name: t.name.clone(),
                error: t.error.clone(),
                duration_ms: t.duration_ms,
            });
        }
    }

    result
}

/// Print test results with formatting.
fn print_test_results(result: &TestResult, flags: &TestFlags) {
    if flags.json {
        print_json_result(result);
        return;
    }

    let total = result.pass + result.fail + result.skip;
    println!();

    if flags.verbose && !result.tests.is_empty() {
        // Already printed during execution, just print summary
    }

    // Summary line
    let status = if result.fail == 0 { "✓" } else { "✗" };
    println!(
        "{} {} test(s): {} passed, {} failed, {} skipped ({:.0}ms)",
        status,
        total,
        result.pass,
        result.fail,
        result.skip,
        result.total_time_ms
    );

    // Print failures
    if !result.failures.is_empty() {
        println!();
        for f in &result.failures {
            println!("  ✗ {}", f.name);
            println!("    {}", f.error);
        }
    }

    println!();
}

/// Print test results as JSON for CI.
fn print_json_result(result: &TestResult) {
    let mut tests_json = String::from("[");
    for (i, t) in result.tests.iter().enumerate() {
        if i > 0 {
            tests_json.push(',');
        }
        tests_json.push_str(&format!(
            r#"{{"name":"{}","status":"{}","duration":{},"error":{}}}"#,
            t.name.replace('\\', "\\\\").replace('"', "\\\""),
            t.status,
            t.duration_ms,
            if t.error.is_empty() {
                "null".to_string()
            } else {
                format!(
                    "\"{}\"",
                    t.error.replace('\\', "\\\\").replace('"', "\\\"")
                )
            }
        ));
    }
    tests_json.push(']');

    println!(
        r#"{{"pass":{},"fail":{},"skip":{},"totalTime":{},"tests":{}}}"#,
        result.pass, result.fail, result.skip, result.total_time_ms, tests_json
    );
}

// ── Data types ──

#[derive(Debug, Clone, Default)]
struct TestEntry {
    name: String,
    status: String,
    error: String,
    duration_ms: f64,
}

#[derive(Debug, Clone, Default)]
struct TestFailure {
    name: String,
    error: String,
    #[allow(dead_code)]
    duration_ms: f64,
}

#[derive(Debug, Default)]
struct TestResult {
    pass: u32,
    fail: u32,
    skip: u32,
    tests: Vec<TestEntry>,
    failures: Vec<TestFailure>,
    total_time_ms: f64,
}
