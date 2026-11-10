use std::f32::consts::{E, PI};
use std::thread::{self, sleep_ms};
use std::time::Duration;

use crate::dimensions::Array3D;
use crate::geometry::{Constants, Obstacle};

mod dimensions;
mod geometry;
mod terminal;
mod viewer;

const NX: usize = Constants::MAX_I * 2;
const NY: usize = Constants::MAX_J * 2;
const NT: usize = Constants::MAX_TIME * 2;

fn setup_field() -> Array3D<f32> {
    Array3D::new(NX, NY, NT)
}
fn main() {
    let mut E_z = setup_field();
    let mut H_x = setup_field();
    let mut H_y = setup_field();

    let mut obst = Obstacle {
        x: (60, 60 + 10),
        y: (60, 60 + 40),
    };

    // initial condition
    // pulse uniform in x travelling in +y towards the obstacle
    const J0: usize = 2;
    for j in (J0..=J0 + 8).step_by(2) {
        let val = (((j - J0) as f32 - 4.0) * PI * 5.0 / (3.0 * 8.0) + PI / 2.0).sin();
        for i in (2..NX - 2).step_by(2) {
            E_z.set(i, j, 0, val);
            H_x.set(i, j + 1, 1, val / Constants::IMPEDANCE);
        }
    }

    // update the coefficients
    let ca_x = Constants::DELTA_T / (Constants::IMPEDANCE * Constants::DELTA_X);
    let ca_y = Constants::DELTA_T / (Constants::IMPEDANCE * Constants::DELTA_Y);
    let cb_x = Constants::IMPEDANCE * Constants::DELTA_T / Constants::DELTA_X;
    let cb_y = Constants::IMPEDANCE * Constants::DELTA_T / Constants::DELTA_Y;

    for n in (2..NT).step_by(2) {
        // 1) E_z(n) from E_z(n-2) and H(n-1)
        for i in (2..NX - 2).step_by(2) {
            for j in (2..NY - 2).step_by(2) {
                let dhy = H_y[(i + 1, j, n - 1)] - H_y[(i - 1, j, n - 1)];
                let dhx = H_x[(i, j + 1, n - 1)] - H_x[(i, j - 1, n - 1)];
                let ez = if obst.contains(i, j) {
                    0.0 // PEC: E_z = 0 on and inside the closed rectangle
                } else {
                    E_z[(i, j, n - 2)] + cb_x * dhy - cb_y * dhx
                };
                E_z.set(i, j, n, ez);
            }
        }
        for i in (0..NX - 2).step_by(2) {
            for j in (0..NY - 2).step_by(2) {
                let hy = H_y[(i + 1, j, n - 1)] + ca_x * (E_z[(i + 2, j, n)] - E_z[(i, j, n)]);
                H_y.set(i + 1, j, n + 1, hy);

                let hx = H_x[(i, j + 1, n - 1)] - ca_y * (E_z[(i, j + 2, n)] - E_z[(i, j, n)]);
                H_x.set(i, j + 1, n + 1, hx);
            }
        }
    }

    for t in (2..NT).step_by(2) {
        terminal::clear_screen();
        if terminal::print_time(t, &E_z, &obst).is_err() {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
}
