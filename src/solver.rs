//! 2-D TMz yee scheme (leapfrog on a staggered grid)
//!
//! # Lattice
//!
//! the three field components sit on interleaved grids. with cell indices
//! `(i, j)` and the time level `t` (in units of `delta_t`):
//!
//! | array | component | position                | time level |
//! |-------|-----------|-------------------------|------------|
//! | `ez`  | `E_z`     | `(i,       j      )`    | `t`        |
//! | `hx`  | `H_x`     | `(i,       j + 1/2)`    | `t + 1/2`  |
//! | `hy`  | `H_y`     | `(i + 1/2, j      )`    | `t + 1/2`  |
//!
//! this is the classic "index doubling" lattice (`E_z` at even/even/even,
//! `H_x` at even/odd/odd, `H_y` at odd/even/odd) with the unused slots
//! squeezed out. the update formulas are identical; only the bookkeeping
//! differs: `h_y[(i+1, j, n-1)]` in doubled indices is `hy[i, j]` here, and
//! `h_y[(i-1, j, n-1)]` is `hy[i-1, j]`. every time step overwrites the
//! previous level in place, so the whole solver state is three
//! `nx * ny` arrays
//!
//! # Boundaries
//!
//! `E_z` on the outermost ring of nodes is never updated, so it stays zero:
//! the box is a perfect electric conductor. nothing absorbs energy, so waves
//! bounce around the box indefinitely
//!
//! # Units
//!
//! see [`Constants`](crate::geometry::Constants): `c = 1`, `E` and `H`
//! related through the free space impedance

use crate::frames::Frames;
use crate::geometry::{Constants, Obstacle};

// the 2-D courant condition, checked at compile time
const _: () = assert!(
    Constants::DELTA_T
        * Constants::DELTA_T
        * (1.0 / (Constants::DELTA_X * Constants::DELTA_X)
            + 1.0 / (Constants::DELTA_Y * Constants::DELTA_Y))
        <= 1.0,
    "time step violates the 2-D courant stability condition"
);

/// the complete solver state at one instant
pub struct Simulation {
    nx: usize,
    ny: usize,
    /// `E_z` at the current time level `t`
    ez: Vec<f32>,
    /// `H_x` at `t + 1/2`
    hx: Vec<f32>,
    /// `H_y` at `t + 1/2`
    hy: Vec<f32>,
    obstacle: Obstacle,
    /// `delta_t / (eta * delta_x)`, the `H_y` update coefficient
    ca_x: f32,
    /// `delta_t / (eta * delta_y)`, the `H_x` update coefficient
    ca_y: f32,
    /// `eta * delta_t / delta_x`, the `E_z` update coefficient for `dH_y`
    cb_x: f32,
    /// `eta * delta_t / delta_y`, the `E_z` update coefficient for `dH_x`
    cb_y: f32,
}

impl Simulation {
    /// creates a solver on an `nx * ny` grid with the initial pulse in place
    ///
    /// # Panics
    ///
    /// panics if the grid is smaller than `3 * 3` or the obstacle
    /// does not [fit](Obstacle::fits_in) inside it
    pub fn new(nx: usize, ny: usize, obstacle: Obstacle) -> Self {
        assert!(nx >= 3 && ny >= 3, "grid too small");
        assert!(obstacle.fits_in(nx, ny), "obstacle must lie inside the box");

        let mut sim = Self {
            nx,
            ny,
            ez: vec![0.0; nx * ny],
            hx: vec![0.0; nx * ny],
            hy: vec![0.0; nx * ny],
            obstacle,
            ca_x: Constants::DELTA_T / (Constants::IMPEDANCE * Constants::DELTA_X),
            ca_y: Constants::DELTA_T / (Constants::IMPEDANCE * Constants::DELTA_Y),
            cb_x: Constants::IMPEDANCE * Constants::DELTA_T / Constants::DELTA_X,
            cb_y: Constants::IMPEDANCE * Constants::DELTA_T / Constants::DELTA_Y,
        };
        sim.init_pulse();
        sim
    }

    /// the current `E_z` field, `ny` rows of `nx` values
    pub fn ez(&self) -> &[f32] {
        &self.ez
    }

    /// seeds a plane-wave pulse travelling towards `+y`
    ///
    /// the pulse is a raised-cosine ("hann") envelope times a cosine carrier,
    /// uniform along x:
    ///
    /// ```text
    /// f(s) = sin^2(pi s / L) * cos(2 pi (s - L/2) / lambda)     0 <= s <= L
    /// ```
    ///
    /// with `s` the distance (in cells) from the start of the pulse,
    /// `L =` [`PULSE_CELLS`](Constants::PULSE_CELLS) and
    /// `lambda =` [`WAVELENGTH_CELLS`](Constants::WAVELENGTH_CELLS).
    /// the envelope goes smoothly to zero at both ends, so no
    /// broadband step is injected into the grid
    ///
    /// `E_z` is set at `t = 0` and `H_x` at `t = delta_t / 2`. for a wave
    /// moving along `+y`, `H_x = E_z / eta`, but `H_x` lives half a cell further
    /// along y and half a step later, so it is sampled from the pulse at
    /// the retarded position `s + 1/2 - c * delta_t / (2 * delta_y)`.
    /// using the unshifted value instead would launch part of the pulse
    /// backwards (about 13 % of its energy for a 5 cell pulse)
    fn init_pulse(&mut self) {
        let l = Constants::PULSE_CELLS as f64;
        let lambda = Constants::WAVELENGTH_CELLS as f64;
        let start = Constants::PULSE_START as f64;
        let eta = Constants::IMPEDANCE as f64;
        let retard = 0.5 - (Constants::DELTA_T / Constants::DELTA_Y) as f64 / 2.0;

        let f = |s: f64| -> f64 {
            if !(0.0..=l).contains(&s) {
                return 0.0;
            }
            let env = (std::f64::consts::PI * s / l).sin().powi(2);
            env * (2.0 * std::f64::consts::PI * (s - l / 2.0) / lambda).cos()
        };

        for j in 0..self.ny {
            let ez = f(j as f64 - start) as f32;
            let hx = (f(j as f64 + retard - start) / eta) as f32;
            for i in 1..self.nx - 1 {
                self.ez[j * self.nx + i] = ez;
                self.hx[j * self.nx + i] = hx;
            }
        }
        self.pin_conductors();
    }

    /// forces `E_z = 0` on the outer wall and inside the obstacle
    fn pin_conductors(&mut self) {
        let nx = self.nx;
        let Obstacle { x, y } = self.obstacle;
        for j in y.0..=y.1 {
            self.ez[j * nx + x.0..=j * nx + x.1].fill(0.0);
        }
        // outer wall. these nodes are never updated; this just guards against
        // an initial condition that wrote to them
        self.ez[..nx].fill(0.0);
        self.ez[(self.ny - 1) * nx..].fill(0.0);
        for j in 0..self.ny {
            self.ez[j * nx] = 0.0;
            self.ez[j * nx + nx - 1] = 0.0;
        }
    }

    /// advances the fields by one full time step `delta_t`
    ///
    /// 1. `E_z(t + dt)` from `E_z(t)` and `H(t + dt/2)`
    /// 2. `H(t + 3dt/2)` from `H(t + dt/2)` and `E_z(t + dt)`
    ///
    /// the order of the floating point operations is part of the contract
    /// (it is checked bit for bit against the original index-doubled
    /// implementation in the tests)
    pub fn step(&mut self) {
        let nx = self.nx;
        let (ca_x, ca_y, cb_x, cb_y) = (self.ca_x, self.ca_y, self.cb_x, self.cb_y);

        // ---- E_z update (interior nodes) ----
        for j in 1..self.ny - 1 {
            let row = j * nx..(j + 1) * nx;
            let below = (j - 1) * nx..j * nx;
            let ez = &mut self.ez[row.clone()];
            let hy = &self.hy[row.clone()];
            let hx = &self.hx[row];
            let hx_below = &self.hx[below];
            for i in 1..nx - 1 {
                // centred differences of H over one full cell
                let dhy = hy[i] - hy[i - 1];
                let dhx = hx[i] - hx_below[i];
                ez[i] = ez[i] + cb_x * dhy - cb_y * dhx;
            }
        }
        // e_z = 0 on and inside the closed rectangle
        let Obstacle { x, y } = self.obstacle;
        for j in y.0..=y.1 {
            self.ez[j * nx + x.0..=j * nx + x.1].fill(0.0);
        }

        // ---- H update ----
        for j in 0..self.ny - 1 {
            let row = j * nx..(j + 1) * nx;
            let above = (j + 1) * nx..(j + 2) * nx;
            let ez = &self.ez[row.clone()];
            let ez_above = &self.ez[above];
            let hy = &mut self.hy[row.clone()];
            let hx = &mut self.hx[row];
            for i in 0..nx - 1 {
                hy[i] = hy[i] + ca_x * (ez[i + 1] - ez[i]);
                hx[i] = hx[i] - ca_y * (ez_above[i] - ez[i]);
            }
        }
    }
}

/// runs the simulation and records `E_z` after every step
///
/// returns [`Constants::MAX_TIME`] frames: the initial state followed by
/// `MAX_TIME - 1` time steps. when this returns, the staggered `H` fields
/// and all intermediate state have already been dropped
pub fn run(nx: usize, ny: usize, frames: usize, obstacle: Obstacle) -> Frames {
    let mut sim = Simulation::new(nx, ny, obstacle);
    let mut out = Frames::with_capacity(nx, ny, frames);
    for k in 0..frames {
        if k > 0 {
            sim.step();
        }
        out.push(sim.ez());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMALL: (usize, usize) = (40, 34);

    fn obstacle() -> Obstacle {
        // symmetric about the centre of a 40 cell wide box (centre = 19.5)
        Obstacle {
            x: (17, 22),
            y: (12, 20),
        }
    }

    /// the original index-doubled implementation, kept as an executable
    /// specification of what [`Simulation::step`] must compute. a single
    /// `(2nx, 2ny, 2frames)` array per field, `E_z` on (even, even, even),
    /// `H_x` on (even, odd, odd), `H_y` on (odd, even, odd)
    fn reference(sim: &Simulation, frames: usize) -> Vec<Vec<f32>> {
        let (nxd, nyd, ntd) = (2 * sim.nx, 2 * sim.ny, 2 * frames);
        let idx = |x: usize, y: usize, n: usize| x + y * nxd + n * nxd * nyd;
        let mut e_z = vec![0.0f32; nxd * nyd * ntd];
        let mut h_x = e_z.clone();
        let mut h_y = e_z.clone();
        for j in 0..sim.ny {
            for i in 0..sim.nx {
                e_z[idx(2 * i, 2 * j, 0)] = sim.ez[j * sim.nx + i];
                h_x[idx(2 * i, 2 * j + 1, 1)] = sim.hx[j * sim.nx + i];
                h_y[idx(2 * i + 1, 2 * j, 1)] = sim.hy[j * sim.nx + i];
            }
        }
        let dbl = |a: usize| 2 * a;
        for n in (2..ntd).step_by(2) {
            for i in (2..nxd - 2).step_by(2) {
                for j in (2..nyd - 2).step_by(2) {
                    let dhy = h_y[idx(i + 1, j, n - 1)] - h_y[idx(i - 1, j, n - 1)];
                    let dhx = h_x[idx(i, j + 1, n - 1)] - h_x[idx(i, j - 1, n - 1)];
                    let inside = i >= dbl(sim.obstacle.x.0)
                        && i <= dbl(sim.obstacle.x.1)
                        && j >= dbl(sim.obstacle.y.0)
                        && j <= dbl(sim.obstacle.y.1);
                    let ez = if inside {
                        0.0
                    } else {
                        e_z[idx(i, j, n - 2)] + sim.cb_x * dhy - sim.cb_y * dhx
                    };
                    e_z[idx(i, j, n)] = ez;
                }
            }
            for i in (0..nxd - 2).step_by(2) {
                for j in (0..nyd - 2).step_by(2) {
                    let hy = h_y[idx(i + 1, j, n - 1)]
                        + sim.ca_x * (e_z[idx(i + 2, j, n)] - e_z[idx(i, j, n)]);
                    h_y[idx(i + 1, j, n + 1)] = hy;
                    let hx = h_x[idx(i, j + 1, n - 1)]
                        - sim.ca_y * (e_z[idx(i, j + 2, n)] - e_z[idx(i, j, n)]);
                    h_x[idx(i, j + 1, n + 1)] = hx;
                }
            }
        }
        (0..frames)
            .map(|k| {
                let mut f = vec![0.0f32; sim.nx * sim.ny];
                for j in 0..sim.ny {
                    for i in 0..sim.nx {
                        f[j * sim.nx + i] = e_z[idx(2 * i, 2 * j, 2 * k)];
                    }
                }
                f
            })
            .collect()
    }

    #[test]
    fn matches_reference_kernel_bit_for_bit() {
        let frames = 60;
        let spec = reference(&Simulation::new(SMALL.0, SMALL.1, obstacle()), frames);
        let got = run(SMALL.0, SMALL.1, frames, obstacle());
        assert_eq!(got.count(), frames);
        for (k, want) in spec.iter().enumerate() {
            let have = got.frame(k);
            let same = want
                .iter()
                .zip(have)
                .all(|(a, b)| a.to_bits() == b.to_bits());
            assert!(same, "frame {k} differs from the reference kernel");
        }
    }

    #[test]
    fn walls_and_obstacle_stay_at_zero() {
        let frames = run(SMALL.0, SMALL.1, 80, obstacle());
        let o = obstacle();
        for k in 0..frames.count() {
            let f = frames.frame(k);
            for j in 0..SMALL.1 {
                for i in 0..SMALL.0 {
                    let wall = i == 0 || j == 0 || i == SMALL.0 - 1 || j == SMALL.1 - 1;
                    if wall || o.contains(i, j) {
                        assert_eq!(f[j * SMALL.0 + i], 0.0, "frame {k} node ({i},{j})");
                    }
                }
            }
        }
    }

    #[test]
    fn energy_is_conserved() {
        // lossless PEC cavity: U = 1/2 eps E^2 + 1/2 mu H(t-dt/2) H(t+dt/2)
        // (eps = 1/eta, mu = eta) is constant for the leapfrog scheme
        let mut sim = Simulation::new(SMALL.0, SMALL.1, obstacle());
        let eta = Constants::IMPEDANCE as f64;
        let energy = |s: &Simulation, hx_old: &[f32], hy_old: &[f32]| -> f64 {
            let e: f64 = s.ez.iter().map(|&v| (v as f64).powi(2)).sum();
            let hx: f64 =
                s.hx.iter()
                    .zip(hx_old)
                    .map(|(&a, &b)| a as f64 * b as f64)
                    .sum();
            let hy: f64 =
                s.hy.iter()
                    .zip(hy_old)
                    .map(|(&a, &b)| a as f64 * b as f64)
                    .sum();
            0.5 / eta * e + 0.5 * eta * (hx + hy)
        };
        let (mut hx_old, mut hy_old) = (sim.hx.clone(), sim.hy.clone());
        sim.step();
        let u0 = energy(&sim, &hx_old, &hy_old);
        assert!(u0 > 0.0);
        for _ in 0..300 {
            hx_old.copy_from_slice(&sim.hx);
            hy_old.copy_from_slice(&sim.hy);
            sim.step();
            let u = energy(&sim, &hx_old, &hy_old);
            assert!((u / u0 - 1.0).abs() < 1e-5, "energy drifted to {}", u / u0);
        }
    }

    #[test]
    fn solution_is_mirror_symmetric_about_the_box_centre() {
        // pulse uniform in x + obstacle centred in x  =>  E_z(i) == E_z(nx-1-i)
        let (nx, ny) = SMALL;
        let frames = run(nx, ny, 120, obstacle());
        for k in 0..frames.count() {
            let f = frames.frame(k);
            for j in 0..ny {
                for i in 0..nx / 2 {
                    assert_eq!(
                        f[j * nx + i].to_bits(),
                        f[j * nx + (nx - 1 - i)].to_bits(),
                        "frame {k}: asymmetry at ({i},{j})"
                    );
                }
            }
        }
    }

    /// a wide box with the scatterer pushed into a far corner, probed only in
    /// the central columns: the side walls shed diffraction fronts that need
    /// `nx / 2` cells to reach the middle, so a plane wave survives there
    fn free_space(nx: usize, ny: usize) -> Simulation {
        let far = Obstacle {
            x: (2, 2),
            y: (ny - 3, ny - 3),
        };
        Simulation::new(nx, ny, far)
    }

    fn central_energy(sim: &Simulation, rows: std::ops::Range<usize>) -> f64 {
        let nx = sim.nx;
        rows.flat_map(|j| (nx / 2 - 10..nx / 2 + 10).map(move |i| j * nx + i))
            .map(|k| (sim.ez[k] as f64).powi(2))
            .sum()
    }

    #[test]
    fn pulse_travels_one_way() {
        // after the pulse has moved 20 cells nothing may be left behind it.
        // this is what the retarded H initialisation buys
        let (nx, ny) = (100, 160);
        let mut sim = free_space(nx, ny);
        let total = central_energy(&sim, 0..ny);
        for _ in 0..(20.0 / Constants::COURANT) as usize {
            sim.step();
        }
        let behind = central_energy(&sim, 0..Constants::PULSE_START + 5);
        assert!(
            behind / total < 1e-3,
            "{:.3}% of the energy went backwards",
            100.0 * behind / total
        );
    }

    #[test]
    fn pulse_keeps_its_shape() {
        // 12 wavelengths of travel through the grid must not smear the pulse
        let (nx, ny) = (260, 200);
        let mut sim = free_space(nx, ny);
        let peak = |s: &Simulation| {
            (0..ny)
                .map(|j| s.ez[j * nx + nx / 2].abs())
                .fold(0.0f32, f32::max)
        };
        assert!(
            (peak(&sim) - 1.0).abs() < 1e-6,
            "initial peak is {}",
            peak(&sim)
        );
        for _ in 0..(120.0 / Constants::COURANT) as usize {
            sim.step();
        }
        assert!(peak(&sim) > 0.9, "pulse peak decayed to {}", peak(&sim));
    }

    #[test]
    #[should_panic(expected = "obstacle must lie inside")]
    fn rejects_obstacle_touching_the_wall() {
        Simulation::new(
            10,
            10,
            Obstacle {
                x: (0, 3),
                y: (2, 3),
            },
        );
    }
}
