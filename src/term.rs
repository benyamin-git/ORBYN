use std::io::{self, IsTerminal, Read, Write};
use std::os::raw::{c_int, c_ulong};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use crate::scene::{Grid, Tone, CUTOFF};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Visual {
    Orb,
    Carrion,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ColorMode {
    Auto,
    TrueColor,
    Ansi256,
    Ansi16,
    Mono,
}

impl ColorMode {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "auto" => Ok(Self::Auto),
            "truecolor" | "true" | "24bit" => Ok(Self::TrueColor),
            "256" | "ansi256" => Ok(Self::Ansi256),
            "16" | "ansi16" => Ok(Self::Ansi16),
            "mono" | "none" => Ok(Self::Mono),
            other => Err(format!(
                "invalid color mode '{other}' (expected auto, truecolor, 256, 16, mono)"
            )),
        }
    }
}

pub fn resolve(mode: ColorMode) -> ColorMode {
    match mode {
        ColorMode::Auto => detect_color_mode(),
        explicit => explicit,
    }
}

fn detect_color_mode() -> ColorMode {
    if !io::stdout().is_terminal() {
        return ColorMode::Mono;
    }
    if std::env::var_os("NO_COLOR").is_some() {
        return ColorMode::Mono;
    }
    let colorterm = std::env::var("COLORTERM")
        .unwrap_or_default()
        .to_lowercase();
    if colorterm.contains("truecolor") || colorterm.contains("24bit") {
        return ColorMode::TrueColor;
    }
    let term = std::env::var("TERM").unwrap_or_default().to_lowercase();
    if term.contains("256color") {
        return ColorMode::Ansi256;
    }
    if term.is_empty() || term == "dumb" {
        return ColorMode::Mono;
    }
    ColorMode::Ansi16
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Color {
    Default,
    Basic(u8),
    Indexed(u8),
    Rgb(u8, u8, u8),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cell {
    pub ch: char,
    pub color: Color,
}

impl Cell {
    pub const BLANK: Cell = Cell {
        ch: ' ',
        color: Color::Default,
    };
}

const RAMP: &[u8] = b" .:-=+*#%@";

pub fn shade(v: f32, mode: ColorMode) -> (char, Color) {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.0 {
        return (' ', Color::Default);
    }
    let idx = ((v * (RAMP.len() - 1) as f32).round() as usize).min(RAMP.len() - 1);
    let ch = RAMP[idx] as char;
    let color = match mode {
        ColorMode::Mono => Color::Default,
        ColorMode::Ansi16 => Color::Basic(basic_code(v)),
        ColorMode::Ansi256 => Color::Indexed(indexed_code(v)),
        ColorMode::TrueColor | ColorMode::Auto => {
            let (r, g, b) = rgb(v);
            Color::Rgb(r, g, b)
        }
    };
    (ch, color)
}

fn rgb(v: f32) -> (u8, u8, u8) {
    let (from, to, t) = if v < 0.5 {
        ((6.0, 14.0, 44.0), (34.0, 96.0, 255.0), v * 2.0)
    } else {
        ((34.0, 96.0, 255.0), (168.0, 216.0, 255.0), (v - 0.5) * 2.0)
    };
    let lerp = |a: f32, b: f32| (a + (b - a) * t).round().clamp(0.0, 255.0) as u8;
    (lerp(from.0, to.0), lerp(from.1, to.1), lerp(from.2, to.2))
}

fn basic_code(v: f32) -> u8 {
    if v >= 0.85 {
        97
    } else if v >= 0.6 {
        96
    } else if v >= 0.3 {
        94
    } else {
        34
    }
}

fn indexed_code(v: f32) -> u8 {
    let (r, g, b) = rgb(v);
    let q = |c: u8| ((c as u16 * 5) / 255) as u8;
    16 + 36 * q(r) + 6 * q(g) + q(b)
}

pub const FLESH_RED: Color = Color::Rgb(150, 14, 14);
pub const GROOVE_GREY: Color = Color::Rgb(40, 38, 38);

pub const FLESH_CH: char = '#';
pub const GROOVE_CH: char = '=';
pub const GORE_CH: char = '-';

fn flesh_color(mode: ColorMode) -> Color {
    match mode {
        ColorMode::Mono => Color::Default,
        ColorMode::Ansi16 => Color::Basic(31),
        ColorMode::Ansi256 => Color::Indexed(88),
        ColorMode::TrueColor | ColorMode::Auto => FLESH_RED,
    }
}

fn groove_color(mode: ColorMode) -> Color {
    match mode {
        ColorMode::Mono => Color::Default,
        ColorMode::Ansi16 => Color::Basic(90),
        ColorMode::Ansi256 => Color::Indexed(235),
        ColorMode::TrueColor | ColorMode::Auto => GROOVE_GREY,
    }
}

fn carrion_cell(cell: &crate::scene::Cell, mode: ColorMode) -> Cell {
    if cell.v <= 0.0 && cell.tone == Tone::Blank {
        return Cell::BLANK;
    }
    let tone = if cell.tone == Tone::Blank {
        Tone::Flesh
    } else {
        cell.tone
    };
    let (default_ch, color) = match tone {
        Tone::Groove => (GROOVE_CH, groove_color(mode)),
        Tone::Gore => (GORE_CH, flesh_color(mode)),
        Tone::Blank | Tone::Flesh => (FLESH_CH, flesh_color(mode)),
    };
    let ch = if cell.ch == 0 {
        default_ch
    } else {
        cell.ch as char
    };
    Cell { ch, color }
}

fn orb_cell(v: f32, ch: u8, mode: ColorMode) -> Cell {
    if v <= 0.0 || (v <= CUTOFF && ch == 0) {
        Cell::BLANK
    } else {
        let (ramp_ch, color) = shade(v, mode);
        let ch = if ch == 0 { ramp_ch } else { ch as char };
        Cell { ch, color }
    }
}

pub fn cells_for(grid: &Grid, visual: Visual, mode: ColorMode) -> Vec<Cell> {
    grid.data
        .iter()
        .map(|cell| match visual {
            Visual::Orb => orb_cell(cell.v, cell.ch, mode),
            Visual::Carrion => carrion_cell(cell, mode),
        })
        .collect()
}

pub fn snapshot_ansi(cells: &[Cell], w: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(cells.len() * 8);
    let mut color = Color::Default;
    for (i, cell) in cells.iter().enumerate() {
        if i > 0 && w > 0 && i % w == 0 {
            out.extend_from_slice(b"\x1b[0m\r\n");
            color = Color::Default;
        }
        if cell.color != color {
            push_sgr(cell.color, &mut out);
            color = cell.color;
        }
        out.push(cell.ch as u8);
    }
    out.extend_from_slice(b"\x1b[0m\r\n");
    out
}

pub struct Renderer {
    w: usize,
    prev: Vec<Cell>,
    cursor: Option<(u32, u32)>,
    color: Color,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            w: 0,
            prev: Vec::new(),
            cursor: None,
            color: Color::Default,
        }
    }

    pub fn reset(&mut self, w: usize, h: usize) {
        self.w = w;
        self.prev.clear();
        self.prev.resize(w * h, Cell::BLANK);
        self.cursor = None;
        self.color = Color::Default;
    }

    pub fn present(&mut self, cells: &[Cell], out: &mut Vec<u8>) {
        if self.w == 0 {
            return;
        }
        for (i, &cell) in cells.iter().enumerate().take(self.prev.len()) {
            if self.prev[i] == cell {
                continue;
            }
            self.prev[i] = cell;

            let row = (i / self.w) as u32 + 1;
            let col = (i % self.w) as u32 + 1;
            if self.cursor != Some((row, col)) {
                out.extend_from_slice(b"\x1b[");
                push_u32(row, out);
                out.push(b';');
                push_u32(col, out);
                out.push(b'H');
            }
            if self.color != cell.color {
                push_sgr(cell.color, out);
                self.color = cell.color;
            }
            out.push(cell.ch as u8);
            self.cursor = if col as usize == self.w {
                None
            } else {
                Some((row, col + 1))
            };
        }
    }
}

fn push_u32(mut n: u32, out: &mut Vec<u8>) {
    let mut buf = [0u8; 10];
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    out.extend_from_slice(&buf[i..]);
}

fn push_sgr(color: Color, out: &mut Vec<u8>) {
    match color {
        Color::Default => out.extend_from_slice(b"\x1b[39m"),
        Color::Basic(code) => {
            out.extend_from_slice(b"\x1b[");
            push_u32(code as u32, out);
            out.push(b'm');
        }
        Color::Indexed(n) => {
            out.extend_from_slice(b"\x1b[38;5;");
            push_u32(n as u32, out);
            out.push(b'm');
        }
        Color::Rgb(r, g, b) => {
            out.extend_from_slice(b"\x1b[38;2;");
            push_u32(r as u32, out);
            out.push(b';');
            push_u32(g as u32, out);
            out.push(b';');
            push_u32(b as u32, out);
            out.push(b'm');
        }
    }
}

#[repr(C)]
#[derive(Default)]
struct Winsize {
    rows: u16,
    cols: u16,
    xpixels: u16,
    ypixels: u16,
}

extern "C" {
    fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
    fn tcgetattr(fd: c_int, termios_p: *mut std::ffi::c_void) -> c_int;
    fn tcsetattr(fd: c_int, optional_actions: c_int, termios_p: *const std::ffi::c_void) -> c_int;
    fn signal(signum: c_int, handler: extern "C" fn(c_int)) -> usize;
    fn write(fd: c_int, buf: *const std::ffi::c_void, count: usize) -> isize;
    fn _exit(status: c_int) -> !;
}

const TCSANOW: c_int = 0;
const SIGHUP: c_int = 1;
const SIGINT: c_int = 2;
const SIGTERM: c_int = 15;

const TERMIOS_WORDS: usize = 64;
static mut SAVED_TERMIOS: [usize; TERMIOS_WORDS] = [0; TERMIOS_WORDS];
static mut SAVED_VALID: bool = false;

extern "C" fn restore_and_exit(signum: c_int) {
    // SAFETY: async-signal-safe calls only (tcsetattr, write, _exit).
    unsafe {
        if *std::ptr::addr_of!(SAVED_VALID) {
            let saved = std::ptr::addr_of!(SAVED_TERMIOS).cast::<std::ffi::c_void>();
            tcsetattr(0, TCSANOW, saved);
        }
        let restore = b"\x1b[0m\x1b[?25h\x1b[?1049l";
        write(1, restore.as_ptr().cast(), restore.len());
        _exit(128 + signum);
    }
}

fn install_signal_handlers() {
    // SAFETY: captures termios before raw mode and installs handlers once at startup.
    unsafe {
        let saved = std::ptr::addr_of_mut!(SAVED_TERMIOS).cast::<std::ffi::c_void>();
        if tcgetattr(0, saved) == 0 {
            *std::ptr::addr_of_mut!(SAVED_VALID) = true;
        }
        signal(SIGINT, restore_and_exit);
        signal(SIGTERM, restore_and_exit);
        signal(SIGHUP, restore_and_exit);
    }
}

#[cfg(target_os = "linux")]
const TIOCGWINSZ: c_ulong = 0x5413;
#[cfg(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
))]
const TIOCGWINSZ: c_ulong = 0x4008_7468;
#[cfg(not(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
)))]
const TIOCGWINSZ: c_ulong = 0x5413;

pub fn window_size() -> (usize, usize) {
    let mut ws = Winsize::default();
    // SAFETY: ioctl is called with a valid pointer to a Winsize we own.
    let rc = unsafe { ioctl(1, TIOCGWINSZ, &mut ws as *mut Winsize) };
    if rc == 0 && ws.cols > 0 && ws.rows > 0 {
        (ws.cols as usize, ws.rows as usize)
    } else {
        (80, 24)
    }
}

fn tty_cmd(args: &[&str]) -> Option<String> {
    let tty = std::fs::File::open("/dev/tty").ok()?;
    let output = Command::new("stty")
        .args(args)
        .stdin(Stdio::from(tty))
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

pub struct Terminal {
    mode: ColorMode,
    visual: Visual,
    renderer: Renderer,
    cells: Vec<Cell>,
    w: usize,
    h: usize,
    saved_stty: Option<String>,
    restored: bool,
}

impl Terminal {
    pub fn new(mode: ColorMode, visual: Visual) -> Self {
        install_signal_handlers();
        let interactive = io::stdin().is_terminal() && io::stdout().is_terminal();
        let saved_stty = if interactive { tty_cmd(&["-g"]) } else { None };
        if saved_stty.is_some() {
            let _ = tty_cmd(&["raw", "-echo"]);
        }

        let mut stdout = io::stdout();
        let title: &[u8] = match visual {
            Visual::Orb => b"\x1b[?1049h\x1b[?25l\x1b[2J\x1b[H\x1b]0;ORBYN\x1b\\",
            Visual::Carrion => b"\x1b[?1049h\x1b[?25l\x1b[2J\x1b[H\x1b]0;ORBYN // CARRION\x1b\\",
        };
        let _ = stdout.write_all(title);
        let _ = stdout.flush();

        let (w, h) = window_size();
        let mut renderer = Renderer::new();
        renderer.reset(w, h);
        Self {
            mode,
            visual,
            renderer,
            cells: Vec::new(),
            w,
            h,
            saved_stty,
            restored: false,
        }
    }

    pub fn sync(&mut self) -> io::Result<(usize, usize)> {
        let (w, h) = window_size();
        if w != self.w || h != self.h {
            self.w = w;
            self.h = h;
            self.renderer.reset(w, h);
            let mut stdout = io::stdout();
            stdout.write_all(b"\x1b[0m\x1b[2J")?;
            stdout.flush()?;
        }
        Ok((w, h))
    }

    pub fn draw(&mut self, grid: &Grid) -> io::Result<()> {
        self.cells = cells_for(grid, self.visual, self.mode);

        let mut buf = Vec::with_capacity(1024);
        self.renderer.present(&self.cells, &mut buf);
        if !buf.is_empty() {
            let mut stdout = io::stdout();
            stdout.write_all(&buf)?;
            stdout.flush()?;
        }
        Ok(())
    }

    pub fn restore(&mut self) {
        if self.restored {
            return;
        }
        self.restored = true;
        let mut stdout = io::stdout();
        let _ = stdout.write_all(b"\x1b[0m\x1b[?25h\x1b[?1049l");
        let _ = stdout.flush();
        if let Some(saved) = self.saved_stty.take() {
            if !saved.is_empty() {
                let _ = tty_cmd(&[saved.as_str()]);
            }
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        self.restore();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Key(u8),
    Eof,
}

pub fn input_events() -> Receiver<Event> {
    let (tx, rx) = mpsc::channel();
    if !io::stdin().is_terminal() {
        return rx;
    }
    thread::spawn(move || {
        let stdin = io::stdin();
        let mut lock = stdin.lock();
        let mut byte = [0u8; 1];
        loop {
            match lock.read(&mut byte) {
                Ok(0) => {
                    let _ = tx.send(Event::Eof);
                    return;
                }
                Ok(_) => {
                    if tx.send(Event::Key(byte[0])).is_err() {
                        return;
                    }
                }
                Err(_) => {
                    let _ = tx.send(Event::Eof);
                    return;
                }
            }
        }
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shade_blank_at_zero() {
        assert_eq!(shade(0.0, ColorMode::TrueColor).0, ' ');
        assert_eq!(shade(-3.0, ColorMode::TrueColor).0, ' ');
    }

    #[test]
    fn ramp_is_monotonic() {
        let mut last = 0;
        for i in 0..=100 {
            let (ch, _) = shade(i as f32 / 100.0, ColorMode::Mono);
            let idx = " .:-=+*#%@".find(ch).unwrap();
            assert!(idx >= last);
            last = idx;
        }
        assert_eq!(shade(1.0, ColorMode::Mono).0, '@');
    }

    #[test]
    fn rgb_grows_toward_white() {
        let (r0, g0, b0) = rgb(0.05);
        let (r1, g1, b1) = rgb(0.95);
        assert!(b1 > b0);
        assert!(g1 > g0);
        assert!(r1 > r0);
    }

    #[test]
    fn fallback_palettes_stay_in_range() {
        for i in 0..=100 {
            let v = i as f32 / 100.0;
            if let (_, Color::Indexed(n)) = shade(v, ColorMode::Ansi256) {
                assert!(n >= 16);
            }
            if let (_, Color::Basic(code)) = shade(v, ColorMode::Ansi16) {
                assert!(matches!(code, 34 | 94 | 96 | 97));
            }
        }
    }

    #[test]
    fn renderer_emits_only_changes() {
        let mut renderer = Renderer::new();
        renderer.reset(4, 2);
        let blank = vec![Cell::BLANK; 8];

        let mut out = Vec::new();
        renderer.present(&blank, &mut out);
        assert!(out.is_empty());

        let mut cells = blank.clone();
        cells[5] = Cell {
            ch: '#',
            color: Color::Rgb(10, 20, 30),
        };
        renderer.present(&cells, &mut out);
        let text = String::from_utf8(out.clone()).expect("ascii output");
        assert!(text.contains("\x1b[2;2H"));
        assert!(text.contains("38;2;10;20;30"));
        assert!(text.ends_with('#'));

        out.clear();
        renderer.present(&cells, &mut out);
        assert!(out.is_empty());

        renderer.present(&blank, &mut out);
        assert!(!out.is_empty());
    }

    #[test]
    fn renderer_reset_drops_shadow_state() {
        let mut renderer = Renderer::new();
        renderer.reset(2, 2);
        let mut cells = vec![Cell::BLANK; 4];
        cells[0] = Cell {
            ch: 'X',
            color: Color::Basic(94),
        };
        let mut out = Vec::new();
        renderer.present(&cells, &mut out);
        renderer.reset(2, 2);
        out.clear();
        renderer.present(&cells, &mut out);
        assert!(out.ends_with(b"X"));
    }

    #[test]
    fn push_u32_formats_numbers() {
        let mut out = Vec::new();
        push_u32(0, &mut out);
        assert_eq!(out, b"0");
        out.clear();
        push_u32(12345, &mut out);
        assert_eq!(out, b"12345");
    }

    #[test]
    fn snapshot_ansi_writes_one_line_per_row() {
        let cells = vec![
            Cell {
                ch: 'A',
                color: Color::Rgb(1, 2, 3),
            };
            6
        ];
        let text = String::from_utf8(snapshot_ansi(&cells, 3)).expect("ascii output");
        let rows: Vec<&str> = text.split("\r\n").collect();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].matches('A').count(), 3);
        assert_eq!(rows[1].matches('A').count(), 3);
        assert!(text.contains("38;2;1;2;3"));
        assert!(text.ends_with("\x1b[0m\r\n"));
    }

    #[test]
    fn snapshot_ansi_blank_cells_emit_no_color() {
        let text = String::from_utf8(snapshot_ansi(&[Cell::BLANK; 4], 2)).expect("ascii output");
        assert!(!text.contains("38;"));
    }

    #[test]
    fn snapshot_ansi_tolerates_zero_width() {
        assert_eq!(snapshot_ansi(&[], 0), b"\x1b[0m\r\n");
    }

    #[test]
    fn cells_for_matches_visual() {
        let mut grid = Grid::new(2, 1);
        grid.stamp(0, 0, 1.0);
        let orb = cells_for(&grid, Visual::Orb, ColorMode::TrueColor);
        let carrion = cells_for(&grid, Visual::Carrion, ColorMode::TrueColor);
        assert_eq!(orb.len(), 2);
        assert_eq!(carrion.len(), 2);
        assert_eq!(orb[0].ch, '@');
        assert_eq!(carrion[0].color, FLESH_RED);
    }

    fn lit(tone: Tone) -> crate::scene::Cell {
        crate::scene::Cell {
            v: 1.0,
            ch: 0,
            tone,
        }
    }

    #[test]
    fn carrion_flesh_is_red_and_groove_is_grey() {
        let flesh = carrion_cell(&lit(Tone::Flesh), ColorMode::TrueColor);
        let groove = carrion_cell(&lit(Tone::Groove), ColorMode::TrueColor);
        let gore = carrion_cell(&lit(Tone::Gore), ColorMode::TrueColor);
        assert_eq!(flesh.color, FLESH_RED);
        assert_eq!(groove.color, GROOVE_GREY);
        assert_eq!(gore.color, FLESH_RED);
        assert_ne!(groove.color, flesh.color);
        assert_ne!(groove.color, Color::Rgb(150, 14, 14));
    }

    #[test]
    fn carrion_resolves_only_two_colors_in_every_mode() {
        let tones = [Tone::Blank, Tone::Flesh, Tone::Groove, Tone::Gore];
        for mode in [
            ColorMode::TrueColor,
            ColorMode::Ansi256,
            ColorMode::Ansi16,
            ColorMode::Mono,
        ] {
            for tone in tones {
                for step in 0..=10 {
                    let cell = crate::scene::Cell {
                        v: step as f32 / 10.0,
                        ch: 0,
                        tone,
                    };
                    let out = carrion_cell(&cell, mode);
                    match mode {
                        ColorMode::TrueColor | ColorMode::Auto => {
                            assert!(matches!(
                                out.color,
                                Color::Default | FLESH_RED | GROOVE_GREY
                            ));
                        }
                        ColorMode::Ansi256 => {
                            assert!(matches!(
                                out.color,
                                Color::Default | Color::Indexed(88) | Color::Indexed(235)
                            ));
                        }
                        ColorMode::Ansi16 => {
                            assert!(matches!(
                                out.color,
                                Color::Default | Color::Basic(31) | Color::Basic(90)
                            ));
                        }
                        ColorMode::Mono => assert_eq!(out.color, Color::Default),
                    }
                }
            }
        }
    }

    #[test]
    fn carrion_glyphs_distinguish_tone_in_mono() {
        let flesh = carrion_cell(&lit(Tone::Flesh), ColorMode::Mono);
        let groove = carrion_cell(&lit(Tone::Groove), ColorMode::Mono);
        let gore = carrion_cell(&lit(Tone::Gore), ColorMode::Mono);
        assert_eq!(flesh.ch, FLESH_CH);
        assert_eq!(groove.ch, GROOVE_CH);
        assert_eq!(gore.ch, GORE_CH);
        assert_ne!(flesh.ch, groove.ch);
    }

    #[test]
    fn carrion_lit_untinted_cell_reads_as_flesh() {
        let cell = crate::scene::Cell {
            v: 0.6,
            ch: b'X',
            tone: Tone::Blank,
        };
        let out = carrion_cell(&cell, ColorMode::TrueColor);
        assert_eq!(out.ch, 'X');
        assert_eq!(out.color, FLESH_RED);
    }

    #[test]
    fn carrion_blank_cell_stays_blank() {
        let out = carrion_cell(&crate::scene::Cell::EMPTY, ColorMode::TrueColor);
        assert_eq!(out, Cell::BLANK);
    }
}
