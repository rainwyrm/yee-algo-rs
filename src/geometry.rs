// rectangle
#[derive(Clone, Copy, Debug)]
pub struct Obstacle {
    pub x: (usize, usize),
    pub y: (usize, usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

impl Obstacle {
    pub fn contains(&self, i: usize, j: usize) -> bool {
        i >= self.x.0 && i <= self.x.1 && j >= self.y.0 && j <= self.y.1
    }
    pub fn edge_at(&self, i: usize, j: usize) -> Option<Edge> {
        let (x0, x1) = self.x;
        let (y0, y1) = self.y;

        let within_x = i > x0 && i < x1;
        let within_y = j > y0 && j < y1;

        if within_x && j == y0 {
            Some(Edge::Top)
        } else if within_x && j == y1 {
            Some(Edge::Bottom)
        } else if within_y && i == x0 {
            Some(Edge::Left)
        } else if within_y && i == x1 {
            Some(Edge::Right)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct View {
    pub obstacle: Obstacle,
}

#[derive(Debug)]
pub struct Constants {
    max_i: usize,
    max_j: usize,
    max_time: usize,
    impedance: f32,
    init_wave_width: f32,
    delta_t: f32,
    delta_x: f32,
    delta_y: f32,
}

impl Constants {
    pub const MAX_I: usize = 60;
    pub const MAX_J: usize = 60;
    pub const MAX_TIME: usize = 400;
    pub const IMPEDANCE: f32 = 376.7;
    pub const INIT_WAVE_WIDTH: f32 = 4.0;
    pub const DELTA_T: f32 = Constants::INIT_WAVE_WIDTH / (16.0 * 4.0);
    pub const DELTA_X: f32 = Constants::INIT_WAVE_WIDTH / 8.0;
    pub const DELTA_Y: f32 = Constants::INIT_WAVE_WIDTH / 8.0;
}
