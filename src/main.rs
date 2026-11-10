use std::io;

use crate::argparse::{Command, Renderer};
use crate::geometry::{Constants, Obstacle};

mod argparse;
mod colormap;
mod frames;
mod geometry;
mod solver;
mod terminal;
mod viewer;

/// the scatterer: a conductor one wavelength wide and four wavelengths
/// tall, centred in x so the whole setup is mirror symmetric,
/// and placed a third of the way up the box
///
/// in cell indices: `x = 95..=104` and `y = 80..=120`, i.e. a 9 x 40 cell
/// conductor for the default 200 x 200 grid
fn scene_obstacle() -> Obstacle {
    let cx = Constants::MAX_I / 2;
    let wavelength = Constants::WAVELENGTH_CELLS as usize;
    Obstacle {
        x: (cx - wavelength / 2, cx + wavelength / 2 - 1),
        y: (2 * Constants::MAX_J / 5, 3 * Constants::MAX_J / 5),
    }
}

/// parses the command line, runs the simulation to completion, then hands
/// the recorded `E_z` history to the selected renderer
fn main() -> Result<(), lexopt::Error> {
    let config = match argparse::parse_args()? {
        Command::Help => {
            print!("{}", argparse::USAGE);
            return Ok(());
        }
        Command::Run(config) => config,
    };

    let obstacle = scene_obstacle();
    let frames = solver::run(
        Constants::MAX_I,
        Constants::MAX_J,
        Constants::MAX_TIME,
        obstacle,
    );

    match config.renderer {
        Renderer::OpenGL => viewer::run(frames, obstacle),
        Renderer::Terminal => {
            // a closed pipe (`| head`) is a normal way to stop, not an error
            if let Err(e) = terminal::play(&frames, &obstacle)
                && e.kind() != io::ErrorKind::BrokenPipe
            {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_scene_is_valid_and_symmetric() {
        let o = scene_obstacle();
        assert!(o.fits_in(Constants::MAX_I, Constants::MAX_J));
        // mirror symmetry about the box centre (nx - 1) / 2
        assert_eq!(o.x.0 + o.x.1, Constants::MAX_I - 1);
        // the pulse must reach the obstacle well before the run ends,
        // and the leading edge must not hit the top wall
        let front0 = Constants::PULSE_START as f32 + Constants::PULSE_CELLS;
        let travel = (Constants::MAX_TIME - 1) as f32 * Constants::COURANT;
        assert!(
            front0 + travel < Constants::MAX_J as f32,
            "pulse would reach the far wall"
        );
        assert!(
            front0 + travel > o.y.0 as f32 + Constants::PULSE_CELLS,
            "pulse never gets past the obstacle"
        );
    }
}
