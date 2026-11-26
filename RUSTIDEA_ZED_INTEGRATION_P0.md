# RustIdea Zed integration P0

Status: integration branch created and Zed POC merged for evaluation only.
The branch is not merged into `main` and is not released.

## Baseline

| Component | Revision |
|---|---|
| `main` | `2c3c84bcc7d75838d57650b923a0a9ce7886e78e` |
| `integration/zed-workbench` merge commit | `1d359f0861415dcb566581b96320abdcbe205f0a` |
| `experiment/zed-editor` | `01874d0bde0608096c7435d59bf99872e4fbf0da` |
| Zed upstream | `2664a0aa1eae5dabf8f8273c03f88d53061ca7dc` |
| OpenIdea backend baseline | `ae7a8e74c836b89473162bd486eec0f13c52d962` |

The integration branch starts from the latest 0.2.0 `main` and merges the
Zed experiment with `--no-ff`. The main workbench, release scripts and
Apache-2.0 production graph are preserved.

## Diff from main

- 48 changed files, approximately 50,976 insertions.
- The isolation boundary is `experiments/zed-poc`, excluded from the root
  Cargo workspace.
- `third_party/zed` and `experiments/zed-poc/target` are ignored build inputs.
- `scripts/fetch-zed.sh` recreates Zed at the pinned revision and applies the
  tree-sitter compatibility patch.
- Shared backend additions required by the POC are limited to `ide-vcs`
  revision patch support and the GitHub operations used by the workbench.
- The production `crates/rustidea` editor entry has not been replaced yet;
  no main release is built from this branch.

Diff command:

```sh
git diff --stat main...integration/zed-workbench
```

## Dependency unification

The POC uses one Zed checkout for `gpui`, `gpui_platform`, `language`,
`editor`, `multi_buffer`, `project`, `settings`, `theme` and related crates.
Cargo metadata resolves a single `gpui v0.2.2` package from that path.

```text
gpui v0.2.2 (third_party/zed/crates/gpui)
tree-sitter v0.25.10
```

The tree-sitter resolution is deliberate, not an accidental duplicate:

- Zed at the pinned revision requests tree-sitter 0.28 APIs.
- The OpenIdea backend uses tree-sitter 0.25.
- Both declare `links = "tree-sitter"`, so two versions cannot link into one
  binary.
- `experiments/zed-poc/patches/zed-tree-sitter-0.25.patch` adapts the Zed
  call sites to 0.25.10 and exposes the completion-menu entry needed by the
  host.
- The trade-off is that the newest Zed tree-sitter query APIs are unavailable
  until the backend or Zed fork is migrated to a shared version.

## Clean clone build recipe

From a clean clone of the integration branch:

```sh
scripts/fetch-zed.sh
cd experiments/zed-poc
cargo check -p rustidea-zed-workbench
cargo test -p rustidea-zed-workbench
```

The POC has its own `rust-toolchain.toml` and `Cargo.lock`. A clean fetch must
produce `third_party/zed/.rustidea-zed-rev` containing the pinned Zed SHA.

## Current evidence

- `cargo check -p rustidea-zed-workbench`: pass in the integrating worktree.
- `cargo test -p rustidea-zed-workbench`: 66 passed, 0 failed.
- `cargo clippy -p rustidea-zed-workbench --all-targets -- -D warnings`: pass.
- `cargo tree`: one `gpui` source and one tree-sitter version.
- `scripts/fetch-zed.sh`: recreates the pinned checkout and applies the patch.
- Clean clone at `/tmp/rustidea-zed-integration-check.ENA2wM`:
  - `scripts/fetch-zed.sh` recreated Zed at `2664a0a…` and applied the patch.
  - `cargo metadata --locked` resolved 1534 packages and one Zed path source.
  - `cargo check --locked -p rustidea-zed-workbench` passed.
  - `cargo test --locked -p rustidea-zed-workbench` passed: 66 tests.
  - The clean build used `CARGO_PROFILE_DEV_DEBUG=0`,
    `CARGO_INCREMENTAL=0` and `RUSTFLAGS=-C debuginfo=0` to fit the review
    machine's disk budget.
- CI reproducibility gate: `.github/workflows/zed-integration.yml` runs the
  same fetch/check/test/Clippy path on macOS. Run `45810060881` for
  `e000a919cd289d01acb52b5b5ff3f317c6c9d435` completed successfully:
  fetch, check, 66 tests and Clippy all passed.

## Open gates before P1

- Maintainer must approve the GPL-3.0-or-later combined distribution.
- The Zed workbench must be wired into the formal RustIdea workspace rather
  than remaining an isolated POC binary.
- Interactive macOS acceptance, Chinese IME and real Maven project workflow
  remain to be completed.
