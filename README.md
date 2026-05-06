# Glas CLI

Zero-dependency Rust CLI for GlassHouse projects. Single binary compiled with `rustc` — no cargo, no crates.

## Install

Download the installer from [GitHub Releases](https://github.com/OniuUI/Glas-CLI/releases/latest):

```
glas-installer.exe
```

Or download `glas.exe` directly and place it on your PATH.

## Commands

```
glas init <name>                    Scaffold a new GlassHouse project
glas dev [--port N] [--open]        Dev server with hot reload
  --lint                              Lint on each file change
glas serve [--port N]               Serve production build from dist/
glas build [--dev] [--lint]         Production build (hyper-compaction)
glas lint [--fix] [--json]          Lint project source files
glas test [--filter] [--watch]      Run project tests
glas install <source> [-f]          Install a package
glas uninstall <name>               Remove a package
glas list                           List installed packages
glas info <name>                    Show package details
glas audit [--deep] [--fix]         Full project audit
glas upgrade [--major] [--dry-run]  Upgrade packages
glas run <script>                   Run a script from glass.json
glas help                           Show all commands
glas version                        Show version
```

## Build from Source

Requires `rustc` (no cargo needed):

```
rustc --edition 2021 src/main.rs -o glas.exe
```

For the installer:

```
rustc --edition 2021 installer/main.rs -o glas-installer.exe
```

## License

MIT — see [LICENSE](LICENSE).
