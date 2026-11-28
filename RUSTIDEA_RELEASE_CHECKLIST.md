# macOS release checklist

Status: `0.2.0` released as immutable tag `v0.2.0` at commit `0678891`.
The universal app is Developer ID signed, notarized (`Accepted`, submission
`0ee3c7d4-1dfb-417d-a8cd-3d2e1b5681cf`), stapled and `spctl` accepted. The
GitHub Release asset SHA-256 is
`02c1da12a7ff53d290988afed16c6d7b6a47ba834d2aefa9336f68471a4d97e5`.
The three-platform CI matrix was stopped at the release owner's request and is
not claimed green. Interactive Phase 2 acceptance and comparative UI
performance remain open because the macOS console is screen-locked.

1. Keep `RUSTIDEA_MIGRATION_AUDIT.md`, `RUSTIDEA_LEGACY_FEATURE_PARITY.md` and
   `RUSTIDEA_ARCHITECTURE.md` accurate. Do not describe placeholders as
   completed features.
2. Run `scripts/update-license-bundle.sh` after the final `Cargo.lock`. Retain
   `LICENSE`, `NOTICE`, `THIRD_PARTY_LICENSES.md`,
   `THIRD_PARTY_RUST_LICENSES.md` and all `THIRD_PARTY_SOURCES/` entries in the
   app bundle.
3. Verify `cargo fmt --all --check`, targeted tests and Clippy on the exact
   release commit. Run the macOS, Ubuntu 24.04 and Windows 2025 CI matrix and
   follow it until every job is green.
4. Confirm the new patch tag does not exist remotely. `v0.1.1` and `v0.1.2`
   must not move or be overwritten; the intended tag is `v0.2.0`.
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
