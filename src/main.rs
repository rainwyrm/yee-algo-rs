use std::f32::consts::PI;
use std::thread;
use std::time::Duration;

use crate::dimensions::Array3D;
use crate::geometry::{Constants, Obstacle};

mod argparse;
mod dimensions;
mod geometry;
mod terminal;
mod viewer;

/// array length along x in doubled grid indices (`2 * MAX_I`)
const NX: usize = Constants::MAX_I * 2;
/// array length along y in doubled grid indices (`2 * MAX_J`)
const NY: usize = Constants::MAX_J * 2;
/// array length along t in doubled grid indices (`2 * MAX_TIME`)
const NT: usize = Constants::MAX_TIME * 2;

/// allocates a zero-initialised field array covering
/// the whole `(NX, NY, NT)` space-time volume
fn setup_field() -> Array3D<f32> {
    Array3D::new(NX, NY, NT)
}

/// runs the simulation to completion, then hands the stored `E_z`
/// history to the selected renderer
///
fn main() -> Result<(), lexopt::Error> {
    let args = argparse::parse_args()?;

    let mut e_z = setup_field();
    let mut h_x = setup_field();
    let mut h_y = setup_field();

    // PEC scatterer in doubled grid indices:
    // physical cells x = 30..=35, y = 30..=50
    let obst = Obstacle {
        x: (60, 60 + 10),
        y: (60, 60 + 40),
    };

    // --- initial condition ----
    // a short sinusoidal plane-wave pulse covering five e_z rows
    // (j = J0, J0+2, ..., J0+8) uniform along i.
    // E_z is set at n = 0 and H_x at n = 1 with H_x = E_z / eta,
    // which is  the plane-wave relation for a wave travelling
    // in the +y direction, so the pulse moves towards the obstacle
    // instead of splitting into two counter-propagating halves
    const J0: usize = 2;
    for j in (J0..=J0 + 8).step_by(2) {
        let val = (((j - J0) as f32 - 4.0) * PI * 5.0 / (3.0 * 8.0) + PI / 2.0).sin();
        for i in (2..NX - 2).step_by(2) {
            e_z.set(i, j, 0, val);
            h_x.set(i, j + 1, 1, val / Constants::IMPEDANCE);
        }
    }

    // ---- update coefficients ----
    // E update uses cb = eta delta_t / delta
    // H update uses ca = delta_t / (eta delta)
    let ca_x = Constants::DELTA_T / (Constants::IMPEDANCE * Constants::DELTA_X);
    let ca_y = Constants::DELTA_T / (Constants::IMPEDANCE * Constants::DELTA_Y);
    let cb_x = Constants::IMPEDANCE * Constants::DELTA_T / Constants::DELTA_X;
    let cb_y = Constants::IMPEDANCE * Constants::DELTA_T / Constants::DELTA_Y;

    // ---- time stepping ----
    // `n` advances in steps of 2 (one physical time step)
    // each iteration computes E at time n, then H at time n+1
    for n in (2..NT).step_by(2) {
        // e_z(n) from e_z(n-2) and h(n-1)
        for i in (2..NX - 2).step_by(2) {
            for j in (2..NY - 2).step_by(2) {
                // centred differences of H over one full cell
                let dhy = h_y[(i + 1, j, n - 1)] - h_y[(i - 1, j, n - 1)];
                let dhx = h_x[(i, j + 1, n - 1)] - h_x[(i, j - 1, n - 1)];
                let ez = if obst.contains(i, j) {
                    0.0 // e_z = 0 on and inside the closed rectangle
                } else {
                    e_z[(i, j, n - 2)] + cb_x * dhy - cb_y * dhx
                };
                e_z.set(i, j, n, ez);
            }
        }
        // h(n+1) from h(n-1) and e_z(n)
        for i in (0..NX - 2).step_by(2) {
            for j in (0..NY - 2).step_by(2) {
                let hy = h_y[(i + 1, j, n - 1)] + ca_x * (e_z[(i + 2, j, n)] - e_z[(i, j, n)]);
                h_y.set(i + 1, j, n + 1, hy);

                let hx = h_x[(i, j + 1, n - 1)] - ca_y * (e_z[(i, j + 2, n)] - e_z[(i, j, n)]);
                h_x.set(i, j + 1, n + 1, hx);
            }
        }
    }

    // ---- visualisation ----
    if args.renderer == argparse::Renderer::OpenGL {
        viewer::run(e_z, obst);
        Ok(())
    } else {
        // terminal playback:
        // - one frame per physical time step ~5ms apart
        // - stops early on error (i.e. broken pipe)
        for t in (2..NT).step_by(2) {
            terminal::clear_screen();
            if terminal::print_time(t, &e_z, &obst).is_err() {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }
}
