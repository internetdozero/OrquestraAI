# windows-typecheck

Staging crate that compiles the desktop shell's `#[cfg(windows)]` modules from
a Linux or macOS host.

## Why this exists

`cargo check --target x86_64-pc-windows-msvc -p orquestra-ai` fails before it
reaches any of this repository's code: `ring`'s build script and `rusqlite`'s
bundled SQLite need a Windows compiler (`lib.exe`). The shell is the only crate
with Windows-only modules, so on a Linux runner those modules — the health
collector, the single-instance mutex, the updater's image retirement — were
never parsed, and a typo in them would only surface in a release build.

This crate closes that gap without copying source:

```rust
#[path = "../../../../src-tauri/src/janitor/health/windows.rs"]
mod windows;
```

`#[path]` compiles the file that ships. The surrounding `mod.rs` only restates
what the real parent module provides (`super::{…}` types and helpers), plus a
`crate::paths` stand-in. A signature change in the shell shows up here as a
mismatch instead of drifting silently.

Two consequences worth knowing:

- The included file must spell its own child modules explicitly
  (`#[path = "windows/native.rs"] mod native;`). Without `#[path]` on the
  parent, a bare `mod native;` resolves differently under this crate.
- The `windows-sys` feature list in `Cargo.toml` mirrors the `cfg(windows)`
  block in `src-tauri/Cargo.toml`. A feature missing here would hide a `use`
  from the check instead of failing it.

## Adding another module

1. Create `src/<area>/mod.rs`: the parent-side items that module imports
   (`super::`), copied verbatim from its real parent, plus a `crate::paths`
   stand-in only if it needs one.
2. Add the real file with `#[path = "../../../../src-tauri/src/<path>.rs"]`.
3. Expose one `pub fn` that the crate root calls from `build_graph()`, so
   `-D warnings` checks the code instead of reporting it all as dead.

## Running it

```bash
./scripts/windows-typecheck.sh
```

The script installs nothing: it requires the `x86_64-pc-windows-msvc` target
(`rustup target add x86_64-pc-windows-msvc`) and fails with that instruction.
