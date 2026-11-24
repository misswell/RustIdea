# macOS release checklist

Status: `0.1.2` release preparation. Static native architecture, targeted
tests, formatting, Clippy and license inventory have passed locally. Interactive
Phase 1–2 acceptance and comparative performance remain open. The release is a
native GPUI MVP, not OpenIDEA parity.

1. Keep `RUSTIDEA_MIGRATION_AUDIT.md`, `RUSTIDEA_FEATURE_PARITY.md` and
   `RUSTIDEA_ARCHITECTURE.md` accurate. Do not describe placeholders as
   completed features.
2. Run `scripts/update-license-bundle.sh` after the final `Cargo.lock`. Retain
   `LICENSE`, `NOTICE`, `THIRD_PARTY_LICENSES.md`,
   `THIRD_PARTY_RUST_LICENSES.md` and all `THIRD_PARTY_SOURCES/` entries in the
   app bundle.
3. Verify `cargo fmt --all --check`, targeted tests and Clippy on the exact
   release commit. Run the macOS, Ubuntu 24.04 and Windows 2025 CI matrix and
   follow it until every job is green.
4. Confirm the new patch tag does not exist remotely. `v0.1.1` must not move or
   be overwritten; the intended tag is `v0.1.2`.
5. Validate the notary profile in the current session:

   ```sh
   xcrun notarytool history --keychain-profile agentspace-notary
   ```

6. Build arm64 and x86_64 release binaries, create a universal executable,
   package `RustIdea.app` with `assets/RustIdea.icns`, sign with Developer ID
   Application and hardened runtime, and verify the signature/timestamp.
7. Submit the signed archive to Apple, wait for `Accepted`, staple and validate
   the ticket, then run `spctl --assess --type execute`.
8. Produce the final universal zip from the stapled app, record its SHA-256 and
   run `otool -L` plus dependency checks for hidden WebView/browser frameworks.
9. Push the verified commit and tag without force, create a stable GitHub
   Release (not prerelease), upload the universal archive and include the
   SHA-256, install instructions, limitations and no-WebView statement.
10. Verify the remote commit, immutable tag, stable Release, downloadable asset
    and digest. Report the GitHub commit/Release links and absolute artifact
    paths.

Reference: `/Users/guofeng/.codex/skills/macos-github-release/SKILL.md` and the
verified signing/notarization approach in
`/Users/guofeng/Code/solo/TypeDown/scripts/build-release.sh`. The latter is a
Tauri project and is a release reference only; none of its WebView
architecture is imported into RustIdea.
