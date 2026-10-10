# ORBYN Red ASCII Worm — Design Spec

**Status:** Draft
**Date:** 2026-10-10

## Objective / why

Add a third being to ORBYN: `worm` — a single large, dark-red worm made of
dense, irregular ASCII characters, drifting slowly and hypnotically through an
otherwise empty black terminal. It joins orb and carrion as a full being in the
menu, playlist, rotation and capture paths, giving the screensaver a darker,
quieter third mood.

Reference / inspiration: "Rogue AI Agents" — <https://youtu.be/OwZrotHHn4I>,
the red ASCII worm visual that this mode recreates.

## Success criteria / definition of done

1. `orbyn --worm`, `orbyn --mode worm`, and the menu mode list all start the
   worm; it appears in `--playlist all` and rotates after carrion.
2. The worm renders as one continuous curved body drawn only from
   `# % X O + = - : .`, on a pure black background — no trail, HUD, text,
   borders or background pattern.
3. Truecolor palette is exactly dark `#65101A`, main `#D91F32`, bright
   `#FF3548`; 256 and 16 color modes fall back to nearest reds; mono renders
   glyph-density only. Characters shimmer subtly via coherent tone switching,
   with bright characters appearing intermittently throughout the body.
4. Motion is slow, fluid and continuous: a traveling wave runs head-to-tail,
   the spine may freely morph (S-curves, loops), the C rotates slowly, and
   there are no abrupt jumps. Screen edges wrap toroidally: body parts crossing
   an edge are drawn on the opposite edge in the same frame.
5. `--size` scales thickness and length together; auto size targets a body
   spanning ~70–80% of terminal height (clamped by width); body length is
   ≈ 10× thickness at the base pose; drift speed uses the same scaling as
   orb/carrion.
6. `h` toggles texture detail; detail-off drops the sparse edge fragments and
   fine characters while keeping a solid core.
7. `--snapshot --mode worm` is deterministic under a fixed `--seed`, and
   `--cast` writes a valid asciinema v2 recording.
8. The full gate passes: `cargo fmt --check`,
   `cargo clippy --all-targets -- -D warnings`, `cargo test`,
   `cargo build --release`; existing tests keep passing.
9. README documents the mode; `scripts/samples.sh` regenerates
   `assets/worm.gif`; `Cargo.toml` (and lock) are at `0.3.0`; the superseded
   modes-redo spec/plan docs are deleted.

## Scope

### In scope

- New `src/worm.rs` being module.
- New `scene::Tone` variants and a worm colorizer in `term.rs`.
- `Being::Worm` identity, `Sim::Worm` variant and all dispatch arms.
- `-worm`/`--worm` flag, MODES/KEYS help text, generic `--size` wording.
- Tests in the established inline style.
- README, `scripts/samples.sh`, `assets/worm.gif`, version bump 0.3.0.
- Deletion of the superseded modes-redo spec/plan docs.

### Non-goals (explicitly out)

- Any change to orb/carrion visuals or behavior beyond shared help wording.
- Per-being parameter sets; any new CLI option beyond `-worm`/`--worm`.
- Trail, gore, telemetry, or HUD content for the worm.
- Menu layout redesign — the worm only adds one mode row and one mark slot.
- New dependencies, Windows-specific terminal work, disk/session state.
- Animated asset generation in CI; GIFs stay a local `agg` workflow.

## Decision log

| # | Decision | Rationale | Status | Source |
| --- | --- | --- | --- | --- |
| D1 | Mode `worm`; flags `-worm`/`--worm`; window title `ORBYN // WORM`; description "Red ASCII worm curled in the dark" | Mirrors carrion's naming and flag pattern | decided | user |
| D2 | Full being: menu list, `--playlist all`, rotation, `n`, capture | Consistency; no special-casing | decided | user |
| D3 | Spec + plan live in `docs/superpowers/`, committed after each gate | Established planning flow | decided | user |
| D4 | `--size` is a uniform scale (thickness and length together); help becomes "Body size in text rows" | One option works for all beings | decided | user |
| D5 | No trail and no HUD; frame is hard-cleared of old body cells; `--trail`/`--hud` ignored (carrion ignores hud today) | Description demands a pure black frame | decided | user |
| D6 | `h` toggles texture detail; off drops sparse edge fragments/fine chars, keeps solid core | Carrion precedent for the key | decided | user |
| D7 | Discrete flat tones `WormDark`/`WormMid`/`WormHot` → `#65101A`/`#D91F32`/`#FF3548`; shimmer switches tones via coherent noise + jitter | Matches carrion's tone architecture; description's three colors | decided | user |
| D8 | Wrap-around with toroidal slide: crossing parts drawn on the opposite edge same-frame | Chosen over steer/bounce; avoids pop-in/out | decided | user |
| D9 | Spine is a node-chain follower (head drifts, nodes ease into slots behind); free morph; tapered rounded ends | Only model that supports the chosen free morph | decided | user |
| D10 | Auto size targets ~70–80% of terminal height, clamped by width | "Single, large" worm presence | decided | user |
| D11 | Body length ≈ 10× thickness at the base pose | Loose crescent with open center | decided | user |
| D12 | Drift speed scaling identical to orb/carrion | `--speed` behaves the same everywhere | decided | user |
| D13 | `Being::all() == [Orb, Carrion, Worm]` | Orb stays default; worm appended | decided | user |
| D14 | Add worm capture to `scripts/samples.sh`, `assets/worm.gif`, README third image; bump 0.2.0 → 0.3.0 | New user-facing mode deserves a sample and minor bump | decided | user |
| D15 | Delete `2026-10-09-orbyn-modes-redo` spec/plan docs | Feature shipped; docs superseded | decided | user |
| D16 | Core chars `#%XO`, edge chars `+= -:.`; texture is attached to the body and follows it | Description specifies both | decided | user (description) |
| D17 | Spine node count and wave/noise tuning constants fixed in the implementation plan to meet the described feel (subtle, slow, coherent) | Feel-level constants are plan detail, not user-facing; flag for review | assumed | recommendation |
| D18 | Fallback palettes: 256 → `52/160/203`, 16 → `31/91/91`, mono → default color with glyph density only | Nearest-red matching of the three truecolor tones | assumed | recommendation |

## Architecture

### Identity and integration

`Being` gains `Worm`: `name() == "worm"`, `aliases() == ["worm"]`, description
per D1, `all() == [Orb, Carrion, Worm]`. `from_name` and `normalize_playlist`
pick it up automatically. `Sim` gains `Worm(Box<Worm>)` with the existing
dispatched methods: `new`, `update`, `resize`, `show_too_small`,
`toggle_overlay` (→ worm detail), `toggle_pause`, `paused`, `adjust_speed`,
`grid`.

`args.rs`: add `-worm`/`--worm` beside the carrion flag; extend the MODES and
KEYS sections of `usage()` (h line becomes "telemetry (orb) / body detail
(carrion, worm)"; `--size` line becomes "Body size in text rows"). Everything
else — menu rows, marks, preview, playlist, rotation, capture, title,
`cells_for` — is generic over `Being` and needs only the new match arms.

### `src/worm.rs`

`pub struct Worm` holding: `grid`, `rng`, a fixed spine array, head heading and
wander state, `base_t` (base thickness in rows), `auto_size`, `speed`,
`detail`, `paused`, `time`, and scratch buffers (mask/dirty) reused across
frames. Public API mirrors `Carrion`.

Spine (D9, D11):

- `N` nodes (plan fixes N; carrion's `RING_N = 14` is precedent) at a fixed
  spacing of roughly half the thickness; total chain length ≈ `10 × base_t`.
- Head moves along a slowly wandering heading at the shared
  `base_speed(h, speed)` scale (extracted/duplicated per repo style); each
  following node eases toward a point at fixed spacing behind its predecessor,
  giving a smooth chain that can freely S-curve and loop.
- Positions live in toroidal coordinates (modulo w/h); no wall logic (D8).
- A traveling lateral wave and slow curvature drift modulate the chain so the
  body undulates and the C rotates/deforms gradually.
- Radius profile tapers toward both ends (rounded caps) with subtle
  noise-driven unevenness along the body.

Rendering (D5, D7, D8, D16):

- Each frame, compute the torus-expanded bounding region of the chain plus
  maximum reach; clear the previously covered region (no persistence, no
  stale pixels), then paint.
- Coverage per cell: distance to the spine polyline compared against the
  local tapered radius. Core cells get dense glyphs from `#%XO`; cells near /
  just outside the radius get sparse glyphs from `+= -: .` with noise-driven
  probability, producing granular, protruding edges. Detail-off skips the
  fringe band and fine chars.
- Tone per cell: a slow coherent noise field over body coordinates plus a
  small per-cell jitter selects `WormHot` / `WormMid` / `WormDark`. The
  noise is independent of radius, so bright characters appear intermittently
  throughout the body, not only at edges.
- Wrapping: stamping uses modulo coordinates; clearing is wrap-aware so a body
  straddling an edge leaves no residue on either side.
- Per-frame work is O(covered area) with reused buffers; no per-frame heap
  allocation in the hot path.

Non-hot methods per house style: `new`, `resize` (re-fit auto size, clamp),
`show_too_small`, `toggle_detail`, `toggle_pause`, `paused`, `adjust_speed`
(clamped 0.05–20.0), `grid`.

### `scene.rs` and `term.rs`

- `scene::Tone` gains `WormDark`, `WormMid`, `WormHot` (D7).
- `term.rs` gains `worm_cell(cell, mode)` following `carrion_cell`'s shape:
  tone → (default glyph, color); explicit `ch` always wins. Truecolor uses the
  exact D7 values; 256/16/mono use D18 fallbacks. `cells_for` and `title` gain
  their `Being::Worm` arms.
- Worm cells are written via the existing `Grid::write(v, ch)` path with
  `tone` set; no `scene` API changes beyond the enum variants.

### Small terminal behavior

Same as the other beings: `resize` re-fits; below `MIN_W`/`MIN_H` the shared
"terminal too small" message path is used. Capture defaults (100×30) render a
usable worm.

## Risks + rollback

- **Free morph + wrap drifts from the described look.** Mitigation: bounded
  curvature bias keeping the base pose C-like, node spacing constraints, and
  visual review of a seeded cast/GIF before the docs task.
- **Texture reads as noise rather than a body.** Mitigation: coverage-driven
  density with attached texture (no independent particles), coherent
  low-frequency tone noise, tests asserting lit-cell count and allowed
  glyph/tone sets.
- **Wrap seams or residue at edges.** Mitigation: toroidal stamping plus
  wrap-aware clearing, with tests placing the chain across an edge and
  asserting continuity and no stale cells.
- **Large-area per-frame rendering cost.** Mitigation: dirty-region work only,
  reused buffers, size-bounded reach; measure at 200×60 and 500×200 during
  development.
- **`Being::all()` growth breaks menu layout tests / small-terminal rows.**
  Mitigation: menu already computes rows from `all().len()`; update the tests
  that hardcode two rows in the same task that wires the being.
- **GIF regeneration requires `agg`.** Mitigation: samples script keeps its
  existing graceful error; GIF is committed after a local run.
- **Rollback:** purely additive feature — revert the worm commits, or
  `git revert` the merge; no data migration, no persisted state.

## Dependencies / ordering

- Tone/color infrastructure (T1) → worm module (T2, T3) → integration (T4) →
  docs/packaging (T5).
- The superseded-docs deletion (D15) rides the final docs task.
- No external dependencies; `agg` only for the local GIF regeneration.

## Open questions

None — all decisions are `decided` except D17/D18, which are `assumed` and
flagged for review.
