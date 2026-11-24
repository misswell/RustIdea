# macOS release checklist

Status: static code verification and dependency notice inventory are complete.
Cross-platform builds, the measured performance baseline and Phase 1–2
interactive acceptance remain open, so the formal release is still pending. Do
not describe this build as OpenIDEA parity or Phase 2 acceptance.

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
5. Build the release app bundle, sign with a verified Developer ID Application
   identity and hardened runtime, and verify the signature and timestamp.
6. Submit the signed app archive to Apple with a validated notarytool profile;
   wait for acceptance, staple and validate the ticket, then check Gatekeeper.
7. Produce the final archive from the stapled app, compute SHA-256, push the
   verified commit and a new tag without overwriting any existing tag, and create
   a stable GitHub Release with accurate Phase 0–2 limitations.
8. Verify the remote commit, tag, stable Release, downloadable asset and digest.

Reference: `/Users/guofeng/.codex/skills/macos-github-release/SKILL.md` and
`/Users/guofeng/Code/solo/TypeDown/scripts/build-release.sh`. The latter is a
Tauri project and is a signing/notarization reference only, not an architecture
to import into RustIdea. Local Keychain profile `agentspace-notary` successfully
returned submission history on 2026-11-24; validate it again when releasing.
