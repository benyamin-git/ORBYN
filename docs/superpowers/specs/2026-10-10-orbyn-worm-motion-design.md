# ORBYN Worm Motion Rework — Design Spec

**Status:** Approved (final, 2026-10-10); addendum applied and approved after
visual pass (2026-10-14)
**Date:** 2026-10-10
**Supersedes by reference:** `docs/superpowers/specs/2026-10-10-orbyn-worm-design.md`
success criterion 4 and decisions D9/D11 there (spine model and base pose motion);
everything else in that spec stands (palette, glyphs, detail toggle, integration,
capture determinism, size/auto-size rules, asset/version handling).

## Objective / why

The shipped worm's movement reads as unnatural: the head drives in constant-rate
circles, every body node adds a permanent one-sided curl, the "wave" is a standing
pattern relative to the body, and steering re-rolls land as kinks. This rework
replaces the node-chain-pulled-by-head model with a true follow-the-leader
serpentine: the head's steering weaves smoothly, the body follows the head's exact
carved path, and the worm steers around its own body instead of raveling through
it — producing a calm, living glide that still shimmers and wraps exactly as today.

## Success criteria / definition of done

1. Movement reads as a living serpentine glide: no constant-turn circling, no
   permanent one-sided coil, no visible kinks or abrupt direction changes at
   any moment (wander is smooth noise; no timer re-rolls).
2. The body follows the head's carved path; the visible undulation travels
   head-to-tail with subtle amplitude, and its wavelength stays stable when
   `--speed` changes (weave phase is tied to distance traveled).
3. Self-contact: during normal motion, no two non-adjacent body parts come
   within ~2 body-thicknesses of each other (clearance guaranteed by early
   sensing at ~4–6 thicknesses and a gentle turn-rate cap). After ~4–6 seconds
   with no available clearance, the worm takes the cleanest crossing (escape
   valve) and re-arms avoidance; it never becomes permanently stuck.
4. Glide speed stays constant on the shared orb/carrion scale; `--speed` and
   `+`/`-` still work; toroidal wrap remains seamless with zero stale cells;
   seeded runs remain fully deterministic (run and `--snapshot`/capture paths).
5. Naturalness is verified by behavioral tests plus the user's visual review of
   a regenerated GIF/cast before merge:
   - continuity test still holds (bounded per-frame node step),
   - new: non-adjacent clearance holds over long runs,
   - new: a self-crossing only ever occurs via the escape valve (or not at all
     in the covered runs),
   - new: visible weaving exists and its phase advances with distance traveled,
   - new: wrap continuity and no-residue behavior unchanged.
6. All previously passing tests, the full gate (`cargo fmt --check`,
   `cargo clippy --all-targets -- -D warnings`, `cargo test`,
   `cargo build --release`) pass; palette, glyph set, detail toggle, size rules,
   menu/playlist/capture integration, and `0.3.0` are unchanged.
7. README worm paragraph and GIF alt text describe the serpentine glide;
   `assets/worm.gif` is regenerated via `scripts/samples.sh` before merge; the
   installed `~/.local/bin/orbyn` is refreshed for the user's test.

## Scope

### In scope

- `src/worm.rs` motion core: replace node-chain easing (`NODE_K`, `CHAIN_CURL`,
  `WAVE_*`, wander re-roll timer) with trail-following head + body sampling,
  distance-based wander/weave steering, rate-limited turning, and self-contact
  avoidance with escape valve.
- New trail buffer (fixed capacity, no heap) and arc-length sampling helpers.
- Tests in `src/worm.rs` updated/extended per success criterion 5; `snap()`
  keeps laying a straight chain behind the head so edge/clear tests remain
  meaningful.
- README worm wording + GIF alt text; `assets/worm.gif` regeneration; local
  reinstall for user testing.

### Non-goals (explicitly out)

- Any change to rendering, tone/shimmer, glyphs, fringe, tapering, or wrap
  stamping/clearing logic (`render`, `body_field`, `tone_at`, `core_glyph`,
  `fringe_*`, `node_radii`, dirty-rect logic).
- Any change to CLI, help text, menu, playlist, capture, palette fallbacks.
- Orb/carrion behavior or shared speed-scale semantics.
- New dependencies, disk/session state, animated assets in CI.
- Version policy change: stays 0.3.0, unreleased until the user merges.

## Decision log

| # | Decision | Rationale | Status | Source |
| --- | --- | --- | --- | --- |
| D1 | Continue on branch `worm-mode` in existing worktree; movement fix ships in unreleased 0.3.0 | Nothing merged/pushed; user tests before merge | decided | user |
| D2 | True follow-the-leader serpentine: head steering weaves; body follows the exact carved path | Most natural; chosen over smooth-head+ripple and combined | decided | user |
| D3 | Undulation amplitude: subtle/tranquil | User preference; hypnotic over busy | decided | user |
| D4 | Weave phase tied to distance traveled, so wavelength is stable and follows `--speed` smoothly | Coupled weave chosen over fixed-frequency | decided | user |
| D5 | Wander from low-frequency smoothed noise over distance/time; no abrupt re-rolls | "Smooth aimless drift" chosen over foragey/looping | decided | user |
| D6 | Constant glide speed; shared orb/carrion scale (old D12) preserved | No breathing/surges | decided | user |
| D7 | Gentle turn-rate cap; occasional loops (open curls under avoidance) | Gentle chosen over dreamy-no-loops and lively | decided | user |
| D8 | Strict self-contact avoidance with an escape valve after ~4–6 s of no clearance | Strict chosen; valve prevents permanent stuck states and is the only legal crossing path | decided | user |
| D9 | Avoidance senses early (~4–6 thicknesses) around a lookahead point; ignores trail arc occupied by the body | Early sensing matches gentle turn cap; "never looks surprised" | decided | user |
| D10 | Straight spawn: `snap()` still lays a straight chain behind the head; weave emerges organically | Keeps node-poking tests meaningful; first-second look acceptable | decided | user |
| D11 | Verification: behavioral tests + user visual review of regenerated GIF/cast before merge | Chosen over brittle numeric feel tests and minimal tests | decided | user |
| D12 | README worm paragraph + GIF alt updated to serpentine wording; `assets/worm.gif` regenerated | Round-1 choice; asset otherwise stale | decided | user |
| D13 | New spec supersedes old criterion 4 and D9/D11 by reference; old documents untouched | History stays intact | decided | user |
| D14 | Keep render/tone/taper/wrap and public API; only spine evolution changes; trail buffer fixed-capacity (no heap); `SPINE_N = 24` retained | Minimal blast radius; node arrays stay toroidal so render and tests survive | assumed | recommendation |
| D15 | Feel constants (wave amplitude/wavelength, wander bandwidth, avoidance radius/strength, escape timeout, turn cap, trail sample step) fixed in the implementation plan to meet the described feel; re-tunable after visual pass | Mirrors old D17; constants are plan detail | assumed | recommendation |
| D16 | Spec/plan commits land on `worm-mode`, not `main` | This rework lives on the branch; implementers work there | assumed | recommendation |

## Architecture

### Motion core (`src/worm.rs` only)

- **State.** Head position + heading; a fixed-capacity ring buffer of recent
  toroidal path points sampled at a small fixed arc step (sub-step interpolation
  for sampling); an avoidance timer; existing `rng`, `time`, `noise_time`,
  `base_t`, `auto_size`, `speed`, `detail`, `paused`, `dirty`, `nodes_x/y`.
  No allocation; buffer capacity covers comfortably more than body length plus
  the avoidance lookback window at the largest supported sizes.
- **Steering.** Desired heading = slow smoothed wander (low-frequency noise over
  distance traveled) + subtle serpentine weave (sine whose phase is distance
  traveled / wavelength). Actual heading eases toward desired under a gentle
  max turn rate (radius-limited arcs). Weave amplitude is small; wavelength is
  a small multiple of body length, giving roughly one visible S along the body.
- **Follow-the-leader.** Each node i is placed at arc distance
  `i × spacing` behind the head along the trail (`spacing` and total length as
  today via `LENGTH_RATIO`/`base_t`); positions interpolate between trail
  samples and are stored modulo w/h, so existing render/body_field/tone code
  and toroidal stamping operate unchanged.
- **Self-contact avoidance.** A lookahead point ahead of the head samples the
  trail; near points older than the body arc (plus margin) within the sensing
  radius produce a wrap-aware repulsion that biases the desired heading away
  from them, clamped so it never fights the turn cap into a snap. While a
  clearance-producing steering option exists, crossings do not occur. If no
  clearance exists for the escape timeout, avoidance suspends until the worm
  has cleared (escape valve), then re-arms.
- **Determinism.** All motion derives from the seeded `Rng` and distance-based
  noise; no wall-clock or frame-count-dependent branching beyond `dt`, matching
  the existing run/capture determinism guarantees.
- **Resize.** All trail points are re-wrapped modulo the new size; if auto size
  grows the required body length beyond stored trail (a grow-resize), the trail
  extends behind its oldest point along the local tangent; `dirty` resets.
- **Removed:** `NODE_K` easing, `CHAIN_CURL`, `WAVE_AMP/WAVE_SPEED/WAVE_K`
  standing wave, wander re-roll timer and its `rng.range` draws.

### Unchanged

Rendering (`render`, `body_field`, `tone_at`, `jitter_at`, `core_glyph`,
`fringe_visible`, `fringe_glyph`, `node_radii`, rect splitting/clearing),
`term.rs` colorizer, integration (`being.rs`, `args.rs`, `menu.rs`, `capture.rs`),
tests other than those named in criterion 5, README structure, samples script
structure (GIF regenerated through it), version.

## Risks + rollback

- **Trail-copying exposes steering noise as body wobble.** Mitigation: gentle
  turn cap + low-frequency wander + subtle weave; visual review before merge.
- **Strict avoidance plus gentle turns grazes or traps.** Mitigation: early
  sensing, escape valve, and a behavioral test over long seeded runs.
- **Subtle weave reads as "still straight".** Mitigation: weave parameters are
  plan constants flagged for review and cheap to raise after the visual pass.
- **Grow-resize underruns the trail.** Mitigation: tangent extension behind the
  oldest sample, covered by the existing resize test row.
- **Existing tests poke internals (`snap()`, `nodes_*`, `heading`).**
  Mitigation: D10/D14 keep those entry points; only expected trajectories
  change where they must.
- **Rollback:** revert the rework commits on `worm-mode`; nothing is merged or
  pushed, v0.3.0 remains the current candidate.

## Dependencies / ordering

- Spec (this document) → implementation plan → `worm.rs` rework + tests →
  gate → GIF regeneration + README wording → user visual review → merge
  decision (user).
- No external dependencies. `agg` remains the local GIF tool.

## Addendum — 2026-10-14 (visual-pass retune)

The first visible build still read too calm (near-straight drift, few curves)
and it raveled through itself on real screens. Diagnostics found the cause in
the D8/D9 model itself: avoidance sensed trail points *older than the body arc*
— i.e. it was blind to the body it was about to hit and instead sensed
invisible history. On real screens that history sat permanently inside the
sense radius, so "no clearance" never ended, the escape valve fired roughly
every 36 s, and every valve firing was a visible self-crossing. Tuning within
the model could not fix it; the addendum replaces the sensing model and halves
the default size at the user's request ("half the current size on both length
and thickness... relative thickness to length should stay the same"), then
re-tunes feel under D15. The user approved the result in visual review
(2026-10-14) and chose merge + v0.3.0 release.

| # | Decision | Rationale | Status | Source |
| --- | --- | --- | --- | --- |
| A1 | Avoidance senses body nodes directly (nodes 6..24, `NECK_NODES = 6`), not trail older than the body; the head probes candidate headings over `AVOID_AHEAD = 8` thicknesses and steers toward a clearance-maximizing option (`AVOID_SAFE = 3.5`, `AVOID_STEPS = 24`, `AVOID_HOLD = 1.0` s commitment) | Supersedes D9: a follow-the-leader worm's own body *is* the obstacle set; sensing phantom old trail caused permanent pockets → valve crossings. Body-node sensing + probe steering measured zero crossings in all covered runs | decided | visual pass |
| A2 | Auto thickness halves: `auto = h × 0.0675` (was `h × 0.135`); `LENGTH_RATIO = 10` retained, so relative thickness to length is unchanged; `fit_thickness` additionally clamps the straight spawn to the screen (`(h−2)/10`, `((w−2)/ASPECT)/10`) so tiny screens cannot start self-overlapping | Supersedes old-spec D10 ("~70–80% of terminal height") and motion criterion 6's "size rules unchanged": a straight body is now ≈68% of height but only ≈40% of an 80-col width, matching the user's halving request | decided | user |
| A3 | Feel constants re-tuned under D15: `TURN_RATE 0.15 → 0.4`, `WEAVE_AMP 0.30 → 0.45`, `WEAVE_LEN 1.25 → 1.1`, `WANDER_AMP 1.4 → 1.0`, `WANDER_LEN 3.0 → 2.0` | First build read as straight drift; curves/economy of loops now visible while staying tranquil | decided | user |
| A4 | Escape valve retained as a dormant last resort (state machine unchanged), no longer the expected crossing path | With A1 the valve never fires in the covered runs (80×24 ×3 seeds, 60×18, 100×30, 30×10 ×2 seeds; 12k frames each). Criterion 3's "valve is the only legal crossing path" becomes "crossings do not occur in practice; the valve remains the fail-safe" | decided | user |
| A5 | Supersedes motion criterion 3's sensing wording: clearance is guaranteed by probe-based steering at an 8-thickness lookahead; contact threshold stays `CONTACT = 2.0` thicknesses, release `RELEASE = 2.5`, escape `ESCAPE_SEC = 5.0` | The 4–6-thickness radius clause described the removed model | decided | user |
| A6 | Behavior tests updated: `no_self_contact_without_escape` now uses a non-adjacent body-node gap oracle (index pairs ≥ 8 apart) on 7 realistic/cramped cases; new `worm_unwinds_when_coiled_on_itself` (coiled spawn must unwind without the valve); `escape_valve_fires_when_blocked_and_rearms` unit-tests the valve lifecycle directly; `default_size_is_half_of_previous`, `worm_turns_fast_enough_to_curve_visibly`, and the phantom-seam test updated. The emergent boxed/cramped valve tests were removed — with A1 they can no longer fire | Criterion 5 preserved in spirit: emergent zero-contact runs plus a white-box valve fallback test | decided | user |
| A7 | `assets/worm.gif` regenerated at sample size 6 (showcase ≈3× default, mirroring the old sample-to-default ratio); orb/carrion GIFs restored if regeneration rewrites them | Criterion 7; old sample size 12 would be ~6× the new default and misleading | decided | user |
| A8 | Spec, plan, and README remain on `worm-mode`; merge to `main` and tag `v0.3.0` as the repo's first release after the user's visual approval | D16 extended: the release is the merge | decided | user |

Unchanged by this addendum: success criteria 1, 2, 4 (determinism, glide
speed, wrap), palette/glyph/detail/integration non-goals, trail buffer and
follow-the-leader architecture, `SPINE_N = 24`.

## Open questions

None. All decisions are `decided` except D14–D16, which are `assumed` and
flagged for review in the decision log.
