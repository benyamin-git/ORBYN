pub const CUTOFF: f32 = 0.015;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    #[default]
    Blank,
    Flesh,
    Groove,
    Gore,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cell {
    pub v: f32,
    pub ch: u8,
    pub tone: Tone,
}

impl Cell {
    pub const EMPTY: Cell = Cell {
        v: 0.0,
        ch: 0,
        tone: Tone::Blank,
    };
}

pub struct Grid {
    pub w: usize,
    pub h: usize,
    pub data: Vec<Cell>,
}

impl Grid {
    pub fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            data: vec![Cell::EMPTY; w * h],
        }
    }

    pub fn resize(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.data.clear();
        self.data.resize(w * h, Cell::EMPTY);
    }

    pub fn decay(&mut self, factor: f32) {
        if factor <= 0.0 {
            self.data.fill(Cell::EMPTY);
            return;
        }
        for cell in &mut self.data {
            cell.v *= factor;
            if cell.v < CUTOFF {
                *cell = Cell::EMPTY;
            }
        }
    }

    #[inline]
    pub fn stamp(&mut self, x: usize, y: usize, v: f32) {
        if x >= self.w || y >= self.h {
            return;
        }
        let cell = &mut self.data[y * self.w + x];
        if v > cell.v {
            cell.v = v;
            if v >= 0.5 {
                cell.ch = 0;
            }
        }
    }

    #[inline]
    pub fn write(&mut self, x: usize, y: usize, v: f32, ch: u8) {
        if x >= self.w || y >= self.h {
            return;
        }
        let cell = &mut self.data[y * self.w + x];
        if v >= cell.v {
            cell.v = v;
            cell.ch = ch;
        }
    }

    #[inline]
    pub fn paint(&mut self, x: usize, y: usize, ch: u8, tone: Tone) {
        if x >= self.w || y >= self.h {
            return;
        }
        let cell = &mut self.data[y * self.w + x];
        cell.v = 1.0;
        cell.ch = ch;
        cell.tone = tone;
    }

    pub fn clear_rect(&mut self, x0: usize, y0: usize, x1: usize, y1: usize) {
        let x1 = x1.min(self.w);
        let y1 = y1.min(self.h);
        for y in y0.min(y1)..y1 {
            let row = y * self.w;
            for x in x0.min(x1)..x1 {
                self.data[row + x] = Cell::EMPTY;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamp_keeps_brightest_and_ignores_out_of_bounds() {
        let mut grid = Grid::new(4, 3);
        grid.stamp(1, 1, 0.4);
        grid.stamp(1, 1, 0.2);
        assert_eq!(grid.data[5].v, 0.4);
        grid.stamp(1, 1, 0.9);
        assert_eq!(grid.data[5].v, 0.9);
        grid.stamp(99, 99, 1.0);
        assert_eq!(grid.data.len(), 12);
    }

    #[test]
    fn write_sets_character() {
        let mut grid = Grid::new(4, 3);
        grid.write(2, 0, 0.5, b'A');
        assert_eq!(grid.data[2].ch, b'A');
        grid.write(2, 0, 0.7, b'B');
        assert_eq!(grid.data[2].ch, b'B');
        grid.write(2, 0, 0.1, b'C');
        assert_eq!(grid.data[2].ch, b'B');
    }

    #[test]
    fn bright_stamp_clears_text_character() {
        let mut grid = Grid::new(4, 3);
        grid.write(1, 1, 0.3, b'A');
        grid.stamp(1, 1, 0.2);
        assert_eq!(grid.data[5].ch, b'A');
        grid.stamp(1, 1, 0.8);
        assert_eq!(grid.data[5].ch, 0);
    }

    #[test]
    fn decay_fades_and_snaps_to_empty() {
        let mut grid = Grid::new(2, 2);
        grid.stamp(0, 0, 1.0);
        grid.stamp(1, 1, 0.02);
        grid.decay(0.5);
        assert!((grid.data[0].v - 0.5).abs() < 1e-6);
        assert_eq!(grid.data[3], Cell::EMPTY);
    }

    #[test]
    fn zero_decay_clears_everything() {
        let mut grid = Grid::new(2, 2);
        grid.stamp(0, 0, 1.0);
        grid.write(1, 0, 0.5, b'X');
        grid.decay(0.0);
        assert!(grid.data.iter().all(|c| *c == Cell::EMPTY));
    }

    #[test]
    fn resize_preserves_dimensions() {
        let mut grid = Grid::new(4, 3);
        grid.stamp(3, 2, 1.0);
        grid.resize(2, 2);
        assert_eq!(grid.data.len(), 4);
        assert!(grid.data.iter().all(|c| *c == Cell::EMPTY));
    }

    #[test]
    fn paint_overwrites_and_ignores_out_of_bounds() {
        let mut grid = Grid::new(4, 3);
        grid.write(1, 1, 0.9, b'A');
        grid.paint(1, 1, b'#', Tone::Flesh);
        assert_eq!(grid.data[5].ch, b'#');
        assert_eq!(grid.data[5].tone, Tone::Flesh);
        assert_eq!(grid.data[5].v, 1.0);
        grid.paint(9, 9, b'#', Tone::Groove);
        assert_eq!(grid.data.len(), 12);
    }

    #[test]
    fn clear_rect_only_clears_inside() {
        let mut grid = Grid::new(4, 3);
        grid.paint(0, 0, b'#', Tone::Flesh);
        grid.paint(3, 2, b'=', Tone::Groove);
        grid.clear_rect(0, 0, 2, 2);
        assert_eq!(grid.data[0], Cell::EMPTY);
        assert_eq!(grid.data[11].tone, Tone::Groove);
    }

    #[test]
    fn clear_rect_clamps_out_of_bounds() {
        let mut grid = Grid::new(4, 3);
        grid.paint(3, 2, b'#', Tone::Flesh);
        grid.clear_rect(2, 1, 99, 99);
        assert!(grid.data.iter().all(|c| *c == Cell::EMPTY));
    }
}
