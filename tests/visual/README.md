# Visual baselines

`openidea/` holds screenshots of the pinned OpenIDEA build
(`fe3e647abb804cffbfff6bd8de42e01762d6de40`, v0.1.54). They are the product
spec for RustIdea's native renderer: layout, spacing, colour and control shapes
are compared against these images, not against an idea of how IDEA should look.

`manifest.json` is the contract. Each scenario records the exact state it shows,
the file, the window geometry, and any masks. `rustidea/` receives the matching
capture of the native app; `run-visual-regression.sh` compares the pair.

A scenario is only `captured-reference` when the intended state is visible in
the image. Missing or wrong captures are `pending-capture` and the file is not
committed — the runner treats absent files as `SKIP`, never as a pass.

## Capture procedure

1. Unlock the display. Window capture returns a stale or partial surface while
   the screen is locked or asleep, and `screencapture -x` is black. For
   GPU-rendered apps (RustIdea) a locked display yields a frame with the layout
   shapes but no text or icons, which looks like a real screenshot. Keep the
   display awake with `caffeinate -d -u -t 3600`, and note that
   `capture-window.sh` refuses to capture while
   `tests/visual/session-locked.swift` reports "locked".
   Window lookup uses the ASCII unit separator (0x1f), not tabs: bash treats a
   tab as whitespace in `IFS`, so an empty window title would silently shift
   every later field.

   To tell "the app paints no glyphs" apart from "the capture is stale", render
   GPUI text without a window:

   ```bash
   cd experiments/zed-poc
   cargo test --release -p rustidea-zed-workbench --test headless_render -- --nocapture
   ```

   The test draws a red box with black text through the headless software
   renderer and asserts both the box and the glyph pixels exist, plus a negative
   control that an empty label produces none. It passes on a locked screen, so
   it is a reliable signal for shaping and CPU rasterization.

   It is **not** a window-presentation check. The real window uses the Metal
   renderer, which also needs the `font-kit` feature on `gpui_platform`
   (`experiments/zed-poc/Cargo.toml`). With that feature missing, the window
   paints backgrounds, borders and rounded controls but no text and no icons,
   while the headless test still passes. A blank-looking workbench must be
   checked against that feature before blaming the display lock.
2. Open the pinned OpenIDEA app and the comparison project
   (`/Users/guofeng/Code/cscec3b/tg-tenant-fork`), size the window to 1600x900
   and keep it fully on screen. Record the window origin; clicks are addressed in
   screen coordinates, captures in window coordinates.
3. Reproduce the scenario state, then park the pointer somewhere neutral
   (for example the terminal area) so hover tooltips do not leak into the frame.
4. Capture and normalise:

   ```bash
   tests/visual/capture-window.sh OpenIDEA tests/visual-baselines/openidea/04-java-editor.png
   ```

   The script resolves the CGWindowID, captures it, converts to logical pixels
   (Retina 2x -> 1x, LANCZOS) and writes the size used by the comparison.
5. Update `manifest.json` in the same commit: state, file, geometry, masks.
6. Verify the image actually shows the state (open it). Do not infer from a
   successful capture command.

## Driving the UI

`cliclick` is the only input helper needed, plus one Swift helper:

| Action | Command |
| --- | --- |
| Click | `cliclick c:X,Y` |
| Double click | `cliclick dc:X,Y` |
| Right click | `cliclick rc:X,Y` |
| Drag | `cliclick dd:X,Y du:X,Y` |
| Move pointer | `cliclick m:X,Y` |
| Type text | `cliclick t:"text"` |
| Named key | `cliclick kp:esc`, `kp:enter`, `kp:arrow-left`, `kp:home`, `kp:end`, `kp:f6` |
| Hotkey | `cliclick kd:cmd t:, ku:cmd` (comma is a character, not `kp:comma`) |
| Search Everywhere | `swift tests/visual/double-shift.swift` |

Gotchas learned on OpenIDEA 0.1.54:

- `kp:` only accepts named keys; `kp:comma`, `kp:shift` and plain digits fail.
  Use `t:","`, `t:"1"` or `kd:shift`/`ku:shift`.
- Click coordinates are screen points, capture pixels are window-relative.
  `screen = window_origin + capture_pixel`; re-read the window origin before a
  batch (`swift tests/visual/window.swift OpenIDEA`), because the window can be
  moved or resized between steps.
- Search Everywhere needs two Shift taps; `tests/visual/double-shift.swift`
  posts real Shift key events with a 180 ms gap.
- Completion opens with Ctrl+Space on an existing `.` position; typing into a
  live Java file to force a popup dirties the comparison project and its undo
  history does not reliably restore LSP-applied edits. Prefer caret placement
  over typing, and `git checkout --` the file afterwards if typing was needed.
- Reload an out-of-date editor buffer with Cmd+Alt+Y (`file.reloadFromDisk`)
  instead of closing the tab; closing a dirty tab in OpenIDEA may silently
  discard it.
- The Settings window is a separate 980x720 window; capture it by title
  (`capture-window.sh OpenIDEA out.png Settings`).
- OpenIDEA keeps a per-window session including dirty buffers. If a buffer is
  stale or corrupted, quit the app, restore the file from git, and reopen the
  project rather than fighting the buffer.

## Validation

```bash
python3 tests/visual/validate-manifest.py
```

Checks the contract without a GUI: unique and contiguous ids, real PNGs whose
pixels match the declared geometry, no two scenarios sharing an image (a copy is
not a capture), no shipped image behind a `pending-capture` status, and matching
OpenIDEA commits in `manifest.json` / `environment.json`. CI runs it on hosted
runners for every change under `tests/visual-baselines/`.

## Comparison

```bash
tests/visual/run-visual-regression.sh
```

writes `openidea.png`, `rustidea.png`, `diff.png` and `metrics.json` per scenario
(MAE/RMSE, ffmpeg SSIM, target SSIM >= 0.98) and skips scenarios without a
native capture. SSIM is auxiliary: the acceptance rule is <= 2 logical pixels of
container/control deviation, checked visually on the pair.
