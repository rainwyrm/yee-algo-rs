/// axis-aligned rectangular perfect electric conductor
///
/// the rectangle is closed, meaning both bounds are inclusive,
/// so a point on the boundary counts as being inside.
/// the solver forces `E_z` to zero at every node for which
/// [`Obstacle::contains`] is true, which models a
/// PEC scatterer (tangential `E` vanishes on a perfect conductor)
///
/// the sides are always aligned with the grid axes, so the model is exact
/// (there is no staircase error). note that the PEC surface runs through the
/// outermost nodes: a `6 x 21` node obstacle is a `5 x 20` cell conductor
#[derive(Clone, Copy, Debug)]
pub struct Obstacle {
    /// inclusive `(min, max)` range of the cell index `i` (x)
    pub x: (usize, usize),
    /// inclusive `(min, max)` range of the cell index `j` (y)
    pub y: (usize, usize),
}

/// one of the four sides of an [`Obstacle`]
///
/// the names refer to the index ordering, not to an on-screen orientation
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
    /// returns `true` if the node `(i, j)` lies on
    /// or inside the rectangle (bounds inclusive)
    pub fn contains(&self, i: usize, j: usize) -> bool {
        i >= self.x.0 && i <= self.x.1 && j >= self.y.0 && j <= self.y.1
    }

    /// returns `true` if the rectangle is well formed and lies strictly
    /// inside an `nx * ny` grid, i.e. it does not touch the outer PEC wall
    pub fn fits_in(&self, nx: usize, ny: usize) -> bool {
        self.x.0 <= self.x.1
            && self.y.0 <= self.y.1
            && self.x.0 >= 1
            && self.y.0 >= 1
            && self.x.1 < nx - 1
            && self.y.1 < ny - 1
    }

    /// classifies a point on the obstacle's boundary
    ///
    /// returns the [`Edge`] that `(i, j)` lies on,
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
/// with `delta_x = delta_y = 0.5` and [`COURANT`](Constants::COURANT) `= 0.5`
/// the left hand side is `~ 0.707`, which is the largest value the
/// scheme tolerates divided by `sqrt(2)`
/// (see the compile time check in [`crate::solver`])
///
/// # Resolution
///
/// numerical dispersion of the Yee scheme depends on how many cells span one
/// wavelength. the pulse uses [`WAVELENGTH_CELLS`](Constants::WAVELENGTH_CELLS)
/// `= 10`, for which the phase velocity error along an axis is about `-1.2 %`
/// (a wavelength of 4.8 cells would be off by `-6.9 %` and its group velocity
/// would be only `0.8 c`)
pub struct Constants;

impl Constants {
    /// number of yee cells along x (`i` direction)
    pub const MAX_I: usize = 200;
    /// number of yee cells along y (`j` direction)
    pub const MAX_J: usize = 200;
    /// simulated time span in simulation units
    pub const DURATION: f32 = 75.0;
    /// number of stored frames: the initial state plus `MAX_TIME - 1`
    /// full time steps
    pub const MAX_TIME: usize = (Constants::DURATION / Constants::DELTA_T) as usize;
    /// intrinsic impedance of free space `eta_0 ~ 376.7 ohm`
    pub const IMPEDANCE: f32 = 376.7;
    /// grid spacing in x
    pub const DELTA_X: f32 = 0.5;
    /// grid spacing in y
    pub const DELTA_Y: f32 = 0.5;
    /// courant number `c * delta_t / delta_x`. the 2-D stability limit is
    /// `1 / sqrt(2) ~ 0.707`
    pub const COURANT: f32 = 0.5;
    /// time step, `delta_t = COURANT * delta_x` (with `c = 1`)
    pub const DELTA_T: f32 = Constants::COURANT * Constants::DELTA_X;
    /// carrier wavelength of the initial pulse, in cells
    pub const WAVELENGTH_CELLS: f32 = 10.0;
    /// length of the initial pulse (its raised-cosine envelope), in cells.
    /// three wavelengths keep the spectrum narrow enough that dispersion
    /// does not smear the pulse
    pub const PULSE_CELLS: f32 = 30.0;
    /// first row (cell index `j`) covered by the initial pulse
    pub const PULSE_START: usize = 1;
}

#[cfg(test)]
mod tests {
    use super::*;

    const OBST: Obstacle = Obstacle {
        x: (3, 5),
        y: (4, 8),
    };

    #[test]
    fn contains_is_inclusive() {
        assert!(OBST.contains(3, 4));
        assert!(OBST.contains(5, 8));
        assert!(OBST.contains(4, 6));
        assert!(!OBST.contains(2, 6));
        assert!(!OBST.contains(4, 9));
    }

    #[test]
    fn fits_in_rejects_wall_contact_and_bad_rects() {
        assert!(OBST.fits_in(10, 10));
        assert!(!OBST.fits_in(6, 10)); // x.1 == nx - 1
        assert!(
            !Obstacle {
                x: (0, 2),
                y: (1, 2)
            }
            .fits_in(10, 10)
        );
        assert!(
            !Obstacle {
                x: (4, 3),
                y: (1, 2)
            }
            .fits_in(10, 10)
        );
    }

    #[test]
    fn edges_exclude_corners_and_interior() {
        assert_eq!(OBST.edge_at(4, 4), Some(Edge::Top));
        assert_eq!(OBST.edge_at(4, 8), Some(Edge::Bottom));
        assert_eq!(OBST.edge_at(3, 6), Some(Edge::Left));
        assert_eq!(OBST.edge_at(5, 6), Some(Edge::Right));
        assert_eq!(OBST.edge_at(3, 4), None); // corner
        assert_eq!(OBST.edge_at(4, 6), None); // interior
    }

    #[test]
    fn derived_constants_are_consistent() {
        assert_eq!(Constants::DELTA_T, 0.25);
        assert_eq!(Constants::MAX_TIME, 300);
        // the pulse must fit above the bottom wall and below the top wall
        assert!(Constants::PULSE_START as f32 + Constants::PULSE_CELLS < Constants::MAX_J as f32);
    }
}
