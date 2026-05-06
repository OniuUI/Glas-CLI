# Contributing to Glas CLI

## Build

```
rustc --edition 2021 src/main.rs -o glas.exe
```

## Structure

```
src/
  main.rs          Entry point and command dispatch
  json.rs          Hand-written JSON parser
  utils.rs         File system, network, shell utilities
  server.rs        Dev HTTP server
  quickjs.rs       QuickJS subprocess bridge
  commands/
    init.rs        glas init
    install.rs     glas install / uninstall
    serve.rs       glas serve
    dev.rs         glas dev (watch + TS compilation)
    build.rs       glas build (hyper-compaction)
    audit.rs       glas audit
    lint.rs        glas lint
    test.rs        glas test
    list.rs        glas list / info
    run.rs         glas run
    upgrade.rs     glas upgrade
```

## Pull Requests

- Keep changes focused and minimal
- No external dependencies — `rustc` only
- Follow existing code style (compact, no unnecessary comments)
- Test on Windows before submitting
