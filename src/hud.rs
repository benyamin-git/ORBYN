use std::f32::consts::TAU;

use crate::rng::Rng;
use crate::scene::Grid;

const WORDS: &[&str] = &[
    "ORBYN", "ONLINE", "SYNC", "UPLINK", "SCAN", "NODE", "RELAY", "CORE", "VECTOR", "TGT LOCK",
];

struct Fragment {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    text: Vec<u8>,
    ttl: f32,
    age: f32,
    bright: f32,
}

impl Fragment {
    fn spawn(rng: &mut Rng, w: usize, h: usize) -> Self {
        let text = generate_text(rng);
        let width = text.len();
        let x = rng.range(0.0, w.saturating_sub(width).max(1) as f32);
        let y = rng.range(0.0, h.saturating_sub(1).max(1) as f32);
        let speed = rng.range(1.5, 5.5);
        let dir = rng.range(0.0, TAU);
        Self {
            x,
            y,
            vx: dir.cos() * speed * 2.0,
            vy: dir.sin() * speed * 0.5,
            text,
            ttl: rng.range(5.0, 14.0),
            age: 0.0,
            bright: rng.range(0.22, 0.45),
        }
    }

    fn update(&mut self, dt: f32, w: usize, h: usize, rng: &mut Rng) {
        self.age += dt;
        self.x += self.vx * dt;
        self.y += self.vy * dt;

        let max_x = (w as f32 - self.text.len() as f32).max(0.0);
        let max_y = (h as f32 - 1.0).max(0.0);
        if self.x < 0.0 {
            self.x = 0.0;
            self.vx = self.vx.abs();
        }
        if self.x > max_x {
            self.x = max_x;
            self.vx = -self.vx.abs();
        }
        if self.y < 0.0 {
            self.y = 0.0;
            self.vy = self.vy.abs();
        }
        if self.y > max_y {
            self.y = max_y;
            self.vy = -self.vy.abs();
        }

        if self.age >= self.ttl {
            *self = Self::spawn(rng, w, h);
        }
    }

    fn draw(&self, grid: &mut Grid) {
        let fade = (self.age / self.ttl * std::f32::consts::PI).sin().max(0.0);
        let v = self.bright * fade;
        if v <= 0.0 {
            return;
        }
        let y = self.y.round() as usize;
        let x0 = self.x.round() as i32;
        for (i, &byte) in self.text.iter().enumerate() {
            let x = x0 + i as i32;
            if x >= 0 {
                grid.write(x as usize, y, v, byte);
            }
        }
    }
}

pub struct Hud {
    fragments: Vec<Fragment>,
}

impl Hud {
    pub fn new(count: u32, rng: &mut Rng, w: usize, h: usize) -> Self {
        let mut fragments = Vec::with_capacity(count as usize);
        for _ in 0..count {
            fragments.push(Fragment::spawn(rng, w, h));
        }
        Self { fragments }
    }

    pub fn update(&mut self, dt: f32, w: usize, h: usize, rng: &mut Rng) {
        for fragment in &mut self.fragments {
            fragment.update(dt, w, h, rng);
        }
    }

    pub fn draw(&self, grid: &mut Grid) {
        for fragment in &self.fragments {
            fragment.draw(grid);
        }
    }

    #[cfg(test)]
    pub fn count(&self) -> usize {
        self.fragments.len()
    }
}

fn generate_text(rng: &mut Rng) -> Vec<u8> {
    let text = match rng.below(10) {
        0 => format!("SYS {:>4.1}%", rng.range(0.0, 100.0)),
        1 => format!("PWR {:>4.1}%", rng.range(0.0, 100.0)),
        2 => format!("0x{:04X}", rng.next_u64() & 0xFFFF),
        3 => format!("TGT LOCK {:>3.0}", rng.range(0.0, 100.0)),
        4 => format!("NODE {}{}{}", letter(rng), rng.below(10), rng.below(10)),
        5 => format!("{:.3}", rng.range(0.0, 9.999)),
        6 => format!("SCAN {:>4.1}%", rng.range(0.0, 100.0)),
        7 => format!("SEC {:>6.2}", rng.range(0.0, 9999.99)),
        8 => WORDS[rng.below(WORDS.len())].to_string(),
        _ => format!("{} {:03}", WORDS[rng.below(WORDS.len())], rng.below(1000)),
    };
    text.into_bytes()
}

fn letter(rng: &mut Rng) -> char {
    (b'A' + rng.below(26) as u8) as char
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_text_is_ascii_and_compact() {
        let mut rng = Rng::new(5);
        for _ in 0..1000 {
            let text = generate_text(&mut rng);
            assert!(!text.is_empty());
            assert!(text.len() <= 16);
            assert!(text.iter().all(|b| b.is_ascii_graphic() || *b == b' '));
        }
    }

    #[test]
    fn fragments_drift_and_stay_on_screen() {
        let mut rng = Rng::new(17);
        let (w, h) = (80, 24);
        let mut hud = Hud::new(6, &mut rng, w, h);
        let before: Vec<(f32, f32)> = hud.fragments.iter().map(|f| (f.x, f.y)).collect();
        for _ in 0..600 {
            hud.update(1.0 / 30.0, w, h, &mut rng);
            for fragment in &hud.fragments {
                assert!(fragment.x >= 0.0 && fragment.x <= w as f32);
                assert!(fragment.y >= 0.0 && fragment.y <= h as f32);
            }
        }
        let after: Vec<(f32, f32)> = hud.fragments.iter().map(|f| (f.x, f.y)).collect();
        assert_ne!(before, after);
    }

    #[test]
    fn fragments_respawn_after_ttl() {
        let mut rng = Rng::new(3);
        let mut hud = Hud::new(1, &mut rng, 80, 24);
        let ttl = hud.fragments[0].ttl;
        hud.update(ttl + 0.1, 80, 24, &mut rng);
        assert!(hud.fragments[0].age < ttl);
    }

    #[test]
    fn draw_marks_cells_with_characters() {
        let mut rng = Rng::new(21);
        let mut grid = Grid::new(80, 24);
        let mut hud = Hud::new(6, &mut rng, 80, 24);
        hud.update(1.0, 80, 24, &mut rng);
        hud.draw(&mut grid);
        assert!(grid.data.iter().any(|c| c.ch != 0));
    }
}
