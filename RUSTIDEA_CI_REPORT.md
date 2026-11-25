# CI report

## Baseline failures observed

| Commit/run | Failure | Fix |
|---|---|---|
| `47d95a0` / run 45376849176 | Ubuntu `glib-sys` missing | CI installs GLib/GTK/cairo/pango/atk development packages |
| `b711f9d` / run 45381745413 | plugin fixture `dist/plugin.wasm` missing | checked in the upstream fixture components and unignored `crates/ide-plugin/tests/fixtures/*/dist/` |
| `4e4ba7a` / run 45389131502 | `src/bindings.ts` missing in `ide-host/tests/rpc.rs` | copied upstream generated `src/bindings.ts` |

The macOS job in run 45389131502 was cancelled; Ubuntu and Windows failed only
on the missing `bindings.ts` file. No test was skipped or deleted to produce a
green result.

## Required matrix

`.github/workflows/ci.yml` runs on macOS 15, Ubuntu 24.04 and Windows 2025:

1. Rust toolchain and `clippy`/`rustfmt`;
2. Linux native packages (including GTK/GLib);
3. `wasm32-wasip2` target;
4. plugin fixture build/check;
5. `cargo fmt --all --check`;
6. release build of `rustidea`;
7. `cargo test --locked --workspace`;
8. `cargo clippy --locked --workspace --all-targets -- -D warnings`.

The fixture step builds `files-events` and `path-events` with
`plugins/build.sh --check` and rejects any checked-in `plugin.toml` fixture with
a missing `dist/plugin.wasm`.

## Current status

The final verification run for the stability changes is pending. Do not claim
all three platforms green until the exact final commit's run completes with
success. A previous successful run (`370c14d`, run 45317630209) is historical
evidence only and is not a substitute for the latest commit.

## Local pre-CI evidence

- `cargo fmt --all --check`: pass for the working tree before final commit.
- `cargo test -p rustidea`: 31 unit + 12 document sync tests + the expanded
  regression files are expected to pass; see the final session log.
- `cargo test -p ide-host --test rpc`: rerun after restoring
  `src/bindings.ts`.
