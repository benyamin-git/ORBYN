mod args;
mod being;
mod capture;
mod carrion;
mod globe;
mod hud;
mod menu;
mod rng;
mod rotation;
mod scene;
mod term;

use std::io::{self, IsTerminal};
use std::time::{Duration, Instant};

use args::{Action, Config};
use being::{Being, Sim};
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

    let result = match &cfg.capture {
        Some(capture) => capture::run(&cfg, capture),
        None => {
            let rx = term::input_events();
            if args::should_show_menu(&cfg, io::stdin().is_terminal(), io::stdout().is_terminal()) {
                let mode = term::resolve(cfg.color);
                let mut menu_terminal = Terminal::new(mode, cfg.being.unwrap_or(Being::Orb));
                match menu::run(&mut menu_terminal, &rx, &cfg) {
                    Some(req) => {
                        let next = menu::to_config(&req, &cfg);
                        run(&next, &rx)
                    }
                    None => Ok(()),
                }
            } else {
                run(&cfg, &rx)
            }
        }
    };

    if let Err(err) = result {
        eprintln!("orbyn: {err}");
        std::process::exit(1);
    }
}

fn swap_to(sim: &mut Sim, next: Being, c: &Config, w: usize, h: usize, session_paused: bool) {
    *sim = Sim::new(next, c, w, h);
    if session_paused {
        sim.toggle_pause();
    }
    if sim.paused() {
        sim.update(0.0, w, h);
    }
}

fn run(cfg: &Config, rx: &std::sync::mpsc::Receiver<Event>) -> std::io::Result<()> {
    let mode = term::resolve(cfg.color);
    let rotation = rotation::rotation_for(cfg);
    let mut current_kind = rotation::resolve_start_kind(cfg.being, rotation.as_ref());
    let mut terminal = Terminal::new(mode, current_kind);

    let (mut w, mut h) = terminal.sync()?;
    let mut sim = Sim::new(current_kind, cfg, w, h);
    let mut session_speed = cfg.speed;
    let session_trail = cfg.trail;
    let mut session_paused = false;
    let mut elapsed = 0.0f32;
    let interval = rotation.as_ref().and_then(|r| r.interval);
    let frame = Duration::from_secs_f32(1.0 / cfg.fps as f32);
    let mut last = Instant::now();

    loop {
        let frame_start = Instant::now();
        let dt = (frame_start - last).as_secs_f32().min(0.1);
        last = frame_start;

        let mut quit = false;
        let mut manual = false;
        while let Ok(event) = rx.try_recv() {
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
                    b' ' => {
                        sim.toggle_pause();
                        session_paused = !session_paused;
                    }
                    b'h' | b'H' => sim.toggle_overlay(),
                    b'+' | b'=' => {
                        session_speed = (session_speed * 1.15).clamp(0.05, 20.0);
                        sim.adjust_speed(1.15);
                    }
                    b'-' | b'_' => {
                        session_speed = (session_speed * (1.0 / 1.15)).clamp(0.05, 20.0);
                        sim.adjust_speed(1.0 / 1.15);
                    }
                    b'n' | b'N' => manual = true,
                    _ => {}
                },
            }
        }
        if quit {
            break;
        }

        elapsed += dt;
        if manual || rotation::swap_due(elapsed, interval) {
            let list: Vec<Being> = rotation
                .as_ref()
                .map(|r| r.list.clone())
                .unwrap_or_else(|| Being::all().to_vec());
            if let Some(next) = rotation::next_after(&list, current_kind) {
                let mut c = cfg.clone();
                c.speed = session_speed;
                c.trail = session_trail;
                swap_to(&mut sim, next, &c, w, h, session_paused);
                current_kind = next;
                elapsed = 0.0;
                terminal.set_title(current_kind);
            }
        }

        let (new_w, new_h) = terminal.sync()?;
        if (new_w, new_h) != (w, h) {
            w = new_w;
            h = new_h;
            sim.resize(w, h);
        }

        if w < MIN_W || h < MIN_H {
            sim.show_too_small(w, h);
        } else if !sim.paused() {
            sim.update(dt, w, h);
        }

        terminal.draw(current_kind, sim.grid())?;

        let frame_elapsed = frame_start.elapsed();
        if frame_elapsed < frame {
            std::thread::sleep(frame - frame_elapsed);
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

    fn playlist_cfg() -> Config {
        Config {
            seed: Some(7),
            playlist: Some(vec![Being::Orb, Being::Carrion]),
            rotate: Some(60.0),
            ..Config::default()
        }
    }

    #[test]
    fn n_swap_builds_next_being_and_paused_priming_lights_every_being() {
        let cfg = playlist_cfg();
        let rotation = rotation::rotation_for(&cfg).expect("rotation");
        let (w, h) = (40, 12);
        let start = rotation::resolve_start_kind(cfg.being, Some(&rotation));
        assert_eq!(start, Being::Orb);
        let next = rotation::next_after(&rotation.list, start).expect("next");
        assert_eq!(next, Being::Carrion);

        let mut sim = Sim::new(start, &cfg, w, h);
        let mut c = cfg.clone();
        c.speed = 0.5;
        c.trail = 0.5;
        swap_to(&mut sim, next, &c, w, h, true);
        assert!(matches!(sim, Sim::Carrion(_)));
        assert!(sim.paused());
        assert!(sim.grid().data.iter().any(|cell| cell.v > 0.0));

        for being in Being::all() {
            let mut primed = Sim::new(being, &cfg, w, h);
            swap_to(&mut primed, being, &cfg, w, h, true);
            assert!(primed.paused(), "{being:?} did not stay paused");
            assert!(
                primed.grid().data.iter().any(|cell| cell.v > 0.0),
                "{being:?} blank after primed swap"
            );
        }
    }

    #[test]
    fn timer_swap_advances_being_when_due_and_stays_put_before() {
        let cfg = Config {
            seed: Some(7),
            playlist: Some(vec![Being::Orb, Being::Carrion]),
            rotate: Some(2.0),
            ..Config::default()
        };
        let rotation = rotation::rotation_for(&cfg).expect("rotation");
        let interval = rotation.interval;
        assert_eq!(interval, Some(2.0));
        let (w, h) = (40, 12);
        let mut current = rotation::resolve_start_kind(cfg.being, Some(&rotation));
        let mut sim = Sim::new(current, &cfg, w, h);
        let dt = 1.0 / 30.0;
        let mut elapsed = 1.98;

        assert!(!rotation::swap_due(elapsed, interval));
        elapsed += dt;
        if rotation::swap_due(elapsed, interval) {
            let next = rotation::next_after(&rotation.list, current).expect("next");
            swap_to(&mut sim, next, &cfg, w, h, false);
            current = next;
            elapsed = 0.0;
        }
        assert_eq!(current, Being::Carrion);
        assert!(matches!(sim, Sim::Carrion(_)));
        assert_eq!(elapsed, 0.0);
        assert!(!rotation::swap_due(dt, Some(60.0)));
    }

    #[test]
    fn speed_adjustment_carries_into_swap_config_and_new_sim() {
        let cfg = Config {
            seed: Some(7),
            speed: 1.0,
            trail: 0.82,
            ..Config::default()
        };
        let (w, h) = (40, 12);
        let mut sim = Sim::new(Being::Orb, &cfg, w, h);
        let mut session_speed = cfg.speed;
        let ratio = 2.0;
        session_speed = (session_speed * ratio).clamp(0.05, 20.0);
        sim.adjust_speed(ratio);
        assert!((session_speed - 2.0).abs() < 1e-6);

        let mut c = cfg.clone();
        c.speed = session_speed;
        c.trail = cfg.trail;
        assert_eq!(c.speed, session_speed);

        let fresh = Sim::new(Being::Orb, &c, w, h);
        match &fresh {
            Sim::Orb(app) => assert!((app.speed - session_speed).abs() < 1e-6),
            Sim::Carrion(_) => panic!("expected orb"),
        }

        swap_to(&mut sim, Being::Carrion, &c, w, h, false);
        assert!(matches!(sim, Sim::Carrion(_)));
    }

    #[test]
    fn resolve_start_kind_drives_initial_sim() {
        let explicit = Config {
            seed: Some(7),
            being: Some(Being::Carrion),
            ..Config::default()
        };
        let start = rotation::resolve_start_kind(
            explicit.being,
            rotation::rotation_for(&explicit).as_ref(),
        );
        assert!(matches!(
            Sim::new(start, &explicit, 40, 12),
            Sim::Carrion(_)
        ));

        let playlist = Config {
            seed: Some(7),
            playlist: Some(vec![Being::Carrion]),
            ..Config::default()
        };
        let rotation = rotation::rotation_for(&playlist);
        let start = rotation::resolve_start_kind(playlist.being, rotation.as_ref());
        assert_eq!(start, Being::Carrion);
        assert!(matches!(
            Sim::new(start, &playlist, 40, 12),
            Sim::Carrion(_)
        ));

        let bare = Config {
            seed: Some(7),
            ..Config::default()
        };
        let start =
            rotation::resolve_start_kind(bare.being, rotation::rotation_for(&bare).as_ref());
        assert_eq!(start, Being::Orb);
        assert!(matches!(Sim::new(start, &bare, 40, 12), Sim::Orb(_)));
    }

    #[test]
    fn n_cycles_all_when_no_rotation() {
        let cfg = Config {
            seed: Some(7),
            ..Config::default()
        };
        let rotation = rotation::rotation_for(&cfg);
        assert!(rotation.is_none());
        let list = Being::all().to_vec();
        let current = rotation::resolve_start_kind(cfg.being, rotation.as_ref());
        assert_eq!(current, Being::Orb);
        let next = rotation::next_after(&list, current).expect("next");
        assert_eq!(next, Being::Carrion);
        let mut sim = Sim::new(current, &cfg, 40, 12);
        swap_to(&mut sim, next, &cfg, 40, 12, false);
        assert!(matches!(sim, Sim::Carrion(_)));
    }
}
