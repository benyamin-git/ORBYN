use crate::rng::Rng;
use crate::scene::Grid;
use crate::ASPECT;

const TAU: f32 = std::f32::consts::TAU;
const BURST_SEED: u32 = 0x0B57_0001;

struct Edge {
    a: u16,
    b: u16,
    ring: usize,
}

struct EdgePulse {
    seed: u32,
    sector_seed: u32,
    rate: f32,
    phase: f32,
}

struct PointPulse {
    seed: u32,
    rate: f32,
    phase: f32,
    amount: f32,
    spike_seed: u32,
    spike_rate: f32,
    spike_phase: f32,
}

pub struct Globe {
    rings: usize,
    meridians: usize,
    points: Vec<[f32; 3]>,
    edges: Vec<Edge>,
    point_pulses: Vec<PointPulse>,
    edge_pulses: Vec<EdgePulse>,
    rot_x: f32,
    rot_y: f32,
    pulse_phase: f32,
}

impl Globe {
    pub fn new(rings: usize, meridians: usize) -> Self {
        let mut globe = Self {
            rings: 0,
            meridians: 0,
            points: Vec::new(),
            edges: Vec::new(),
            point_pulses: Vec::new(),
            edge_pulses: Vec::new(),
            rot_x: 0.0,
            rot_y: 0.0,
            pulse_phase: 0.0,
        };
        globe.set_resolution(rings, meridians);
        globe
    }

    pub fn set_resolution(&mut self, rings: usize, meridians: usize) {
        let rings = rings.max(2);
        let meridians = meridians.max(3);
        if rings == self.rings && meridians == self.meridians {
            return;
        }
        self.rings = rings;
        self.meridians = meridians;

        let mut points = Vec::with_capacity(rings * meridians);
        for ri in 0..rings {
            let theta = -std::f32::consts::FRAC_PI_2
                + (ri as f32 + 1.0) * std::f32::consts::PI / (rings as f32 + 1.0);
            let (st, ct) = theta.sin_cos();
            for mi in 0..meridians {
                let phi = mi as f32 / meridians as f32 * TAU;
                let (sp, cp) = phi.sin_cos();
                points.push([ct * cp, st, ct * sp]);
            }
        }

        let mut edges = Vec::with_capacity(rings * meridians * 2 - meridians);
        for ri in 0..rings {
            for mi in 0..meridians {
                let a = (ri * meridians + mi) as u16;
                let b = (ri * meridians + (mi + 1) % meridians) as u16;
                edges.push(Edge { a, b, ring: ri });
            }
        }
        for ri in 0..rings - 1 {
            for mi in 0..meridians {
                let a = (ri * meridians + mi) as u16;
                let b = ((ri + 1) * meridians + mi) as u16;
                edges.push(Edge { a, b, ring: ri });
            }
        }

        let mut rng = Rng::new(0x0B57_4E21 ^ ((rings * 131 + meridians) as u64));
        let point_pulses = (0..points.len())
            .map(|_| PointPulse {
                seed: rng.next_u64() as u32,
                rate: rng.range(0.35, 1.5),
                phase: rng.range(0.0, 50.0),
                amount: rng.range(0.015, 0.045),
                spike_seed: rng.next_u64() as u32,
                spike_rate: rng.range(0.8, 2.4),
                spike_phase: rng.range(0.0, 50.0),
            })
            .collect();
        let edge_pulses = edges
            .iter()
            .map(|edge| EdgePulse {
                seed: rng.next_u64() as u32,
                sector_seed: hash_u32((edge.ring as u32) ^ 0x51ED_0000),
                rate: rng.range(0.5, 2.6),
                phase: rng.range(0.0, 50.0),
            })
            .collect();

        self.points = points;
        self.edges = edges;
        self.point_pulses = point_pulses;
        self.edge_pulses = edge_pulses;
    }

    pub fn update(&mut self, dt: f32, time: f32, speed: f32) {
        self.rot_y = (self.rot_y + dt * 0.9 * speed) % TAU;
        self.rot_x = 0.6 * (time * 0.35 * speed).sin();
        self.pulse_phase = time;
    }

    pub fn draw(&self, grid: &mut Grid, cx: f32, cy: f32, r: f32) {
        let t = self.pulse_phase;
        let breath = 1.0 + 0.06 * (organic(t * 0.6, 0x5EED_0001) - 0.5);
        let burst = pulse_envelope(t, BURST_SEED);
        let flare = 1.0 + 0.35 * burst;

        let mut projected = Vec::with_capacity(self.points.len());
        for (i, p) in self.points.iter().enumerate() {
            let q = rotate(*p, self.rot_x, self.rot_y);
            let scale = point_scale(&self.point_pulses[i], t, burst);
            let s = r * breath * scale;
            projected.push((cx + q[0] * s * ASPECT, cy + q[1] * s, q[2] * scale));
        }

        for (i, edge) in self.edges.iter().enumerate() {
            let (x1, y1, z1) = projected[edge.a as usize];
            let (x2, y2, z2) = projected[edge.b as usize];
            let z = (z1 + z2) * 0.5;
            let ep = &self.edge_pulses[i];
            let base = if z >= 0.0 { 0.5 + 0.5 * z } else { 0.16 };
            let bright = base * (0.55 + 0.9 * edge_surge(ep, t).powi(2)) * flare;
            draw_line(grid, x1, y1, x2, y2, bright);
        }

        self.draw_core(grid, cx, cy, r, burst);
    }

    fn draw_core(&self, grid: &mut Grid, cx: f32, cy: f32, r: f32, burst: f32) {
        let t = self.pulse_phase;
        let pulse = organic(t * 0.8, 0xCAFE_0002);
        let core = r * (0.26 + 0.06 * pulse + 0.18 * burst);
        if core < 1.0 {
            return;
        }
        let flare = organic(t * 3.1 + 5.0, 0xC0FE_0001);
        let bright = (0.30 + 0.28 * pulse + 0.5 * flare.powi(3)) * (1.0 + 0.3 * burst);
        let cxi = cx.round() as i32;
        let cyi = cy.round() as i32;
        let range = core.ceil() as i32;
        for dy in -range..=range {
            for dx in -range..=range {
                let nx = dx as f32 / ASPECT;
                let ny = dy as f32;
                let d = (nx * nx + ny * ny).sqrt();
                if d <= core {
                    let falloff = (1.0 - d / core).powi(2);
                    let x = cxi + dx;
                    let y = cyi + dy;
                    if x >= 0 && y >= 0 {
                        grid.stamp(x as usize, y as usize, bright * (0.35 + 0.65 * falloff));
                    }
                }
            }
        }
    }
}

fn pulse_envelope(t: f32, seed: u32) -> f32 {
    let n = organic(t * 1.5, seed);
    let s = ((n - 0.55) / 0.30).clamp(0.0, 1.0);
    s * s
}

fn point_scale(pulse: &PointPulse, t: f32, burst: f32) -> f32 {
    let idle = pulse.amount * (organic(t * pulse.rate + pulse.phase, pulse.seed) - 0.5) * 2.0;
    let raw = organic(t * pulse.spike_rate + pulse.spike_phase, pulse.spike_seed);
    let shaped = ((raw - 0.25) / 0.75).clamp(0.0, 1.0);
    let spike = shaped * shaped;
    (1.0 + idle + burst * (0.45 + 0.55 * spike)).clamp(0.8, 1.5)
}

fn edge_surge(pulse: &EdgePulse, t: f32) -> f32 {
    let sector = organic(t * 0.45 + pulse.phase * 0.13, pulse.sector_seed);
    let spark = organic(t * pulse.rate + pulse.phase, pulse.seed);
    0.55 * sector + 0.45 * spark
}

fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    x
}

fn hash_unit(x: u32) -> f32 {
    hash_u32(x) as f32 / u32::MAX as f32
}

fn value_noise(t: f32, seed: u32) -> f32 {
    let i = t.floor();
    let f = t - i;
    let u = f * f * (3.0 - 2.0 * f);
    let base = seed.wrapping_mul(0x9E37_79B9);
    let a = hash_unit((i as i64 as u32).wrapping_add(base));
    let b = hash_unit(((i as i64 + 1) as u32).wrapping_add(base));
    a + (b - a) * u
}

fn organic(t: f32, seed: u32) -> f32 {
    let a = value_noise(t, seed);
    let b = value_noise(t * 2.17 + 11.3, seed ^ 0x9E37_79B9);
    0.68 * a + 0.32 * b
}

pub(crate) fn rotate(p: [f32; 3], rot_x: f32, rot_y: f32) -> [f32; 3] {
    let (sin_x, cos_x) = rot_x.sin_cos();
    let y1 = p[1] * cos_x - p[2] * sin_x;
    let z1 = p[1] * sin_x + p[2] * cos_x;
    let (sin_y, cos_y) = rot_y.sin_cos();
    let x2 = p[0] * cos_y + z1 * sin_y;
    let z2 = -p[0] * sin_y + z1 * cos_y;
    [x2, y1, z2]
}

fn draw_line(grid: &mut Grid, x1: f32, y1: f32, x2: f32, y2: f32, bright: f32) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let steps = dx.abs().max(dy.abs()).ceil() as i32;
    if steps <= 0 {
        return;
    }
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = x1 + dx * t;
        let y = y1 + dy * t;
        if x >= 0.0 && y >= 0.0 {
            grid.stamp(x.round() as usize, y.round() as usize, bright);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_preserves_length() {
        let p: [f32; 3] = [0.3, -0.5, 0.81];
        let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        for step in 0..50 {
            let angle = step as f32 * 0.37;
            let q = rotate(p, angle, angle * 0.7);
            let qlen = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt();
            assert!((qlen - len).abs() < 1e-4);
        }
    }

    #[test]
    fn edges_reference_valid_points() {
        let globe = Globe::new(5, 8);
        assert_eq!(globe.points.len(), 40);
        for edge in &globe.edges {
            assert!((edge.a as usize) < globe.points.len());
            assert!((edge.b as usize) < globe.points.len());
        }
        assert_eq!(globe.edge_pulses.len(), globe.edges.len());
        assert_eq!(globe.point_pulses.len(), globe.points.len());
    }

    #[test]
    fn unit_sphere_points_have_unit_length() {
        let globe = Globe::new(6, 12);
        for p in &globe.points {
            let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-4);
        }
    }

    #[test]
    fn set_resolution_is_idempotent() {
        let mut globe = Globe::new(4, 8);
        let count = globe.points.len();
        globe.set_resolution(4, 8);
        assert_eq!(globe.points.len(), count);
        globe.set_resolution(6, 10);
        assert_eq!(globe.points.len(), 60);
    }

    #[test]
    fn organic_noise_is_deterministic_and_bounded() {
        for seed in [0u32, 1, 0xDEAD_BEEF, 0xFFFF_FFFF] {
            for step in 0..500 {
                let t = step as f32 * 0.13;
                let v = organic(t, seed);
                assert!((0.0..=1.0).contains(&v));
                assert!((v - organic(t, seed)).abs() < f32::EPSILON);
            }
        }
    }

    #[test]
    fn value_noise_is_smooth_over_time() {
        let mut previous = value_noise(0.0, 42);
        for step in 1..10_000 {
            let t = step as f32 * 0.01;
            let current = value_noise(t, 42);
            assert!((current - previous).abs() < 0.06);
            previous = current;
        }
    }

    #[test]
    fn surges_are_unevenly_distributed() {
        let globe = Globe::new(6, 12);
        let t = 3.7;
        let surges: Vec<f32> = globe
            .edge_pulses
            .iter()
            .map(|pulse| edge_surge(pulse, t))
            .collect();
        let min = surges.iter().cloned().fold(f32::MAX, f32::min);
        let max = surges.iter().cloned().fold(f32::MIN, f32::max);
        assert!(max - min > 0.15, "expected uneven surge, got {min}..{max}");
        let mean = surges.iter().sum::<f32>() / surges.len() as f32;
        assert!((0.15..0.85).contains(&mean));
    }

    #[test]
    fn burst_envelope_fires_often_and_is_bounded() {
        let mut samples = Vec::new();
        let mut t = 0.0;
        while t < 120.0 {
            samples.push(pulse_envelope(t, BURST_SEED));
            t += 0.05;
        }
        assert!(samples.iter().cloned().fold(f32::MIN, f32::max) > 0.7);
        let active = samples.iter().filter(|s| **s > 0.25).count() as f32 / samples.len() as f32;
        assert!((0.02..0.6).contains(&active), "active fraction {active}");
    }

    #[test]
    fn point_scale_stays_within_radius_limits() {
        let globe = Globe::new(6, 12);
        let mut max_seen: f32 = 0.0;
        let mut t = 0.0;
        while t < 90.0 {
            let burst = pulse_envelope(t, BURST_SEED);
            for pulse in &globe.point_pulses {
                let scale = point_scale(pulse, t, burst);
                assert!((0.8..=1.5).contains(&scale), "scale {scale}");
                max_seen = max_seen.max(scale);
            }
            t += 0.05;
        }
        assert!(max_seen > 1.4, "expected spikes near max, saw {max_seen}");
        for pulse in &globe.point_pulses {
            assert!(point_scale(pulse, 1.0, 1.0) >= 1.4);
        }
    }

    #[test]
    fn draw_lights_pixels_and_stays_in_bounds() {
        let mut globe = Globe::new(6, 12);
        globe.update(0.016, 1.234, 1.0);
        let mut grid = Grid::new(80, 24);
        globe.draw(&mut grid, 40.0, 12.0, 8.0);
        assert!(grid.data.iter().any(|c| c.v > 0.0));
        assert_eq!(grid.data.len(), 80 * 24);
    }

    #[test]
    fn draw_changes_over_time() {
        let mut globe = Globe::new(6, 12);
        globe.update(0.0, 2.0, 0.0);
        let mut first = Grid::new(80, 24);
        globe.draw(&mut first, 40.0, 12.0, 8.0);

        globe.update(0.0, 2.8, 0.0);
        let mut second = Grid::new(80, 24);
        globe.draw(&mut second, 40.0, 12.0, 8.0);
        assert_ne!(first.data, second.data);
    }

    #[test]
    fn silhouette_respects_burst_bounds() {
        let mut globe = Globe::new(5, 10);
        let mut width_at = |t: f32| {
            globe.update(0.0, t, 0.0);
            let mut grid = Grid::new(120, 50);
            globe.draw(&mut grid, 60.0, 25.0, 8.0);
            let mut min_x = usize::MAX;
            let mut max_x = 0;
            for (i, cell) in grid.data.iter().enumerate() {
                if cell.v > 0.0 {
                    let x = i % 120;
                    min_x = min_x.min(x);
                    max_x = max_x.max(x);
                }
            }
            max_x - min_x + 1
        };
        let calm = width_at(3.0);
        let peak = width_at(6.0);
        eprintln!("calm={calm} peak={peak}");
        assert!(calm <= 36, "calm width {calm}");
        assert!(peak > calm, "peak {peak} should exceed calm {calm}");
        assert!(peak <= 50, "peak width {peak}");
    }

    #[test]
    fn draw_handles_tiny_grids_without_panicking() {
        let globe = Globe::new(3, 6);
        let mut grid = Grid::new(3, 2);
        globe.draw(&mut grid, 1.0, 1.0, 1.0);
    }
}
