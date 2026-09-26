# Carrion mode — v2 design brief

This document is the starting point for the rewritten `-carrion` visual mode.
It captures what we actually want, and — importantly — the mistakes made on the
first attempt (`carrion-softbody-v1`) so we do not repeat them.

Read this before writing any code. The v1 branch is preserved for reference and
for the working screenshot/behavior, but its **rendering and palette approach
should not be copied**.

---

## 1. What we are making

A second visual mode for ORBYN, selected with `-carrion` (also accept
`--carrion`). It replaces the blue orb with a monstrous flesh creature that
roams the terminal. It is a **screensaver first** — it exists to move bright
pixels around the screen to prevent burn-in. Everything else is secondary.

The reference is the monster from the game **Carrion** (Phobia Game Studio):
a red amorphous mass of flesh with tentacles that crawls, grips, and lunges
around a space.

Concretely we want:

- A **red flesh creature** that moves around the terminal.
- **Dark grey grooves** across the red mass, like the folds of a brain.
- **Exactly two colors**: blood red and dark grey. Nothing else.
- Same zero-dependency, single-binary, lightweight constraints as the orb mode.

## 2. Hard requirements

- **Zero dependencies.** Stay on `std` only. No new crates.
- **Lightweight.** This is a screensaver. It must stay well under ~1% of one CPU
  core and a few MB of RAM at the default ~30 FPS, on a large terminal. If a
  feature costs a full-screen pass or a per-pixel allocation, reconsider it.
- **Do not regress the orb mode.** The default blue ORBYN orb must be visually
  and behaviorally unchanged. The orb path (`src/globe.rs`, `src/main.rs` `App`)
  must keep working and its tests must keep passing.
- **Only two colors for carrion.** Blood red and dark grey. No pale highlight,
  no white center, no third tone. See §3.
- **CI must pass:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test`, `cargo build --release`.
- Terminal title in this mode: `ORBYN // CARRION`.
- Keys stay consistent: `q`/`Ctrl-C` quit, `space` pause, `+`/`-` speed,
  `h` toggles an optional overlay.

## 3. Color — the core of v2

The requirement is simple and absolute:

> The creature is **blood red**. The grooves are **dark grey**.
> Those are the only two colors.

This means the renderer should think in terms of **two colors**, not a
continuous brightness ramp that happens to pass through red and grey.

Recommended approach:

- Decide, per cell, a **boolean: is this cell flesh or groove?**
  - Flesh → one fixed blood-red color.
  - Groove → one fixed dark-grey color.
- Optionally allow a tiny bit of shading *within* red only, but never let a
  value wander into a third hue. If shading risks introducing a third color,
  drop it.

Do **not** rely on a multi-stop ramp where some values map to red, some to
grey, and some to in-between dark red. That is exactly what broke v1 (see §5).

Concretely, the palette layer should support two explicit colors, e.g.:

- `FLESH_RED` ≈ `(150, 14, 14)` (blood red)
- `GROOVE_GREY` ≈ `(40, 38, 38)` (dark grey)

Background/empty cells stay `Color::Default` (terminal background).

ANSI fallbacks: red → `31`/`91`, grey → `90`. Mono → just distinct characters.

## 4. How the creature should look and move

Shape and motion were the good part of v1; keep the spirit, simplify the
implementation.

- **2D soft body**, not a 1D rope. A ring of nodes with constraints is fine,
  but keep the constraint set minimal (see §6).
- **Teardrop / wall-hugger** resting shape: pointed front, wider rear.
- **Locomotion** in phases: crawl along a wall → grip with a tendril → lunge
  across open space → land and crawl again.
- **Squash & stretch** on the lunge is desired and looked good.
- **Trailing tentacle bundle** at the rear is desired.
- **Gore trail** (fading shed biomass) is desired but must be cheap — reuse a
  fixed-size pool, never grow without bound.

Keep the creature legible: a viewer should immediately read "red flesh monster
with brain folds", not "noisy blob".

## 5. Mistakes on v1 — do not repeat

These are the specific things that went wrong. The v2 implementation should
avoid all of them.

1. **Grooves rendered as dark red, not grey.**
   The body used a continuous brightness ramp and the groove was implemented as
   a *multiply/blend of brightness*. Groove-edge cells landed in the ramp's
   dark-red transition band, so the "grey" grooves were actually dark red.
   → Fix: pick color by a **boolean flesh/groove decision**, not by a brightness
   value that passes through red on the way down.

2. **The whole ramp got reworked to chase the grooves, and this changed the
   blob's edges.**
   Widening the grey band to catch groove values turned the **silhouette edges
   and outer cells grey too**. So the fix for the grooves broke the body color.
   → Fix: keep body color and groove color **independent**. Edge cells are red
   because they are flesh; groove cells are grey because they are grooves.
   Never make one depend on a global ramp boundary.

3. **Overcomplication.**
   v1 accumulated: ring + hub, perimeter/radial/area constraints, a collapse
   safety-net re-inflate, per-substep travel caps, three fissure methods tried
   in sequence, a `Grid::paint` escape hatch, domain-warp + ridged-sine + smooth
   step for grooves. Each fix added a mechanism, and the interactions caused
   the next bug. → For v2: design the two-color render and the groove **first**,
   on paper, then build the minimum simulation needed to move it. Prefer fewer,
   well-understood mechanisms.

4. **Visible jitter.**
   Motion jittered. Partly from Verlet integration interacting with per-substep
   clamps and the collapse safety-net snapping the ring back to a clean circle.
   The safety-net in particular teleports nodes and shows as popping/jitter.
   → Fix: make the simulation stable by construction (bounded forces, sane rest
   lengths, no teleport-style corrections) rather than by reactive recovery.
   If a safety-net is needed at all, make it invisible (damped) not instant.

5. **More CPU/memory than a screensaver should use.**
   Multiple full-grid passes (metaball buffer, blur removed but still a
   spotlight pass, groove evaluation per body cell, separate shed stamping)
   plus heap allocations per frame (`vec![0.0; w*h]` in blur, etc.).
   → Fix: allocate buffers **once** and reuse. Keep per-frame work O(cells
   touched), ideally only the creature's bounding box and a small trail. No
   per-frame `Vec` allocation in the hot path.

6. **Chasing the visual by re-reading ASCII dumps.**
   We repeatedly guessed at the look from greyscale ASCII previews, which hid
   the actual color bug (dark red vs grey). → For v2: validate color with a
   unit test that inspects the **resolved `Color`**, not the brightness ramp.
   E.g. assert groove cells resolve to a grey `Color::Rgb` and flesh cells to a
   red `Color::Rgb`. Greyscale ASCII is fine for shape, useless for color.

## 6. Suggested minimal architecture for v2

Not prescriptive, but this is the shape that avoids the v1 traps.

- **Simulation**: a small ring of nodes (e.g. 12–16) + a heading + a hub.
  Constraints limited to "these neighbours keep this distance". Add area
  preservation only if squash & stretch genuinely needs it, and only in a
  numerically stable form. Avoid stacking corrective mechanisms.
- **State machine**: Crawl / Grip / Lunge, with timers. This part of v1 worked;
  keep it simple.
- **Two-color renderer**:
  1. Build a coarse boolean/coverage mask for the body over its bounding box.
  2. Compute a groove mask (a cheap warped pattern is fine) over the same box.
  3. For each cell: `groove ? GREY : RED`. Write the color directly.
  Keep the mask as a `Vec<u8>`/`Vec<bool>` allocated once and reused.
- **Integration with the existing code**:
  - Reuse `Grid`, `Rng`, `Hud`, `ASPECT`.
  - Add the `Visual` enum / `-carrion` flag parsing (v1's `src/args.rs` work is
    a fine reference and can likely be kept nearly as-is).
  - The palette/`term` layer should expose an explicit two-color flesh palette
    rather than another multi-stop ramp.
  - `Sim` dispatch in `main.rs` may be reused as-is.

## 7. Definition of done

- `orbyn -carrion` shows a red flesh monster with dark grey brain grooves.
- Inspecting resolved colors shows exactly two: blood red and dark grey.
- No grey silhouette/edges, no white or pale center, no third hue.
- No visible jitter.
- CPU/memory comparable to the orb mode; no per-frame allocations in the hot
  path.
- Default `orbyn` orb mode unchanged; all tests pass; CI green.

## 8. Reference

The v1 attempt lives on branch `carrion-softbody-v1`. Useful to look at:
the movement/state machine, the `-carrion` arg wiring, the HUD flavor, and the
overall silhouette. **Do not** copy its palette approach or its accumulated
constraint/safety-net machinery.

## 9. v2 implementation notes

Built on branch `carrion-v2`:

- `scene::Tone` (`Blank`/`Flesh`/`Groove`/`Gore`) is carried on `Cell`;
  `term::carrion_cell` resolves tones to exactly two colors per color mode and
  `term::Visual` selects the title and render path.
- The body is 14 ring nodes on springs toward teardrop slots around a hub, with
  a rate-limited `body_heading` so the visual frame never outruns the nodes.
  Rendering is a two-pass boolean mask over the body bbox; the outermost
  covered cell is always flesh, so grooves can never touch the silhouette.
- Crawl/Grip/Lunge is timer-driven. The lunge aims away from the gripped wall,
  and heading turns at a limited rate while the body trails the hub, so the
  ring cannot shear through its own center.
- A 16-slot shed pool fades by glyph density (`#+.-`), never by a third color.
  `--trail` controls gore lifetime. (The first v2 pass had latching tendrils
  and a trailing bundle; both were removed as too busy.)
- Dirty-rect clearing and a reused mask buffer; no per-frame allocations in the
  hot path.

v1 traps addressed: colors decided independently of any ramp, grooves rotate
with the body, resize recomputes radius, no teleporting safety-nets, bounded
noise time, and wall rays target the real screen bounds.
