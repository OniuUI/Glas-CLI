# Glas CLI

Zero-dependency Rust CLI for GlassHouse projects. Single binary compiled with `rustc` — no cargo, no crates, no external dependencies. Scaffold projects, run dev servers, compile TypeScript, lint, test, build production bundles, and manage packages. Everything through one binary.

Built for the [GlassHouse](https://github.com/OniuUI/GlassHouse) framework.

---

**Commands** · **Install** · **Build from source** · **Repository layout** · **Environment** · **Contributing**

---

## Why this CLI?

| You want to… | Glas CLI helps by… |
|-------------|-------------------|
| Start a new GlassHouse project | `glas init my-app` — scaffolds project + fetches framework from GitHub Releases |
| Develop locally with hot reload | `glas dev` — file watcher + HTTP server + TypeScript compilation on change |
| Compile TypeScript to JavaScript | Built-in TS compiler via QuickJS — no external tsc needed |
| Catch issues before they ship | `glas lint` — security, size, type safety, handler logic checks |
| Run tests without a browser | `glas test` — executes test files through QuickJS with assertion helpers |
| Build for production | `glas build` — resolve → tree-shake → hyper-compact → single bundle |
| Install reusable packages | `glas install` — local paths, remote URLs, name@version |
| Audit the entire project | `glas audit` — structure, security, types, WCAG, performance, dependencies |
| Stay up to date | `glas upgrade` — semver-aware package updates |

The CLI does not embed domain logic; it manipulates files, shells out to QuickJS, and talks to GitHub Releases for framework fetching.

## Install

**Windows (recommended):**

Download the latest installer from [GitHub Releases](https://github.com/OniuUI/Glas-CLI/releases/latest):

```
glas-installer.exe
```

The installer downloads `glas.exe`, caches the GlassHouse framework, adds itself to PATH, and creates an uninstaller.

**Manual (all platforms):**

Download `glas.exe` (or the platform binary) from [GitHub Releases](https://github.com/OniuUI/Glas-CLI/releases/latest) and place it on your PATH.

**QuickJS requirement:**

Some commands (`lint`, `test`, `build`, `dev` with TS compilation) require QuickJS. Download `qjs.exe` from [bellard.org/quickjs](https://bellard.org/quickjs/) and place it next to `glas.exe` or in a `quickjs/` subdirectory.

## Commands

```
glas init <name> [--glasshouse VERSION]
glas dev [--port N] [--open] [--lint]
glas serve [--port N]
glas build [--dev] [--lint]
glas lint [--fix] [--json] [--strict] [--realtime]
glas test [--filter PATTERN] [--verbose] [--json] [--watch]
glas install <source> [-f]
glas install <name@1.2.3>
glas uninstall <name>
glas list
glas info <name>
glas audit [--deep] [--fix]
glas upgrade [<name>] [--major] [--dry-run]
glas run <script>
glas help
glas version
```

### `glas init`

Scaffolds a new GlassHouse project and downloads the framework from GitHub Releases.

```
glas init my-app                    Uses latest GlassHouse release
glas init my-app --glasshouse 2.0.0 Specific version
glas init my-app --glasshouse nightly  Nightly build
```

Created structure:

```
my-app/
├── index.html               Entry point with script tags
├── glasshouse/              Framework files (downloaded from releases)
│   ├── core/                glasshouse.js, types.js, dom.js, lint.js, ...
│   ├── blocks/              pebble.js, handler.js, shine.js, pane.js
│   ├── compiler/            TypeScript compiler (6 files)
│   ├── pipeline/            Build pipeline (6 files)
│   ├── pkg/                 Package system (3 files)
│   └── tools/               auditor, wcag-validator, cli
├── src/                     Your application code
│   └── app.js
├── packages/                Installed packages
├── styles/
│   └── main.css
├── .gitignore
└── glass.json               Project manifest
```

### `glas dev`

Starts a development server with file watching and TypeScript compilation.

```
glas dev                     Start on :3000
glas dev --port 8080         Custom port
glas dev --open              Open browser automatically
glas dev --lint              Run lint on each file change
```

- Watches `src/`, `packages/`, `glasshouse/` for changes
- Compiles `.ts`/`.tsx` files via the built-in TypeScript compiler
- Serves `index.html` with all framework and app scripts
- Reloads browser on change (if `--open`)

### `glas build`

Production build — resolves dependencies, tree-shakes dead code, hyper-compacts identifiers, and outputs a single bundle.

```
glas build                   Production build → dist/
glas build --dev             Readable output (no compaction)
glas build --lint            Run lint before build, abort on errors
```

### `glas lint`

Lints project source files for security issues, type safety violations, size limits, and handler logic in Pebbles.

```
glas lint                    Lint all .ts/.tsx files in src/ and packages/
glas lint --fix              Auto-fix where possible (add 'use strict')
glas lint --realtime         Watch mode — re-lint on file changes
glas lint --json             JSON output for CI
glas lint --strict           Treat warnings as errors
```

### `glas test`

Runs test files through QuickJS with a built-in assertion framework.

```
glas test                    Run all tests in tests/
glas test --verbose          Detailed output with test names and timings
glas test --filter "should render"   Run only matching tests
glas test --json             JSON output for CI
glas test --watch            Re-run on file changes
```

Test files use `test()`, `assert()`, `assertEqual()`, `assertDeepEqual()`:

```javascript
test('should greet', function () {
    var result = greet('World');
    assertEqual(result, 'Hello, World!');
});
```

### `glas install`

Installs packages from local paths, remote URLs, or the registry.

```
glas install ./packages/my-pebble     Local directory
glas install button@1.2.3            Specific version
glas install button@latest           Latest version
glas install -f ./packages/my-pebble Force reinstall
glas uninstall button                 Remove package
```

### `glas upgrade`

Checks for package updates with semver support.

```
glas upgrade                 Upgrade all packages within their ranges
glas upgrade button          Upgrade specific package
glas upgrade --major         Allow major version bumps
glas upgrade --dry-run       Preview without applying
```

### `glas audit`

Runs a full project audit across structure, security, types, WCAG compliance, performance, and dependencies.

```
glas audit                   Standard audit
glas audit --deep            Comprehensive analysis
glas audit --fix             Auto-fix where possible
```

## Flags

| Flag | Commands | Purpose |
|------|----------|---------|
| `--force, -f` | install | Force reinstall (overwrites existing) |
| `--dev` | build | Development mode (no compaction, readable output) |
| `--port N` | dev, serve | Set server port (default: 3000) |
| `--open` | dev | Open browser on start |
| `--lint` | dev, build | Run lint before/alongside operation |
| `--glasshouse VERSION` | init | Specific GlassHouse version to install |
| `--fix` | lint, audit | Auto-fix where possible |
| `--json` | lint, test | Output as JSON for CI |
| `--strict` | lint | Treat warnings as errors |
| `--realtime, -r` | lint | Watch mode |
| `--filter PATTERN` | test | Run only matching tests |
| `--verbose, -v` | test | Detailed output |
| `--watch` | test | Re-run on file changes |
| `--deep` | audit | Full analysis |
| `--major` | upgrade | Allow major version bumps |
| `--dry-run` | upgrade | Preview without applying |

## Environment

| Variable | Purpose |
|----------|---------|
| `GLAS_REGISTRY` | Default registry URL for package operations (otherwise uses local `.registry.json`) |

## Repository Layout

```
src/
  main.rs              Entry point and command dispatch
  json.rs              Hand-written JSON parser (zero-dependency)
  utils.rs             File system, network fetch, shell utilities
  server.rs            Dev HTTP server with directory listing
  quickjs.rs           QuickJS subprocess bridge (lint, test, build, compile)
  compact.js           JS compaction utility
  commands/
    init.rs            glas init — scaffold + framework fetch from GitHub Releases
    install.rs         glas install / uninstall — package management
    serve.rs           glas serve — production build server
    dev.rs             glas dev — watch + TS compile + hot reload
    build.rs           glas build — resolve → tree-shake → hyper-compact → bundle
    audit.rs           glas audit — full project inspection
    lint.rs            glas lint — JS security/type/size linting via QuickJS
    test.rs            glas test — test runner via QuickJS
    list.rs            glas list / info — package listing
    run.rs             glas run — project script execution
    upgrade.rs         glas upgrade — semver-aware updates
    mod.rs             Module re-exports
installer/
  main.rs              glas-installer — custom Rust installer (separate binary)
```

## Building from Source

Requires `rustc` only — no cargo, no crates.

```
rustc --edition 2021 src/main.rs -o glas.exe
```

For the installer:

```
rustc --edition 2021 installer/main.rs -o glas-installer.exe
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Bug reports and focused PRs are welcome.

- Keep changes focused and minimal
- No external dependencies — `rustc` only
- Follow existing code style (compact, no unnecessary comments)
- Test on Windows before submitting

## License

MIT — see [LICENSE](LICENSE).

---

Related: [GlassHouse](https://github.com/OniuUI/GlassHouse) — the framework this CLI serves.
