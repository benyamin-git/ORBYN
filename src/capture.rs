use std::fs::File;
use std::io::{self, BufWriter, Write};

use crate::args::{Capture, Config};
use crate::term::{self, ColorMode, Renderer};
use crate::Sim;

pub const DEFAULT_COLS: usize = 100;
pub const DEFAULT_ROWS: usize = 30;

pub fn run(cfg: &Config, capture: &Capture) -> io::Result<()> {
    let mode = color_mode(cfg.color);
    let w = cfg.cols.unwrap_or(DEFAULT_COLS);
    let h = cfg.rows.unwrap_or(DEFAULT_ROWS);
    let dt = 1.0 / cfg.fps as f32;
    let mut sim = Sim::new(cfg, w, h);

    for _ in 0..frames(cfg.warmup, cfg.fps) {
        sim.update(dt, w, h);
    }

    match capture {
        Capture::Snapshot => {
            let bytes = snapshot(&mut sim, cfg, mode, dt);
            let mut stdout = io::stdout().lock();
            stdout.write_all(&bytes)?;
            stdout.flush()
        }
        Capture::Cast(path) => {
            let mut out = BufWriter::new(File::create(path)?);
            record(&mut sim, cfg, mode, dt, &mut out)?;
            out.flush()
        }
    }
}

fn color_mode(requested: ColorMode) -> ColorMode {
    match requested {
        ColorMode::Auto => ColorMode::TrueColor,
        explicit => term::resolve(explicit),
    }
}

fn frames(seconds: f32, fps: u32) -> u32 {
    (seconds * fps as f32).round() as u32
}

fn snapshot(sim: &mut Sim, cfg: &Config, mode: ColorMode, dt: f32) -> Vec<u8> {
    let (w, h) = (sim.grid().w, sim.grid().h);
    for _ in 0..frames(cfg.duration, cfg.fps).max(1) {
        sim.update(dt, w, h);
    }
    let cells = term::cells_for(sim.grid(), cfg.visual, mode);
    term::snapshot_ansi(&cells, w)
}

fn record(
    sim: &mut Sim,
    cfg: &Config,
    mode: ColorMode,
    dt: f32,
    out: &mut impl Write,
) -> io::Result<()> {
    let (w, h) = (sim.grid().w, sim.grid().h);
    writeln!(
        out,
        "{{\"version\":2,\"width\":{w},\"height\":{h},\"timestamp\":0,\
         \"env\":{{\"TERM\":\"xterm-256color\",\"COLORTERM\":\"truecolor\"}}}}"
    )?;

    let mut renderer = Renderer::new();
    renderer.reset(w, h);
    let mut diff = Vec::with_capacity(8192);
    let mut line = String::with_capacity(16384);

    for i in 0..frames(cfg.duration, cfg.fps).max(1) {
        sim.update(dt, w, h);
        let cells = term::cells_for(sim.grid(), cfg.visual, mode);
        diff.clear();
        renderer.present(&cells, &mut diff);
        if diff.is_empty() {
            continue;
        }
        line.clear();
        line.push('[');
        line.push_str(&format!("{:.4}", i as f64 / cfg.fps as f64));
        line.push_str(",\"o\",\"");
        escape_json(&diff, &mut line);
        line.push_str("\"]");
        writeln!(out, "{line}")?;
    }
    Ok(())
}

fn escape_json(bytes: &[u8], out: &mut String) {
    for &b in bytes {
        match b {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            0x08 => out.push_str("\\b"),
            0x09 => out.push_str("\\t"),
            0x0a => out.push_str("\\n"),
            0x0c => out.push_str("\\f"),
            0x0d => out.push_str("\\r"),
            0x00..=0x1f | 0x7f..=0xff => out.push_str(&format!("\\u{b:04x}")),
            _ => out.push(b as char),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        Config {
            seed: Some(7),
            fps: 20,
            warmup: 0.0,
            duration: 0.5,
            ..Config::default()
        }
    }

    #[test]
    fn auto_color_stays_truecolor_headless() {
        assert_eq!(color_mode(ColorMode::Auto), ColorMode::TrueColor);
        assert_eq!(color_mode(ColorMode::Ansi256), ColorMode::Ansi256);
        assert_eq!(color_mode(ColorMode::Mono), ColorMode::Mono);
    }

    #[test]
    fn snapshot_is_deterministic_and_full_screen() {
        let cfg = cfg();
        let dt = 1.0 / cfg.fps as f32;
        let mut a = Sim::new(&cfg, 40, 12);
        let first = snapshot(&mut a, &cfg, ColorMode::TrueColor, dt);
        let mut b = Sim::new(&cfg, 40, 12);
        let second = snapshot(&mut b, &cfg, ColorMode::TrueColor, dt);
        assert_eq!(first, second);

        let text = String::from_utf8(first).expect("ascii output");
        assert_eq!(text.split("\r\n").count() - 1, 12);
        assert!(text.contains("38;2;"));
    }

    #[test]
    fn record_writes_asciicast_v2() {
        let cfg = cfg();
        let dt = 1.0 / cfg.fps as f32;
        let mut sim = Sim::new(&cfg, 40, 12);
        let mut out = Vec::new();
        record(&mut sim, &cfg, ColorMode::TrueColor, dt, &mut out).expect("record");

        let text = String::from_utf8(out).expect("utf8");
        let mut lines = text.lines();
        let header = lines.next().expect("header");
        assert!(header.contains("\"version\":2"));
        assert!(header.contains("\"width\":40"));
        assert!(header.contains("\"height\":12"));
        assert!(header.contains("COLORTERM"));

        let events: Vec<&str> = lines.collect();
        assert!(!events.is_empty());
        assert!(events.len() <= 10);
        for event in events {
            assert!(event.starts_with('['));
            assert!(event.contains(",\"o\",\""));
            assert!(event.ends_with("\"]"));
        }
    }

    #[test]
    fn record_is_deterministic_for_seed() {
        let cfg = cfg();
        let dt = 1.0 / cfg.fps as f32;
        let run = || {
            let mut sim = Sim::new(&cfg, 40, 12);
            let mut out = Vec::new();
            record(&mut sim, &cfg, ColorMode::TrueColor, dt, &mut out).expect("record");
            out
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn escape_json_escapes_terminal_bytes() {
        let mut out = String::new();
        escape_json(b"\x1b[2J\"\\\n", &mut out);
        assert_eq!(out, "\\u001b[2J\\\"\\\\\\n");
    }
}
