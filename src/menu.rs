use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use crate::args::Config;
use crate::being::{Being, Sim};
use crate::rotation::AUTO_INTERVAL;
use crate::term::{self, menu_color, Cell, Color, ColorMode, Event};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Plus,
    Minus,
    Q,
    Esc,
    Other(u8),
}

pub fn decode(bytes: &[u8]) -> Option<(Key, usize)> {
    let first = *bytes.first()?;
    if first == 0x1b {
        return match bytes.get(1) {
            None => Some((Key::Esc, 1)),
            Some(b'[') => match bytes.get(2) {
                Some(b'A') => Some((Key::Up, 3)),
                Some(b'B') => Some((Key::Down, 3)),
                Some(b'C') => Some((Key::Right, 3)),
                Some(b'D') => Some((Key::Left, 3)),
                _ => None,
            },
            Some(_) => None,
        };
    }
    let key = match first {
        0x0d | 0x0a => Key::Enter,
        b'q' => Key::Q,
        b'+' | b'=' => Key::Plus,
        b'-' | b'_' => Key::Minus,
        other => Key::Other(other),
    };
    Some((key, 1))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    List,
    Params,
    Start,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamRow {
    Speed,
    Trail,
    Size,
    Color,
    Fps,
    Hud,
    Interval,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Params {
    pub speed: f32,
    pub trail: f32,
    pub size: Option<f32>,
    pub color: ColorMode,
    pub fps: u32,
    pub hud: u32,
    pub interval: f32,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            speed: 1.0,
            trail: 0.82,
            size: None,
            color: ColorMode::Auto,
            fps: 30,
            hud: 6,
            interval: 300.0,
        }
    }
}

const COLORS: [ColorMode; 5] = [
    ColorMode::Auto,
    ColorMode::TrueColor,
    ColorMode::Ansi256,
    ColorMode::Ansi16,
    ColorMode::Mono,
];

impl Params {
    pub fn adjust(&mut self, row: ParamRow, delta: i32) {
        match row {
            ParamRow::Speed => {
                self.speed = (self.speed + delta as f32 * 0.1).clamp(0.05, 20.0);
            }
            ParamRow::Trail => {
                self.trail = (self.trail + delta as f32 * 0.01).clamp(0.0, 0.97);
            }
            ParamRow::Size => {
                self.size = match self.size {
                    None => {
                        if delta > 0 {
                            Some(1.0)
                        } else {
                            None
                        }
                    }
                    Some(value) => Some((value + delta as f32).clamp(1.0, 60.0)),
                };
            }
            ParamRow::Color => {
                let current = self.color;
                let index = COLORS.iter().position(|mode| *mode == current).unwrap_or(0);
                let next = (index as i32 + delta).rem_euclid(COLORS.len() as i32) as usize;
                self.color = COLORS[next];
            }
            ParamRow::Fps => {
                self.fps = (self.fps as i32 + delta).clamp(1, 240) as u32;
            }
            ParamRow::Hud => {
                self.hud = (self.hud as i32 + delta).clamp(0, 64) as u32;
            }
            ParamRow::Interval => {
                self.interval = (self.interval + delta as f32).clamp(1.0, 3600.0);
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MenuState {
    pub playlist_on: bool,
    pub marked: [bool; 2],
    pub list_index: usize,
    pub param_index: usize,
    pub focus: Focus,
    pub at_start: bool,
    pub params: Params,
}

impl Default for MenuState {
    fn default() -> Self {
        Self {
            playlist_on: false,
            marked: [false, false],
            list_index: 0,
            param_index: 0,
            focus: Focus::List,
            at_start: false,
            params: Params::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableAction {
    Run,
    Quit,
    Noop,
}

const BASE_ROWS: [ParamRow; 6] = [
    ParamRow::Speed,
    ParamRow::Trail,
    ParamRow::Size,
    ParamRow::Color,
    ParamRow::Fps,
    ParamRow::Hud,
];

const PLAYLIST_ROWS: [ParamRow; 7] = [
    ParamRow::Speed,
    ParamRow::Trail,
    ParamRow::Size,
    ParamRow::Color,
    ParamRow::Fps,
    ParamRow::Hud,
    ParamRow::Interval,
];

pub fn param_rows(playlist_on: bool) -> &'static [ParamRow] {
    if playlist_on {
        &PLAYLIST_ROWS
    } else {
        &BASE_ROWS
    }
}

fn list_rows() -> usize {
    1 + Being::all().len()
}

pub fn on_key(state: &mut MenuState, key: Key) -> Option<TableAction> {
    match key {
        Key::Q | Key::Esc => Some(TableAction::Quit),
        Key::Up => {
            if state.at_start {
                state.at_start = false;
                state.focus = Focus::List;
            } else {
                match state.focus {
                    Focus::List => state.list_index = state.list_index.saturating_sub(1),
                    Focus::Params => state.param_index = state.param_index.saturating_sub(1),
                    Focus::Start => {}
                }
            }
            Some(TableAction::Noop)
        }
        Key::Down => {
            if !state.at_start {
                match state.focus {
                    Focus::List => {
                        if state.list_index + 1 < list_rows() {
                            state.list_index += 1;
                        } else {
                            state.at_start = true;
                            state.focus = Focus::Start;
                        }
                    }
                    Focus::Params => {
                        if state.param_index + 1 < param_rows(state.playlist_on).len() {
                            state.param_index += 1;
                        } else {
                            state.at_start = true;
                            state.focus = Focus::Start;
                        }
                    }
                    Focus::Start => {}
                }
            }
            Some(TableAction::Noop)
        }
        Key::Right => {
            if !state.at_start && state.focus == Focus::List && state.list_index > 0 {
                state.focus = Focus::Params;
            }
            Some(TableAction::Noop)
        }
        Key::Left => {
            if !state.at_start && state.focus == Focus::Params {
                state.focus = Focus::List;
            }
            Some(TableAction::Noop)
        }
        Key::Plus | Key::Minus => {
            if !state.at_start && state.focus == Focus::Params {
                let delta = if key == Key::Plus { 1 } else { -1 };
                if let Some(&row) = param_rows(state.playlist_on).get(state.param_index) {
                    state.params.adjust(row, delta);
                }
            }
            Some(TableAction::Noop)
        }
        Key::Enter => {
            if state.at_start {
                return Some(TableAction::Run);
            }
            match state.focus {
                Focus::Params | Focus::Start => Some(TableAction::Run),
                Focus::List => {
                    if state.list_index == 0 {
                        state.playlist_on = !state.playlist_on;
                        let rows = param_rows(state.playlist_on).len();
                        if state.param_index >= rows {
                            state.param_index = rows - 1;
                        }
                        Some(TableAction::Noop)
                    } else if state.playlist_on {
                        if let Some(mark) = state.marked.get_mut(state.list_index - 1) {
                            *mark = !*mark;
                        }
                        Some(TableAction::Noop)
                    } else {
                        Some(TableAction::Run)
                    }
                }
            }
        }
        Key::Other(_) => None,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreviewFrame {
    pub origin: (usize, usize),
    pub size: (usize, usize),
    pub cells: Vec<Cell>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuRects {
    pub preview: Option<(usize, usize, usize, usize)>,
    pub modes: (usize, usize),
    pub params: (usize, usize),
    pub start: (usize, usize),
}

fn color_name(mode: ColorMode) -> &'static str {
    match mode {
        ColorMode::Auto => "auto",
        ColorMode::TrueColor => "truecolor",
        ColorMode::Ansi256 => "256",
        ColorMode::Ansi16 => "16",
        ColorMode::Mono => "mono",
    }
}

fn param_text(row: ParamRow, params: &Params) -> String {
    match row {
        ParamRow::Speed => format!("speed {:.1}", params.speed),
        ParamRow::Trail => format!("trail {:.2}", params.trail),
        ParamRow::Size => match params.size {
            Some(value) => format!("size {value}"),
            None => "size auto".to_string(),
        },
        ParamRow::Color => format!("color {}", color_name(params.color)),
        ParamRow::Fps => format!("fps {}", params.fps),
        ParamRow::Hud => format!("hud {}", params.hud),
        ParamRow::Interval => format!("interval {}", params.interval),
    }
}

pub fn layout(w: usize, h: usize) -> MenuRects {
    let half_w = w / 2;
    let half_h = h / 2;
    let quadrant_h = h - half_h;
    let interior_w = half_w.saturating_sub(2);
    let interior_h = quadrant_h.saturating_sub(2);
    let preview = if interior_w >= 8 && interior_h >= 5 {
        Some((1, half_h + 1, interior_w, interior_h))
    } else {
        None
    };
    let mut modes_w = "Playlist [off]".chars().count();
    for being in Being::all() {
        modes_w = modes_w.max(2 + being.name().chars().count());
    }
    let mut params_w = 0;
    for row in param_rows(true) {
        params_w = params_w.max(param_text(*row, &Params::default()).chars().count());
    }
    let modes_x = w.saturating_sub(modes_w);
    let params_x = modes_x.saturating_sub(params_w);
    let start = match preview {
        Some((x, y, _, ih)) => (x, y + ih - 1),
        None => (0, h.saturating_sub(1)),
    };
    MenuRects {
        preview,
        modes: (modes_x, 0),
        params: (params_x, 0),
        start,
    }
}

fn put(buf: &mut [Cell], w: usize, h: usize, x: usize, y: usize, s: &str, color: Color) {
    if w == 0 || y >= h {
        return;
    }
    for (cx, ch) in (x..).zip(s.chars()) {
        if cx >= w {
            break;
        }
        buf[y * w + cx] = Cell { ch, color };
    }
}

fn put_right(buf: &mut [Cell], w: usize, h: usize, end: usize, y: usize, s: &str, color: Color) {
    if w == 0 || end == 0 || y >= h {
        return;
    }
    let len = s.chars().count();
    let skip = len.saturating_sub(end);
    for (x, ch) in (end.saturating_sub(len)..).zip(s.chars().skip(skip)) {
        if x >= w {
            break;
        }
        buf[y * w + x] = Cell { ch, color };
    }
}

pub fn frame(state: &MenuState, preview: Option<&PreviewFrame>, w: usize, h: usize) -> Vec<Cell> {
    let mut buf = vec![Cell::BLANK; w.saturating_mul(h)];
    if w == 0 || h == 0 {
        return buf;
    }
    let rects = layout(w, h);
    let half_w = w / 2;
    let half_h = h / 2;
    let accent = menu_color(ColorMode::TrueColor, true);
    if let Some((ix, iy, iw, ih)) = rects.preview {
        let white = Color::Rgb(255, 255, 255);
        for x in 0..half_w {
            buf[half_h * w + x] = Cell {
                ch: '─',
                color: white,
            };
            buf[(h - 1) * w + x] = Cell {
                ch: '─',
                color: white,
            };
        }
        for y in half_h..h {
            buf[y * w] = Cell {
                ch: '│',
                color: white,
            };
            buf[y * w + half_w - 1] = Cell {
                ch: '│',
                color: white,
            };
        }
        buf[half_h * w] = Cell {
            ch: '╭',
            color: white,
        };
        buf[half_h * w + half_w - 1] = Cell {
            ch: '╮',
            color: white,
        };
        buf[(h - 1) * w] = Cell {
            ch: '╰',
            color: white,
        };
        buf[(h - 1) * w + half_w - 1] = Cell {
            ch: '╯',
            color: white,
        };
        if let Some(pv) = preview {
            for row in 0..pv.size.1 {
                for col in 0..pv.size.0 {
                    let idx = row * pv.size.0 + col;
                    if idx >= pv.cells.len() {
                        break;
                    }
                    let x = pv.origin.0 + col;
                    let y = pv.origin.1 + row;
                    if x >= ix && x < ix + iw && y >= iy && y < iy + ih {
                        buf[y * w + x] = pv.cells[idx];
                    }
                }
            }
        }
    }

    put(&mut buf, w, h, 0, 0, "ORBYN", accent);

    let playlist_text = if state.playlist_on {
        "Playlist [on]"
    } else {
        "Playlist [off]"
    };
    let playlist_color = if state.focus == Focus::List && state.list_index == 0 {
        accent
    } else {
        Color::Default
    };
    put_right(&mut buf, w, h, w, 0, playlist_text, playlist_color);

    for (i, being) in Being::all().iter().enumerate() {
        let row = i + 1;
        let mark = if state.marked[i] { '▪' } else { '○' };
        let text = format!("{mark} {}", being.name());
        let color = if state.focus == Focus::List && state.list_index == row {
            accent
        } else {
            Color::Default
        };
        put_right(&mut buf, w, h, w, row, &text, color);
    }

    let params_end = rects.modes.0;
    for (i, row) in param_rows(state.playlist_on).iter().enumerate() {
        let text = param_text(*row, &state.params);
        let color = if state.focus == Focus::Params && state.param_index == i {
            accent
        } else {
            Color::Default
        };
        put_right(&mut buf, w, h, params_end, i, &text, color);
    }

    let (sx, sy) = rects.start;
    put(&mut buf, w, h, sx, sy, "[ START ]", accent);

    buf
}

#[derive(Clone, Debug, PartialEq)]
pub struct RunRequest {
    pub single: Option<Being>,
    pub playlist: Option<Vec<Being>>,
    pub interval: Option<f32>,
    pub params: Params,
}

impl Params {
    pub fn from_cfg(cfg: &Config) -> Self {
        Self {
            speed: cfg.speed,
            trail: cfg.trail,
            size: cfg.size,
            color: cfg.color,
            fps: cfg.fps,
            hud: cfg.hud,
            interval: cfg.rotate.unwrap_or(AUTO_INTERVAL),
        }
    }
}

pub fn to_config(req: &RunRequest, base: &Config) -> Config {
    let mut cfg = base.clone();
    cfg.being = req.single;
    cfg.playlist = req.playlist.clone();
    cfg.rotate = req.interval;
    cfg.speed = req.params.speed;
    cfg.trail = req.params.trail;
    cfg.size = req.params.size;
    cfg.color = req.params.color;
    cfg.fps = req.params.fps;
    cfg.hud = req.params.hud;
    cfg
}

pub fn preview_cells(being: Being, cfg: &Config, w: usize, h: usize, dt: f32) -> Vec<Cell> {
    let mut sim = Sim::new(being, cfg, w, h);
    for _ in 0..12 {
        sim.update(dt, w, h);
    }
    term::cells_for(sim.grid(), being, ColorMode::TrueColor)
}

fn preview_config(params: &Params, seed: Option<u64>) -> Config {
    Config {
        seed,
        speed: params.speed,
        trail: params.trail,
        size: params.size,
        color: params.color,
        hud: params.hud,
        fps: params.fps,
        ..Config::default()
    }
}

pub fn drain(rx: &Receiver<Event>) {
    while rx.try_recv().is_ok() {}
}

fn hovered_being(state: &MenuState, cfg: &Config) -> Being {
    if state.list_index == 0 {
        cfg.being.unwrap_or(Being::Orb)
    } else {
        Being::all()
            .get(state.list_index - 1)
            .copied()
            .unwrap_or(Being::Orb)
    }
}

fn run_request(state: &MenuState, cfg: &Config) -> RunRequest {
    if state.playlist_on {
        let marked: Vec<Being> = Being::all()
            .iter()
            .enumerate()
            .filter(|(index, _)| state.marked[*index])
            .map(|(_, being)| *being)
            .collect();
        let playlist = if marked.is_empty() {
            Being::all().to_vec()
        } else {
            marked
        };
        RunRequest {
            single: None,
            playlist: Some(playlist),
            interval: Some(state.params.interval),
            params: state.params.clone(),
        }
    } else {
        RunRequest {
            single: Some(hovered_being(state, cfg)),
            playlist: None,
            interval: None,
            params: state.params.clone(),
        }
    }
}

fn take_key(buffer: &mut Vec<u8>) -> Option<Key> {
    loop {
        if buffer.is_empty() {
            return None;
        }
        if buffer[0] == 0x1b && buffer.len() < 3 {
            return None;
        }
        match decode(buffer) {
            Some((key, n)) => {
                buffer.drain(..n);
                return Some(key);
            }
            None => {
                buffer.remove(0);
            }
        }
    }
}

pub fn run(term: &mut term::Terminal, rx: &Receiver<Event>, cfg: &Config) -> Option<RunRequest> {
    let mut state = MenuState {
        params: Params::from_cfg(cfg),
        ..MenuState::default()
    };
    let dt = 1.0f32 / 12.0;
    let frame_time = Duration::from_secs_f32(dt);
    let mut buffer = Vec::new();

    loop {
        let frame_start = Instant::now();
        let (w, h) = term.sync().ok()?;
        let hovered = hovered_being(&state, cfg);

        let preview = layout(w, h).preview.and_then(|(ix, iy, iw, ih)| {
            let pw = iw.saturating_sub(2);
            let ph = ih.saturating_sub(2);
            if pw == 0 || ph == 0 {
                return None;
            }
            Some(PreviewFrame {
                origin: (ix + 1, iy + 1),
                size: (pw, ph),
                cells: preview_cells(
                    hovered,
                    &preview_config(&state.params, cfg.seed),
                    pw,
                    ph,
                    dt,
                ),
            })
        });

        let cells = frame(&state, preview.as_ref(), w, h);
        let _ = term.draw_cells(&cells);

        while let Ok(event) = rx.try_recv() {
            match event {
                Event::Eof => {
                    term.restore();
                    return None;
                }
                Event::Key(byte) => buffer.push(byte),
            }
        }

        while let Some(key) = take_key(&mut buffer) {
            match on_key(&mut state, key) {
                Some(TableAction::Quit) => {
                    term.restore();
                    return None;
                }
                Some(TableAction::Run) => {
                    let request = run_request(&state, cfg);
                    drain(rx);
                    term.restore();
                    return Some(request);
                }
                _ => {}
            }
        }

        let elapsed = frame_start.elapsed();
        if elapsed < frame_time {
            std::thread::sleep(frame_time - elapsed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::term::ColorMode;

    fn in_params() -> MenuState {
        let mut state = MenuState::default();
        on_key(&mut state, Key::Down);
        on_key(&mut state, Key::Right);
        state
    }

    #[test]
    fn decode_arrows() {
        assert_eq!(decode(&[0x1b, b'[', b'A']), Some((Key::Up, 3)));
        assert_eq!(decode(&[0x1b, b'[', b'B']), Some((Key::Down, 3)));
        assert_eq!(decode(&[0x1b, b'[', b'C']), Some((Key::Right, 3)));
        assert_eq!(decode(&[0x1b, b'[', b'D']), Some((Key::Left, 3)));
    }

    #[test]
    fn decode_enter() {
        assert_eq!(decode(b"\r"), Some((Key::Enter, 1)));
        assert_eq!(decode(b"\n"), Some((Key::Enter, 1)));
    }

    #[test]
    fn decode_single_bytes() {
        assert_eq!(decode(b"q"), Some((Key::Q, 1)));
        assert_eq!(decode(&[0x1b]), Some((Key::Esc, 1)));
        assert_eq!(decode(b"+"), Some((Key::Plus, 1)));
        assert_eq!(decode(b"="), Some((Key::Plus, 1)));
        assert_eq!(decode(b"-"), Some((Key::Minus, 1)));
        assert_eq!(decode(b"_"), Some((Key::Minus, 1)));
    }

    #[test]
    fn decode_unknown_is_other() {
        assert_eq!(decode(b"x"), Some((Key::Other(b'x'), 1)));
        assert_eq!(decode(b" "), Some((Key::Other(b' '), 1)));
    }

    #[test]
    fn decode_empty_is_none() {
        assert_eq!(decode(&[]), None);
    }

    #[test]
    fn decode_esc_resync_returns_none() {
        assert_eq!(decode(&[0x1b, b'[', b'?']), None);
        assert_eq!(decode(&[0x1b, b'a']), None);
        assert_eq!(decode(&[0x1b, b'[']), None);
    }

    #[test]
    fn default_state_shape() {
        let state = MenuState::default();
        assert!(!state.playlist_on);
        assert_eq!(state.marked, [false, false]);
        assert_eq!(state.list_index, 0);
        assert_eq!(state.param_index, 0);
        assert_eq!(state.focus, Focus::List);
        assert!(!state.at_start);
        assert_eq!(state.params, Params::default());
    }

    #[test]
    fn down_advances_then_enters_start() {
        let mut state = MenuState::default();
        on_key(&mut state, Key::Down);
        assert_eq!(state.list_index, 1);
        on_key(&mut state, Key::Down);
        assert_eq!(state.list_index, 2);
        on_key(&mut state, Key::Down);
        assert_eq!(state.list_index, 2);
        assert!(state.at_start);
        assert_eq!(state.focus, Focus::Start);
    }

    #[test]
    fn up_clamps_in_list() {
        let mut state = MenuState::default();
        on_key(&mut state, Key::Up);
        assert_eq!(state.list_index, 0);
        assert_eq!(state.focus, Focus::List);
        assert!(!state.at_start);
    }

    #[test]
    fn right_on_being_row_focuses_params() {
        let mut state = MenuState::default();
        on_key(&mut state, Key::Down);
        on_key(&mut state, Key::Right);
        assert_eq!(state.focus, Focus::Params);
        assert_eq!(state.param_index, 0);
    }

    #[test]
    fn right_on_playlist_row_is_noop() {
        let mut state = MenuState::default();
        on_key(&mut state, Key::Right);
        assert_eq!(state.focus, Focus::List);
        assert_eq!(state.list_index, 0);
    }

    #[test]
    fn params_up_down_move_and_clamp() {
        let mut state = in_params();
        on_key(&mut state, Key::Down);
        assert_eq!(state.param_index, 1);
        on_key(&mut state, Key::Up);
        assert_eq!(state.param_index, 0);
        on_key(&mut state, Key::Up);
        assert_eq!(state.param_index, 0);
        for _ in 0..10 {
            on_key(&mut state, Key::Down);
        }
        assert!(state.at_start);
        assert_eq!(state.focus, Focus::Start);
    }

    #[test]
    fn left_returns_to_list_and_right_in_params_is_noop() {
        let mut state = in_params();
        on_key(&mut state, Key::Right);
        assert_eq!(state.focus, Focus::Params);
        on_key(&mut state, Key::Left);
        assert_eq!(state.focus, Focus::List);
    }

    #[test]
    fn left_in_list_is_noop() {
        let mut state = MenuState::default();
        on_key(&mut state, Key::Left);
        assert_eq!(state.focus, Focus::List);
    }

    #[test]
    fn up_from_start_returns_to_list() {
        let mut state = MenuState::default();
        for _ in 0..3 {
            on_key(&mut state, Key::Down);
        }
        assert!(state.at_start);
        on_key(&mut state, Key::Up);
        assert_eq!(state.focus, Focus::List);
        assert!(!state.at_start);
    }

    #[test]
    fn down_from_last_params_row_enters_start() {
        let mut state = in_params();
        for _ in 0..6 {
            on_key(&mut state, Key::Down);
        }
        assert!(state.at_start);
        assert_eq!(state.focus, Focus::Start);
    }

    #[test]
    fn param_rows_depends_on_playlist() {
        assert_eq!(
            param_rows(false),
            &[
                ParamRow::Speed,
                ParamRow::Trail,
                ParamRow::Size,
                ParamRow::Color,
                ParamRow::Fps,
                ParamRow::Hud
            ][..]
        );
        assert_eq!(
            param_rows(true),
            &[
                ParamRow::Speed,
                ParamRow::Trail,
                ParamRow::Size,
                ParamRow::Color,
                ParamRow::Fps,
                ParamRow::Hud,
                ParamRow::Interval
            ][..]
        );
    }

    #[test]
    fn interval_row_absent_when_playlist_off() {
        let mut state = in_params();
        for _ in 0..5 {
            on_key(&mut state, Key::Down);
        }
        assert_eq!(state.param_index, 5);
        assert!(!state.at_start);
        assert_eq!(param_rows(false)[5], ParamRow::Hud);
        on_key(&mut state, Key::Down);
        assert!(state.at_start);
    }

    #[test]
    fn interval_row_present_when_playlist_on() {
        let mut state = MenuState::default();
        on_key(&mut state, Key::Enter);
        on_key(&mut state, Key::Down);
        on_key(&mut state, Key::Right);
        for _ in 0..6 {
            on_key(&mut state, Key::Down);
        }
        assert_eq!(state.param_index, 6);
        assert_eq!(
            param_rows(state.playlist_on)[state.param_index],
            ParamRow::Interval
        );
    }

    #[test]
    fn enter_on_playlist_toggles() {
        let mut state = MenuState::default();
        assert_eq!(on_key(&mut state, Key::Enter), Some(TableAction::Noop));
        assert!(state.playlist_on);
        on_key(&mut state, Key::Enter);
        assert!(!state.playlist_on);
    }

    #[test]
    fn playlist_toggle_off_clamps_param_index() {
        let mut state = MenuState::default();
        on_key(&mut state, Key::Enter);
        on_key(&mut state, Key::Down);
        on_key(&mut state, Key::Right);
        for _ in 0..6 {
            on_key(&mut state, Key::Down);
        }
        assert_eq!(state.param_index, 6);
        on_key(&mut state, Key::Up);
        on_key(&mut state, Key::Left);
        on_key(&mut state, Key::Up);
        on_key(&mut state, Key::Enter);
        assert!(!state.playlist_on);
        assert_eq!(state.param_index, 5);
    }

    #[test]
    fn enter_on_being_row_runs_without_playlist() {
        let mut state = MenuState::default();
        on_key(&mut state, Key::Down);
        assert_eq!(on_key(&mut state, Key::Enter), Some(TableAction::Run));
    }

    #[test]
    fn enter_on_being_row_marks_with_playlist() {
        let mut state = MenuState::default();
        on_key(&mut state, Key::Enter);
        assert!(state.playlist_on);
        on_key(&mut state, Key::Down);
        on_key(&mut state, Key::Enter);
        assert_eq!(state.marked, [true, false]);
        on_key(&mut state, Key::Enter);
        assert_eq!(state.marked, [false, false]);
    }

    #[test]
    fn enter_on_params_runs() {
        let mut state = in_params();
        assert_eq!(on_key(&mut state, Key::Enter), Some(TableAction::Run));
    }

    #[test]
    fn enter_at_start_runs() {
        let mut state = MenuState::default();
        for _ in 0..3 {
            on_key(&mut state, Key::Down);
        }
        assert!(state.at_start);
        assert_eq!(on_key(&mut state, Key::Enter), Some(TableAction::Run));
    }

    #[test]
    fn plus_minus_adjust_params_only() {
        let mut state = in_params();
        on_key(&mut state, Key::Plus);
        assert!((state.params.speed - 1.1).abs() < 1e-5);
        let mut list = MenuState::default();
        on_key(&mut list, Key::Plus);
        assert_eq!(list.params, Params::default());
    }

    #[test]
    fn quit_keys_quit_from_anywhere() {
        for key in [Key::Q, Key::Esc] {
            let mut state = MenuState::default();
            assert_eq!(on_key(&mut state, key), Some(TableAction::Quit));
        }
    }

    #[test]
    fn unknown_key_is_none() {
        let mut state = MenuState::default();
        assert_eq!(on_key(&mut state, Key::Other(b'x')), None);
    }

    #[test]
    fn params_default_matches_cli() {
        let params = Params::default();
        assert_eq!(params.speed, 1.0);
        assert_eq!(params.trail, 0.82);
        assert_eq!(params.size, None);
        assert_eq!(params.color, ColorMode::Auto);
        assert_eq!(params.fps, 30);
        assert_eq!(params.hud, 6);
        assert_eq!(params.interval, 300.0);
    }

    #[test]
    fn adjust_speed_bounds() {
        let mut params = Params::default();
        for _ in 0..1000 {
            params.adjust(ParamRow::Speed, 1);
        }
        assert_eq!(params.speed, 20.0);
        for _ in 0..1000 {
            params.adjust(ParamRow::Speed, -1);
        }
        assert_eq!(params.speed, 0.05);
    }

    #[test]
    fn adjust_trail_bounds() {
        let mut params = Params::default();
        for _ in 0..1000 {
            params.adjust(ParamRow::Trail, 1);
        }
        assert_eq!(params.trail, 0.97);
        for _ in 0..1000 {
            params.adjust(ParamRow::Trail, -1);
        }
        assert_eq!(params.trail, 0.0);
    }

    #[test]
    fn adjust_size_from_none() {
        let mut params = Params::default();
        params.adjust(ParamRow::Size, 1);
        assert_eq!(params.size, Some(1.0));
        params.adjust(ParamRow::Size, -1);
        assert_eq!(params.size, Some(1.0));
        let mut none = Params::default();
        none.adjust(ParamRow::Size, -1);
        assert_eq!(none.size, None);
    }

    #[test]
    fn adjust_size_bounds() {
        let mut params = Params::default();
        params.adjust(ParamRow::Size, 1);
        for _ in 0..1000 {
            params.adjust(ParamRow::Size, 1);
        }
        assert_eq!(params.size, Some(60.0));
        for _ in 0..1000 {
            params.adjust(ParamRow::Size, -1);
        }
        assert_eq!(params.size, Some(1.0));
    }

    #[test]
    fn adjust_color_cycles_forward_and_back() {
        let mut params = Params::default();
        params.adjust(ParamRow::Color, 1);
        assert_eq!(params.color, ColorMode::TrueColor);
        params.adjust(ParamRow::Color, 1);
        assert_eq!(params.color, ColorMode::Ansi256);
        params.adjust(ParamRow::Color, 1);
        assert_eq!(params.color, ColorMode::Ansi16);
        params.adjust(ParamRow::Color, 1);
        assert_eq!(params.color, ColorMode::Mono);
        params.adjust(ParamRow::Color, 1);
        assert_eq!(params.color, ColorMode::Auto);
        params.adjust(ParamRow::Color, -1);
        assert_eq!(params.color, ColorMode::Mono);
    }

    #[test]
    fn adjust_fps_hud_interval_bounds() {
        let mut params = Params::default();
        for _ in 0..1000 {
            params.adjust(ParamRow::Fps, 1);
        }
        assert_eq!(params.fps, 240);
        for _ in 0..1000 {
            params.adjust(ParamRow::Fps, -1);
        }
        assert_eq!(params.fps, 1);
        for _ in 0..1000 {
            params.adjust(ParamRow::Hud, 1);
        }
        assert_eq!(params.hud, 64);
        for _ in 0..1000 {
            params.adjust(ParamRow::Hud, -1);
        }
        assert_eq!(params.hud, 0);
        for _ in 0..10000 {
            params.adjust(ParamRow::Interval, 1);
        }
        assert_eq!(params.interval, 3600.0);
        for _ in 0..10000 {
            params.adjust(ParamRow::Interval, -1);
        }
        assert_eq!(params.interval, 1.0);
    }

    fn base_cfg() -> Config {
        Config {
            seed: Some(7),
            ..Config::default()
        }
    }

    fn full_params() -> Params {
        Params {
            speed: 2.0,
            trail: 0.5,
            size: Some(9.0),
            color: ColorMode::Ansi256,
            fps: 60,
            hud: 3,
            interval: 30.0,
        }
    }

    #[test]
    fn to_config_playlist_request_sets_rotation_and_params() {
        let base = base_cfg();
        let req = RunRequest {
            single: None,
            playlist: Some(vec![Being::Carrion, Being::Orb]),
            interval: Some(30.0),
            params: full_params(),
        };
        let cfg = to_config(&req, &base);
        assert_eq!(cfg.being, None);
        assert_eq!(cfg.playlist, Some(vec![Being::Carrion, Being::Orb]));
        assert_eq!(cfg.rotate, Some(30.0));
        assert_eq!(cfg.speed, 2.0);
        assert_eq!(cfg.trail, 0.5);
        assert_eq!(cfg.size, Some(9.0));
        assert_eq!(cfg.color, ColorMode::Ansi256);
        assert_eq!(cfg.fps, 60);
        assert_eq!(cfg.hud, 3);
        assert_eq!(cfg.seed, Some(7));
    }

    #[test]
    fn to_config_single_request_clears_rotation() {
        let base = base_cfg();
        let req = RunRequest {
            single: Some(Being::Carrion),
            playlist: None,
            interval: None,
            params: Params::from_cfg(&base),
        };
        let cfg = to_config(&req, &base);
        assert_eq!(cfg.being, Some(Being::Carrion));
        assert!(cfg.playlist.is_none());
        assert!(cfg.rotate.is_none());
    }

    #[test]
    fn from_cfg_copies_cli_params() {
        let cfg = Config {
            speed: 2.0,
            trail: 0.4,
            size: Some(5.0),
            color: ColorMode::Mono,
            fps: 45,
            hud: 0,
            rotate: Some(30.0),
            ..Config::default()
        };
        let params = Params::from_cfg(&cfg);
        assert_eq!(params.speed, 2.0);
        assert_eq!(params.trail, 0.4);
        assert_eq!(params.size, Some(5.0));
        assert_eq!(params.color, ColorMode::Mono);
        assert_eq!(params.fps, 45);
        assert_eq!(params.hud, 0);
        assert_eq!(params.interval, 30.0);
    }

    #[test]
    fn from_cfg_defaults_interval_to_auto() {
        let params = Params::from_cfg(&Config::default());
        assert_eq!(params.interval, AUTO_INTERVAL);
    }

    #[test]
    fn preview_cells_are_deterministic_and_sized() {
        let cfg = base_cfg();
        let dt = 1.0 / 12.0;
        let first = preview_cells(Being::Orb, &cfg, 30, 10, dt);
        let second = preview_cells(Being::Orb, &cfg, 30, 10, dt);
        assert_eq!(first, second);
        assert_eq!(first.len(), 30 * 10);
        assert!(first.iter().any(|cell| cell.ch != ' '));
    }

    #[test]
    fn preview_tracks_live_parameters() {
        let seed = Some(7);
        let dt = 1.0 / 12.0;
        let auto = Params::default();
        let sized = Params {
            size: Some(20.0),
            ..Params::default()
        };
        let auto_cells = preview_cells(Being::Orb, &preview_config(&auto, seed), 60, 20, dt);
        let sized_cells = preview_cells(Being::Orb, &preview_config(&sized, seed), 60, 20, dt);
        assert_ne!(auto_cells, sized_cells);

        let again = preview_cells(Being::Orb, &preview_config(&auto, seed), 60, 20, dt);
        assert_eq!(auto_cells, again);

        let slow = Params {
            speed: 0.05,
            ..Params::default()
        };
        let fast = Params {
            speed: 20.0,
            ..Params::default()
        };
        let slow_cells = preview_cells(Being::Orb, &preview_config(&slow, seed), 60, 20, dt);
        let fast_cells = preview_cells(Being::Orb, &preview_config(&fast, seed), 60, 20, dt);
        assert_ne!(slow_cells, fast_cells);
    }

    #[test]
    fn drain_empties_the_channel() {
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Event::Key(b'q')).expect("send");
        tx.send(Event::Key(b'+')).expect("send");
        tx.send(Event::Eof).expect("send");
        drain(&rx);
        assert_eq!(rx.try_recv(), Err(std::sync::mpsc::TryRecvError::Empty));
    }

    #[test]
    fn hovered_being_tracks_list_index() {
        let cfg = Config::default();
        let mut state = MenuState::default();
        assert_eq!(hovered_being(&state, &cfg), Being::Orb);
        state.list_index = 1;
        assert_eq!(hovered_being(&state, &cfg), Being::Orb);
        state.list_index = 2;
        assert_eq!(hovered_being(&state, &cfg), Being::Carrion);
    }

    #[test]
    fn run_request_assembles_single_and_playlist() {
        let cfg = Config {
            being: Some(Being::Orb),
            ..Config::default()
        };
        let single = run_request(&MenuState::default(), &cfg);
        assert_eq!(single.single, Some(Being::Orb));
        assert!(single.playlist.is_none());
        assert!(single.interval.is_none());

        let mut marked = MenuState {
            playlist_on: true,
            marked: [false, true],
            ..MenuState::default()
        };
        let req = run_request(&marked, &cfg);
        assert_eq!(req.single, None);
        assert_eq!(req.playlist, Some(vec![Being::Carrion]));
        assert_eq!(req.interval, Some(marked.params.interval));

        marked.marked = [false, false];
        let req = run_request(&marked, &cfg);
        assert_eq!(req.playlist, Some(Being::all().to_vec()));
    }

    #[test]
    fn take_key_waits_for_split_escape() {
        let mut buffer = vec![0x1b];
        assert_eq!(take_key(&mut buffer), None);
        assert_eq!(buffer, vec![0x1b]);
        buffer.push(b'[');
        assert_eq!(take_key(&mut buffer), None);
        assert_eq!(buffer, vec![0x1b, b'[']);
        buffer.push(b'D');
        assert_eq!(take_key(&mut buffer), Some(Key::Left));
        assert!(buffer.is_empty());
    }

    #[test]
    fn take_key_resyncs_on_garbage_escape() {
        let mut buffer = vec![0x1b, b'[', b'?'];
        assert_eq!(take_key(&mut buffer), Some(Key::Other(b'[')));
        assert_eq!(buffer, vec![b'?']);
        assert_eq!(take_key(&mut buffer), Some(Key::Other(b'?')));
        assert!(buffer.is_empty());
    }

    #[test]
    fn take_key_drains_multiple_keys() {
        let mut buffer = vec![b'q', b'+'];
        assert_eq!(take_key(&mut buffer), Some(Key::Q));
        assert_eq!(take_key(&mut buffer), Some(Key::Plus));
        assert_eq!(take_key(&mut buffer), None);
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;

    const W: usize = 80;
    const H: usize = 24;

    fn blank() -> MenuState {
        MenuState::default()
    }

    fn at(cells: &[Cell], w: usize, x: usize, y: usize) -> Cell {
        cells[y * w + x]
    }

    fn row_text(cells: &[Cell], w: usize, y: usize, x0: usize, x1: usize) -> String {
        (x0..x1).map(|x| at(cells, w, x, y).ch).collect()
    }

    fn accent() -> Color {
        menu_color(ColorMode::TrueColor, true)
    }

    #[test]
    fn layout_80x24_pins_rects() {
        let rects = layout(W, H);
        assert_eq!(rects.preview, Some((1, 13, 38, 10)));
        assert_eq!(rects.modes, (66, 0));
        assert_eq!(rects.params, (54, 0));
        assert_eq!(rects.start, (1, 22));
    }

    #[test]
    fn layout_thresholds_match_brief() {
        assert_eq!(layout(20, 24).preview, Some((1, 13, 8, 10)));
        assert_eq!(layout(19, 24).preview, None);
        assert_eq!(layout(40, 14).preview, Some((1, 8, 18, 5)));
        assert_eq!(layout(40, 13).preview, Some((1, 7, 18, 5)));
        assert_eq!(layout(40, 12).preview, None);
        assert_eq!(layout(16, 6).preview, None);
    }

    #[test]
    fn preview_border_is_white_on_all_four_edges() {
        let cells = frame(&blank(), None, W, H);
        let white = Color::Rgb(255, 255, 255);
        for x in 0..40 {
            assert_eq!(at(&cells, W, x, 12).color, white);
            assert_eq!(at(&cells, W, x, 23).color, white);
        }
        for y in 12..24 {
            assert_eq!(at(&cells, W, 0, y).color, white);
            assert_eq!(at(&cells, W, 39, y).color, white);
        }
        assert_eq!(at(&cells, W, 0, 12).ch, '╭');
        assert_eq!(at(&cells, W, 39, 12).ch, '╮');
        assert_eq!(at(&cells, W, 0, 23).ch, '╰');
        assert_eq!(at(&cells, W, 39, 23).ch, '╯');
    }

    #[test]
    fn preview_cells_are_blitted_into_the_interior() {
        let pv = PreviewFrame {
            origin: (1, 13),
            size: (3, 2),
            cells: (0..6)
                .map(|i| Cell {
                    ch: (b'a' + i) as char,
                    color: Color::Basic(1),
                })
                .collect(),
        };
        let cells = frame(&blank(), Some(&pv), W, H);
        assert_eq!(at(&cells, W, 1, 13).ch, 'a');
        assert_eq!(at(&cells, W, 3, 13).ch, 'c');
        assert_eq!(at(&cells, W, 1, 14).ch, 'd');
        assert_eq!(at(&cells, W, 3, 14).ch, 'f');
        assert_eq!(at(&cells, W, 1, 13).color, Color::Basic(1));
        assert_eq!(at(&cells, W, 4, 13), Cell::BLANK);
        assert_eq!(at(&cells, W, 1, 15), Cell::BLANK);
    }

    #[test]
    fn preview_cells_are_clipped_to_the_interior() {
        let pv = PreviewFrame {
            origin: (0, 12),
            size: (50, 20),
            cells: vec![
                Cell {
                    ch: '#',
                    color: Color::Basic(2),
                };
                1000
            ],
        };
        let cells = frame(&blank(), Some(&pv), W, H);
        assert_eq!(at(&cells, W, 0, 12).ch, '╭');
        assert_eq!(at(&cells, W, 0, 12).color, Color::Rgb(255, 255, 255));
        assert_eq!(at(&cells, W, 1, 13).ch, '#');
        assert_eq!(at(&cells, W, 38, 22).ch, '#');
        assert_eq!(at(&cells, W, 40, 12), Cell::BLANK);
    }

    #[test]
    fn start_button_sits_inside_the_frame() {
        let cells = frame(&blank(), None, W, H);
        for (i, ch) in "[ START ]".chars().enumerate() {
            let cell = at(&cells, W, 1 + i, 22);
            assert_eq!(cell.ch, ch, "offset {i}");
            assert_eq!(cell.color, accent());
        }
    }

    #[test]
    fn start_button_falls_back_to_screen_corner() {
        let cells = frame(&blank(), None, 16, 6);
        assert_eq!(at(&cells, 16, 0, 5).ch, '[');
        assert_eq!(at(&cells, 16, 2, 5).ch, 'S');
        assert_eq!(at(&cells, 16, 8, 5).ch, ']');
    }

    #[test]
    fn modes_rows_are_right_aligned() {
        let cells = frame(&blank(), None, W, H);
        assert_eq!(row_text(&cells, W, 0, 66, 80), "Playlist [off]");
        assert_eq!(row_text(&cells, W, 1, 75, 80), "○ orb");
        assert_eq!(row_text(&cells, W, 2, 71, 80), "○ carrion");
    }

    #[test]
    fn playlist_toggle_shows_on() {
        let state = MenuState {
            playlist_on: true,
            ..blank()
        };
        let cells = frame(&state, None, W, H);
        assert_eq!(row_text(&cells, W, 0, 67, 80), "Playlist [on]");
    }

    #[test]
    fn marks_track_state() {
        let state = MenuState {
            marked: [true, false],
            ..blank()
        };
        let cells = frame(&state, None, W, H);
        assert_eq!(at(&cells, W, 75, 1).ch, '▪');
        assert_eq!(at(&cells, W, 71, 2).ch, '○');

        let flipped = MenuState {
            marked: [false, true],
            ..blank()
        };
        let cells = frame(&flipped, None, W, H);
        assert_eq!(at(&cells, W, 75, 1).ch, '○');
        assert_eq!(at(&cells, W, 71, 2).ch, '▪');
    }

    #[test]
    fn params_column_sits_left_of_modes() {
        let cells = frame(&blank(), None, W, H);
        assert_eq!(row_text(&cells, W, 0, 57, 66), "speed 1.0");
        assert_eq!(row_text(&cells, W, 1, 56, 66), "trail 0.82");
        assert_eq!(row_text(&cells, W, 2, 57, 66), "size auto");
        assert_eq!(row_text(&cells, W, 3, 56, 66), "color auto");
        assert_eq!(row_text(&cells, W, 4, 60, 66), "fps 30");
        assert_eq!(row_text(&cells, W, 5, 61, 66), "hud 6");
    }

    #[test]
    fn param_values_reflect_state() {
        let state = MenuState {
            params: Params {
                speed: 2.5,
                trail: 0.5,
                size: Some(12.0),
                color: ColorMode::Ansi256,
                fps: 60,
                hud: 3,
                interval: 45.0,
            },
            playlist_on: true,
            ..blank()
        };
        let cells = frame(&state, None, W, H);
        assert_eq!(row_text(&cells, W, 0, 57, 66), "speed 2.5");
        assert_eq!(row_text(&cells, W, 1, 56, 66), "trail 0.50");
        assert_eq!(row_text(&cells, W, 2, 59, 66), "size 12");
        assert_eq!(row_text(&cells, W, 3, 57, 66), "color 256");
        assert_eq!(row_text(&cells, W, 4, 60, 66), "fps 60");
        assert_eq!(row_text(&cells, W, 5, 61, 66), "hud 3");
        assert_eq!(row_text(&cells, W, 6, 55, 66), "interval 45");
    }

    #[test]
    fn interval_row_only_when_playlist_on() {
        let off = frame(&blank(), None, W, H);
        for x in 54..66 {
            assert_eq!(at(&off, W, x, 6), Cell::BLANK);
        }
        let on = frame(
            &MenuState {
                playlist_on: true,
                ..blank()
            },
            None,
            W,
            H,
        );
        assert_eq!(row_text(&on, W, 6, 54, 66), "interval 300");
    }

    #[test]
    fn wordmark_sits_top_left_in_accent() {
        let cells = frame(&blank(), None, W, H);
        assert_eq!(row_text(&cells, W, 0, 0, 5), "ORBYN");
        for x in 0..5 {
            assert_eq!(at(&cells, W, x, 0).color, accent());
        }
    }

    #[test]
    fn focused_rows_use_accent() {
        let mut state = blank();
        state.list_index = 1;
        let cells = frame(&state, None, W, H);
        assert_eq!(at(&cells, W, 75, 1).color, accent());
        assert_eq!(at(&cells, W, 66, 0).color, Color::Default);

        let params = MenuState {
            focus: Focus::Params,
            param_index: 0,
            ..blank()
        };
        let cells = frame(&params, None, W, H);
        assert_eq!(at(&cells, W, 57, 0).color, accent());
        assert_eq!(at(&cells, W, 75, 1).color, Color::Default);
    }

    #[test]
    fn tiny_terminals_never_panic_and_keep_size() {
        for (w, h) in [(16, 6), (1, 1), (0, 0), (3, 2), (10, 3), (24, 8), (19, 24)] {
            let cells = frame(&blank(), None, w, h);
            assert_eq!(cells.len(), w * h);
        }
    }

    #[test]
    fn frame_is_deterministic() {
        let pv = PreviewFrame {
            origin: (1, 13),
            size: (4, 4),
            cells: vec![
                Cell {
                    ch: 'x',
                    color: Color::Basic(3),
                };
                16
            ],
        };
        let a = frame(&blank(), Some(&pv), W, H);
        let b = frame(&blank(), Some(&pv), W, H);
        assert_eq!(a, b);
    }
}
