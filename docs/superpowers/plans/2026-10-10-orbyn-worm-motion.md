# ORBYN Worm Motion Rework — Implementation Plan

**Status:** Approved (final, 2026-10-10)
**Date:** 2026-10-10
**Spec:** `docs/superpowers/specs/2026-10-10-orbyn-worm-motion-design.md`
**Branch:** `worm-mode` (worktree `/home/benyamin/dev/orbyn/.worktrees/worm-mode`)

## Global constraints (from the spec)

- Work happens on branch `worm-mode` only; nothing merged or pushed until the
  user decides after visual review.
- Version stays `0.3.0`; zero new dependencies; MSRV 1.70; no comments in
  source; no heap allocation in the hot path.
- Rendering, tones, glyphs, taper, wrap stamping/clearing, CLI, menu, capture,
  and palette fallbacks are untouched.
- Full gate after every task: `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test`,
  `cargo build --release`.
- Commit style: `feat(worm): …`, `docs(release): …`.
- Feel constants below are fixed per D15/old-D17 and re-tunable after the
  user's visual pass; changing them later is expected, not a plan deviation.

## Architecture recap

Replace only the spine evolution in `src/worm.rs`:

1. **Trail.** Ring buffer of the head's recent toroidal path, one point per
   `TRAIL_STEP × base_t` rows moved since the last recorded point, each sample
   storing the wrap-aware segment length from its predecessor. No heap.
2. **Steering.** `heading` eases toward `desired = wander(dist) + weave(dist)`
   under a max turn rate; `dist` is cumulative traveled distance, so the weave
   wavelength is speed-invariant (D4) with a wavelength of
   `WEAVE_LEN × body_len`.
3. **Follow-the-leader.** Node `i` sits at arc distance `i × spacing` behind the
   head along the trail (same `spacing = LENGTH_RATIO × base_t / SPINE_N` as
   today), interpolated between samples; stored modulo w/h so `render`,
   `body_field`, `tone_at`, and wrap clearing operate unchanged.
4. **Avoidance (D8/D9).** Repulsion from trail samples older than
   `body_len + AVOID_MARGIN × base_t`, sensed around a lookahead point; steers
   the desired heading away, clamped by the same turn cap. When the nearest
   eligible sample stays below `CONTACT × base_t` for `ESCAPE_SEC`, the escape
   valve suppresses avoidance until clear, then re-arms.

Removed: `NODE_K`, `CHAIN_CURL`, `WAVE_AMP/WAVE_SPEED/WAVE_K`, `wander`/`turn`
fields and their re-roll timer.

## Constants (new; values tunable per D15)

| Constant | Value | Unit | Meaning |
| --- | --- | --- | --- |
| `TRAIL_CAP` | 512 | samples | ring buffer capacity |
| `TRAIL_STEP` | 0.2 | × base_t | trail sample spacing in rows |
| `WEAVE_AMP` | 0.30 | rad | heading oscillation amplitude (subtle) |
| `WEAVE_LEN` | 1.25 | × body_len | weave wavelength |
| `WANDER_AMP` | 1.4 | rad | max wander heading excursion |
| `WANDER_LEN` | 3.0 | × body_len | wander noise feature size |
| `TURN_RATE` | 0.15 | rad/s | max steering rate (gentle arcs) |
| `LOOKAHEAD` | 1.5 | × base_t | avoidance sensing point ahead of head |
| `SENSE_RADIUS` | 5.0 | × base_t | early sensing radius (D9) |
| `AVOID_MARGIN` | 1.5 | × base_t | trail arc after the body excluded from sensing |
| `CONTACT` | 2.0 | × base_t | centerline distance counted as conflict |
| `RELEASE` | 2.5 | × base_t | distance at which conflict timer resets |
| `ESCAPE_SEC` | 5.0 | s | conflict duration before escape valve fires |
| `ESCAPE_RELEASE` | 2.0 | × SENSE_RADIUS | clearance distance to re-arm avoidance |

Keep: `SPINE_N = 24`, `LENGTH_RATIO = 10.0`, `FRINGE_MARGIN`, `SPAN_RATIO`,
`BREATHE_RATE`, all texture constants, and the speed scale `base_speed(h, mult)`.

## Interfaces (private unless noted)

- `Worm` state: `dist: f64`, `heading: f32`, `trail_x/trail_y: [f32; TRAIL_CAP]`,
  `trail_seg: [f32; TRAIL_CAP]`, `trail_len: usize`, `trail_head: usize`,
  `blocked_time: f32`, `escaping: bool`, `escapes: u32`; existing public API
  unchanged.
- `fn desired_heading(&self) -> f32` — wander + weave over `dist`.
- `fn advance_heading(&mut self, dt: f32)` — rate-limited move toward either
  the avoidance-biased heading (when repulsion is significant) or
  `desired_heading`.
- `fn record_trail(&mut self)` — append head position when moved ≥ step.
- `fn place_nodes(&mut self, w: usize, h: usize)` — single walk outward from the
  head placing all 24 nodes; extrapolates straight behind the oldest sample
  when history is short (spawn/extrapolation path).
- `fn repulsion(&self, w: usize, h: usize) -> (f32, f32)` — wrap-aware
  sum over eligible samples near the lookahead point.
- `fn update_conflict(&mut self, dt, w, h)` — conflict timer, escape
  activation/re-arm, `escapes` counter.
- `fn snap(&mut self)` — unchanged entry point; now prefills the trail with a
  straight line behind the head along `heading` at the current step, so the
  straight-chain tests keep their meaning (D10).

## Task 1 — Follow-the-leader serpentine core (no avoidance yet)

**Files:** `src/worm.rs` only.

**Changes:**
- Add trail state + constants above (except avoidance ones may be added in
  Task 2), `record_trail`, `desired_heading`, `advance_heading`, `place_nodes`
  (trail-walk sampling without repulsion).
- Rework `update`: advance `dist` by the actual step length, steer heading
  toward `desired_heading`, move head, record trail, place nodes, render.
- Rework `snap` to prefill a straight trail (D10) reproducing today's straight
  chain within interpolation error.
- Rework `resize`: re-wrap trail points; body-length growth is covered by
  `place_nodes` extrapolation.
- Delete the standing-wave/easing constants and `wander`/`turn` fields.
- Adapt existing tests only where steering legitimately changes trajectories
  (expected: `head_wraps_across_left_edge`,
  `texture_stays_continuous_across_edge_crossing` — set `heading` directly and
  start near the edge so the crossing happens before steering matters; keep
  their original assertions).

**New tests (in `src/worm.rs` tests module):**
1. `body_nodes_lie_on_head_path` — record head positions each frame; after
   warmup, every node is within ~0.5 rows of the recorded head-history
   polyline. Proves follow-the-leader.
2. `weave_is_present_and_subtle` — over 30 s at default speed, the heading
   residual (heading minus its 10 s moving average) has peak-to-peak in
   0.15–1.2 rad and at least 2 sign changes. Subtle, but visible.
3. `weave_wavelength_is_speed_invariant` — same seed at speed 1.0 and 2.0
   (`adjust_speed`), track heading-residual extrema against traveled distance;
   mean inter-extremum distance within ±30% between runs.
4. `heading_does_not_spin_monotonically` — over 600 s simulated, the maximum
   net |Δheading| over any sliding 60 s window is < 4 rad (the old model turns
   at a sustained rate, far more than this).
5. Existing `nodes_move_continuously_modulo` keeps `max_step < 3.0`; tighten to
   `< 2.0` if the measured value has margin (record measured value in the
   commit/report).

**Verification:** `cargo test worm::` green; then the full gate. Manual sanity
(controller, allowed): `./target/release/orbyn --snapshot --mode worm --seed 7
--size 8` renders a coherent body; run the PTY for 5 s under `script -qec
'./target/release/orbyn -worm --fps 30' /dev/null` with no panic strings.

**Commit:** `feat(worm): follow-the-leader serpentine motion`

## Task 2 — Self-contact avoidance and escape valve

**Files:** `src/worm.rs` only.

**Changes:**
- Add `repulsion` (eligible samples: arc > `body_len + AVOID_MARGIN × base_t`,
  within `SENSE_RADIUS` of the lookahead point; weight `(1 − d/R)²`; direction
  in aspect-corrected space; scan capped to the most recent 512 eligible
  samples).
- Bias `advance_heading` toward the repulsion direction when its magnitude is
  above a small epsilon; the turn cap stays authoritative (no snaps).
- Add `update_conflict`: accumulate `blocked_time` while the nearest eligible
  sample is below `CONTACT × base_t`; reset when it rises above `RELEASE`;
  at `ESCAPE_SEC` set `escaping = true`, increment `escapes`; clear
  `escaping` once nearest clearance exceeds `ESCAPE_RELEASE`; while
  `escaping`, repulsion is ignored.
- Keep everything deterministic (state derives only from update inputs).

**New tests:**
1. `no_self_contact_without_escape` — seeds {5, 23, 41}, 900x36, 20 000 frames
   at 1/30: whenever the minimum non-adjacent centerline distance (arc
   separation > `body_len + AVOID_MARGIN × base_t`) drops below
   `CONTACT × base_t`, `escaping` must be true. Assert exactly that invariant
   (no assertion about the escape count); record each seed's escape count in
   the task report.
2. `escape_valve_unsticks_a_boxed_worm` — fixture: prefill the trail with a
   small closed circle of radius ~1.5 × base_t around the head (poking internals
   is fine in-module), then update for ~8 simulated seconds: `escapes >= 1`
   within ~6 s, the worm crosses (its position leaves the circle), and
   `escaping` returns to false afterward.
3. `escape_counter_is_deterministic` — extend the existing
   `seeded_grids_are_deterministic` run to 2 000 frames and additionally assert
   `a.escapes == b.escapes` and `a.escaping == b.escaping` at the end.

**Verification:** `cargo test worm::` green; full gate. Manual: run a 30 s PTY
session and a 10 000-frame seeded cast; confirm no visible body overlap in the
cast via `orbyn --cast` and eyeballing the file's tail (controller check).

**Commit:** `feat(worm): steer around its own body with escape valve`

## Task 3 — Docs, sample, and local install

**Files:** `README.md`, `assets/worm.gif`, `Cargo.toml`/`Cargo.lock` untouched.

**Changes:**
- README: worm paragraph (lines ~20–24) and GIF alt text (line ~14) switch to
  serpentine/slither wording; keep the three-color/shimmer/wrap description.
- Regenerate `assets/worm.gif` via `scripts/samples.sh` (agg 1.9.0 at
  `~/.cargo/bin/agg`); commit the new GIF; verify the other GIFs are
  byte-identical to the previous commit's (restore if the script rewrote them).
- Reinstall for user testing: `cargo install --path . --locked --root
  /home/benyamin/.local --force`.

**Verification:** `bash scripts/samples.sh` completes; `file assets/worm.gif`
shows GIF89a; README renders correctly (`cat` check of the edited lines);
`orbyn -V` prints `orbyn 0.3.0`; full gate still green.

**Commit:** `docs(release): serpentine motion docs and worm sample`

**After Task 3 (out of plan execution, user gate):** user visually reviews
`orbyn -worm` and the GIF; then the merge/finish decision (or a tuning round
that only changes the constants table above).

## Dependencies / ordering

- Task 1 → Task 2 (avoidance layers on trail + steering) → Task 3.
- Gate after every task; Task 3 always last (its GIF must show the final feel).

## Risks + rollback

- **Steering constants feel wrong on first run.** Expected: constants are one
  table in `worm.rs`, flagged D15; a tuning round touches only that table and
  re-runs tests (bounds in tests are deliberately loose).
- **Avoidance creates oscillation (dithering between two sides).** Mitigation:
  repulsion weighs nearby samples smoothly and the turn cap is slow; the
  long-run clearance test would surface it.
- **Adapted edge tests mask a real regression.** Mitigation: keep their
  original assertions untouched; only setup may change.
- **GIF regen rewrites orb/carrion assets (it did before).** Mitigation:
  restore untouched GIFs from `git` before committing.
- **Rollback:** revert the three commits on `worm-mode`; repo is otherwise
  untouched, nothing pushed.

## Open questions

None.
