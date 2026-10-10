use std::f32::consts::{PI, TAU};

use crate::args::Config;
use crate::rng::Rng;
use crate::scene::{Grid, Tone};
use crate::ASPECT;

const SPINE_N: usize = 24;
const LENGTH_RATIO: f32 = 10.0;
const FRINGE_MARGIN: f32 = 1.0;
const TRAIL_CAP: usize = 512;
const TRAIL_STEP: f32 = 0.2;
const WEAVE_AMP: f32 = 0.30;
const WEAVE_LEN: f32 = 1.25;
const WANDER_AMP: f32 = 1.4;
const WANDER_LEN: f32 = 3.0;
const TURN_RATE: f32 = 0.15;
const SPAN_RATIO: f32 = 6.6;
const BREATHE_RATE: f32 = 1.7;
const TONE_SCALE: (f32, f32) = (0.32, 0.20);
const TONE_DRIFT: (f32, f32) = (0.22, -0.13);
const TONE_HOT: f32 = 0.62;
const TONE_DARK: f32 = 0.38;
const JITTER_AMP: f32 = 0.16;
const JITTER_RATE: f32 = 2.5;
const DENSITY_SCALE: (f32, f32) = (0.42, 0.27);
const DENSITY_DRIFT: f32 = 0.09;
const SPARK_SCALE: (f32, f32) = (0.68, 0.42);
const SPARK_DRIFT: (f32, f32) = (0.31, -0.17);
const SPARK_BASE: f32 = 0.42;
const SPARK_SLOPE: f32 = 0.45;
const GRAIN_SCALE: (f32, f32) = (1.42, 0.63);
const GRAIN_DRIFT: f32 = 0.23;

type Rect = (usize, usize, usize, usize);

pub struct Worm {
    grid: Grid,
    heading: f32,
    course: f32,
    weave_phase: f32,
    wander_phase: f32,
    dist: f64,
    base_t: f32,
    auto_size: bool,
    speed: f32,
    detail: bool,
    paused: bool,
    time: f32,
    noise_time: f64,
    trail_x: [f32; TRAIL_CAP],
    trail_y: [f32; TRAIL_CAP],
    trail_seg: [f32; TRAIL_CAP],
    trail_len: usize,
    trail_head: usize,
    nodes_x: [f32; SPINE_N],
    nodes_y: [f32; SPINE_N],
    dirty: Option<(i32, i32, i32, i32)>,
}

impl Worm {
    pub fn new(cfg: &Config, w: usize, h: usize) -> Self {
        let mut rng = Rng::new(cfg.seed.unwrap_or_else(Rng::seed_from_time));
        let auto_size = cfg.size.is_none();
        let base_t = fit_thickness(cfg.size.unwrap_or_else(|| auto_thickness(h)), w, h);
        let course = rng.range(0.0, TAU);
        let weave_phase = rng.range(0.0, TAU);
        let wander_phase = rng.range(0.0, 64.0);
        let mut worm = Self {
            grid: Grid::new(w, h),
            heading: course,
            course,
            weave_phase,
            wander_phase,
            dist: 0.0,
            base_t,
            auto_size,
            speed: cfg.speed,
            detail: true,
            paused: false,
            time: 0.0,
            noise_time: 0.0,
            trail_x: [0.0; TRAIL_CAP],
            trail_y: [0.0; TRAIL_CAP],
            trail_seg: [0.0; TRAIL_CAP],
            trail_len: 0,
            trail_head: 0,
            nodes_x: [0.0; SPINE_N],
            nodes_y: [0.0; SPINE_N],
            dirty: None,
        };
        worm.nodes_x[0] = w as f32 * 0.5;
        worm.nodes_y[0] = h as f32 * 0.5;
        worm.heading = worm.desired_heading();
        worm.snap();
        worm
    }

    pub fn update(&mut self, dt: f32, w: usize, h: usize) {
        self.time = (self.time + dt) % TAU;
        self.noise_time += dt as f64;
        let base = base_speed(h, self.speed);
        self.dist += (base * dt) as f64;
        self.advance_heading(dt);
        let wf = w.max(1) as f32;
        let hf = h.max(1) as f32;
        let head_x = self.nodes_x[0] + self.heading.cos() * base * ASPECT * dt;
        let head_y = self.nodes_y[0] + self.heading.sin() * base * dt;
        self.nodes_x[0] = head_x.rem_euclid(wf);
        self.nodes_y[0] = head_y.rem_euclid(hf);
        self.record_trail(w, h);
        self.place_nodes(w, h);
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
        for i in 0..TRAIL_CAP {
            self.trail_x[i] = self.trail_x[i].rem_euclid(wf);
            self.trail_y[i] = self.trail_y[i].rem_euclid(hf);
        }
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

    fn desired_heading(&self) -> f32 {
        let body_len = LENGTH_RATIO * self.base_t;
        let dist = self.dist as f32;
        let wander_q = dist / (WANDER_LEN * body_len) + self.wander_phase;
        let wander = WANDER_AMP * (2.0 * noise2(wander_q, 11.7) - 1.0);
        let weave_q = TAU * dist / (WEAVE_LEN * body_len) + self.weave_phase;
        let weave = WEAVE_AMP * weave_q.sin();
        wrap_angle(self.course + wander + weave)
    }

    fn advance_heading(&mut self, dt: f32) {
        let delta = wrap_delta(self.desired_heading() - self.heading, TAU);
        let step = delta.clamp(-TURN_RATE * dt, TURN_RATE * dt);
        self.heading = wrap_angle(self.heading + step);
    }

    fn push_trail(&mut self, x: f32, y: f32, seg: f32) {
        self.trail_head = (self.trail_head + 1) % TRAIL_CAP;
        self.trail_x[self.trail_head] = x;
        self.trail_y[self.trail_head] = y;
        self.trail_seg[self.trail_head] = seg;
        self.trail_len = (self.trail_len + 1).min(TRAIL_CAP);
    }

    fn record_trail(&mut self, w: usize, h: usize) {
        let wf = w.max(1) as f32;
        let hf = h.max(1) as f32;
        let step = TRAIL_STEP * self.base_t;
        let head_x = self.nodes_x[0];
        let head_y = self.nodes_y[0];
        if self.trail_len == 0 {
            self.push_trail(head_x, head_y, 0.0);
            return;
        }
        let last_x = self.trail_x[self.trail_head];
        let last_y = self.trail_y[self.trail_head];
        let dx = wrap_delta(head_x - last_x, wf);
        let dy = wrap_delta(head_y - last_y, hf);
        let ax = dx / ASPECT;
        let total = (ax * ax + dy * dy).sqrt();
        if total < step {
            return;
        }
        let mut walked = 0.0f32;
        while total - walked >= step {
            walked += step;
            let t = walked / total;
            let x = (last_x + dx * t).rem_euclid(wf);
            let y = (last_y + dy * t).rem_euclid(hf);
            self.push_trail(x, y, step);
        }
    }

    fn place_nodes(&mut self, w: usize, h: usize) {
        let wf = w.max(1) as f32;
        let hf = h.max(1) as f32;
        let head_x = self.nodes_x[0].rem_euclid(wf);
        let head_y = self.nodes_y[0].rem_euclid(hf);
        self.nodes_x[0] = head_x;
        self.nodes_y[0] = head_y;
        let spacing = LENGTH_RATIO * self.base_t / SPINE_N as f32;
        let mut dir_x = -self.heading.cos();
        let mut dir_y = -self.heading.sin();
        let mut sx = head_x;
        let mut sy = head_y;
        let mut arc = 0.0f32;
        let mut samples_left = self.trail_len;
        let mut idx = self.trail_head;
        let mut seg_dx = 0.0f32;
        let mut seg_dy = 0.0f32;
        let mut seg = 0.0f32;
        if samples_left > 0 {
            seg_dx = wrap_delta(self.trail_x[idx] - sx, wf);
            seg_dy = wrap_delta(self.trail_y[idx] - sy, hf);
            let ax = seg_dx / ASPECT;
            seg = (ax * ax + seg_dy * seg_dy).sqrt();
        }
        for i in 1..SPINE_N {
            let target = spacing * i as f32;
            loop {
                if samples_left > 0 && arc + seg >= target {
                    let t = if seg > 1e-6 {
                        (target - arc) / seg
                    } else {
                        1.0
                    };
                    self.nodes_x[i] = (sx + seg_dx * t).rem_euclid(wf);
                    self.nodes_y[i] = (sy + seg_dy * t).rem_euclid(hf);
                    break;
                }
                if samples_left == 0 {
                    let back = target - arc;
                    self.nodes_x[i] = (sx + dir_x * ASPECT * back).rem_euclid(wf);
                    self.nodes_y[i] = (sy + dir_y * back).rem_euclid(hf);
                    break;
                }
                arc += seg;
                sx = (sx + seg_dx).rem_euclid(wf);
                sy = (sy + seg_dy).rem_euclid(hf);
                if seg > 1e-6 {
                    dir_x = (seg_dx / ASPECT) / seg;
                    dir_y = seg_dy / seg;
                }
                let newer = idx;
                samples_left -= 1;
                idx = (idx + TRAIL_CAP - 1) % TRAIL_CAP;
                seg_dx = wrap_delta(self.trail_x[idx] - sx, wf);
                seg_dy = wrap_delta(self.trail_y[idx] - sy, hf);
                seg = self.trail_seg[newer];
            }
        }
    }

    fn snap(&mut self) {
        let wf = self.grid.w.max(1) as f32;
        let hf = self.grid.h.max(1) as f32;
        let step = TRAIL_STEP * self.base_t;
        let needed = (LENGTH_RATIO / TRAIL_STEP).ceil() as usize + 2;
        let head_x = self.nodes_x[0].rem_euclid(wf);
        let head_y = self.nodes_y[0].rem_euclid(hf);
        self.nodes_x[0] = head_x;
        self.nodes_y[0] = head_y;
        self.trail_len = 0;
        self.trail_head = 0;
        let dx = -self.heading.cos() * step * ASPECT;
        let dy = -self.heading.sin() * step;
        for k in (0..=needed).rev() {
            let x = (head_x + dx * k as f32).rem_euclid(wf);
            let y = (head_y + dy * k as f32).rem_euclid(hf);
            let seg = if k == needed { 0.0 } else { step };
            self.push_trail(x, y, seg);
        }
        self.place_nodes(self.grid.w, self.grid.h);
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
        let mut arc = [0.0f32; SPINE_N];
        for i in 1..SPINE_N {
            let dx = (ux[i] - ux[i - 1]) / ASPECT;
            let dy = uy[i] - uy[i - 1];
            arc[i] = arc[i - 1] + (dx * dx + dy * dy).sqrt();
        }
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
                let (margin, along, across) = body_field(px, py, &ux, &uy, &radii, &arc);
                if margin <= 0.0 {
                    let tone = self.tone_at(along, across, sx, sy);
                    let ch = if self.detail {
                        self.core_glyph(along, across)
                    } else {
                        b'#'
                    };
                    self.grid.paint(sx, sy, ch, tone);
                } else if self.detail
                    && margin <= FRINGE_MARGIN
                    && self.fringe_visible(along, across, margin)
                {
                    let tone = self.tone_at(along, across, sx, sy);
                    let ch = self.fringe_glyph(along, across);
                    self.grid.paint(sx, sy, ch, tone);
                }
            }
        }
        self.dirty = Some((x0, y0, x1, y1));
    }

    fn tone_at(&self, along: f32, across: f32, sx: usize, sy: usize) -> Tone {
        let base = noise2(
            along * TONE_SCALE.0 + (self.noise_time * TONE_DRIFT.0 as f64) as f32,
            across * TONE_SCALE.1 + (self.noise_time * TONE_DRIFT.1 as f64) as f32,
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
        let quantum = (self.noise_time * JITTER_RATE as f64) as u32;
        let key = (sx as u32)
            .wrapping_mul(0x9e37_79b9)
            .wrapping_add((sy as u32).wrapping_mul(0x85eb_ca6b))
            .wrapping_add(hash_u32(quantum));
        (hash_unit(hash_u32(key)) - 0.5) * JITTER_AMP
    }

    fn core_glyph(&self, along: f32, across: f32) -> u8 {
        let v = noise2(
            along * DENSITY_SCALE.0 + 7.3,
            across * DENSITY_SCALE.1 - 3.1 + (self.noise_time * DENSITY_DRIFT as f64) as f32,
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

    fn fringe_visible(&self, along: f32, across: f32, margin: f32) -> bool {
        let spark = noise2(
            along * SPARK_SCALE.0 + 12.9 + (self.noise_time * SPARK_DRIFT.0 as f64) as f32,
            across * SPARK_SCALE.1 - 5.7 + (self.noise_time * SPARK_DRIFT.1 as f64) as f32,
        );
        spark > SPARK_BASE + margin * SPARK_SLOPE
    }

    fn fringe_glyph(&self, along: f32, across: f32) -> u8 {
        let v = noise2(
            along * GRAIN_SCALE.0 + 40.1,
            across * GRAIN_SCALE.1 + 17.3 + (self.noise_time * GRAIN_DRIFT as f64) as f32,
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

fn body_field(
    px: f32,
    py: f32,
    ux: &[f32; SPINE_N],
    uy: &[f32; SPINE_N],
    radii: &[f32; SPINE_N],
    arc: &[f32; SPINE_N],
) -> (f32, f32, f32) {
    let mut best = f32::INFINITY;
    let mut along = 0.0;
    let mut across = 0.0;
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
        let dist = (ddx * ddx + ddy * ddy).sqrt();
        let r = radii[i] + (radii[i + 1] - radii[i]) * t;
        let margin = dist - r;
        if margin < best {
            best = margin;
            along = arc[i] + (arc[i + 1] - arc[i]) * t;
            let side = (dx / ASPECT) * ddy - dy * ddx;
            across = if side < 0.0 { -dist } else { dist };
        }
    }
    (best, along, across)
}

fn auto_thickness(h: usize) -> f32 {
    (h as f32 * 0.135).max(1.5)
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
        assert!(max_step < 2.0, "max node step {max_step}");
    }

    #[test]
    fn body_nodes_lie_on_head_path() {
        let (w, h) = (100, 30);
        let mut worm = Worm::new(&config(7), w, h);
        let dt = 1.0 / 30.0;
        let wf = w as f32;
        let hf = h as f32;
        let mut hx = worm.nodes_x[0];
        let mut hy = worm.nodes_y[0];
        let mut hist: Vec<(f32, f32, f64)> = vec![(hx, hy, 0.0)];
        for frame in 0..1800 {
            worm.update(dt, w, h);
            hx += wrap_delta(worm.nodes_x[0] - hx.rem_euclid(wf), wf);
            hy += wrap_delta(worm.nodes_y[0] - hy.rem_euclid(hf), hf);
            hist.push((hx, hy, worm.dist));
            if frame < 900 {
                continue;
            }
            let spacing = LENGTH_RATIO * worm.base_t / SPINE_N as f32;
            for i in 1..SPINE_N {
                let target = worm.dist - (spacing * i as f32) as f64;
                let pos = hist.partition_point(|e| e.2 < target);
                let (ex, ey) = if pos == 0 {
                    (hist[0].0, hist[0].1)
                } else if pos >= hist.len() {
                    let e = hist[hist.len() - 1];
                    (e.0, e.1)
                } else {
                    let a = hist[pos - 1];
                    let b = hist[pos];
                    let t = ((target - a.2) / (b.2 - a.2)) as f32;
                    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
                };
                let dx = wrap_delta(ex - worm.nodes_x[i], wf) / ASPECT;
                let dy = wrap_delta(ey - worm.nodes_y[i], hf);
                let d = (dx * dx + dy * dy).sqrt();
                assert!(
                    d < 0.5,
                    "node {i} off the head path by {d} at frame {frame}"
                );
            }
        }
    }

    #[test]
    fn weave_is_present_and_subtle() {
        let (w, h) = (100, 30);
        let mut worm = Worm::new(&config(7), w, h);
        let dt = 1.0 / 30.0;
        let window = (30.0 / dt) as usize;
        let mut hd = worm.heading;
        let mut headings = vec![hd];
        for _ in 0..window * 2 {
            worm.update(dt, w, h);
            hd += wrap_delta(worm.heading - wrap_angle(hd), TAU);
            headings.push(hd);
        }
        let mut residual = Vec::with_capacity(window);
        for k in window..headings.len() {
            let mean: f32 = headings[k - window..k].iter().sum::<f32>() / window as f32;
            residual.push(headings[k] - mean);
        }
        let rmin = residual.iter().copied().fold(f32::INFINITY, f32::min);
        let rmax = residual.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let p2p = rmax - rmin;
        let mean: f32 = residual.iter().sum::<f32>() / residual.len() as f32;
        let mut sign = 0i32;
        let mut changes = 0usize;
        for &r in &residual {
            let v = r - mean;
            let s = if v > 0.0 {
                1
            } else if v < 0.0 {
                -1
            } else {
                0
            };
            if s != 0 {
                if sign != 0 && s != sign {
                    changes += 1;
                }
                sign = s;
            }
        }
        assert!((0.15..=2.0).contains(&p2p), "weave residual p2p {p2p}");
        assert!(changes >= 2, "weave residual sign changes {changes}");
    }

    fn residual_extrema_distances(speed: f32) -> Vec<f32> {
        let (w, h) = (80, 24);
        let mut worm = Worm::new(&config(41), w, h);
        worm.adjust_speed(speed);
        let dt = 1.0 / 30.0;
        let window = (30.0 / dt) as usize;
        let total = (180.0 / dt) as usize;
        let mut headings = Vec::with_capacity(total + 1);
        let mut dists = Vec::with_capacity(total + 1);
        let mut residuals = Vec::with_capacity(total);
        let mut hd = worm.heading;
        headings.push(hd);
        dists.push(worm.dist);
        for _ in 0..total {
            worm.update(dt, w, h);
            hd += wrap_delta(worm.heading - wrap_angle(hd), TAU);
            headings.push(hd);
            dists.push(worm.dist);
            if headings.len() > window {
                let k = headings.len() - 1;
                let mean: f32 = headings[k - window..k].iter().sum::<f32>() / window as f32;
                residuals.push(headings[k] - mean);
            }
        }
        let mut extrema: Vec<f32> = Vec::new();
        let mut dir = 0i32;
        let mut last_k = 0usize;
        for j in 1..residuals.len() {
            let r = residuals[j];
            let prev = residuals[j - 1];
            let d = if r > prev {
                1
            } else if r < prev {
                -1
            } else {
                dir
            };
            let k = j + window;
            if dir != 0 && d != dir && k - last_k >= 15 {
                extrema.push(dists[k] as f32);
                last_k = k;
            }
            if d != 0 {
                dir = d;
            }
        }
        extrema
    }

    #[test]
    fn weave_wavelength_is_speed_invariant() {
        let slow = residual_extrema_distances(1.0);
        let fast = residual_extrema_distances(2.0);
        assert!(slow.len() >= 4, "too few slow extrema: {}", slow.len());
        assert!(fast.len() >= 4, "too few fast extrema: {}", fast.len());
        let mean_gap = |v: &[f32]| {
            let gaps: Vec<f32> = v.windows(2).map(|p| p[1] - p[0]).collect();
            gaps.iter().sum::<f32>() / gaps.len() as f32
        };
        let ms = mean_gap(&slow);
        let mf = mean_gap(&fast);
        let ratio = mf / ms;
        assert!(
            (0.7..=1.3).contains(&ratio),
            "wavelength ratio {ratio} (slow {ms}, fast {mf})"
        );
    }

    #[test]
    fn heading_does_not_spin_monotonically() {
        let (w, h) = (80, 24);
        let mut worm = Worm::new(&config(53), w, h);
        let dt = 1.0 / 30.0;
        let win = (60.0 / dt) as usize;
        let total = (600.0 / dt) as usize;
        let mut headings = Vec::with_capacity(total + 1);
        let mut hd = worm.heading;
        headings.push(hd);
        for _ in 0..total {
            worm.update(dt, w, h);
            hd += wrap_delta(worm.heading - wrap_angle(hd), TAU);
            headings.push(hd);
        }
        let mut max_net = 0.0f32;
        for k in 0..headings.len() - win {
            max_net = max_net.max((headings[k + win] - headings[k]).abs());
        }
        assert!(max_net < 4.0, "heading net {max_net} rad over 60 s");
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

    fn tone_churn(before: &[Tone], after: &[Tone]) -> (usize, usize) {
        let mut changed = 0usize;
        let mut common = 0usize;
        for (a, b) in before.iter().zip(after.iter()) {
            if *a != Tone::Blank && *b != Tone::Blank {
                common += 1;
                if a != b {
                    changed += 1;
                }
            }
        }
        (changed, common)
    }

    fn tones(worm: &Worm) -> Vec<Tone> {
        worm.grid.data.iter().map(|c| c.tone).collect()
    }

    #[test]
    fn shimmer_changes_over_time() {
        let (w, h) = (80, 24);
        let mut worm = Worm::new(&config(23), w, h);
        for _ in 0..90 {
            worm.update(1.0 / 30.0, w, h);
        }
        let mut sampled = 0usize;
        for row in 0..24 {
            for col in 0..32 {
                let along = col as f32 * 0.4 + 0.5;
                let across = (row as f32 - 12.0) * 0.35;
                worm.noise_time = 0.0;
                let before = worm.tone_at(along, across, col, row);
                worm.noise_time = 3.0;
                let after = worm.tone_at(along, across, col, row);
                if before != after {
                    sampled += 1;
                }
            }
        }
        assert!(sampled > 0, "per-cell tones did not change over time");
        worm.time = 0.0;
        worm.noise_time = 0.0;
        worm.render(w, h);
        let frame = tones(&worm);
        worm.time = TAU / BREATHE_RATE;
        worm.noise_time = (TAU / BREATHE_RATE) as f64;
        worm.render(w, h);
        let (changed, _) = tone_churn(&frame, &tones(&worm));
        assert!(changed > 0, "sampled frames rendered identical tones");
    }

    #[test]
    fn tone_churn_stays_bounded_across_time_wrap() {
        let (w, h) = (80, 24);
        let mut worm = Worm::new(&config(31), w, h);
        for _ in 0..300 {
            worm.update(1.0 / 30.0, w, h);
        }
        worm.time = TAU - 0.1;
        let mut prev = tones(&worm);
        let mut crossed = false;
        let mut crossing = (0usize, 0usize);
        for _ in 0..20 {
            let before = worm.time;
            worm.update(1.0 / 30.0, w, h);
            let wrapped = worm.time < before;
            crossed |= wrapped;
            let cur = tones(&worm);
            let churn = tone_churn(&prev, &cur);
            if wrapped {
                crossing = churn;
            }
            prev = cur;
        }
        assert!(crossed, "time never wrapped during the sample");
        assert!(
            crossing.0 * 3 < crossing.1,
            "tone churn spiked across the wrap: {crossing:?}"
        );
    }

    #[test]
    fn texture_stays_continuous_across_edge_crossing() {
        let (w, h) = (80, 24);
        let mut worm = Worm::new(&config(37), w, h);
        worm.heading = PI;
        worm.nodes_x[0] = 3.0;
        worm.nodes_y[0] = 12.0;
        worm.snap();
        worm.render(w, h);
        let mut prev = tones(&worm);
        let mut crossed = false;
        let mut crossing = (0usize, 0usize);
        for _ in 0..40 {
            let before = worm.nodes_x[0];
            worm.update(1.0 / 30.0, w, h);
            let jumped = before < 5.0 && worm.nodes_x[0] > w as f32 - 5.0;
            crossed |= jumped;
            let cur = tones(&worm);
            let churn = tone_churn(&prev, &cur);
            if jumped {
                crossing = churn;
            }
            prev = cur;
        }
        assert!(crossed, "head never crossed the screen edge");
        assert!(
            crossing.0 * 3 < crossing.1,
            "tone churn spiked across the edge: {crossing:?}"
        );
    }

    #[test]
    fn noise_clock_advances_after_long_uptime() {
        let (w, h) = (80, 24);
        let mut worm = Worm::new(&config(11), w, h);
        worm.noise_time = 3.0e8;
        let before = worm.noise_time;
        let dt = 1.0 / 30.0;
        worm.update(dt, w, h);
        let advanced = worm.noise_time - before;
        assert!(
            (advanced - dt as f64).abs() < 1e-6,
            "noise clock advanced by {advanced} after long uptime"
        );
    }

    #[test]
    fn auto_thickness_grows_past_old_cap_on_tall_screens() {
        assert!(
            auto_thickness(200) > 12.0,
            "auto thickness {} still capped",
            auto_thickness(200)
        );
        let worm = Worm::new(&config(3), 400, 200);
        assert!(
            worm.base_t > 12.0,
            "fitted auto thickness {} still capped",
            worm.base_t
        );
    }
}
