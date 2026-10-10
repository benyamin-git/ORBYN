# ORBYN Red ASCII Worm Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the `worm` being — a large, dark-red ASCII worm drifting over a
pure black frame with toroidal wrapping, coherent red shimmer, and a detail
toggle — as a full being in the menu, playlist, rotation and capture paths.

**Architecture:** New `src/worm.rs` sim (node-chain spine follower, tapered
tube, distance-field ASCII texture, wrap-aware clearing) plus three new
`scene::Tone` variants and a `term.rs` worm colorizer. `Being::Worm` and
`Sim::Worm` join the existing concrete enum + match dispatch; menu, rotation,
playlist, preview and capture are generic over `Being` and need only new arms
and updated two-being assumptions.

**Tech Stack:** Rust (edition 2021), zero dependencies, MSRV 1.70, termios via
`stty`, alternate screen, inline `#[cfg(test)]` suites.

**Spec:** `docs/superpowers/specs/2026-10-10-orbyn-worm-design.md`

## Global Constraints

- Zero new dependencies.
- MSRV 1.70 (do not introduce syntax/APIs above it).
- No comments in source code.
- Session-only config: no disk writes.
- Capture (`--snapshot`/`--cast`) stays headless, single being, deterministic
  under a fixed seed; rotation configs rejected at parse.
- Uniform gate after every task: `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test`,
  `cargo build --release`.
- Version stays `0.2.0` until the final task, then `0.3.0`.
- Existing suite keeps passing (tests are adjusted only where the worm
  legitimately changes a two-being assumption).
- Temporary `#[allow(dead_code)]` markers are permitted for not-yet-wired
  items (repo precedent `5ad8100`); every one is removed in Task 4.
- No new CLI options beyond `-worm`/`--worm`; no changes to orb/carrion
  visuals.

## Review Focus

- Palette: truecolor uses the exact spec values; 256/16/mono fall back without
  leaving the red family; mono still distinguishes tones by glyph. → T1
- Wrap: body crossing an edge is drawn on the opposite edge the same frame,
  and clearing never leaves stale cells on either side. → T2
- No trail: every frame clears the previous body region; no persistence, no
  background marks. → T2/T3
- Texture discipline: only `# % X O + = - : .` glyphs; only the three worm
  tones; bright tones appear both inside the body and at its edges. → T3
- `h` detail-off drops the sparse fringe; the solid core remains. → T3
- `--size` scales thickness and length together; auto size ≈ 70–80% of screen
  height clamped by width; length ≈ 10× thickness. → T2
- Live params carry into worm like any being (speed clamp 0.05–20.0). → T2/T4
- Menu with three beings: list rows, marks `[bool; 3]`, START reachable via
  four Downs; no panic on small terminals. → T4
- Capture determinism for worm under a fixed seed. → T5
- README/samples/version coherent; superseded docs deleted. → T6

## File Structure

- Create: `src/worm.rs` (`Worm`, spine, render, texture, tests)
- Modify: `src/scene.rs` (`Tone::{WormDark, WormMid, WormHot}`)
- Modify: `src/term.rs` (`worm_cell`, palette consts, `cells_for`/`title` arms)
- Modify: `src/being.rs` (`Being::Worm`, `Sim::Worm`, identity tests)
- Modify: `src/args.rs` (`-worm`/`--worm`, usage text, tests)
- Modify: `src/main.rs` (`mod worm;`, allow-removal; order-driven tests)
- Modify: `src/menu.rs` (marked array width + layout tests)
- Modify: `src/capture.rs` (worm determinism tests)
- Modify: `README.md`, `scripts/samples.sh`, `Cargo.toml`, `Cargo.lock`
- Delete: `docs/superpowers/specs/2026-10-09-orbyn-modes-redo-design.md`,
  `docs/superpowers/plans/2026-10-09-orbyn-modes-redo.md`
- Unchanged: `src/globe.rs`, `src/hud.rs`, `src/rng.rs`, `src/rotation.rs`,
  `src/carrion.rs`

---

### Task 1: Worm tone palette and colorizer

**Files:**
- Modify: `src/scene.rs`, `src/term.rs`
- Test: inline `#[cfg(test)]` in `src/term.rs`

**Interfaces:**
- Consumes: existing `scene::{Cell, Tone}`, `term::{Cell, Color, ColorMode}`.
- Produces: `Tone::WormDark`, `Tone::WormMid`, `Tone::WormHot` (each
  temporarily `#[allow(dead_code)]`). In `term.rs`:
  `pub const WORM_DARK_RED: Color = Color::Rgb(0x65, 0x10, 0x1A);`
  `pub const WORM_MID_RED: Color = Color::Rgb(0xD9, 0x1F, 0x32);`
  `pub const WORM_HOT_RED: Color = Color::Rgb(0xFF, 0x35, 0x48);`
  `pub const WORM_DARK_CH: char = ':';`
  `pub const WORM_MID_CH: char = '#';`
  `pub const WORM_HOT_CH: char = '%';`
  `fn worm_cell(cell: &crate::scene::Cell, mode: ColorMode) -> Cell` (private,
  temporarily `#[allow(dead_code)]`). Fallbacks: Ansi256 → `Indexed(52)/Indexed(160)/Indexed(203)`;
  Ansi16 → `Basic(31)/Basic(91)/Basic(91)`; Mono → `Color::Default`.
  A lit `Tone::Blank` cell reads as WormMid; a blank cell stays `Cell::BLANK`.

- [ ] **Step 1: Write the failing tests** — in `term.rs` tests, mirroring the
  carrion tests: `worm_truecolor_palette_is_exact` (three tones → the three
  `WORM_*_RED` consts), `worm_fallback_palette_stays_in_range` (every mode and
  brightness yields only that mode's allowed worm colors),
  `worm_glyphs_distinguish_tone_in_mono` (three distinct default glyphs),
  `worm_lit_untinted_cell_reads_as_mid` (v > 0, tone Blank, explicit `X` keeps
  `X` and mid color), `worm_blank_cell_stays_blank`.
- [ ] **Step 2: Run to verify failure** — `cargo test worm_` fails
  (symbols missing).
- [ ] **Step 3: Implement** — add the three variants to `scene.rs` (each
  `#[allow(dead_code)]` for now); add the consts and `worm_cell` to `term.rs`,
  modeled on `carrion_cell`: blank check (`v <= 0.0 && tone == Blank` → blank),
  Blank-and-lit → WormMid, then tone → (default glyph, color), explicit
  `cell.ch` always wins. Mark every new `term.rs` item `#[allow(dead_code)]`.
- [ ] **Step 4: Run to verify pass** — `cargo test worm_` passes; full suite
  still green.
- [ ] **Step 5: Gate + commit** — run the full gate;
  `git add src/scene.rs src/term.rs`;
  `git commit -m "feat(term): worm red tone palette and colorizer"`.

### Task 2: Spine motion, toroidal wrap, silhouette render

**Files:**
- Create: `src/worm.rs`
- Modify: `src/main.rs` (add `#[allow(dead_code)] mod worm;` after `mod term;`)
- Test: inline `#[cfg(test)]` in `src/worm.rs`

**Interfaces:**
- Consumes: `args::Config`, `rng::Rng`, `scene::{Grid, Tone}`, `crate::ASPECT`,
  `main::MIN_W`-style small-screen conventions.
- Produces: `pub struct Worm`; `pub fn new(cfg: &Config, w: usize, h: usize) -> Self`;
  `pub fn update(&mut self, dt: f32, w: usize, h: usize)`;
  `pub fn resize(&mut self, w: usize, h: usize)`;
  `pub fn show_too_small(&mut self, w: usize, h: usize)`;
  `pub fn toggle_detail(&mut self)`; `pub fn toggle_pause(&mut self)`;
  `pub fn paused(&self) -> bool`; `pub fn adjust_speed(&mut self, ratio: f32)`;
  `pub fn grid(&self) -> &Grid`. Constants: `SPINE_N: usize = 24`;
  `LENGTH_RATIO: f32 = 10.0`; auto thickness
  `(h as f32 * 0.135).clamp(1.5, 12.0)` rows; explicit `--size` sets thickness
  directly (1–60) and length follows `LENGTH_RATIO × thickness`; both clamped
  by `fit_thickness(t, w, h)` so the base C fits inside the screen.

**Behavior to implement:**
- Head drifts on a slowly wandering heading (carrion-style turn noise + timer)
  at the shared height-proportional `base_speed(h, speed)` scaling (local copy,
  as carrion does). Subsequent nodes ease toward a slot at fixed spacing
  (`LENGTH_RATIO × t / SPINE_N`) behind their predecessor, holding a smooth
  chain that can freely S-curve. Positions are kept toroidal via `rem_euclid`.
- A traveling lateral wave modulates the chain so the body undulates; a slow
  curvature bias keeps the resting pose a loose C. Per-node radius tapers
  slightly toward rounded ends and varies subtly over time.
- Render (silhouette only this task): coverage = distance from cell to the
  spine polyline ≤ local radius (x distances scaled by `ASPECT`); paint covered
  cells `WormMid` with the `#` glyph via `grid.paint`.
- Clearing: track the previous frame's covered bounding box; on each render
  first clear that region with a wrap-aware helper that splits a box crossing
  an edge into up to four sub-rects, then repaint. No persistence, no trail.
- `show_too_small` mirrors the carrion implementation (clear grid, centered
  message). `resize` re-fits auto thickness and clamps. `adjust_speed` clamps
  0.05–20.0.

- [ ] **Step 1: Write the failing tests** — inline, mirroring carrion's suite:
  `nodes_stay_in_torus_bounds`, `nodes_move_continuously_modulo` (per-frame
  torus-space step below threshold; no teleports),
  `head_wraps_across_left_edge` (head near x = 0 moving left wraps to near w,
  body lit on both sides, no panic), `old_body_cells_get_cleared` (move the
  head far, old region blank), `body_never_collapses` (lit count stays above a
  floor over thousands of frames), `resize_shrinks_body_into_bounds`,
  `show_too_small_writes_message`, `speed_adjustment_is_clamped`,
  `pause_and_detail_toggle`, `seeded_grids_are_deterministic` (two sims, same
  seed, identical grid bytes after N frames), `wrap_rects_split_at_edges`
  (helper unit test).
- [ ] **Step 2: Run to verify failure** — `cargo test worm` fails (module
  missing).
- [ ] **Step 3: Implement `src/worm.rs`** and wire `#[allow(dead_code)] mod
  worm;` into `main.rs`. Copy the private `hash`/`noise2` helpers from
  `carrion.rs` (repo precedent: duplicates per module).
- [ ] **Step 4: Run to verify pass** — `cargo test worm` passes; full suite
  green.
- [ ] **Step 5: Gate + commit** — `git add src/worm.rs src/main.rs`;
  `git commit -m "feat(worm): spine motion with toroidal wrap and silhouette render"`.

### Task 3: Ember ASCII texture, tone shimmer, detail toggle

**Files:**
- Modify: `src/worm.rs`
- Test: inline `#[cfg(test)]` in `src/worm.rs`

**Interfaces:**
- Consumes: T2 `Worm` internals and `update`/`render`.
- Produces: no API change — render gains per-cell glyph and tone selection.
  Allowed glyphs: core `#`, `%`, `X`, `O`; fringe `+`, `=`, `-`, `:`, `.`.
  Tones: `WormDark`/`WormMid`/`WormHot` via a slow coherent noise field over
  body coordinates (independent of radius so highlights appear throughout the
  body), plus a small per-cell jitter that changes over time. Detail-off
  (`toggle_detail`) removes the sparse fringe band and the fine glyphs,
  leaving the dense core only.

- [ ] **Step 1: Write the failing tests** — `only_allowed_glyphs` (every lit
  cell char is in the nine-glyph set), `only_worm_tones` (every lit cell tone
  is one of the three worm tones), `fringe_present_when_detail_on`
  (`+ = - : .` cells exist while detail is on),
  `fringe_absent_when_detail_off` (zero fringe glyphs; core still lit),
  `hot_tones_appear_across_the_body` (over many frames, WormHot cells exist
  both in the body interior and in cells adjacent to blank), and
  `shimmer_changes_over_time` (tone histogram or per-cell tones differ between
  two sampled frames under a fixed seed).
- [ ] **Step 2: Run to verify failure** — `cargo test worm` fails on the new
  assertions.
- [ ] **Step 3: Implement** — extend the render pass: sample coherent noise at
  body coordinates (slow time evolution) for tone selection with thresholds,
  pick glyphs from the core/fringe sets by local density noise, make the
  silhouette edge granular by noise-thresholded fringe cells just outside the
  core radius, and gate the fringe on `detail`.
- [ ] **Step 4: Run to verify pass** — `cargo test worm` passes; full suite
  green; visually sanity-check one frame with
  `cargo run -- --snapshot --mode worm --seed 7` after T4 (not required here).
- [ ] **Step 5: Gate + commit** — `git add src/worm.rs`;
  `git commit -m "feat(worm): ember ASCII texture, tone shimmer, and detail toggle"`.

### Task 4: Wire worm into being, terms, menu, and CLI

**Files:**
- Modify: `src/being.rs`, `src/term.rs`, `src/scene.rs`, `src/main.rs`,
  `src/args.rs`, `src/menu.rs`
- Test: inline suites in each

**Interfaces:**
- Consumes: T1 tones/colorizer; T2/T3 `Worm`; existing dispatch patterns.
- Produces:
  - `Being::Worm`; `name() == "worm"`; `aliases() == ["worm"]`;
    `description() == "Red ASCII worm curled in the dark"`;
    `all() -> [Self; 3] == [Orb, Carrion, Worm]`.
  - `Sim::Worm(Box<Worm>)` with all existing dispatched methods.
  - `term::cells_for` `Worm` arm → `worm_cell`; `term::title(Worm)` →
    `b"\x1b]0;ORBYN // WORM\x1b\\"`.
  - `args.rs`: `"-worm" | "--worm"` sets `being = Some(Being::Worm)`; usage
    gains the worm MODES line, `h` line becomes
    `toggle telemetry (orb) / body detail (carrion, worm)`, `--size` line
    becomes `Body size in text rows`.
  - All temporary `#[allow(dead_code)]` markers removed (`scene.rs` variants,
    `term.rs` worm items, `main.rs` `mod worm;`).

- [ ] **Step 1: Write/update the failing tests**
  - `being.rs`: `identity_table_is_complete` → `[Orb, Carrion, Worm]`;
    `aliases_are_expected` + worm; `from_name_accepts...` + `worm`/`WORM`;
    `sim_new_worm_builds_grid`; keep `serpent`/`dragon`/`""` rejected.
  - `term.rs`: `title_matches_being` + worm; `cells_for_matches_being` +
    worm paints `FLESH`-free `WORM_MID_RED`.
  - `args.rs`: `worm_flag_parses_in_both_forms` (`-worm`, `--worm`);
    `--playlist all` now yields three beings (existing assertions use
    `Being::all()` and keep passing); usage tests assert the worm name, the
    updated `h` and `--size` lines.
  - `menu.rs`: `MenuState.marked` becomes `[bool; 3]`; update
    `default_state_shape`, `down_advances_then_enters_start` and
    `start_button_highlights_only_when_focused` (four Downs to reach START),
    `marks_track_state` (three-wide arrays),
    `modes_rows_are_left_aligned_in_top_left_box` (+ `○ worm` row),
    `hovered_being_tracks_list_index` (index 3 → Worm). Run the full suite and
    fix any other two-being assumption that surfaces.
- [ ] **Step 2: Run to verify failure** — the updated tests fail against the
  current code.
- [ ] **Step 3: Implement the wiring** — add the `Being` variant and update
  the `[Self; 2]` signature to `[Self; 3]`; add `Sim::Worm` and its nine
  dispatch arms; add the `term.rs` arms; remove all temporary allows; add the
  CLI flag and usage lines; adjust the menu tests and any layout assumption.
- [ ] **Step 4: Run to verify pass** — full suite green; then manual smoke:
  `cargo run -- --snapshot --mode worm --seed 7 | head -c 200` prints a
  non-empty ANSI frame; `cargo run -- -worm --snapshot --seed 7` likewise.
- [ ] **Step 5: Gate + commit** — `git add src/being.rs src/term.rs src/scene.rs src/main.rs src/args.rs src/menu.rs`;
  `git commit -m "feat(being): wire worm into being, terms, menu, and CLI"`.

### Task 5: Capture determinism and end-to-end smoke

**Files:**
- Modify: `src/capture.rs` (tests only)
- Test: inline in `src/capture.rs`

**Interfaces:**
- Consumes: T4-compiled mode.
- Produces: `worm_snapshot_is_deterministic` (two `Sim::new(Being::Worm, …)`
  runs, same seed, byte-identical snapshots; output has one line per row and
  contains `38;2;`), and a worm `record` smoke test asserting the asciicast v2
  header and events (mirror the existing orb tests). Manual checklist, not
  code: run `orbyn --worm` in a terminal and verify against the spec — single
  curved worm, granular edges, three reds, pure black frame, seamless wrap,
  `h` toggles fringe, `+`/`-` speed, `space` pause, resize; then
  `orbyn -worm --cast /tmp/worm.cast --duration 5` and inspect or play it.

- [ ] **Step 1: Write the failing tests** — the two tests above.
- [ ] **Step 2: Prove the tests bite** — temporarily assert the two runs
  differ (or flip a seed) and confirm the test fails, then restore; a
  determinism test that cannot fail is worthless.
- [ ] **Step 3: Implement** — add only the tests; no production change is
  expected.
- [ ] **Step 4: Run to verify pass** — `cargo test capture` green; run the
  manual checklist and report the result back.
- [ ] **Step 5: Gate + commit** — `git add src/capture.rs`;
  `git commit -m "test(capture): worm snapshot determinism"`.

### Task 6: Docs, sample GIF, version 0.3.0, doc cleanup

**Files:**
- Modify: `README.md`, `scripts/samples.sh`, `Cargo.toml`, `Cargo.lock`
- Delete: `docs/superpowers/specs/2026-10-09-orbyn-modes-redo-design.md`,
  `docs/superpowers/plans/2026-10-09-orbyn-modes-redo.md`

**Interfaces:**
- Consumes: everything above.
- Produces: README describing three visuals (intro line, worm paragraph,
  `assets/worm.gif` as a third image, `-worm` in the direct-run examples);
  `scripts/samples.sh` gains `SIZE_WORM` (default 12), `SEED_WORM` (default
  23) and a `capture worm "$SEED_WORM" "$SIZE_WORM" --worm` line; version
  `0.3.0` in `Cargo.toml` with `Cargo.lock` updated by the build.

- [ ] **Step 1: Update README** — load the `readme` skill and follow it for
  every README edit; match the existing tone and structure (intro, image row,
  usage, options table if the worm adds none, samples section).
- [ ] **Step 2: Update `scripts/samples.sh`** — add the variable defaults and
  the worm capture line after carrion; keep the existing style.
- [ ] **Step 3: Bump the version** — `Cargo.toml` → `0.3.0`; run
  `cargo build --release` so `Cargo.lock` records it; confirm with
  `./target/release/orbyn -V` printing `orbyn 0.3.0`.
- [ ] **Step 4: Regenerate the GIF** — run `scripts/samples.sh` (requires
  `agg`); if `agg` is unavailable, stop and ask the user rather than committing
  without `assets/worm.gif`. Inspect the GIF against the spec before staging.
- [ ] **Step 5: Delete the superseded docs** —
  `git rm docs/superpowers/specs/2026-10-09-orbyn-modes-redo-design.md docs/superpowers/plans/2026-10-09-orbyn-modes-redo.md`.
- [ ] **Step 6: Gate + commit** — `git add README.md scripts/samples.sh Cargo.toml Cargo.lock assets/worm.gif`;
  `git commit -m "docs(release): worm mode docs, sample, and v0.3.0"`.

---

## Dependencies / ordering

T1 → T2 → T3 → T4 → T5 → T6. T2/T3 are independently testable via
`cargo test worm` even before T4 wires the mode. T4 removes every temporary
`#[allow(dead_code)]`. T6 depends on a green T5.

## Risks + rollback

- **Worm look drifts from the description** — review `--snapshot` output and
  the T6 GIF against the spec; the render constants (wave amplitude, noise
  frequencies, taper) live only in `src/worm.rs` and are cheap to tune.
- **Wrap seams or stale pixels** — covered by `head_wraps_across_left_edge`,
  `old_body_cells_get_cleared`, and `wrap_rects_split_at_edges`.
- **Menu assumptions** — three beings change START reachability and marks
  width; the T4 tests enumerate the known spots and the full suite catches the
  rest.
- **GIF workflow** — requires local `agg`; T6 stops and asks if missing.
- **Rollback** — additive feature; revert the worm commits in order, or
  `git revert` the final merge. No persisted state, no migration.

## Open questions

None. The two assumed decisions (D17 tuning constants, D18 fallback palettes)
are fixed concretely in T2/T3 and T1 respectively, and are flagged in the spec
for review.
