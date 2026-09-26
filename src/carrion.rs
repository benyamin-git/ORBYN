use std::f32::consts::{PI, TAU};

use crate::args::Config;
use crate::rng::Rng;
use crate::scene::{Grid, Tone};
use crate::ASPECT;

const RING_N: usize = 14;
const FOLDS: f32 = 3.0;
const GROOVE_WIDTH: f32 = 0.34;
const GROOVE_PHASE: f32 = 0.45;
const TEARDROP: f32 = 0.35;
const NODE_K: f32 = 25.0;
const NODE_DAMP: f32 = 6.0;
const SHED_N: usize = 16;
const GORE_RAMP: &[u8] = b"#+.-";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Crawl,
    Grip,
    Lunge,
}

#[derive(Clone, Copy)]
struct Node {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
}

#[derive(Clone, Copy)]
struct Shed {
    x: f32,
    y: f32,
    life: f32,
    max: f32,
}

impl Shed {
    const DEAD: Shed = Shed {
        x: 0.0,
        y: 0.0,
        life: 0.0,
        max: 1.0,
    };
}

pub struct Carrion {
    grid: Grid,
    rng: Rng,
    hub_x: f32,
    hub_y: f32,
    hub_vx: f32,
    hub_vy: f32,
    body_x: f32,
    body_y: f32,
    heading: f32,
    body_heading: f32,
    turn: f32,
    base_r: f32,
    auto_size: bool,
    speed: f32,
    detail: bool,
    paused: bool,
    time: f32,
    wander: f32,
    mode: Mode,
    mode_timer: f32,
    roam_timer: f32,
    wall_cooldown: f32,
    grip_x: f32,
    grip_y: f32,
    aim: f32,
    stretch: f32,
    stretch_target: f32,
    nodes: [Node; RING_N],
    profile: [f32; RING_N],
    shed: [Shed; SHED_N],
    shed_next: usize,
    trail: f32,
    mask: Vec<bool>,
    dirty: Option<(usize, usize, usize, usize)>,
}

impl Carrion {
    pub fn new(cfg: &Config, w: usize, h: usize) -> Self {
        let mut rng = Rng::new(cfg.seed.unwrap_or_else(Rng::seed_from_time));
        let auto_size = cfg.size.is_none();
        let base_r = fit_radius(cfg.size.unwrap_or_else(|| auto_radius(w, h)), w, h);
        let heading = rng.range(0.0, TAU);
        let turn = rng.range(-0.4, 0.4);
        let wander = rng.range(1.5, 4.0);
        let roam_timer = rng.range(2.5, 5.0);
        let hub_x = w as f32 * 0.5;
        let hub_y = h as f32 * 0.5;
        let nodes = [Node {
            x: hub_x,
            y: hub_y,
            vx: 0.0,
            vy: 0.0,
        }; RING_N];
        let mut carrion = Self {
            grid: Grid::new(w, h),
            rng,
            hub_x,
            hub_y,
            hub_vx: 0.0,
            hub_vy: 0.0,
            body_x: hub_x,
            body_y: hub_y,
            heading,
            body_heading: heading,
            turn,
            base_r,
            auto_size,
            speed: cfg.speed,
            detail: true,
            paused: false,
            time: 0.0,
            wander,
            mode: Mode::Crawl,
            mode_timer: 0.0,
            roam_timer,
            wall_cooldown: 0.0,
            grip_x: hub_x,
            grip_y: hub_y,
            aim: heading,
            stretch: 1.0,
            stretch_target: 1.0,
            nodes,
            profile: [0.0; RING_N],
            shed: [Shed::DEAD; SHED_N],
            shed_next: 0,
            trail: cfg.trail,
            mask: Vec::new(),
            dirty: None,
        };
        carrion.snap_nodes();
        carrion
    }

    pub fn update(&mut self, dt: f32, w: usize, h: usize) {
        self.time = (self.time + dt) % TAU;
        self.step_mode(dt);
        self.update_heading(dt);
        self.body_heading = steer(self.body_heading, self.heading, dt * 1.5);
        self.update_hub(dt, w, h);
        self.update_nodes(dt, w, h);
        self.update_body_center();
        self.update_shed(dt);
        let rate = (dt * 6.0).min(1.0);
        self.stretch += (self.stretch_target - self.stretch) * rate;
        self.render();
    }

    pub fn resize(&mut self, w: usize, h: usize) {
        self.grid.resize(w, h);
        if self.auto_size {
            self.base_r = auto_radius(w, h);
        }
        self.base_r = fit_radius(self.base_r, w, h);
        self.hub_x = self.hub_x.clamp(0.5, (w as f32 - 0.5).max(0.5));
        self.hub_y = self.hub_y.clamp(0.5, (h as f32 - 0.5).max(0.5));
        for node in &mut self.nodes {
            node.x = node.x.clamp(0.5, (w as f32 - 0.5).max(0.5));
            node.y = node.y.clamp(0.5, (h as f32 - 0.5).max(0.5));
            node.vx = 0.0;
            node.vy = 0.0;
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

    fn step_mode(&mut self, dt: f32) {
        self.wall_cooldown = (self.wall_cooldown - dt).max(0.0);
        match self.mode {
            Mode::Crawl => {
                self.wander -= dt;
                if self.wander <= 0.0 {
                    self.wander = self.rng.range(1.5, 4.0);
                    self.turn = self.rng.range(-0.7, 0.7);
                }
                self.roam_timer -= dt;
                let (_, _, wall_dist) = self.wall_hit();
                let near_wall = wall_dist < self.base_r * 1.4 && self.wall_cooldown <= 0.0;
                if self.roam_timer <= 0.0 || near_wall {
                    self.mode = Mode::Grip;
                    self.mode_timer = self.rng.range(0.8, 1.6);
                    let pull = self.base_r * 0.5;
                    let (wx, wy, _) = self.wall_hit();
                    self.grip_x = wx - self.heading.cos() * pull * ASPECT;
                    self.grip_y = wy - self.heading.sin() * pull;
                    self.stretch_target = 0.75;
                }
            }
            Mode::Grip => {
                self.mode_timer -= dt;
                if self.mode_timer <= 0.0 {
                    self.mode = Mode::Lunge;
                    self.mode_timer = self.rng.range(0.5, 0.9);
                    self.lunge();
                    self.stretch_target = 1.3;
                    let (rx, ry) = self.rear_point();
                    let (jx, jy) = (self.rng.range(-1.5, 1.5), self.rng.range(-1.5, 1.5));
                    self.spawn_shed(rx, ry);
                    self.spawn_shed(rx + jx, ry + jy);
                }
            }
            Mode::Lunge => {
                self.mode_timer -= dt;
                if self.mode_timer <= 0.0 {
                    self.mode = Mode::Crawl;
                    self.roam_timer = self.rng.range(2.5, 5.0);
                    self.wall_cooldown = 1.0;
                    self.stretch_target = 1.0;
                }
            }
        }
    }

    fn update_heading(&mut self, dt: f32) {
        match self.mode {
            Mode::Crawl => {
                self.heading = wrap_angle(self.heading + self.turn * dt);
            }
            Mode::Grip => {
                let dx = (self.grip_x - self.hub_x) / ASPECT;
                let dy = self.grip_y - self.hub_y;
                let target = dy.atan2(dx);
                self.heading = steer(self.heading, target, dt * 2.5);
            }
            Mode::Lunge => {
                self.heading = steer(self.heading, self.aim, dt * 3.0);
            }
        }
    }

    fn update_hub(&mut self, dt: f32, w: usize, h: usize) {
        let base = base_speed(h, self.speed);
        match self.mode {
            Mode::Crawl => {
                let tx = self.heading.cos() * base * ASPECT;
                let ty = self.heading.sin() * base;
                let k = (dt * 3.0).min(1.0);
                self.hub_vx += (tx - self.hub_vx) * k;
                self.hub_vy += (ty - self.hub_vy) * k;
            }
            Mode::Grip => {
                let dx = (self.grip_x - self.hub_x) / ASPECT;
                let dy = self.grip_y - self.hub_y;
                let dist = (dx * dx + dy * dy).sqrt();
                let approach = base * 2.5;
                let (tx, ty) = if dist > 0.05 {
                    (dx / dist * approach * ASPECT, dy / dist * approach)
                } else {
                    (0.0, 0.0)
                };
                let k = (dt * 4.0).min(1.0);
                self.hub_vx += (tx - self.hub_vx) * k;
                self.hub_vy += (ty - self.hub_vy) * k;
            }
            Mode::Lunge => {
                let drag = (dt * 1.2).min(0.6);
                self.hub_vx *= 1.0 - drag;
                self.hub_vy *= 1.0 - drag;
            }
        }
        self.hub_x += self.hub_vx * dt;
        self.hub_y += self.hub_vy * dt;

        let min_x = 1.0;
        let max_x = (w as f32 - 1.0).max(min_x);
        let min_y = 1.0;
        let max_y = (h as f32 - 1.0).max(min_y);
        if self.hub_x < min_x {
            self.hub_x = min_x;
            self.hub_vx = self.hub_vx.max(0.0);
        } else if self.hub_x > max_x {
            self.hub_x = max_x;
            self.hub_vx = self.hub_vx.min(0.0);
        }
        if self.hub_y < min_y {
            self.hub_y = min_y;
            self.hub_vy = self.hub_vy.max(0.0);
        } else if self.hub_y > max_y {
            self.hub_y = max_y;
            self.hub_vy = self.hub_vy.min(0.0);
        }
    }

    fn update_nodes(&mut self, dt: f32, w: usize, h: usize) {
        let k = (NODE_K * dt).min(0.6);
        let damp = 1.0 / (1.0 + NODE_DAMP * dt);
        let min_x = 0.5;
        let max_x = (w as f32 - 0.5).max(min_x);
        let min_y = 0.5;
        let max_y = (h as f32 - 0.5).max(min_y);
        for i in 0..RING_N {
            let (tx, ty) = self.slot_position(i);
            let node = &mut self.nodes[i];
            node.vx = (node.vx + (tx - node.x) * k) * damp;
            node.vy = (node.vy + (ty - node.y) * k) * damp;
            node.x += node.vx * dt;
            node.y += node.vy * dt;
            if node.x < min_x {
                node.x = min_x;
                node.vx = node.vx.max(0.0);
            } else if node.x > max_x {
                node.x = max_x;
                node.vx = node.vx.min(0.0);
            }
            if node.y < min_y {
                node.y = min_y;
                node.vy = node.vy.max(0.0);
            } else if node.y > max_y {
                node.y = max_y;
                node.vy = node.vy.min(0.0);
            }
        }
    }

    fn lunge(&mut self) {
        let mut dx = (self.hub_x - self.grip_x) / ASPECT;
        let mut dy = self.hub_y - self.grip_y;
        if dx * dx + dy * dy < 1e-4 {
            dx = -self.heading.cos();
            dy = -self.heading.sin();
        }
        let aim = wrap_angle(dy.atan2(dx) + self.rng.range(-0.5, 0.5));
        let base = base_speed(self.grid.h, self.speed);
        let impulse = (base * 6.0).clamp(6.0, 40.0);
        self.aim = aim;
        let vx = aim.cos() * impulse * ASPECT;
        let vy = aim.sin() * impulse;
        self.hub_vx += vx;
        self.hub_vy += vy;
        for node in &mut self.nodes {
            node.vx += vx;
            node.vy += vy;
        }
    }

    fn wall_hit(&self) -> (f32, f32, f32) {
        let w = self.grid.w as f32;
        let h = self.grid.h as f32;
        let dx = self.heading.cos() * ASPECT;
        let dy = self.heading.sin();
        let mut px = self.hub_x;
        let mut py = self.hub_y;
        let mut dist = 0.0;
        while dist < 1024.0 {
            let nx = px + dx * 0.5;
            let ny = py + dy * 0.5;
            dist += 0.5;
            if nx < 0.0 || nx > w || ny < 0.0 || ny > h {
                break;
            }
            px = nx;
            py = ny;
        }
        (px, py, dist)
    }

    fn slot_position(&self, i: usize) -> (f32, f32) {
        let theta = TAU * i as f32 / RING_N as f32;
        let wobble = 1.0 + 0.05 * (self.time * 3.0 + i as f32 * 2.3).sin();
        let r = self.base_r * (1.0 - TEARDROP * theta.cos()) * self.stretch * wobble;
        let along = r * theta.cos();
        let across = r * theta.sin();
        let cos = self.body_heading.cos();
        let sin = self.body_heading.sin();
        (
            self.hub_x + (along * cos - across * sin) * ASPECT,
            self.hub_y + (along * sin + across * cos),
        )
    }

    fn snap_nodes(&mut self) {
        for i in 0..RING_N {
            let (x, y) = self.slot_position(i);
            self.nodes[i] = Node {
                x,
                y,
                vx: 0.0,
                vy: 0.0,
            };
        }
    }

    fn update_body_center(&mut self) {
        let mut sx = 0.0;
        let mut sy = 0.0;
        for node in &self.nodes {
            sx += node.x;
            sy += node.y;
        }
        self.body_x = sx / RING_N as f32;
        self.body_y = sy / RING_N as f32;
    }

    fn rear_point(&self) -> (f32, f32) {
        let r = self.base_r * (1.0 + TEARDROP) * self.stretch;
        let cos = self.body_heading.cos();
        let sin = self.body_heading.sin();
        (self.body_x - r * cos * ASPECT, self.body_y - r * sin)
    }

    fn update_shed(&mut self, dt: f32) {
        for shed in &mut self.shed {
            if shed.life > 0.0 {
                shed.life -= dt;
            }
        }
    }

    fn spawn_shed(&mut self, x: f32, y: f32) {
        let max = 1.0 + self.trail * 8.0;
        let slot = self.shed_next;
        self.shed_next = (self.shed_next + 1) % SHED_N;
        self.shed[slot] = Shed {
            x,
            y,
            life: max,
            max,
        };
    }

    fn render(&mut self) {
        if let Some((x0, y0, x1, y1)) = self.dirty.take() {
            self.grid.clear_rect(x0, y0, x1, y1);
        }
        self.update_body_center();
        self.refresh_profile();

        let mut reach = self.base_r * 0.5;
        for node in &self.nodes {
            let dx = (node.x - self.body_x) / ASPECT;
            let dy = node.y - self.body_y;
            reach = reach.max((dx * dx + dy * dy).sqrt());
        }
        let reach = reach + 1.5;
        let x0 = (self.body_x - reach * ASPECT).floor().max(0.0) as usize;
        let y0 = (self.body_y - reach).floor().max(0.0) as usize;
        let x1 = ((self.body_x + reach * ASPECT).ceil().max(0.0) as usize + 1).min(self.grid.w);
        let y1 = ((self.body_y + reach).ceil().max(0.0) as usize + 1).min(self.grid.h);
        let x0 = x0.min(x1);
        let y0 = y0.min(y1);
        let mut dirty = (x0, y0, x1, y1);

        for shed in &self.shed {
            if shed.life <= 0.0 {
                continue;
            }
            let frac = (shed.life / shed.max).clamp(0.0, 1.0);
            let idx = ((1.0 - frac) * (GORE_RAMP.len() - 1) as f32).round() as usize;
            let ch = GORE_RAMP[idx.min(GORE_RAMP.len() - 1)];
            stamp_disc(
                &mut self.grid,
                &mut dirty,
                shed.x,
                shed.y,
                1.2,
                Tone::Gore,
                ch,
            );
        }

        let bw = x1 - x0;
        let bh = y1 - y0;
        if self.mask.len() < bw * bh {
            self.mask.resize(bw * bh, false);
        }

        let cos = self.body_heading.cos();
        let sin = self.body_heading.sin();

        for yy in 0..bh {
            let y = y0 + yy;
            let ly = y as f32 + 0.5 - self.body_y;
            for xx in 0..bw {
                let x = x0 + xx;
                let lx = (x as f32 + 0.5 - self.body_x) / ASPECT;
                let rho = (lx * lx + ly * ly).sqrt();
                let along = lx * cos + ly * sin;
                let across = -lx * sin + ly * cos;
                let theta = wrap_angle(across.atan2(along));
                self.mask[yy * bw + xx] = rho <= self.profile_at(theta);
            }
        }

        let time = self.time;
        let detail = self.detail;
        for yy in 0..bh {
            let y = y0 + yy;
            let ly = y as f32 + 0.5 - self.body_y;
            for xx in 0..bw {
                if !self.mask[yy * bw + xx] {
                    continue;
                }
                let x = x0 + xx;
                let edge = xx == 0
                    || yy == 0
                    || xx + 1 == bw
                    || yy + 1 == bh
                    || !self.mask[(yy - 1) * bw + xx]
                    || !self.mask[(yy + 1) * bw + xx]
                    || !self.mask[yy * bw + xx - 1]
                    || !self.mask[yy * bw + xx + 1];
                let tone = if edge {
                    Tone::Flesh
                } else {
                    let lx = (x as f32 + 0.5 - self.body_x) / ASPECT;
                    let along = lx * cos + ly * sin;
                    let across = -lx * sin + ly * cos;
                    let theta = wrap_angle(across.atan2(along));
                    let r = self.profile_at(theta);
                    let rho = (lx * lx + ly * ly).sqrt();
                    let warp = if detail {
                        1.4 * (noise2(
                            along * 0.30 + 0.7 * (time * 2.0).sin(),
                            across * 0.30 + 0.7 * time.cos(),
                        ) - 0.5)
                    } else {
                        0.0
                    };
                    let phase = (rho / r * FOLDS + GROOVE_PHASE + warp).rem_euclid(1.0);
                    if phase < GROOVE_WIDTH {
                        Tone::Groove
                    } else {
                        Tone::Flesh
                    }
                };
                self.grid.paint(x, y, 0, tone);
            }
        }
        self.dirty = Some(dirty);
    }

    fn refresh_profile(&mut self) {
        let cos = self.body_heading.cos();
        let sin = self.body_heading.sin();
        for i in 0..RING_N {
            let theta = TAU * i as f32 / RING_N as f32;
            let wobble = 1.0 + 0.05 * (self.time * 3.0 + i as f32 * 2.3).sin();
            let slot_r = self.base_r * (1.0 - TEARDROP * theta.cos()) * self.stretch * wobble;
            let ox = (self.nodes[i].x - self.body_x) / ASPECT;
            let oy = self.nodes[i].y - self.body_y;
            let bx = ox * cos + oy * sin;
            let by = -ox * sin + oy * cos;
            let r = bx * theta.cos() + by * theta.sin();
            self.profile[i] = r.clamp(slot_r * 0.55, slot_r * 1.45);
        }
    }

    fn profile_at(&self, theta: f32) -> f32 {
        let t = theta / TAU * RING_N as f32;
        let i = t as usize % RING_N;
        let f = t - t.floor();
        let a = self.profile[i];
        let b = self.profile[(i + 1) % RING_N];
        a + (b - a) * f
    }
}

fn stamp_disc(
    grid: &mut Grid,
    dirty: &mut (usize, usize, usize, usize),
    cx: f32,
    cy: f32,
    r: f32,
    tone: Tone,
    ch: u8,
) {
    let x0 = ((cx - r * ASPECT).floor().max(0.0) as usize).min(grid.w);
    let y0 = ((cy - r).floor().max(0.0) as usize).min(grid.h);
    let x1 = ((cx + r * ASPECT).ceil().max(0.0) as usize + 1).min(grid.w);
    let y1 = ((cy + r).ceil().max(0.0) as usize + 1).min(grid.h);
    for y in y0..y1 {
        let dy = y as f32 + 0.5 - cy;
        for x in x0..x1 {
            let dx = (x as f32 + 0.5 - cx) / ASPECT;
            if dx * dx + dy * dy <= r * r {
                grid.paint(x, y, ch, tone);
            }
        }
    }
    dirty.0 = dirty.0.min(x0);
    dirty.1 = dirty.1.min(y0);
    dirty.2 = dirty.2.max(x1);
    dirty.3 = dirty.3.max(y1);
}

fn auto_radius(w: usize, h: usize) -> f32 {
    let by_height = h as f32 / 4.5;
    let by_width = w as f32 / 9.0;
    by_height.min(by_width).clamp(2.0, 14.0)
}

fn fit_radius(r: f32, w: usize, h: usize) -> f32 {
    let max_by_height = (h as f32 * 0.5 - 1.0).max(1.0);
    let max_by_width = ((w as f32 * 0.5 - 1.0) / ASPECT).max(1.0);
    r.min(max_by_height).min(max_by_width).max(1.0)
}

fn base_speed(h: usize, multiplier: f32) -> f32 {
    (h as f32 * 0.09).clamp(1.2, 12.0) * multiplier
}

fn wrap_angle(a: f32) -> f32 {
    let a = a % TAU;
    if a < 0.0 {
        a + TAU
    } else {
        a
    }
}

fn steer(current: f32, target: f32, max_step: f32) -> f32 {
    let mut diff = (target - current) % TAU;
    if diff > PI {
        diff -= TAU;
    } else if diff < -PI {
        diff += TAU;
    }
    wrap_angle(current + diff.clamp(-max_step, max_step))
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
    fn hub_and_nodes_stay_on_screen() {
        let (w, h) = (80, 24);
        let mut carrion = Carrion::new(&config(7), w, h);
        for _ in 0..3000 {
            carrion.update(1.0 / 30.0, w, h);
            assert!(carrion.hub_x >= 0.0 && carrion.hub_x <= w as f32);
            assert!(carrion.hub_y >= 0.0 && carrion.hub_y <= h as f32);
            for node in &carrion.nodes {
                assert!(node.x.is_finite() && node.y.is_finite());
                assert!(node.x >= 0.49 && node.x <= w as f32 - 0.49);
                assert!(node.y >= 0.49 && node.y <= h as f32 - 0.49);
            }
        }
    }

    #[test]
    fn mode_cycles_through_all_states() {
        let (w, h) = (80, 24);
        let mut carrion = Carrion::new(&config(31), w, h);
        let (mut crawl, mut grip, mut lunge) = (false, false, false);
        for _ in 0..6000 {
            carrion.update(1.0 / 30.0, w, h);
            match carrion.mode {
                Mode::Crawl => crawl = true,
                Mode::Grip => grip = true,
                Mode::Lunge => lunge = true,
            }
        }
        assert!(
            crawl && grip && lunge,
            "states seen: {crawl} {grip} {lunge}"
        );
    }

    #[test]
    fn nodes_do_not_teleport() {
        let (w, h) = (80, 24);
        let mut carrion = Carrion::new(&config(29), w, h);
        for _ in 0..600 {
            carrion.update(1.0 / 30.0, w, h);
        }
        let mut prev: [(f32, f32); RING_N] = carrion.nodes.map(|n| (n.x, n.y));
        let mut max_step = 0.0f32;
        for _ in 0..3000 {
            carrion.update(1.0 / 30.0, w, h);
            for (i, node) in carrion.nodes.iter().enumerate() {
                let dx = node.x - prev[i].0;
                let dy = node.y - prev[i].1;
                max_step = max_step.max((dx * dx + dy * dy).sqrt());
                prev[i] = (node.x, node.y);
            }
        }
        assert!(max_step < 2.0, "max node step {max_step}");
    }

    #[test]
    fn profile_stays_bounded() {
        let (w, h) = (80, 24);
        let mut carrion = Carrion::new(&config(37), w, h);
        for _ in 0..3000 {
            carrion.update(1.0 / 30.0, w, h);
            let lo = carrion.base_r * carrion.stretch * 0.3;
            let hi = carrion.base_r * carrion.stretch * 2.1;
            for &r in &carrion.profile {
                assert!(r.is_finite() && r >= lo && r <= hi);
            }
        }
    }

    #[test]
    fn body_never_collapses() {
        let (w, h) = (80, 24);
        let mut carrion = Carrion::new(&config(43), w, h);
        for _ in 0..6000 {
            carrion.update(1.0 / 30.0, w, h);
            let lit = carrion
                .grid
                .data
                .iter()
                .filter(|c| c.tone != Tone::Blank)
                .count();
            assert!(lit > 60, "body collapsed to {lit} cells");
        }
    }

    #[test]
    fn renders_both_tones_and_nothing_else() {
        let (w, h) = (80, 24);
        let mut carrion = Carrion::new(&config(11), w, h);
        let mut flesh = false;
        let mut groove = false;
        for _ in 0..300 {
            carrion.update(1.0 / 30.0, w, h);
            for cell in &carrion.grid.data {
                match cell.tone {
                    Tone::Flesh => flesh = true,
                    Tone::Groove => groove = true,
                    Tone::Blank | Tone::Gore => {}
                }
            }
        }
        assert!(flesh, "body should have flesh cells");
        assert!(groove, "body should have groove cells");
    }

    #[test]
    fn silhouette_is_flesh_never_grey() {
        let (w, h) = (80, 24);
        let mut carrion = Carrion::new(&config(23), w, h);
        for _ in 0..600 {
            carrion.update(1.0 / 30.0, w, h);
            for y in 0..h {
                for x in 0..w {
                    let cell = carrion.grid.data[y * w + x];
                    if cell.tone != Tone::Groove {
                        continue;
                    }
                    let edge = [
                        (x.wrapping_sub(1), y),
                        (x + 1, y),
                        (x, y.wrapping_sub(1)),
                        (x, y + 1),
                    ]
                    .iter()
                    .any(|&(nx, ny)| {
                        nx >= w || ny >= h || carrion.grid.data[ny * w + nx].tone == Tone::Blank
                    });
                    assert!(!edge, "groove cell at ({x},{y}) touches the silhouette");
                }
            }
        }
    }

    #[test]
    fn old_body_cells_get_cleared() {
        let (w, h) = (80, 24);
        let mut carrion = Carrion::new(&config(5), w, h);
        carrion.hub_x = 70.0;
        carrion.hub_y = 20.0;
        carrion.snap_nodes();
        carrion.render();
        assert_ne!(carrion.grid.data[20 * w + 70].tone, Tone::Blank);
        carrion.hub_x = 4.0;
        carrion.hub_y = 4.0;
        carrion.snap_nodes();
        carrion.render();
        assert_eq!(carrion.grid.data[20 * w + 70].tone, Tone::Blank);
    }

    #[test]
    fn resize_shrinks_body_into_bounds() {
        let mut carrion = Carrion::new(&config(3), 120, 40);
        let big = carrion.base_r;
        carrion.resize(24, 8);
        assert!(carrion.base_r < big);
        assert!(carrion.base_r * (1.0 + TEARDROP) <= 4.0 + 0.01);
        assert!(carrion.grid.w == 24 && carrion.grid.h == 8);
        assert!(carrion.hub_x <= 24.0 && carrion.hub_y <= 8.0);
        for node in &carrion.nodes {
            assert!(node.x <= 24.0 && node.y <= 8.0);
        }
    }

    #[test]
    fn teardrop_is_pointed_at_the_front() {
        let (w, h) = (80, 24);
        let mut carrion = Carrion::new(&config(9), w, h);
        carrion.time = 0.0;
        carrion.heading = 0.0;
        carrion.snap_nodes();
        carrion.refresh_profile();
        let front = carrion.profile[0];
        let rear = carrion.profile[RING_N / 2];
        assert!(front < rear);
        assert!(front < carrion.base_r && rear > carrion.base_r);
    }

    #[test]
    fn time_stays_bounded() {
        let (w, h) = (80, 24);
        let mut carrion = Carrion::new(&config(13), w, h);
        for _ in 0..10_000 {
            carrion.update(1.0 / 30.0, w, h);
        }
        assert!((0.0..TAU).contains(&carrion.time));
    }

    #[test]
    fn speed_adjustment_is_clamped() {
        let (w, h) = (80, 24);
        let mut carrion = Carrion::new(&config(17), w, h);
        carrion.adjust_speed(1000.0);
        assert!(carrion.speed <= 20.0);
        carrion.adjust_speed(0.0);
        assert!(carrion.speed >= 0.05);
    }

    #[test]
    fn pause_and_detail_toggle() {
        let (w, h) = (80, 24);
        let mut carrion = Carrion::new(&config(19), w, h);
        assert!(!carrion.paused());
        carrion.toggle_pause();
        assert!(carrion.paused());
        let detail = carrion.detail;
        carrion.toggle_detail();
        assert_ne!(carrion.detail, detail);
    }

    #[test]
    fn gore_pool_is_fixed_and_expires() {
        let (w, h) = (80, 24);
        let mut carrion = Carrion::new(&config(47), w, h);
        carrion.spawn_shed(10.0, 10.0);
        assert_eq!(carrion.shed.len(), SHED_N);
        let alive = carrion.shed.iter().filter(|s| s.life > 0.0).count();
        assert_eq!(alive, 1);
        for shed in &carrion.shed {
            assert!(shed.life <= shed.max);
        }
        carrion.update_shed(1.0 + carrion.trail * 8.0 + 0.1);
        assert!(carrion.shed.iter().all(|s| s.life <= 0.0));
    }

    #[test]
    fn gore_appears_over_time() {
        let (w, h) = (80, 24);
        let mut carrion = Carrion::new(&config(53), w, h);
        let mut saw_gore = false;
        for _ in 0..6000 {
            carrion.update(1.0 / 30.0, w, h);
            saw_gore |= carrion.shed.iter().any(|s| s.life > 0.0);
        }
        assert!(saw_gore, "no gore was ever spawned");
    }

    #[test]
    fn show_too_small_writes_message() {
        let (w, h) = (16, 6);
        let mut carrion = Carrion::new(&config(21), w, h);
        carrion.show_too_small(w, h);
        assert!(carrion.grid.data.iter().any(|c| c.ch != 0));
    }
}
