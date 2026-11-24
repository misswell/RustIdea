# License audit

## Project and reused source

RustIdea and the reused OpenIDEA `ide-core` and `ide-text` crates declare
Apache-2.0. The existing OpenIDEA `NOTICE` and third-party notice file are
retained. OpenIDEA's broader notice references assets that RustIdea does not
ship; it is kept as upstream attribution, not treated as the dependency
inventory for the RustIdea executable.

GPUI is pinned to `=0.2.2` and declares Apache-2.0. No Zed editor or workspace
implementation is intentionally imported. The app's Cargo graph contains no
Tauri, WebView, React or CodeMirror dependency.

## Locked macOS dependency graph

The release-target inventory was generated from `Cargo.lock` for
`aarch64-apple-darwin` with `cargo-about 0.9.3`:

```sh
cargo about generate --manifest-path crates/rustidea/Cargo.toml \
  --config about.toml --format json --target aarch64-apple-darwin \
  --locked --offline --fail --output-file /tmp/rustidea-about.json
```

The command succeeded with `--fail`: all 507 resolved crate records had
recognized license expressions and local license texts, covering 10 SPDX
identifiers. The generated crate inventory and license texts are retained in
[`THIRD_PARTY_RUST_LICENSES.md`](THIRD_PARTY_RUST_LICENSES.md). The inventory
includes build dependencies and is not a claim that every listed crate is
linked into the executable.

`option-ext 0.2.0` is a runtime MPL-2.0 dependency through `dirs` and GPUI. Its
license text is in the generated license bundle, and the unmodified manifest,
README and Rust source are provided in
[`THIRD_PARTY_SOURCES/option-ext-0.2.0`](THIRD_PARTY_SOURCES/option-ext-0.2.0).
This preserves the corresponding source for the MPL-covered runtime crate.
`cbindgen` is a build dependency; its MPL-2.0 text is present in the inventory.
It is not a runtime dependency. The same locked-graph inventory records the
other selected licenses, including BSD, ISC, Unicode and Zlib license texts.

The locked graph also uses `xattr 0.2.3` through `zed-async-tar`. Its upstream
Linux cfg references `libc::ENOATTR`, which does not exist on Linux. The source
is retained under `vendor/xattr-0.2.3` with its MIT/Apache license texts and a
small platform fix mapping Linux/Android `ENOATTR` to `ENODATA`. The local
`[patch.crates-io]` entry makes this fix reproducible without changing the
crate version or behavior on macOS/BSD.

## Distribution requirements and limits

Include `THIRD_PARTY_RUST_LICENSES.md` and
`THIRD_PARTY_SOURCES/option-ext-0.2.0` in each source and binary distribution.
Re-run the command above after changing `Cargo.lock`; an unrecognized license
must be reviewed and deliberately added to `about.toml` before the audit passes.

This is a reproducible dependency inventory and notice check, not a legal
opinion. It covers the Rust crate graph for the macOS arm64 target and the
retained upstream notices; it does not audit external system frameworks or
future downloaded tools that users may install separately.
