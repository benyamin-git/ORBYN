use std::f32::consts::{PI, TAU};

use crate::args::Config;
use crate::rng::Rng;
use crate::scene::{Grid, Tone};
use crate::ASPECT;

const SPINE_N: usize = 24;
const LENGTH_RATIO: f32 = 10.0;
const FRINGE_MARGIN: f32 = 1.0;
const NODE_K: f32 = 9.0;
const CHAIN_CURL: f32 = 0.18;
const WAVE_AMP: f32 = 0.22;
const WAVE_SPEED: f32 = 1.4;
const WAVE_K: f32 = 0.55;
const SPAN_RATIO: f32 = 6.6;
const BREATHE_RATE: f32 = 1.7;
const TONE_SCALE: (f32, f32) = (0.16, 0.20);
const TONE_DRIFT: (f32, f32) = (0.22, -0.13);
const TONE_HOT: f32 = 0.62;
const TONE_DARK: f32 = 0.38;
const JITTER_AMP: f32 = 0.16;
const JITTER_RATE: f32 = 2.5;
const DENSITY_SCALE: (f32, f32) = (0.21, 0.27);
const DENSITY_DRIFT: f32 = 0.09;
const SPARK_SCALE: (f32, f32) = (0.34, 0.42);
const SPARK_BASE: f32 = 0.42;
const SPARK_SLOPE: f32 = 0.45;
const GRAIN_SCALE: (f32, f32) = (0.71, 0.63);

type Rect = (usize, usize, usize, usize);

pub struct Worm {
    grid: Grid,
    rng: Rng,
    heading: f32,
    turn: f32,
    wander: f32,
    base_t: f32,
    auto_size: bool,
    speed: f32,
    detail: bool,
    paused: bool,
    time: f32,
    nodes_x: [f32; SPINE_N],
    nodes_y: [f32; SPINE_N],
    dirty: Option<(i32, i32, i32, i32)>,
}

impl Worm {
    pub fn new(cfg: &Config, w: usize, h: usize) -> Self {
        let mut rng = Rng::new(cfg.seed.unwrap_or_else(Rng::seed_from_time));
        let auto_size = cfg.size.is_none();
        let base_t = fit_thickness(cfg.size.unwrap_or_else(|| auto_thickness(h)), w, h);
        let heading = rng.range(0.0, TAU);
        let turn = rng.range(-0.4, 0.4);
        let wander = rng.range(1.5, 4.0);
        let mut worm = Self {
            grid: Grid::new(w, h),
            rng,
            heading,
            turn,
            wander,
            base_t,
            auto_size,
            speed: cfg.speed,
            detail: true,
            paused: false,
            time: 0.0,
            nodes_x: [0.0; SPINE_N],
            nodes_y: [0.0; SPINE_N],
            dirty: None,
        };
        worm.nodes_x[0] = w as f32 * 0.5;
        worm.nodes_y[0] = h as f32 * 0.5;
        worm.snap();
        worm
    }

    pub fn update(&mut self, dt: f32, w: usize, h: usize) {
        self.time = (self.time + dt) % TAU;
        self.wander -= dt;
        if self.wander <= 0.0 {
            self.wander = self.rng.range(1.5, 4.0);
            self.turn = self.rng.range(-0.7, 0.7);
        }
        self.heading = wrap_angle(self.heading + self.turn * dt);
        let base = base_speed(h, self.speed);
        let wf = w.max(1) as f32;
        let hf = h.max(1) as f32;
        let head_x = self.nodes_x[0] + self.heading.cos() * base * ASPECT * dt;
        let head_y = self.nodes_y[0] + self.heading.sin() * base * dt;
        self.nodes_x[0] = head_x.rem_euclid(wf);
        self.nodes_y[0] = head_y.rem_euclid(hf);
        self.update_nodes(dt, w, h);
        self.render(w, h);
    }

    pub fn resize(&mut self, w: usize, h: usize) {
        self.grid.resize(w, h);
        if self.auto_size {
            self.base_t = auto_thickness(h);
        }
        self.base_t = fit_thickness(self.base_t, w, h);
        let wf = w.max(1) as f32;
        let hf = h.max(1) as f32;
        for i in 0..SPINE_N {
            self.nodes_x[i] = self.nodes_x[i].rem_euclid(wf);
            self.nodes_y[i] = self.nodes_y[i].rem_euclid(hf);
        }
        self.dirty = None;
    }

    pub fn show_too_small(&mut self, w: usize, h: usize) {
        self.grid.decay(0.0);
        self.dirty = None;
        const MESSAGE: &str = "terminal too small";
        let message = if w < MESSAGE.len() { "..." } else { MESSAGE };
        let y = h / 2;
        let x = w.saturating_sub(message.len()) / 2;
        for (i, byte) in message.bytes().enumerate() {
            self.grid.write(x + i, y, 0.6, byte);
        }
    }

    pub fn toggle_detail(&mut self) {
        self.detail = !self.detail;
    }

    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    pub fn paused(&self) -> bool {
        self.paused
    }

    pub fn adjust_speed(&mut self, ratio: f32) {
        self.speed = (self.speed * ratio).clamp(0.05, 20.0);
    }

    pub fn grid(&self) -> &Grid {
        &self.grid
    }

    fn update_nodes(&mut self, dt: f32, w: usize, h: usize) {
        let wf = w.max(1) as f32;
        let hf = h.max(1) as f32;
        let spacing = LENGTH_RATIO * self.base_t / SPINE_N as f32;
        let k = (NODE_K * dt).min(1.0);
        for i in 1..SPINE_N {
            let px = self.nodes_x[i - 1];
            let py = self.nodes_y[i - 1];
            let cx = self.nodes_x[i];
            let cy = self.nodes_y[i];
            let dx = wrap_delta(cx - px, wf);
            let dy = wrap_delta(cy - py, hf);
            let base_angle = if dx * dx + dy * dy > 1e-6 {
                dy.atan2(dx / ASPECT)
            } else {
                self.heading + PI
            };
            let travel = (self.time * WAVE_SPEED - i as f32 * WAVE_K).sin();
            let angle = base_angle + CHAIN_CURL + WAVE_AMP * travel;
            let tx = px + angle.cos() * spacing * ASPECT;
            let ty = py + angle.sin() * spacing;
            let mx = wrap_delta(tx - cx, wf) * k;
            let my = wrap_delta(ty - cy, hf) * k;
            self.nodes_x[i] = (cx + mx).rem_euclid(wf);
            self.nodes_y[i] = (cy + my).rem_euclid(hf);
        }
    }

    fn snap(&mut self) {
        let wf = self.grid.w.max(1) as f32;
        let hf = self.grid.h.max(1) as f32;
        let spacing = LENGTH_RATIO * self.base_t / SPINE_N as f32;
        let dx = -self.heading.cos() * spacing * ASPECT;
        let dy = -self.heading.sin() * spacing;
        for i in 0..SPINE_N {
            self.nodes_x[i] = (self.nodes_x[0] + dx * i as f32).rem_euclid(wf);
            self.nodes_y[i] = (self.nodes_y[0] + dy * i as f32).rem_euclid(hf);
        }
    }

    fn render(&mut self, w: usize, h: usize) {
        if w == 0 || h == 0 {
            return;
        }
        let wf = w as f32;
        let hf = h as f32;
        if let Some((x0, y0, x1, y1)) = self.dirty.take() {
            for &(rx0, ry0, rx1, ry1) in &wrap_rects(x0, y0, x1, y1, w, h) {
                if rx1 > rx0 && ry1 > ry0 {
                    self.grid.clear_rect(rx0, ry0, rx1, ry1);
                }
            }
        }
        let anchor_x = self.nodes_x[0];
        let anchor_y = self.nodes_y[0];
        let mut ux = [0.0f32; SPINE_N];
        let mut uy = [0.0f32; SPINE_N];
        ux[0] = anchor_x;
        uy[0] = anchor_y;
        for i in 1..SPINE_N {
            ux[i] = ux[i - 1] + wrap_delta(self.nodes_x[i] - ux[i - 1], wf);
            uy[i] = uy[i - 1] + wrap_delta(self.nodes_y[i] - uy[i - 1], hf);
        }
        let radii = self.node_radii();
        let max_r = radii.iter().copied().fold(0.0f32, f32::max);
        let reach_x = (max_r + FRINGE_MARGIN) * ASPECT + 1.0;
        let reach_y = max_r + FRINGE_MARGIN + 1.0;
        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for i in 0..SPINE_N {
            min_x = min_x.min(ux[i]);
            min_y = min_y.min(uy[i]);
            max_x = max_x.max(ux[i]);
            max_y = max_y.max(uy[i]);
        }
        let x0 = (min_x - reach_x).floor() as i32;
        let y0 = (min_y - reach_y).floor() as i32;
        let x1 = (max_x + reach_x).ceil() as i32 + 1;
        let y1 = (max_y + reach_y).ceil() as i32 + 1;
        let wi = w as i32;
        let hi = h as i32;
        for yy in y0..y1 {
            let py = yy as f32 + 0.5;
            let sy = yy.rem_euclid(hi) as usize;
            for xx in x0..x1 {
                let px = xx as f32 + 0.5;
                let sx = xx.rem_euclid(wi) as usize;
                let margin = body_margin(px, py, &ux, &uy, &radii);
                if margin <= 0.0 {
                    let tone = self.tone_at(px, py, sx, sy);
                    let ch = if self.detail {
                        self.core_glyph(px, py)
                    } else {
                        b'#'
                    };
                    self.grid.paint(sx, sy, ch, tone);
                } else if self.detail
                    && margin <= FRINGE_MARGIN
                    && self.fringe_visible(px, py, margin)
                {
                    let tone = self.tone_at(px, py, sx, sy);
                    let ch = self.fringe_glyph(px, py);
                    self.grid.paint(sx, sy, ch, tone);
                }
            }
        }
        self.dirty = Some((x0, y0, x1, y1));
    }

    fn tone_at(&self, px: f32, py: f32, sx: usize, sy: usize) -> Tone {
        let base = noise2(
            px * TONE_SCALE.0 + self.time * TONE_DRIFT.0,
            py * TONE_SCALE.1 + self.time * TONE_DRIFT.1,
        );
        let v = base + self.jitter_at(sx, sy);
        if v > TONE_HOT {
            Tone::WormHot
        } else if v > TONE_DARK {
            Tone::WormMid
        } else {
            Tone::WormDark
        }
    }

    fn jitter_at(&self, sx: usize, sy: usize) -> f32 {
        let quantum = (self.time * JITTER_RATE) as u32;
        let key = (sx as u32)
            .wrapping_mul(0x9e37_79b9)
            .wrapping_add((sy as u32).wrapping_mul(0x85eb_ca6b))
            .wrapping_add(hash_u32(quantum));
        (hash_unit(hash_u32(key)) - 0.5) * JITTER_AMP
    }

    fn core_glyph(&self, px: f32, py: f32) -> u8 {
        let v = noise2(
            px * DENSITY_SCALE.0 + 7.3,
            py * DENSITY_SCALE.1 - 3.1 + self.time * DENSITY_DRIFT,
        );
        if v > 0.86 {
            b'O'
        } else if v > 0.68 {
            b'X'
        } else if v > 0.46 {
            b'%'
        } else {
            b'#'
        }
    }

    fn fringe_visible(&self, px: f32, py: f32, margin: f32) -> bool {
        let spark = noise2(
            px * SPARK_SCALE.0 + 12.9 + self.time * 0.31,
            py * SPARK_SCALE.1 - 5.7 - self.time * 0.17,
        );
        spark > SPARK_BASE + margin * SPARK_SLOPE
    }

    fn fringe_glyph(&self, px: f32, py: f32) -> u8 {
        let v = noise2(
            px * GRAIN_SCALE.0 + 40.1,
            py * GRAIN_SCALE.1 + 17.3 + self.time * 0.23,
        );
        if v > 0.78 {
            b'+'
        } else if v > 0.60 {
            b'='
        } else if v > 0.45 {
            b'-'
        } else if v > 0.30 {
            b':'
        } else {
            b'.'
        }
    }

    fn node_radii(&self) -> [f32; SPINE_N] {
        let mut radii = [0.0f32; SPINE_N];
        for (i, r) in radii.iter_mut().enumerate() {
            let u = (i as f32 + 0.5) / SPINE_N as f32;
            let taper = 0.72 + 0.28 * (PI * u).sin();
            let breathe = 1.0 + 0.08 * (self.time * BREATHE_RATE + i as f32 * 0.9).sin();
            *r = self.base_t * taper * breathe;
        }
        radii
    }
}

fn body_margin(
    px: f32,
    py: f32,
    ux: &[f32; SPINE_N],
    uy: &[f32; SPINE_N],
    radii: &[f32; SPINE_N],
) -> f32 {
    let mut best = f32::INFINITY;
    for i in 0..SPINE_N - 1 {
        let ax = ux[i];
        let ay = uy[i];
        let bx = ux[i + 1];
        let by = uy[i + 1];
        let dx = bx - ax;
        let dy = by - ay;
        let len2 = dx * dx + dy * dy;
        let t = if len2 > 1e-6 {
            (((px - ax) * dx + (py - ay) * dy) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let nx = ax + dx * t;
        let ny = ay + dy * t;
        let ddx = (px - nx) / ASPECT;
        let ddy = py - ny;
        let r = radii[i] + (radii[i + 1] - radii[i]) * t;
        let margin = (ddx * ddx + ddy * ddy).sqrt() - r;
        if margin < best {
            best = margin;
        }
    }
    best
}

fn auto_thickness(h: usize) -> f32 {
    (h as f32 * 0.135).clamp(1.5, 12.0)
}

fn fit_thickness(t: f32, w: usize, h: usize) -> f32 {
    let by_height = (h as f32 - 2.0).max(1.0) / SPAN_RATIO;
    let by_width = ((w as f32 - 2.0).max(1.0) / ASPECT) / SPAN_RATIO;
    t.min(by_height).min(by_width).max(0.5)
}

fn base_speed(h: usize, multiplier: f32) -> f32 {
    (h as f32 * 0.09).clamp(1.2, 12.0) * multiplier
}

fn wrap_delta(d: f32, period: f32) -> f32 {
    if period <= 0.0 {
        return 0.0;
    }
    d - period * (d / period).round()
}

fn wrap_angle(a: f32) -> f32 {
    let a = a % TAU;
    if a < 0.0 {
        a + TAU
    } else {
        a
    }
}

fn split_axis(mut lo: i32, mut hi: i32, n: i32) -> [(usize, usize); 2] {
    if n <= 0 || hi <= lo {
        return [(0, 0), (0, 0)];
    }
    if hi - lo >= n {
        return [(0, n as usize), (0, 0)];
    }
    let shift = lo.div_euclid(n);
    lo -= shift * n;
    hi -= shift * n;
    if hi <= n {
        [(lo as usize, hi as usize), (0, 0)]
    } else {
        [(lo as usize, n as usize), (0, (hi - n) as usize)]
    }
}

fn wrap_rects(x0: i32, y0: i32, x1: i32, y1: i32, w: usize, h: usize) -> [Rect; 4] {
    let mut rects = [(0, 0, 0, 0); 4];
    if w == 0 || h == 0 || x1 <= x0 || y1 <= y0 {
        return rects;
    }
    let xs = split_axis(x0, x1, w as i32);
    let ys = split_axis(y0, y1, h as i32);
    let mut n = 0;
    for x in xs {
        if x.1 <= x.0 {
            continue;
        }
        for y in ys {
            if y.1 <= y.0 {
                continue;
            }
            rects[n] = (x.0, y.0, x.1, y.1);
            n += 1;
        }
    }
    rects
}

fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

fn hash_unit(x: u32) -> f32 {
    hash_u32(x) as f32 / u32::MAX as f32
}

fn noise2(x: f32, y: f32) -> f32 {
    let xi = x.floor();
    let yi = y.floor();
    let xf = x - xi;
    let yf = y - yi;
    let u = xf * xf * (3.0 - 2.0 * xf);
    let v = yf * yf * (3.0 - 2.0 * yf);
    let xi = xi as i32 as u32;
    let yi = yi as i32 as u32;
    let x0 = xi.wrapping_mul(0x8da6_b343);
    let x1 = xi.wrapping_add(1).wrapping_mul(0x8da6_b343);
    let y0 = yi.wrapping_mul(0xd816_3841);
    let y1 = yi.wrapping_add(1).wrapping_mul(0xd816_3841);
    let a = hash_unit(x0.wrapping_add(y0));
    let b = hash_unit(x1.wrapping_add(y0));
    let c = hash_unit(x0.wrapping_add(y1));
    let d = hash_unit(x1.wrapping_add(y1));
    let top = a + (b - a) * u;
    let bot = c + (d - c) * u;
    top + (bot - top) * v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::term::ColorMode;

    fn config(seed: u64) -> Config {
        Config {
            seed: Some(seed),
            color: ColorMode::TrueColor,
            ..Config::default()
        }
    }

    #[test]
    fn nodes_stay_in_torus_bounds() {
        let (w, h) = (80, 24);
        let mut worm = Worm::new(&config(7), w, h);
        for _ in 0..3000 {
            worm.update(1.0 / 30.0, w, h);
            for i in 0..SPINE_N {
                assert!(worm.nodes_x[i].is_finite() && worm.nodes_y[i].is_finite());
                assert!((0.0..=w as f32).contains(&worm.nodes_x[i]));
                assert!((0.0..=h as f32).contains(&worm.nodes_y[i]));
            }
        }
    }

    #[test]
    fn nodes_move_continuously_modulo() {
        let (w, h) = (80, 24);
        let mut worm = Worm::new(&config(29), w, h);
        for _ in 0..300 {
            worm.update(1.0 / 30.0, w, h);
        }
        let mut prev = [(0.0f32, 0.0f32); SPINE_N];
        for (i, p) in prev.iter_mut().enumerate() {
            *p = (worm.nodes_x[i], worm.nodes_y[i]);
        }
        let mut max_step = 0.0f32;
        for _ in 0..3000 {
            worm.update(1.0 / 30.0, w, h);
            for (i, p) in prev.iter_mut().enumerate() {
                let dx = wrap_delta(worm.nodes_x[i] - p.0, w as f32);
                let dy = wrap_delta(worm.nodes_y[i] - p.1, h as f32);
                max_step = max_step.max((dx * dx + dy * dy).sqrt());
                *p = (worm.nodes_x[i], worm.nodes_y[i]);
            }
        }
        assert!(max_step < 3.0, "max node step {max_step}");
    }

    #[test]
    fn head_wraps_across_left_edge() {
        let (w, h) = (80, 24);
        let mut worm = Worm::new(&config(13), w, h);
        worm.nodes_x[0] = 1.0;
        worm.nodes_y[0] = 12.0;
        worm.heading = PI;
        worm.snap();
        for _ in 0..10 {
            worm.update(1.0 / 15.0, w, h);
        }
        assert!(
            worm.nodes_x[0] > w as f32 - 5.0,
            "head x {}",
            worm.nodes_x[0]
        );
        let mut left = false;
        let mut right = false;
        for y in 0..h {
            for x in 0..w {
                if worm.grid.data[y * w + x].tone != Tone::Blank {
                    left |= x < 4;
                    right |= x >= w - 4;
                }
            }
        }
        assert!(left && right, "left {left} right {right}");
    }

    #[test]
    fn old_body_cells_get_cleared() {
        let (w, h) = (80, 24);
        let mut worm = Worm::new(&config(5), w, h);
        worm.heading = 0.0;
        worm.nodes_x[0] = 70.0;
        worm.nodes_y[0] = 20.0;
        worm.snap();
        worm.render(w, h);
        assert_ne!(worm.grid.data[20 * w + 70].tone, Tone::Blank);
        worm.heading = PI / 2.0;
        worm.nodes_x[0] = 4.0;
        worm.nodes_y[0] = 4.0;
        worm.snap();
        worm.render(w, h);
        assert_eq!(worm.grid.data[20 * w + 70].tone, Tone::Blank);
    }

    #[test]
    fn straight_chain_has_no_phantom_seam_cells() {
        let (w, h) = (80, 24);
        let mut worm = Worm::new(&config(5), w, h);
        worm.heading = PI;
        worm.nodes_x[0] = 40.0;
        worm.nodes_y[0] = 12.0;
        worm.snap();
        worm.render(w, h);
        assert_eq!(worm.grid.data[12 * w + 30].tone, Tone::Blank);
        assert_ne!(worm.grid.data[12 * w + 22].tone, Tone::Blank);
    }

    #[test]
    fn body_never_collapses() {
        let (w, h) = (80, 24);
        let mut worm = Worm::new(&config(43), w, h);
        for _ in 0..3000 {
            worm.update(1.0 / 30.0, w, h);
            let lit = worm
                .grid
                .data
                .iter()
                .filter(|c| c.tone != Tone::Blank)
                .count();
            assert!(lit > 60, "body collapsed to {lit} cells");
        }
    }

    #[test]
    fn resize_shrinks_body_into_bounds() {
        let mut worm = Worm::new(&config(3), 120, 40);
        let big = worm.base_t;
        worm.resize(24, 8);
        assert!(worm.base_t < big);
        assert!(worm.base_t * SPAN_RATIO <= 6.0 + 0.01);
        assert!(worm.grid.w == 24 && worm.grid.h == 8);
        for i in 0..SPINE_N {
            assert!(worm.nodes_x[i].is_finite() && worm.nodes_y[i].is_finite());
            assert!(worm.nodes_x[i] <= 24.0 && worm.nodes_y[i] <= 8.0);
        }
    }

    #[test]
    fn show_too_small_writes_message() {
        let (w, h) = (16, 6);
        let mut worm = Worm::new(&config(21), w, h);
        worm.show_too_small(w, h);
        assert!(worm.grid.data.iter().any(|c| c.ch != 0));
    }

    #[test]
    fn speed_adjustment_is_clamped() {
        let (w, h) = (80, 24);
        let mut worm = Worm::new(&config(17), w, h);
        worm.adjust_speed(1000.0);
        assert!(worm.speed <= 20.0);
        worm.adjust_speed(0.0);
        assert!(worm.speed >= 0.05);
    }

    #[test]
    fn pause_and_detail_toggle() {
        let (w, h) = (80, 24);
        let mut worm = Worm::new(&config(19), w, h);
        assert!(!worm.paused());
        worm.toggle_pause();
        assert!(worm.paused());
        let detail = worm.detail;
        worm.toggle_detail();
        assert_ne!(worm.detail, detail);
    }

    #[test]
    fn seeded_grids_are_deterministic() {
        let (w, h) = (80, 24);
        let mut a = Worm::new(&config(99), w, h);
        let mut b = Worm::new(&config(99), w, h);
        for _ in 0..600 {
            a.update(1.0 / 30.0, w, h);
            b.update(1.0 / 30.0, w, h);
        }
        assert_eq!(a.grid.data, b.grid.data);
    }

    #[test]
    fn wrap_rects_split_at_edges() {
        let rects = wrap_rects(-2, 5, 3, 8, 40, 20);
        let live: Vec<Rect> = rects
            .iter()
            .copied()
            .filter(|r| r.2 > r.0 && r.3 > r.1)
            .collect();
        assert_eq!(live.len(), 2);
        assert!(live.contains(&(38, 5, 40, 8)));
        assert!(live.contains(&(0, 5, 3, 8)));

        let rects = wrap_rects(-3, -2, 4, 3, 10, 8);
        let live: Vec<Rect> = rects
            .iter()
            .copied()
            .filter(|r| r.2 > r.0 && r.3 > r.1)
            .collect();
        assert_eq!(live.len(), 4);
        let area: usize = live.iter().map(|r| (r.2 - r.0) * (r.3 - r.1)).sum();
        assert_eq!(area, 35);

        let rects = wrap_rects(2, 3, 8, 9, 40, 20);
        assert_eq!(rects[0], (2, 3, 8, 9));
        assert_eq!(rects[1], (0, 0, 0, 0));
    }

    const ALLOWED_GLYPHS: &[u8] = b"#%XO+=-:.";
    const CORE_GLYPHS: &[u8] = b"#%XO";
    const FRINGE_GLYPHS: &[u8] = b"+=-:.";

    fn lit_count(worm: &Worm) -> usize {
        worm.grid
            .data
            .iter()
            .filter(|c| c.tone != Tone::Blank)
            .count()
    }

    #[test]
    fn only_allowed_glyphs() {
        let (w, h) = (80, 24);
        let mut worm = Worm::new(&config(7), w, h);
        let mut seen = [false; 4];
        for _ in 0..240 {
            worm.update(1.0 / 30.0, w, h);
            assert!(lit_count(&worm) > 0);
            for cell in &worm.grid.data {
                if cell.tone == Tone::Blank {
                    continue;
                }
                assert!(
                    ALLOWED_GLYPHS.contains(&cell.ch),
                    "lit glyph {:?} outside the nine-glyph set",
                    cell.ch as char
                );
                if let Some(i) = CORE_GLYPHS.iter().position(|g| *g == cell.ch) {
                    seen[i] = true;
                }
            }
        }
        assert!(
            seen.iter().all(|s| *s),
            "core glyph variety missing: {seen:?}"
        );
    }

    #[test]
    fn only_worm_tones() {
        let (w, h) = (80, 24);
        let mut worm = Worm::new(&config(7), w, h);
        let mut tones = [false; 3];
        for _ in 0..240 {
            worm.update(1.0 / 30.0, w, h);
            assert!(lit_count(&worm) > 0);
            for cell in &worm.grid.data {
                if cell.tone == Tone::Blank {
                    continue;
                }
                match cell.tone {
                    Tone::WormDark => tones[0] = true,
                    Tone::WormMid => tones[1] = true,
                    Tone::WormHot => tones[2] = true,
                    other => panic!("lit cell tone {other:?} outside worm palette"),
                }
            }
        }
        assert!(
            tones.iter().filter(|t| **t).count() >= 2,
            "worm palette barely used: {tones:?}"
        );
    }

    #[test]
    fn fringe_present_when_detail_on() {
        let (w, h) = (80, 24);
        let mut worm = Worm::new(&config(7), w, h);
        let mut fringe = 0usize;
        let mut core = 0usize;
        for _ in 0..240 {
            worm.update(1.0 / 30.0, w, h);
            for cell in &worm.grid.data {
                if cell.tone == Tone::Blank {
                    continue;
                }
                if FRINGE_GLYPHS.contains(&cell.ch) {
                    fringe += 1;
                }
                if CORE_GLYPHS.contains(&cell.ch) {
                    core += 1;
                }
            }
        }
        assert!(fringe > 0, "no fringe glyphs while detail is on");
        assert!(core > 0, "no core glyphs while detail is on");
    }

    #[test]
    fn fringe_absent_when_detail_off() {
        let (w, h) = (80, 24);
        let mut worm = Worm::new(&config(7), w, h);
        for _ in 0..120 {
            worm.update(1.0 / 30.0, w, h);
        }
        worm.toggle_detail();
        for _ in 0..240 {
            worm.update(1.0 / 30.0, w, h);
            let mut core = 0usize;
            for cell in &worm.grid.data {
                if cell.tone == Tone::Blank {
                    continue;
                }
                assert!(
                    !FRINGE_GLYPHS.contains(&cell.ch),
                    "fringe glyph {:?} while detail is off",
                    cell.ch as char
                );
                if CORE_GLYPHS.contains(&cell.ch) {
                    core += 1;
                }
            }
            assert!(core > 0, "core missing while detail is off");
        }
    }

    #[test]
    fn hot_tones_appear_across_the_body() {
        let (w, h) = (100, 30);
        let mut worm = Worm::new(&config(11), w, h);
        let mut interior = false;
        let mut edge = false;
        for _ in 0..1200 {
            worm.update(1.0 / 30.0, w, h);
            for y in 0..h {
                for x in 0..w {
                    if worm.grid.data[y * w + x].tone != Tone::WormHot {
                        continue;
                    }
                    let neighbors = [
                        ((x + w - 1) % w, y),
                        ((x + 1) % w, y),
                        (x, (y + h - 1) % h),
                        (x, (y + 1) % h),
                    ];
                    let lit = neighbors
                        .iter()
                        .filter(|(nx, ny)| worm.grid.data[ny * w + nx].tone != Tone::Blank)
                        .count();
                    edge |= lit < neighbors.len();
                    interior |= lit == neighbors.len();
                }
            }
            if interior && edge {
                break;
            }
        }
        assert!(interior, "no WormHot cells in the body interior");
        assert!(edge, "no WormHot cells adjacent to blank cells");
    }

    #[test]
    fn shimmer_changes_over_time() {
        let (w, h) = (80, 24);
        let mut worm = Worm::new(&config(23), w, h);
        for _ in 0..90 {
            worm.update(1.0 / 30.0, w, h);
        }
        worm.time = 0.0;
        worm.render(w, h);
        let frame: Vec<Tone> = worm.grid.data.iter().map(|c| c.tone).collect();
        let mut sampled = 0usize;
        for y in 0..h {
            for x in 0..w {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                worm.time = 0.0;
                let before = worm.tone_at(px, py, x, y);
                worm.time = TAU / BREATHE_RATE;
                let after = worm.tone_at(px, py, x, y);
                if before != after {
                    sampled += 1;
                }
            }
        }
        assert!(sampled > 0, "per-cell tones did not change over time");
        worm.time = TAU / BREATHE_RATE;
        worm.render(w, h);
        let changed = frame
            .iter()
            .zip(worm.grid.data.iter())
            .filter(|(old, new)| {
                **old != Tone::Blank && new.tone != Tone::Blank && **old != new.tone
            })
            .count();
        assert!(changed > 0, "sampled frames rendered identical tones");
    }
}
