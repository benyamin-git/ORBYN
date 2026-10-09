use crate::being::Being;
use crate::term::ColorMode;

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

#[allow(dead_code)]
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

#[allow(dead_code)]
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
}
