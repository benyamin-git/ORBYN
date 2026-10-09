# ORBYN Modes Redo — Design

Date: 2026-10-09
Branch: `modes-redo`
Status: Approved design (write-up)

---

## 1. Purpose

Rebuild the modes / rotation / menu feature set for ORBYN with a lean, concrete
approach. The prior attempt (branch `modes-menu`) layered a `Mode` trait, a mode
registry, a custom key decoder, a layout engine, and a shared global input
reader on top of the codebase; on-device testing surfaced visual bugs the user
chose not to fix, preferring a clean rewrite.

Design rule: ideas stay, mechanism changes. The ideas: multiple beings, a picker
menu that controls everything, live preview, rotation between beings, and
keyboard control. The mechanism: concrete structs and match dispatch only — no
traits, no registry, no layout subsystem, no global input state.

Scope excludes the two experimental beings (jellyfish, dragon). Beings are orb
and carrion only.

Success criterion: a clean, full-featured TUI menu is the primary interface; a
correct rotation/session driver; visuals without the prior branch's bugs (paused
switch blanking, first-keypress swallow, mode not starting from playlist, speed
carried as absolute).

## 2. Architecture

Modules on top of `main` (orb + carrion at `43917f4`):

| Module | Responsibility |
|---|---|
| `src/being.rs` new | `Being { Orb, Carrion }` identity: `name`, aliases, one-line description, `from_name`, `all() -> [Being; 2]`; the `Sim` concrete enum moved here from `main.rs` |
| `src/main.rs` | entry; arg dispatch; phases (capture / menu / session); the session run loop + rotation swap |
| `src/rotation.rs` new | pure data + logic: playlist normalization, start-kind resolution, interval/timer, next-being |
| `src/menu.rs` new | primary UI: `Key` decode, `State` + `Action` reducer, `frame` render, preview host, `menu::run` |
| `src/args.rs` | `Config` + parse; adds `--mode`, `--playlist`, `--rotate`, `--run`; validation; menu gate |
| `src/capture.rs` | headless single-being capture (unchanged behavior; drives `Sim::new(being, …)`) |
| `src/term.rs` | existing grid→cells pipeline; param swapped for `Being`; menu additions: `draw_cells`, menu text colorizer; ESC-sequence reader |
| `src/scene.rs`, `src/rng.rs`, `src/globe.rs`, `src/hud.rs`, `src/carrion.rs` | unchanged (ripple renames only) |

`Sim` stays a concrete enum over the two being structs with match-dispatched
methods (`new`, `update`, `resize`, `show_too_small`, `toggle_overlay`,
`toggle_pause`, `paused`, `adjust_speed`, `grid`). No trait objects, no registry
table, no dynamic construction.

Control flow:

```
main() → args::parse
        → capture?  → capture::run(single being, headless)      [no menu]
        → menu?     → menu::run(cfg) → user configures → session
        → else      → session                              [direct: --run / --mode / --playlist]
```

One input channel, `term::input_events()`, is opened once by `main` and shared by
the menu and session phases. The menu drains pending keys before handing the
receiver to the session, so the first keypress after launching a session is not
swallowed. No static/global input state.

All menu logic is headless-testable by construction: decode, reducer, and render
are pure functions of (state) → (output).

## 3. Being identity

`Being` is an enum with two variants. It is the canonical mode id (replaces
`term::Visual`). Provides:

- `name()` — display name: `orb`, `carrion`
- `aliases()` — accepted CLI spellings (`orb`, `carrion`; `-carrion`/`--carrion`
  remain as flags)
- `description()` — one-line help text
- `from_name(&str) -> Option<Being>` — case-insensitive lookup
- `all() -> [Being; 2]` — fixed, in stable order Orb then Carrion

On-disk assets and window titles use the display names. `Sim::new(being, cfg, w,
h)` constructs the right variant; every sim operation match-dispatches.

## 4. Session driver and rotation

`rotation::Rotation { list: Vec<Being>, interval: Option<f32> }`:

- `list` is normalized unique in order (duplicates collapsed during parse and in
  the menu; never a panic path).
- `interval` — some seconds = continuous rotation; None = rotation exists but
  only via the `n` key (manual).
- Start-kind resolution (first being shown): explicit single being (menu single
  start or `--mode`) > playlist first entry > default orb.
- `--playlist a,b` alone implies `interval = 300s` (auto). `--playlist a,b
  --rotate 30` sets 30s. Bare `--rotate 30` implies playlist = `all`.

The session loop (run phase), given the chosen request:

- Builds the initial `Sim` with the resolved start kind from the current live
  parameters.
- Every frame: input, resize, update/tick rotation. Rotation swaps construct the
  next being with the same live parameters.
- Two correctness rules inherited from the design:
  1. A swap while paused primes the new sim with one `update(0.0, w, h)` so a
     paused switch never blanks the screen (both the timed path and the `n` key
     path).
  2. The session owns `speed` and `trail` as canonical values. `+`/`-` adjust
     the live sim and the session values together, so a later swap carries the
     tuned speed; no absolute-speed carry-over.

Run-phase keys: `q`/`Ctrl-C`/`Ctrl-D` quit, `space` pause, `h` toggle
overlay/detail, `n` switch being now (next in playlist, or next in `all()` when
no playlist), `+`/`=` speed up, `-`/`_` slow down.

Window title reflects the active being: `ORBYN` for orb, `ORBYN // CARRION` for
carrion.

Seed determinism: with a fixed `--seed`, rotation order is deterministic
(playlist order) and each spawned being inherits the session seed — capture and
tests stay reproducible. Without a seed, each spawn gets a fresh time-based
seed.

## 5. The menu (primary UI)

Bare `orbyn` on an interactive terminal opens the menu.

### Layout

Terminal of width `w`, height `h`.

- **Preview pane — bottom-left quadrant**: rect `x in [0, ⌊w/2⌋)`, `y in
  [⌊h/2⌋, h)`, no less than 8 columns and 5 rows to render a useful preview;
  framed by a white 1-column border plus one blank gutter cell.
- **START button — bottom-left corner of the screen**: a bordered `[ START ]`
  block painted at the preview pane's bottom-left corner, inside its frame,
  in a bright accent color.
- **Modes list — starting at the top-right corner**, running downward. Row 0 is
  the **"Playlist [off/on]"** toggle; below it one row per being:
  `orb`, `carrion`. Each being row shows a mark: `▪` when included in the
  playlist, `○` when not.
- **Parameters list — in the same top-right panel**, a column immediately left
  of the modes list, top-aligned, sharing row 0. Rows: `speed`, `trail`, `size`,
  `color`, `fps`, `hud`; an `interval` row is present only while Playlist is on.
  These are the shared session parameters; the preview always reflects them.
- **Top-left**: the `ORBYN` wordmark.

### Navigation and interaction

- `Up`/`Down` move inside the focused column (modes or parameters); clamp at the
  ends.
- `→` from a mode row moves focus to the parameters column at the same row
  index; `←` returns to the modes column.
- From the last row of either column, `Down` steps onto the START button; `Up`
  returns to the focused column.
- `+`/`-` adjust the focused parameter (within bounds).
- `q` quits the menu (restores terminal, no run).

### Starting and playlist

`Enter` behaves by context:

- On the **Playlist row**: toggles single-mode vs playlist selection.
- On a **being row**: Playlist off → start that single being; Playlist on →
  toggle its mark `▪`/`○`.
- On a **parameter row** or the **START button** (or any non-row context):
  start the current request.

The START button starts the current request: the single hovered being, or with
Playlist on, the marked beings rotating every `interval` seconds. With Playlist
on and nothing marked, START falls back to rotating all beings.

### Parameters

Global, shared across beings. Each row shows the label and current value;
`+`/`-` adjust the focused parameter within documented bounds
mirroring the CLI validation:

- `speed` 0.05–20.0
- `trail` 0.0–0.97
- `size` 1–60 rows (auto shown as `auto`)
- `color` auto, truecolor, 256, 16, mono
- `fps` 1–240
- `hud` 0–64
- `interval` (playlist on) 1–3600s

### Preview

The preview pane renders a live animation of the being currently under the
cursor (or the being whose parameters are being edited), built from the current
shared parameters, at ~12 fps while the menu is open. A parameter change
re-renders the preview that frame. The preview is rendered into its own grid and
composed into the menu frame with the being's own colorization; the rest of the
menu frame is drawn as menu-styled text.

Rendering: the menu builds a frame of `term::Cell`s (not a `scene::Grid`) via a
pure `frame(&State, preview_cells, w, h)` function; the compositor draws the
preview region with each being's `cells_for` colorization and the chrome with
menu text colors. `Terminal` gains a `draw_cells` path so the menu draws without
the being-specific colorizer.

## 6. CLI, config and capture

`Config` gains fields (all optional, safe defaults):

- `being: Option<Being>` — explicit single selection
- `playlist: Option<Vec<Being>>` — normalized unique in order (`all` accepted)
- `rotate: Option<f32>` — seconds between swaps (with `playlist`)
- `run: bool` — skip the menu

Existing flags are kept verbatim (including `-carrion`/`--carrion`). Added
flags: `--mode <name>`, `--playlist <names>` (comma-separated or `all`),
`--rotate <secs>`, `--run`. The help text documents all flags and the menu keys.

Menu gate (`should_show_menu`): capture always headless, no menu. Otherwise the
menu opens iff stdin and stdout are terminals, and not `--run`, and no explicit
`being`, and no `playlist`. Examples: `orbyn` → menu; `orbyn --carrion` or
`orbyn --playlist orb,carrion --rotate 30` → direct run; `orbyn --speed 2` →
menu with the preset applied.

Validation (in `args.rs`, existing `validate` extended):

- unknown playlist or `--mode` name → error naming the bad value
- duplicate playlist entries → collapsed silently (unique, original order)
- `--rotate` with no `--playlist` → playlist becomes `all`
- `--rotate 0` → rejected (interval min 1s)
- `--mode` + `--playlist` → error (single selection vs rotation conflict)
- `--playlist` or `--rotate` with `--snapshot`/`--cast` → error (capture is
  single-being and headless)

Capture (`capture.rs`) behavior is unchanged: headless, single being, warmup +
duration, snapshot/cast, deterministic under a fixed seed. It constructs via
`Sim::new(being, …)` and rejects rotation configs at parse time.

## 7. Testing and gates

Per-task gate, after every task, `cargo fmt --check`, `cargo clippy
--all-targets -- -D warnings`, `cargo test`, `cargo build --release`.

Constraints: zero new dependencies; MSRV 1.70; no comments in code; session-only
config (no disk writes); capture determinism unchanged.

New test coverage:

- `being.rs`: identity (name/alias/from_name/all), unknown name → None.
- `args.rs`: new flags parse; playlist normalization (unknown named error, dup
  collapse, `all`); `--mode`/`--playlist` conflict; rotation+capture conflict;
  `--rotate 0`; `should_show_menu` matrix (capture, `--run`, explicit being,
  playlist, tty-ness, presets-with-menu).
- `rotation.rs`: dedup/order; interval defaults (auto 300s, explicit, manual);
  start-kind resolution in full (explicit > playlist first > orb); next-being
  cycle; swap-prime-when-paused lights every being's grid at `dt = 0`; seed →
  deterministic order.
- `menu.rs`: `decode` byte/ESC-sequence cases; `on_key` reducer — navigation
  across zones, playlist toggle and marking, parameter `+`/`-` bounds, START,
  Enter-start-from-anywhere, `q`; `frame` — white preview border at the quadrant
  edges, START block at bottom-left, modes list starting top-right, params
  column and values, `▪`/`○` marks, `ORBYN` wordmark top-left, `interval` row
  only when playlist on; preview composition respects the being colorizer.
- Session/main: rotation swap preserves live params (speed/trail carry over).

Existing suite (89 tests) keeps passing; target all-green (~180+).

## 8. Docs and versioning

README rework of the Run section: bare `orbyn` opens the menu (primary); menu
keys; `orbyn --run` and direct-flags are the power path; rotation
documentation. `scripts/samples.sh` unchanged (regenerates orb/carrion GIFs).

Version stays `0.1.0` during development; bumped to `0.2.0` as the final step.

## 9. Out of scope

- New beings (jellyfish, dragon) — explicitly excluded from this plan.
- Per-being parameter savings.
- A menu key inside the run phase (`m` to reopen the menu).
- Windows-specific terminal handling.