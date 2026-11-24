# License audit

## Project and reused source

RustIdea, `ide-core`, `ide-text` and the adopted OpenIDEA backend crates declare
Apache-2.0. The upstream `NOTICE` is retained. OpenIDEA notices referencing
assets not shipped by RustIdea remain upstream attribution, not a claim that
every referenced asset is included.

GPUI is pinned to `=0.2.2` and declares Apache-2.0. No Zed Editor or Workspace
GPL implementation is imported. The RustIdea dependency graph contains no
Tauri, Wry, WebView/WebKit, Chromium, Electron, React, CodeMirror, Monaco or
xterm runtime.

## Locked macOS dependency graph

The release-target inventory was generated for `aarch64-apple-darwin`:

```sh
cargo about generate \
  --manifest-path crates/rustidea/Cargo.toml \
  --config about.toml \
  --format json \
  --target aarch64-apple-darwin \
  --locked --offline --fail \
  --output-file /tmp/rustidea-about.json
```

The command succeeds with `--fail`. The report contains 919 resolved crate
records and 12 selected SPDX identifiers: Apache-2.0, MIT, ISC, Unicode-3.0,
BSD-3-Clause, BSD-2-Clause, MPL-2.0, Zlib, CC0-1.0, 0BSD,
CDLA-Permissive-2.0 and MIT-0. The generated inventory and license texts are
in [`THIRD_PARTY_RUST_LICENSES.md`](THIRD_PARTY_RUST_LICENSES.md). It includes
build dependencies and is not proof that every listed crate is linked.

`CDLA-Permissive-2.0` was added deliberately for `webpki-roots 1.0.12`, a
permissive root-certificate data license. The complete text is in the bundle.

## MPL-2.0 runtime sources

The locked graph contains MPL-2.0 runtime crates `option-ext 0.2.0`,
`nucleo-matcher 0.3.1` and `spellbook 0.4.2`. Their unmodified sources are
retained under:

- [`THIRD_PARTY_SOURCES/option-ext-0.2.0`](THIRD_PARTY_SOURCES/option-ext-0.2.0)
- [`THIRD_PARTY_SOURCES/nucleo-matcher-0.3.1`](THIRD_PARTY_SOURCES/nucleo-matcher-0.3.1)
- [`THIRD_PARTY_SOURCES/spellbook-0.4.2`](THIRD_PARTY_SOURCES/spellbook-0.4.2)

`cbindgen 0.28.0` is also MPL-2.0 but is a build dependency, not a runtime
dependency. Its license text is present in the generated inventory.

The locked graph also uses `xattr 0.2.3` through `zed-async-tar`. Upstream
Linux cfg references `libc::ENOATTR`, absent on Linux, so the unmodified
macOS/BSD source is retained under `vendor/xattr-0.2.3` with a small
Linux/Android `ENOATTR` to `ENODATA` mapping and a local `[patch.crates-io]`
entry. This is reproducible and does not change macOS/BSD behavior.

## Distribution requirements and limits

Every source and binary distribution must include:

- `LICENSE`, `NOTICE`, `THIRD_PARTY_LICENSES.md` and
  `THIRD_PARTY_RUST_LICENSES.md`;
- the three MPL-2.0 runtime source directories under `THIRD_PARTY_SOURCES`;
- the retained upstream notice information.

Re-run `scripts/update-license-bundle.sh` after changing `Cargo.lock`. An
unrecognized license must be reviewed and deliberately added to `about.toml`
before the audit passes. This is a reproducible inventory, not a legal opinion,
and it does not audit external system frameworks or future downloaded tools.
