# macOS release checklist

Status: static code verification, three-platform CI, and the macOS
signing/notarization pipeline are complete for 0.1.1. The measured OpenIDEA
baseline and Phase 1–2 interactive acceptance remain open. The 0.1.1 publication
is a native GPUI MVP release, not OpenIDEA parity or Phase 2 acceptance.

1. Keep the audit and feature inventory current; collect the measured OpenIDEA
   baseline and record the results.
2. Verify native window lifecycle, themes, editing, clipboard, undo/redo,
   autosave, saving and reopening, real Chinese IME composition and large-file
   responsiveness in an isolated GUI session. Check the process tree and linked
   frameworks for WebView and browser processes.
3. Re-run the locked macOS dependency license inventory after dependency
   changes. Include `THIRD_PARTY_RUST_LICENSES.md`, the required notice files,
   and the MPL-2.0 `option-ext` source in the app or distribution.
4. Run workspace tests, formatting and Clippy on the release commit. Determine a
   new patch version from remote tags and synchronize package and bundle versions.
   Complete: macOS, Ubuntu 24.04 and Windows 2025 CI passed at
   `bfadf41e47708ea3c48f887e5312584dd6e2e477`.
5. Build the release app bundle, sign with a verified Developer ID Application
   identity and hardened runtime, and verify the signature and timestamp.
6. Submit the signed app archive to Apple with a validated notarytool profile;
   wait for acceptance, staple and validate the ticket, then check Gatekeeper.
   Complete: universal submission `0dd04501-d29a-429e-8491-ceedc1111566`
   was accepted; signature, ticket and Gatekeeper checks passed.
7. Produce the final archive from the stapled app, compute SHA-256, push the
   verified commit and a new tag without overwriting any existing tag, and create
   a stable GitHub Release with accurate Phase 0–2 limitations. Asset SHA-256:
   `1d75b913ad430f9cb526187d2672db5717c236f558e6ffe455d0e1dca325e4ea`.
8. Verify the remote commit, tag, stable Release, downloadable asset and digest.

Reference: `/Users/guofeng/.codex/skills/macos-github-release/SKILL.md` and
`/Users/guofeng/Code/solo/TypeDown/scripts/build-release.sh`. The latter is a
Tauri project and is a signing/notarization reference only, not an architecture
to import into RustIdea. Local Keychain profile `agentspace-notary` successfully
returned submission history on 2026-11-24; validate it again when releasing.
