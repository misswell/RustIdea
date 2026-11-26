# RustIdea Zed license audit

Status: approved for a GPL-3.0-or-later Preview distribution only.
The integration branch is still not the Apache-2.0 production `main` binary.

## Components

| Component | License | Role |
|---|---|---|
| RustIdea workbench and backend crates | Apache-2.0 | Existing product code |
| OpenIdea-derived backend | Apache-2.0 | Project, index, LSP, VCS, run, terminal, tools |
| GPUI | Apache-2.0 | Native UI framework |
| Zed `editor`, `language`, `multi_buffer`, `project`, `workspace`, `terminal` and related crates | GPL-3.0-or-later | Native editor and text infrastructure |
| Third-party Rust crates | Per package | See `THIRD_PARTY_RUST_LICENSES.md` and `cargo about` output |
| Zed bundled grammars/fonts/assets | Per upstream notice | See `third_party/zed/LICENSE-*` and grammar metadata |

The integration POC workspace declares:

```toml
license = "GPL-3.0-or-later"
```

The repository root still declares Apache-2.0 in `Cargo.toml`, `LICENSE` and
the existing release scripts. Those are accurate for `main` and the 0.2.0
GPUI release, which do not link Zed.

## Compatibility conclusion

Apache-2.0 code can be incorporated into a GPL-3.0-or-later combined work, but
the combined application must be distributed under GPL-3.0-or-later with the
corresponding source and notices. The reverse direction is not a substitute:
adding GPL Zed crates does not let the combined binary be distributed under
Apache-2.0.

Therefore:

1. Any future RustIdea binary containing Zed must be treated as GPL.
2. The root license and release artifacts must be updated before such a
   release, while preserving OpenIdea's Apache-2.0 `LICENSE`, `NOTICE` and
   copyright notices.
3. Zed's `LICENSE-GPL` and copyright notices must be shipped with the
   corresponding source and binary distribution.
4. The full corresponding source, pinned Zed revision, `scripts/fetch-zed.sh`,
   Cargo lockfiles and build instructions must accompany binary releases.
5. Dynamic linking, crate splitting or hiding Zed behind a process boundary
   is not an accepted way to avoid GPL obligations.
6. The maintained `v0.2.0` GPUI release must not be retroactively described as
   a Zed build.

## Preview distribution decision

The release owner approved publishing the integration branch as an explicit
Preview, not as a replacement for the stable GPUI product. The Preview:

- is named `RustIdea Zed Preview` and uses bundle identifier
  `com.guofeng.rustidea.zedpreview`;
- is marked as a GitHub pre-release and carries `GPL-3.0-or-later` notices;
- ships Zed's GPL text, Zed's Apache-2.0 text, RustIdea's Apache-2.0 text,
  `NOTICE`, third-party notices and the pinned-source instructions;
- records the exact RustIdea commit and Zed revision in
  `Contents/Resources/SOURCE_CODE.txt`;
- is hardened-runtime signed with narrowly scoped JIT entitlements required by
  the tree-sitter/Wasmtime grammar engine;
- remains arm64-only for this first Preview build;
- does not change the root `LICENSE`, does not merge the experiment into
  `main`, and does not replace the stable editor entry.

The full production integration, universal build, interactive IME acceptance
and OpenIDEA parity remain separate release gates. A Preview distribution is
not evidence that those gates passed.

## Verification commands

```sh
git diff main...integration/zed-workbench -- Cargo.toml LICENSE NOTICE
head -5 third_party/zed/LICENSE-GPL
grep -n 'license' third_party/zed/crates/editor/Cargo.toml
```
