# RustIdea

RustIdea is a native Rust IDE built on [GPUI](https://gpui.rs). It reuses the
Apache-2.0 Rust backend from [OpenIDEA](https://github.com/l0ng-ai/openidea)
while replacing the Tauri/React/CodeMirror workbench with a native Rust UI.

The shipped application does not link a WebView, a browser runtime, React,
CodeMirror, or Tauri.

## Status

The single official application is the **Zed Workbench**:

- Zed's native `editor`/`Buffer`/`language` engine, not a re-written text editor;
- the reusable OpenIDEA `ide-*` backend crates;
- a GPUI workbench being migrated 1:1 from OpenIDEA's Islands design.

The legacy native app (`crates/rustidea`, `crates/rustidea-editor`) is a frozen
historical regression baseline. It is no longer the release application and
receives no new features. See `RUSTIDEA_NATIVE_APP.md` for the decision, the
GPL/bundle-id approval gates and the user-data migration plan.

UI migration is tracked component by component in `RUSTIDEA_UI_PARITY.md`.
Design tokens are generated from the pinned OpenIDEA `tokens.css` by
`tools/openidea-tokens/generate.py`; the renderer does not keep a hand-tuned
theme. Real reference screenshots live under `tests/visual-baselines/`.

This is not yet complete OpenIDEA parity. The current workbench still contains
the experiment shell and must be replaced by the Islands workbench; the parity
ledger records every unresolved gap rather than declaring success from a
compile.

## Build and run

One-time fetch of the pinned Zed tree:

```sh
scripts/fetch-zed.sh
```

Run the official native app:

```sh
scripts/run-native.sh --trust /path/to/project
```

Release binary:

```sh
PROFILE=release scripts/build-native.sh
```

Create a signed and notarized macOS app (native binary only):

```sh
scripts/release-macos.sh
```

`scripts/release-macos.sh` produces `RustIdea Native Preview` until the
maintainer approvals in `RUSTIDEA_NATIVE_APP.md` are recorded; setting
`RUSTIDEA_STABLE_REPLACEMENT_APPROVED=1` switches it to `RustIdea.app`.
The old packaging path remains only as `scripts/release-legacy-macos.sh`.

## Test

```sh
cargo test --workspace
cargo test -p rustidea-design
python3 tools/openidea-tokens/generate.py --check
(cd experiments/zed-poc && cargo test -p rustidea-zed-workbench)
```

The native visual gate (real apps, never mock screenshots):

```sh
tests/visual/run-visual-regression.sh
```

Licensing details are in `THIRD_PARTY_LICENSES.md`,
`THIRD_PARTY_RUST_LICENSES.md`, `THIRD_PARTY_SOURCES/option-ext-0.2.0/`, and
`RUSTIDEA_LICENSE_AUDIT.md`.

Verification evidence and remaining acceptance work are tracked in
`RUSTIDEA_REGRESSION_REPORT.md` and `RUSTIDEA_PERFORMANCE_REPORT.md`.
