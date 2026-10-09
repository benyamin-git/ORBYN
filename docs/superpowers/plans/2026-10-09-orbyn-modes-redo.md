# ORBYN Modes Redo Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild ORBYN's modes/menu/rotation on `modes-redo` with a lean concrete approach — bare `orbyn` opens a full TUI menu (preview, playlist, parameters, START), and rotation/session behavior is correct.

**Architecture:** Concrete-only. A `Being` enum identifies orb/carrion; `Sim` (denumeric enum) stays match-dispatched. `menu.rs` is three headless-testable pieces (byte decode, state reducer, frame render) plus a thin run loop with a live preview. `rotation.rs` is pure data/logic. `main.rs` owns the phases (capture / menu / session) and the run loop.

**Tech Stack:** Rust (edition 2021), zero dependencies, MSRV 1.70, termios via `stty`, alternatescreen.

**Spec:** `docs/superpowers/specs/2026-10-09-orbyn-modes-redo-design.md`

## Global Constraints

- Zero new dependencies.
- MSRV 1.70 (do not introduce syntax/APIs above it).
- No comments in source code.
- Session-only config: no disk writes.
- Capture (`--snapshot`/`--cast`) stays headless, single being, deterministic under a fixed seed; rotation configs rejected at parse.
- Uniform gate after every task: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo build --release`.
- Version stays `0.1.0` until the final task, then `0.2.0`.
- Existing suite (89 tests) keeps passing (adjusted only where a task legitimately renames/reshapes its API).

## Review Focus

- Unknown name in `--mode`/`--playlist` must report a named error (`unknown mode 'dragon'`), not a generic one. → T3
- `--rotate 0` / negative / `--size 0` rejected at parse; bounds match the spec table. → T3
- Tiny terminals: frame clamped, preview never `<8x5`, no panic at `w*h` small. → T7
- Parameter `+`/`-` clamp at bounds; color cycles exactly auto→truecolor→256→16→mono. → T6
- Playlist of one entry: rotation resolves to it, `n` keeps returning it, no duplicate-all panic. → T4
- Duplicate playlist names (`orb,orb`) collapse silently to unique, original-first order. → T3/T4
- Paused rotation/paused `n` swap must not blank the screen (prime with `update(0.0)`). → T5
- Speed/trail adjustments carry across rotation swaps. → T5
- First keypress after START is not swallowed: menu drains pending events before returning the run request. → T8

## File Structure

- Create: `src/being.rs` (`Being`, `Sim`)
- Create: `src/rotation.rs` (`Rotation`, resolution, iterators)
- Create: `src/menu.rs` (decode, `MenuState`, reducer, `frame`, preview, `run`)
- Modify: `src/term.rs` (`cells_for(being)`, `Terminal::new(mode, being)`, `set_title`, `draw_cells`, menu text colorizer)
- Modify: `src/args.rs` (Config fields, flags, validation, `should_show_menu`, help)
- Modify: `src/main.rs` (module wiring, phase dispatch, session run loop)
- Modify: `src/capture.rs` (`Sim::new(being, …)`)
- Modify: `Cargo.toml` (version) and `README.md` (run/menu/keys docs)
- Unchanged: `src/scene.rs`, `src/rng.rs`, `src/globe.rs`, `src/hud.rs`, `src/carrion.rs`, `scripts/samples.sh`

---

### Task 1: `being::Being` identity

**Files:**
- Create: `src/being.rs`
- Test: `src/being.rs` (inline `#[cfg(test)]`)

**Interfaces:**
- Consumes: nothing.
- Produces: `pub enum Being { Orb, Carrion }`; `impl Being { pub fn name(self) -> &'static str; pub fn aliases(self) -> &'static [&'static str]; pub fn description(self) -> &'static str; pub fn from_name(&str) -> Option<Self>; pub fn all() -> [Self; 2]; }` with `Orb.aliases() == ["orb"]`, `Carrion.aliases() == ["carrion"]`, `all() == [Orb, Carrion]`, case-insensitive `from_name`.

- [ ] **Step 1: Write the failing tests** — identity table (`name`/`description` non-empty, ordered `all()`), `from_name` accepts `orb`, `ORB`, `carrion`, `serpent` → `None`; rejects `"dragon"`, `""`.
- [ ] **Step 2: Run to verify failure** — `cargo test being` fails (module/symbols missing).
- [ ] **Step 3: Implement `src/being.rs`** — the enum, its `impl`, an `all()` returning the two variants in Orb, Carrion order. `name()`/`aliases()` match `term.rs`'s existing strings (`orb`, `carrion`). Add `mod being;` to `src/main.rs`. `description()` one line each (say "wireframe globe" / "flesh creature").
- [ ] **Step 4: Run to verify pass** — `cargo test being` passes.
- [ ] **Step 5: Gate + commit** — run the full gate; `git add src/being.rs src/main.rs` + `git commit -m "feat(core): add Being identity"`.

### Task 2: Migrate `Visual` → `Being`, move `Sim`

**Files:**
- Modify: `src/being.rs`, `src/main.rs`, `src/term.rs`, `src/args.rs`, `src/capture.rs`

**Interfaces:**
- Consumes: T1 `Being`; existing `Sim` enum + `App`, `Carrion`.
- Produces: `Sim` now lives in `src/being.rs` with `pub fn new(being: Being, cfg: &Config, w: usize, h: usize) -> Self` plus the existing methods (`update`, `resize`, `show_too_small`, `toggle_overlay`, `toggle_pause`, `paused`, `adjust_speed`, `grid`). `term::cells_for(grid: &Grid, being: Being, mode: ColorMode) -> Vec<Cell>`. `Terminal::new(mode: ColorMode, being: Being)`. New `Terminal::set_title(&mut self, being: Being)` and `Terminal::draw_cells(&mut self, cells: &[Cell]) -> io::Result<()>`. `args::Config.visual: Visual` is replaced by `Config.being: Option<Being>` (default `None`).

- [ ] **Step 1: Write the failing tests** — in `being.rs`: `Sim::new(Being::Orb, &cfg, 30, 10).grid().w == 30`; `Sim::new(Being::Carrion, …).grid().w == 30`; remove/replace tests that reference `Config.visual`.
- [ ] **Step 2: Run to verify failure** — compile fails (old API gone).
- [ ] **Step 3: Implement the migration** — move the `Sim` enum + its `impl` (minus `new`) into `being.rs` (`use crate::{carrion::Carrion, App};`). In `args.rs` delete `visual`; `-carrion`/`--carrion` set `being = Some(Being::Carrion)`; `Config::default().being = None`. In `term.rs` replace `Visual` with `Being` in `cells_for` and `Terminal` (title strings: orb → `ORBYN`, carrion → `ORBYN // CARRION`); add `set_title` (writes `\x1b]0;TITLE\x1b\\`) and `draw_cells` (extract the present-loop from `draw`); `draw(grid)` = `cells_for` + `draw_cells`. In `main.rs` and `capture.rs`, construct with the resolved being (`cfg.being.unwrap_or(Being::Orb)`), drop `Sim::new(cfg,…)` calls.
- [ ] **Step 4: Run to verify pass** — full suite passes (some existing tests updated for the rename).
- [ ] **Step 5: Gate + commit** — `git add src/being.rs src/main.rs src/term.rs src/args.rs src/capture.rs`, message `refactor(core): Being identity replaces Visual; Sim moves to being.rs`.

### Task 3: CLI — `--mode`, `--playlist`, `--rotate`, `--run`, gate, validation

**Files:**
- Modify: `src/args.rs`

**Interfaces:**
- Consumes: T1 `Being`.
- Produces: `Config { being: Option<Being>, playlist: Option<Vec<Being>>, rotate: Option<f32>, run: bool, … }`; `pub fn normalize_playlist(names: &str) -> Result<Vec<Being>, String>`; `pub fn should_show_menu(cfg: &Config, stdin_tty: bool, stdout_tty: bool) -> bool`; help text gains `--mode <NAME>`, `--playlist <NAMES>` (comma-separated or `all`), `--rotate <SECS>`, `--run`, and a ROTATION section.

- [ ] **Step 1: Write failing tests** (in `args.rs`, follow the existing `run([])`/`config([])` helpers):
  - `--mode carrion` → `being == Some(Carrion)`; `--mode dragon` → `Err` containing `dragon`.
  - `--playlist orb,carrion` → `playlist == Some(vec![Orb, Carrion])`; `--playlist all` → all beings; `--playlist orb,orb,carrion` → `[Orb, Carrion]` (collapsed, first-order); `--playlist dragon` → `Err` naming `dragon`; `--mode orb --playlist orb,carrion` → `Err`.
  - `--playlist orb,carrion --rotate 30` → `rotate == Some(30.0)`; `--rotate 0` → `Err`; `--run` → `run == true`; `--rotate 30` (no playlist) → `playlist == Some(all())`.
  - Rotation + capture conflicts: `--playlist orb,carrion --snapshot` → `Err`; `--rotate 30 --cast x.cast` → `Err`.
  - `should_show_menu` matrix: (interactive, no flags) → true; `--run` → false; `--mode` → false; `--playlist` → false; either tty false → false.
  - `usage()` mentions `--mode`, `--playlist`, `--rotate`, `--run`.
- [ ] **Step 2: Run to verify failure** — new tests fail.
- [ ] **Step 3: Implement** — extend `Config`, add the four flags in `parse` (reuse existing value-parse helpers), implement `normalize_playlist` (`"all"` → `Being::all()`, else split on `,`, `from_name` each, unknown → `Err("unknown mode '{name}'")`, collapse duplicates preserving first occurrence), extend `validate` (conflicts: `being`+`playlist`, playlist/rotate+capture, `rotate < 1.0`), add `should_show_menu`, extend `usage()`. Keep `-carrion`/`--carrion` behavior from T2.
- [ ] **Step 4: Run to verify pass** — full suite green.
- [ ] **Step 5: Gate + commit** — `git add src/args.rs`, message `feat(args): mode, playlist, rotate, run flags and menu gate`.

### Task 4: `rotation::Rotation` + resolution

**Files:**
- Create: `src/rotation.rs`
- Modify: `src/main.rs` (add `mod rotation;`)

**Interfaces:**
- Consumes: T1 `Being`.
- Produces: `pub const AUTO_INTERVAL: f32 = 300.0`; `pub struct Rotation { pub list: Vec<Being>, pub interval: Option<f32> }`; `pub fn rotation_for(cfg: &Config) -> Option<Rotation>`; `pub fn resolve_start_kind(being: Option<Being>, rotation: Option<&Rotation>) -> Being`; `pub fn next_after(list: &[Being], current: Being) -> Option<Being>`; `pub fn swap_due(elapsed: f32, interval: Option<f32>) -> bool` (`None` → `false`, else `elapsed >= interval`).

- [ ] **Step 1: Write failing tests** (inline `#[cfg(test)]`): `rotation_for` returns the playlist config; `--playlist a,b` alone → `interval == Some(AUTO_INTERVAL)`; `--rotate 30` alone → playlist `all()`, interval 30; no playlist/rotate → `None`. `resolve_start_kind`: explicit being wins over every playlist; else playlist-first; else Orb; empty playlist → Orb. `next_after([Orb,Carrion], Orb) == Some(Carrion)`; `next_after([Orb,Carrion], Carrion) == Some(Orb)` (wraps); `next_after([Orb], Orb) == Some(Orb)`; `next_after([], Orb) == None`. `swap_due`: `(2.0, Some(1.0)) → true`, `(0.5, Some(1.0)) → false`, `(9.0, None) → false`, `(1.0, Some(1.0)) → true`.
- [ ] **Step 2: Run to verify failure** — fails (module missing).
- [ ] **Step 3: Implement `src/rotation.rs`** — struct + three pure functions as above. `list` is always the normalized-unique playlist (already guaranteed by T3). `next_after` wraps index `+1` mod `len`, empty → `None`.
- [ ] **Step 4: Run to verify pass** — new tests pass.
- [ ] **Step 5: Gate + commit** — `git add src/rotation.rs src/main.rs`, message `feat(rotation): playlist rotation data and resolution`.

### Task 5: Session run loop — rotation swaps, keys, priming

**Files:**
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: T2 `Sim::new(being, cfg, w, h)` + `Sim::update/resize/show_too_small/toggle_overlay/toggle_pause/paused/adjust_speed/grid`; T4 `rotation_for/resolve_start_kind/next_after`; `Terminal::set_title`.
- Produces: the run/session loop (`fn run(cfg: &Config, rx: &std::sync::mpsc::Receiver<term::Event>) -> io::Result<()>` or equivalent) that: sleeps `1/fps`; handles resize; keys `q/Ctrl-C/Ctrl-D` quit, `space` pause, `h` overlay, `n` next, `+`/`-` speed; when `rotation` is `Some`, on tick or `n` swaps the sim to the next being; session carries `session_speed`/`session_trail` f32.

- [ ] **Step 1: Write failing tests** (in `main.rs` `#[cfg(test)]`, mirror the TDD style of the prior branch):
  - `n`-path swap: start `Sim::new(Orb)`; assert the swap builder produces `Sim::new(Carrion, …)` when `next_after` says so, and the paused case primes: `sim` paused → after swap with `update(0.0, w, h)` the grid has a lit cell in every being (loop `Being::all()`).
  - Timer path: the same swap logic fires when `swap_due(elapsed, rotation.interval)`; assert a short interval triggers within one frame and a long one does not.
  - Speed carry: `adjust_speed(2.0)` updates a session-held `session_speed`; a fresh `Sim::new(next, cfg_with(session_speed), …)` reads the multiplied value (assert via the builder's config: `c.speed == session_speed`).
  - `resolve_start_kind` drives the initial sim (already covered in T4; assert integration line: initial being = `resolve_start_kind(cfg.being, rotation)`).
- [ ] **Step 2: Run to verify failure** — tests fail (swap logic absent).
- [ ] **Step 3: Implement the run loop** — the loop from the current `main.rs::run`, extended: hold `rot: Option<Rotation>` and `sim`; a `pending: Option<Being>`/`next_at: f32` timer; on interval expiry or `n`: `let next = next_after(list, current)`; build `config = cfg.clone()` with `config.speed = session_speed; config.trail = session_trail;` `sim = Sim::new(next, &config, w, h)`; if `sim.paused()` then `sim.update(0.0, w, h)` (this also applies when `n` pressed while paused); `terminal.set_title(next)`; keep `session_speed`/`session_trail` updated in `+`/`-` and applied with `sim.adjust_speed`. Keep the too-small guard and `MIN_W/MIN_H` behavior.
- [ ] **Step 4: Run to verify pass** — new + existing tests green.
- [ ] **Step 5: Gate + commit** — `git add src/main.rs`, message `feat(session): rotation swaps, n-key, paused priming, live speed`.

### Task 6: Menu input decode + state reducer

**Files:**
- Create: `src/menu.rs` (decode, state, reducer only)
- Modify: `src/main.rs` (add `mod menu;`)

**Interfaces:**
- Consumes: T1 `Being`; `term::ColorMode`.
- Produces: `pub enum Key { Up, Down, Left, Right, Enter, Plus, Minus, Q, Esc, Other(u8) }`; `pub fn decode(bytes: &[u8]) -> Option<(Key, usize)>` (1 byte for `Enter`=0x0d/`\n`, `Esc`=0x1b, `q`, `+`/`=`, `-`/`_`; 3 bytes for `ESC [ A/B/C/D` = Up/Down/Right/Left; `ESC` alone → `Esc`); `pub struct MenuState { pub playlist_on: bool, pub marked: [bool; 2], pub list_index: usize, pub param_index: usize, pub focus: Focus, pub params: Params, pub at_start: bool }`; `pub enum Focus { List, Params, Start }`; `pub struct Params { pub speed: f32, pub trail: f32, pub size: Option<f32>, pub color: ColorMode, pub fps: u32, pub hud: u32, pub interval: f32 }` with `Default` matching CLI defaults and `pub fn adjust(&mut self, row: ParamRow, delta: i32)`; `pub enum ParamRow { Speed, Trail, Size, Color, Fps, Hud, Interval }`; `pub fn param_rows(playlist_on: bool) -> &'static [ParamRow]`; `pub enum TableAction { Run, Quit, Noop }`; `pub fn on_key(state: &mut MenuState, key: Key) -> Option<TableAction>` (None = no table-level event; `Some(Run/Quit)`).

- [ ] **Step 1: Write failing tests**:
  - `decode`: `[0x1b, b'[', b'A']` → `(Up, 3)`; `[b'\r']` → `(Enter, 1)`; `[b'q']` → `(Q, 1)`; `[0x1b]` → `(Esc, 1)`; `[0x1b, b'[', b'?']` → `None` (resync).
  - `on_key` matrix (fresh `MenuState::default()` — `focus=List, list_index=0, playlist_on=false, param_index=0, at_start=false`):
    - Down → `list_index=1`; Down again clamp; on last row Down → `at_start=true`, `focus=Start`.
    - Right on a being row (`list_index>0`) → `focus=Params`.
    - In Params, Up/Down move `param_index` among `param_rows(false)` (6 rows; `interval` absent); Left → `focus=List`; Right is no-op.
    - From Start, Up returns to the List (and `at_start=false`); Down from the last List row enters Start; Down from the last Params row enters Start too.
    - `interval` row only present when `playlist_on=true` (`param_rows(true)` has 7).
    - Enter contexts: playlist row → toggles `playlist_on`; being row with `playlist_on=false` → returns `Some(Run)`; being row with `playlist_on=true` → toggles `marked[i]`; params row → `Some(Run)`; `at_start` → `Some(Run)`.
    - `+`/`-` on a params row adjusts (see `Params.adjust`); on list rows no-op.
    - `adjust` bounds: `speed` clamps `0.05..=20.0`; `trail` `0.0..=0.97`; `size` `Some(1..=60)`, pressing `+` from `None` → `Some(1.0)`, `-` from `None` → `None`; `fps` `1..=240`; `hud` `0..=64`; `interval` `1.0..=3600.0` (delta step 1); `color` cycles through `Auto, TrueColor, Ansi256, Ansi16, Mono` on `+` and wraps both directions.
    - `q` anywhere → `Some(Quit)`. `Esc` → `Some(Quit)`.
  - `Params::default()` equals the CLI defaults.
- [ ] **Step 2: Run to verify failure** — fails (menu symbols missing).
- [ ] **Step 3: Implement `src/menu.rs`** (this task only: `Key`, `decode`, `MenuState`, `Focus`, `Params`, `ParamRow`, `param_rows`, `TableAction`, `on_key`, `Params::adjust`) — pure, no terminal usage. `marked` and `all()` are indexed 0=Orb, 1=Carrion.
- [ ] **Step 4: Run to verify pass** — menu tests pass.
- [ ] **Step 5: Gate + commit** — `git add src/menu.rs src/main.rs`, message `feat(menu): key decode, state, reducer`.

### Task 7: Menu render — `frame`

**Files:**
- Modify: `src/menu.rs` (add render), `src/term.rs`

**Interfaces:**
- Consumes: T6 `MenuState`/`Params`; `term::Color`, `Cell`, `ColorMode`.
- Produces: `pub struct PreviewFrame { pub origin: (usize, usize), pub size: (usize, usize), pub cells: Vec<Cell> }`; `pub fn frame(state: &MenuState, preview: &PreviewFrame, w: usize, h: usize) -> Vec<Cell>`; internal `pub fn layout(w: usize, h: usize) -> (preview: (usize,usize,usize,usize), modes: (usize,usize), params: (usize,usize), start: (usize,usize))`. `term::menu_color(mode: ColorMode, accented: bool) -> Color` helps the chrome.

- [ ] **Step 1: Write failing tests** (pure, at e.g. `(w,h) = (80, 24)`):
  - Preview quadrant: the rect `x∈[0,40)`, `y∈[12,24)` — white `Color::Rgb(255,255,255)` at every border cell; inner gutter blank.
  - START: `[ START ]` glyphs present at the bottom-left corner region of the preview frame; the two `[`/`]` and letters at the expected offsets.
  - Modes list: text rows for `Playlist [off]`, `orb`, `carrion` begin at the top-right corners; `Playlist [on]` shown when `playlist_on`.
  - Marks: `▪` when `marked[i]`, `○` otherwise; toggling flips glyphs.
  - Params column: labels/values (`speed 1.0`, …) appear to the left of modes, top-aligned; `interval 300` row absent when `playlist_on=false`.
  - Wordmark: `ORBYN` at top-left.
  - Tiny terminal (`(w,h)=(16,6)`): no panic; preview clamped to at least `8x5` or omitted gracefully (pick one: clamp to the quadrant, and if the quadrant is `<8x5`, drop the preview and its border with the region blank). Assert no panic + deterministic output size `w*h`.
  - Deterministic: given the same state, `frame` returns identical `Vec<Cell>`.
- [ ] **Step 2: Run to verify failure** — fails (`frame` absent).
- [ ] **Step 3: Implement** — `layout` computes the quadrant and columns; `frame` clears to blank `Cell::BLANK` at `w*h`, paints the preview `cells` at `origin` (no re-colorization; the preview is already `Cell`s), paints the white border, draws the chrome with `term::Color` selections (wordmark bright, selection row accented, marks, START block). Text drawn cell-by-cell (existing helpers only — no new layout engine). `term::menu_color` returns `Color::Rgb` accent vs `Color::Default` per `ColorMode`.
- [ ] **Step 4: Run to verify pass** — render tests pass.
- [ ] **Step 5: Gate + commit** — `git add src/menu.rs src/term.rs`, message `feat(menu): frame render with preview composition`.

### Task 8: Menu run loop + preview + main wiring

**Files:**
- Modify: `src/menu.rs`, `src/main.rs`, `src/capture.rs`, `src/term.rs`

**Interfaces:**
- Consumes: T2 Sim/`draw_cells`/`set_title`; T3 Config/capture; T4 rotation fns; T6/T7 menu pieces.
- Produces: `pub struct RunRequest { pub single: Option<Being>, pub playlist: Option<Vec<Being>>, pub interval: Option<f32>, pub params: Params }`; `pub fn to_config(req: &RunRequest, base: &Config) -> Config`; `pub fn preview_cells(being: Being, cfg: &Config, w: usize, h: usize, dt: f32) -> Vec<Cell>`; `pub fn run(term: &mut crate::term::Terminal, rx: &std::sync::mpsc::Receiver<crate::term::Event>, cfg: &Config) -> Option<RunRequest>` (None = quit). `main.rs` wires the phases and passes the shared receiver.

- [ ] **Step 1: Write failing tests**:
  - `to_config`: with a request `{ playlist: Some([Carrion, Orb]), interval: Some(30.0), params: Params{ speed: 2.0, … } }` and an interactive `base` → `config.being == None`, `config.playlist == [Carrion, Orb]`, `config.rotate == Some(30.0)`, `config.speed == 2.0`, plus trail/size/color/fps/hud copied. A `single: Some(Carrion)` request → `config.being == Some(Carrion)`, playlist/rotate `None`.
  - `preview_cells`: with `cfg.seed = Some(7)` two calls at `(w,h)=(30,10)` with `dt=1/12` produce identical output; output length == `w*h`; orb preview has a lit (non-blank) cell after ~30 ticks.
  - Event handoff: `run` drains the receiver until empty before returning `Some(request)` — construct a real `std::sync::mpsc::channel`, push `b'q'`… use a seam: `pub fn drain(rx: &Receiver<Event>)` that reads `try_recv` until `Empty`; assert empty after.
  - `run(rx, cfg)` with `cfg.being = Some(Orb)` returns `Some(RunRequest { single: Some(Orb), … })` after one Enter event (state keyed by tests in T6) — requires a terminal; if infeasible to fake a `Terminal`, keep this as an integration assertion over the reducer path (T6 covers the state; here assert the request-assembly through `on_key` + `to_config`, which is headless).
- [ ] **Step 2: Run to verify failure** — new tests fail.
- [ ] **Step 3: Implement** — `preview_cells` builds a `Sim::new(being, cfg, w, h)`, loops ~12 ticks of `update(1/12)`, then `term::cells_for(sim.grid(), being, ColorMode::TrueColor)`. `run`: loop pacing at `12fps` — get winsize (`term::window_size()`), build `PreviewFrame` from the hovered being (`menu::list_state → being`), `frame(...)`, `term.draw_cells(frame_response)`, decode input via `decode` + `on_key` (feeding 1-3 buffered bytes), on `Some(Run)` assemble `RunRequest` and return it after `drain(rx)`. Main `run`/`capture`/`menu` phases: `main()` opens `term::input_events()` once, branches capture → `menu::run|session`; session consumes the channel; menu consumed then handed off.
- [ ] **Step 4: Run to verify pass** — new + full suite green.
- [ ] **Step 5: Gate + commit** — `git add src/menu.rs src/main.rs src/capture.rs src/term.rs`, message `feat(menu): run loop, live preview, main wiring`.

### Task 9: Docs + version

**Files:**
- Modify: `Cargo.toml`, `README.md`

**Interfaces:**
- Consumes: T3 `usage()`; the run key set from T5; menu layout from T7.

- [ ] **Step 1: Write failing test** — bump `version = "0.1.0"` → `"0.2.0"` in `Cargo.toml`; add an `args.rs` test asserting `usage()` includes the run keys (`n`, `q`, `space`, `h`, `+`, `-`) and the rotation flags.
- [ ] **Step 2: Run to verify failure** — test fails (still 0.1.0 / missing keys).
- [ ] **Step 3: Implement docs + version** — `Cargo.toml` version 0.2.0; `README.md` Run section: bare `orbyn` = menu (primary), keys table for menu and run, `orbyn --run`/direct flags as the power path, rotation flags documented. `usage()` already updated in T3; add the run keys line if missing.
- [ ] **Step 4: Run to verify pass** — full suite green; `cargo build --release` banner shows 0.2.0.
- [ ] **Step 5: Gate + commit** — `git add Cargo.toml Cargo.lock README.md src/args.rs`, message `chore(release): document menu-first workflow, bump to 0.2.0`.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-10-09-orbyn-modes-redo.md`. Please review the plan. Does it capture what you want?

For this plan I recommend **subagent-driven**, because the tasks build on each other's exact interfaces (`Sim::new(being,…)`, `rotation_for`, `MenuState`, `frame`, `to_config`) across 9 tightly-chained tasks, and a shipped interface mistake in any one would invalidate the later ones — fresh reviewer gates per task catch those cheaply, and it matches the execution method you used and liked on the previous branch.