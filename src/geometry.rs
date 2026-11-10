/// axis-aligned rectangular perfect electric conductor
///
/// the rectangle is closed, meaning both bounds are inclusive,
/// so a point on the boundary counts as being inside.
/// the solver forces `E_z` to zero at every node for which
/// [`Obstacle::contains`] is true, which models a
/// PEC scatterer (tangential `E` vanishes on a perfect conductor)
///
/// the edge is approximated on the grid, so obstacle sides
/// are always aligned with the grid axes
#[derive(Clone, Copy, Debug)]
pub struct Obstacle {
    /// inclusive `(min, max)` range of the doubled `i` (x) index
    pub x: (usize, usize),
    /// inclusive `(min, max)` range of the doubled `j` (y) index
    pub y: (usize, usize),
}

/// one of the four sides of an [`Obstacle`]
///
/// the names refer to the index ordering,
/// not to a particular on-screen orientation (differs between renderers)
///
/// * `Top` / `Bottom` -- the sides at `j == y.0` / `j == y.1`
/// * `Left` / `Right` -- the sides at `i == x.0` / `i == x.1`
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

impl Obstacle {
    /// returns `true` if the doubled-grid point `(i, j)` lies on
    /// or inside the rectangle (bounds inclusive)
    pub fn contains(&self, i: usize, j: usize) -> bool {
        i >= self.x.0 && i <= self.x.1 && j >= self.y.0 && j <= self.y.1
    }

    /// classifies a point on the obstacle's boundary
    ///
    /// returns [`Edge`] that `(i, j)` lies on,
    /// or `None` if the point is not on the boundary.
    /// corner points are deliberately excluded,
    /// as are points strictly inside or outside the rectangle
    #[allow(dead_code)]
    pub fn edge_at(&self, i: usize, j: usize) -> Option<Edge> {
        let (x0, x1) = self.x;
        let (y0, y1) = self.y;

        // strictly between the corners along each axis
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

/// Placeholder, unused
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub obstacle: Obstacle,
}

/// Simulation constants
///
/// # Units
///
/// the scheme is written in normalised units in which the
/// speed of light is `c = 1`, so [`DELTA_T`](Constants::DELTA_T)
/// has the same unit as the spatial steps. the free space impedance
/// is kept explicit ([`IMPEDANCE`](Constants::IMPEDANCE)) so that
/// `E` and `H` retain their physical ratio `E_z / H_x = eta_0 (impedance)`
/// for a plane wave propagating along +y (or `|E| / |H| = eta_0` generally)
///
/// # Stability
///
/// the 2-D Courant condition requires
/// `c * delta_t * sqrt(1 / (delta_x^2) + 1 / (delta_y^2)) <= 1.0`
///
/// with the values below,
/// `delta_t = 0.0625`, `delta_x = delta_y = 0.5`
/// the left hand side is `~ 0.177`, well inside the stable range (<= 1.0)
pub struct Constants;

impl Constants {
    /// number of yee cells along x (`i` direction)
    /// therefore, the doubled-index array is `2 * MAX_I` wide
    pub const MAX_I: usize = 200;
    /// number of yee cells along y (`j` direction)
    pub const MAX_J: usize = 200;
    /// number of full time steps (`E` updates) to simulate.
    /// the doubled-index time axis has `2 * MAX_TIME` entries.
    pub const MAX_TIME: usize = 800;
    /// intrinsic impedance of free space `eta_0 ~ 376.7 ohm`
    pub const IMPEDANCE: f32 = 376.7;
    /// characteristic width of the initial pulse, in simulation length units.
    /// the grid spacing and time step are derived from it.
    pub const INIT_WAVE_WIDTH: f32 = 4.0;
    /// time step `delta_t = INIT_WAVE_WIDTH / 64`
    pub const DELTA_T: f32 = Constants::INIT_WAVE_WIDTH / (16.0 * 4.0);
    /// grid spacing in x, `delta_x = INIT_WAVE_WIDTH / 8`
    pub const DELTA_X: f32 = Constants::INIT_WAVE_WIDTH / 8.0;
    /// grid spacing in y, `delta_y = INIT_WAVE_WIDTH / 8`
    pub const DELTA_Y: f32 = Constants::INIT_WAVE_WIDTH / 8.0;
}
