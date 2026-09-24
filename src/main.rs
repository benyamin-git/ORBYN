mod args;
mod globe;
mod hud;
mod rng;
mod scene;
mod term;

use std::time::{Duration, Instant};

use args::{Action, Config};
use globe::Globe;
use hud::Hud;
use rng::Rng;
use scene::Grid;
use term::{Event, Terminal};

pub const ASPECT: f32 = 2.0;

const MIN_W: usize = 16;
const MIN_H: usize = 6;

fn main() {
    let cfg = match args::parse(std::env::args().skip(1)) {
        Ok(Action::Run(cfg)) => cfg,
        Ok(Action::Help) => {
            print!("{}", args::usage());
            return;
        }
        Ok(Action::Version) => {
            println!("orbyn {}", args::VERSION);
            return;
        }
        Err(err) => {
            eprintln!("orbyn: {err}");
            eprintln!("try 'orbyn --help'");
            std::process::exit(2);
        }
    };

    if let Err(err) = run(&cfg) {
        eprintln!("orbyn: {err}");
        std::process::exit(1);
    }
}

fn run(cfg: &Config) -> std::io::Result<()> {
    let mode = term::resolve(cfg.color);
    let mut terminal = Terminal::new(mode);
    let events = term::input_events();

    let (mut w, mut h) = terminal.sync()?;
    let mut app = App::new(cfg, w, h);
    let frame = Duration::from_secs_f32(1.0 / cfg.fps as f32);
    let mut last = Instant::now();

    loop {
        let frame_start = Instant::now();
        let dt = (frame_start - last).as_secs_f32().min(0.1);
        last = frame_start;

        let mut quit = false;
        while let Ok(event) = events.try_recv() {
            match event {
                Event::Eof => {
                    quit = true;
                    break;
                }
                Event::Key(key) => match key {
                    b'q' | b'Q' | 0x03 | 0x04 => {
                        quit = true;
                        break;
                    }
                    b' ' => app.paused = !app.paused,
                    b'h' | b'H' => app.toggle_hud(),
                    b'+' | b'=' => app.adjust_speed(1.15),
                    b'-' | b'_' => app.adjust_speed(1.0 / 1.15),
                    _ => {}
                },
            }
        }
        if quit {
            break;
        }

        let (new_w, new_h) = terminal.sync()?;
        if (new_w, new_h) != (w, h) {
            w = new_w;
            h = new_h;
            app.resize(w, h);
        }

        if w < MIN_W || h < MIN_H {
            app.show_too_small(w, h);
        } else if !app.paused {
            app.update(dt, w, h);
        }

        terminal.draw(&app.grid)?;

        let elapsed = frame_start.elapsed();
        if elapsed < frame {
            std::thread::sleep(frame - elapsed);
        }
    }

    terminal.restore();
    Ok(())
}

struct Orb {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    r_rows: f32,
}

impl Orb {
    fn update(&mut self, dt: f32, w: usize, h: usize, rng: &mut Rng) {
        self.x += self.vx * dt;
        self.y += self.vy * dt;

        let r = orb_radius(self.r_rows, w, h);
        let rx = r * ASPECT;
        let min_x = rx;
        let max_x = (w as f32 - rx).max(min_x);
        let min_y = r;
        let max_y = (h as f32 - r).max(min_y);

        if self.x < min_x {
            self.x = min_x;
            self.vx = self.vx.abs();
            deflect(&mut self.vx, &mut self.vy, rng);
        } else if self.x > max_x {
            self.x = max_x;
            self.vx = -self.vx.abs();
            deflect(&mut self.vx, &mut self.vy, rng);
        }

        if self.y < min_y {
            self.y = min_y;
            self.vy = self.vy.abs();
            deflect(&mut self.vx, &mut self.vy, rng);
        } else if self.y > max_y {
            self.y = max_y;
            self.vy = -self.vy.abs();
            deflect(&mut self.vx, &mut self.vy, rng);
        }
    }
}

struct App {
    speed: f32,
    trail: f32,
    hud_count: u32,
    hud_on: bool,
    auto_size: bool,
    orb: Orb,
    globe: Globe,
    grid: Grid,
    hud: Hud,
    rng: Rng,
    time: f32,
    wander: f32,
    paused: bool,
}

impl App {
    fn new(cfg: &Config, w: usize, h: usize) -> Self {
        let mut rng = Rng::new(cfg.seed.unwrap_or_else(Rng::seed_from_time));
        let r_rows = effective_radius(cfg.size, w, h);
        let globe = Globe::new(rings_for(r_rows), meridians_for(r_rows));
        let hud_count = cfg.hud;
        let hud_on = hud_count > 0;
        let hud = Hud::new(hud_count, &mut rng, w, h);
        let base = base_speed(h, cfg.speed);
        let dir = rng.range(0.0, std::f32::consts::TAU);
        let orb = Orb {
            x: w as f32 * 0.5,
            y: h as f32 * 0.5,
            vx: dir.cos() * base * ASPECT,
            vy: dir.sin() * base,
            r_rows,
        };
        Self {
            speed: cfg.speed,
            trail: cfg.trail,
            hud_count,
            hud_on,
            auto_size: cfg.size.is_none(),
            orb,
            globe,
            grid: Grid::new(w, h),
            hud,
            rng,
            time: 0.0,
            wander: 2.0,
            paused: false,
        }
    }

    fn resize(&mut self, w: usize, h: usize) {
        self.grid.resize(w, h);
        if self.auto_size {
            self.orb.r_rows = auto_radius(w, h);
        }
        self.orb.r_rows = orb_radius(self.orb.r_rows, w, h);
        let rings = rings_for(self.orb.r_rows);
        let meridians = meridians_for(self.orb.r_rows);
        self.globe.set_resolution(rings, meridians);
        self.orb.update(0.0, w, h, &mut self.rng);
    }

    fn update(&mut self, dt: f32, w: usize, h: usize) {
        self.time += dt;
        self.orb.update(dt, w, h, &mut self.rng);

        self.wander -= dt;
        if self.wander <= 0.0 {
            self.wander = self.rng.range(1.5, 4.0);
            deflect(&mut self.orb.vx, &mut self.orb.vy, &mut self.rng);
        }

        self.globe.update(dt, self.time, self.speed * 0.6);
        let factor = if self.trail <= 0.0 {
            0.0
        } else {
            self.trail.powf(dt * 4.0)
        };
        self.grid.decay(factor);

        let r = orb_radius(self.orb.r_rows, w, h);
        self.globe.draw(&mut self.grid, self.orb.x, self.orb.y, r);

        self.hud.update(dt, w, h, &mut self.rng);
        self.hud.draw(&mut self.grid);
    }

    fn toggle_hud(&mut self) {
        self.hud_on = !self.hud_on;
        let count = if self.hud_on { self.hud_count } else { 0 };
        let (w, h) = (self.grid.w, self.grid.h);
        self.hud = Hud::new(count, &mut self.rng, w, h);
    }

    fn adjust_speed(&mut self, ratio: f32) {
        let next = (self.speed * ratio).clamp(0.05, 20.0);
        let applied = next / self.speed;
        self.speed = next;
        self.orb.vx *= applied;
        self.orb.vy *= applied;
    }

    fn show_too_small(&mut self, w: usize, h: usize) {
        self.grid.decay(0.0);
        const MESSAGE: &str = "terminal too small";
        let message = if w < MESSAGE.len() { "..." } else { MESSAGE };
        let y = h / 2;
        let x = w.saturating_sub(message.len()) / 2;
        for (i, byte) in message.bytes().enumerate() {
            self.grid.write(x + i, y, 0.6, byte);
        }
    }
}

fn orb_radius(r_rows: f32, w: usize, h: usize) -> f32 {
    let max_by_height = h as f32 * 0.5 - 0.6;
    let max_by_width = (w as f32 * 0.5 - 0.6) / ASPECT;
    let max_r = max_by_height.min(max_by_width).max(1.0);
    if r_rows > max_r {
        max_r
    } else if r_rows < 1.0 {
        1.0
    } else {
        r_rows
    }
}

fn auto_radius(w: usize, h: usize) -> f32 {
    let by_height = h as f32 / 4.5;
    let by_width = w as f32 / 9.0;
    by_height.min(by_width).clamp(2.0, 24.0)
}

fn effective_radius(size: Option<f32>, w: usize, h: usize) -> f32 {
    let r = size.unwrap_or_else(|| auto_radius(w, h));
    orb_radius(r, w, h)
}

fn base_speed(h: usize, multiplier: f32) -> f32 {
    (h as f32 * 0.09).clamp(1.2, 12.0) * multiplier
}

fn deflect(vx: &mut f32, vy: &mut f32, rng: &mut Rng) {
    let speed = (*vx * *vx + *vy * *vy).sqrt();
    if speed < 1e-4 {
        return;
    }
    let dir = vy.atan2(*vx) + rng.range(-0.35, 0.35);
    *vx = dir.cos() * speed;
    *vy = dir.sin() * speed;
}

fn rings_for(r: f32) -> usize {
    (r * 0.65).round().clamp(3.0, 8.0) as usize
}

fn meridians_for(r: f32) -> usize {
    (r * 1.4).round().clamp(6.0, 16.0) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deflect_preserves_speed() {
        let mut rng = Rng::new(1234);
        for _ in 0..100 {
            let (mut vx, mut vy): (f32, f32) = (3.0, -4.0);
            let before = (vx * vx + vy * vy).sqrt();
            deflect(&mut vx, &mut vy, &mut rng);
            let after = (vx * vx + vy * vy).sqrt();
            assert!((before - after).abs() < 1e-3);
        }
    }

    #[test]
    fn orb_radius_fits_inside_screen() {
        for (w, h) in [(16, 6), (40, 12), (80, 24), (200, 60), (300, 90)] {
            let r = orb_radius(30.0, w, h);
            assert!(r * ASPECT <= w as f32 * 0.5 + 0.01);
            assert!(r <= h as f32 * 0.5 + 0.01);
            assert!(r >= 1.0);
        }
    }

    #[test]
    fn auto_radius_scales_down_on_small_screens() {
        assert!(auto_radius(40, 12) < auto_radius(200, 60));
        assert!(auto_radius(16, 6) >= 1.0);
    }

    #[test]
    fn orb_bounces_within_bounds() {
        let mut rng = Rng::new(55);
        let (w, h) = (60, 20);
        let mut orb = Orb {
            x: 30.0,
            y: 10.0,
            vx: 50.0,
            vy: -40.0,
            r_rows: 5.0,
        };
        for _ in 0..2000 {
            orb.update(1.0 / 30.0, w, h, &mut rng);
            let r = orb_radius(orb.r_rows, w, h);
            let rx = r * ASPECT;
            assert!(orb.x >= rx - 0.01 && orb.x <= w as f32 - rx + 0.01);
            assert!(orb.y >= r - 0.01 && orb.y <= h as f32 - r + 0.01);
        }
    }

    #[test]
    fn app_stays_on_screen_and_lights_pixels() {
        let cfg = Config {
            seed: Some(7),
            ..Config::default()
        };
        let (w, h) = (80, 24);
        let mut app = App::new(&cfg, w, h);
        for _ in 0..120 {
            app.update(1.0 / 30.0, w, h);
        }
        assert!(app.grid.data.iter().any(|c| c.v > 0.0));
        assert!(app.grid.data.iter().any(|c| c.ch != 0));
        let r = orb_radius(app.orb.r_rows, w, h);
        assert!(app.orb.x >= r * ASPECT - 0.01);
        assert!(app.orb.y >= r - 0.01);
    }

    #[test]
    fn resize_rebuilds_and_keeps_orb_inside() {
        let cfg = Config {
            seed: Some(3),
            ..Config::default()
        };
        let mut app = App::new(&cfg, 120, 40);
        app.resize(30, 10);
        assert_eq!(app.grid.w, 30);
        assert_eq!(app.grid.h, 10);
        let r = orb_radius(app.orb.r_rows, 30, 10);
        assert!(app.orb.x <= 30.0 - r * ASPECT + 0.01);
        assert!(app.orb.y <= 10.0 - r + 0.01);
    }

    #[test]
    fn speed_adjustment_changes_velocity() {
        let cfg = Config {
            seed: Some(11),
            ..Config::default()
        };
        let mut app = App::new(&cfg, 80, 24);
        let before = (app.orb.vx * app.orb.vx + app.orb.vy * app.orb.vy).sqrt();
        app.adjust_speed(2.0);
        let after = (app.orb.vx * app.orb.vx + app.orb.vy * app.orb.vy).sqrt();
        assert!((after / before - 2.0).abs() < 1e-3);
    }

    #[test]
    fn toggle_hud_turns_fragments_on_and_off() {
        let cfg = Config {
            seed: Some(13),
            ..Config::default()
        };
        let mut app = App::new(&cfg, 80, 24);
        app.toggle_hud();
        assert_eq!(app.hud.count(), 0);
        app.toggle_hud();
        assert_eq!(app.hud.count(), cfg.hud as usize);
    }
}
