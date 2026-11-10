use std::{ffi::OsString, fmt, str::FromStr};

use lexopt::{
    Arg::{Long, Short},
    ValueExt,
};

/// text printed by `--help`
pub const USAGE: &str = "\
yee-algo: 2D FDTD (TMz) simulation of a pulse scattering off a conductor

USAGE:
    yee-algo [OPTIONS]

OPTIONS:
    -r, --renderer <terminal|opengl>   how to display the result
                                       (default: terminal)
    -h, --help                         print this help

The terminal renderer fits the picture to the size of your terminal
(set COLUMNS and LINES if it cannot be detected). The window renderer
is controlled with the keys listed in the README.
";

/// the backend used to display the simulated `E_z` field
/// once the simulation has finished
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Renderer {
    /// draw each frame as a grid of 24bit-color ANSI cells
    /// in the terminal (see [`crate::terminal`]). this is the default.
    Terminal,
    /// open a window and draw the field into a gpu texture
    /// using `miniquad` (see [`crate::viewer`])
    OpenGL,
}

impl Renderer {
    /// returns the canonical command-line spelling of this renderer
    pub fn as_str(&self) -> &'static str {
        match self {
            Renderer::Terminal => "terminal",
            Renderer::OpenGL => "opengl",
        }
    }
}

impl fmt::Display for Renderer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for Renderer {
    type Err = &'static str;

    /// parses `"terminal"` or `"opengl"`
    ///
    /// any other input yields an error message suitable
    /// for showing to the user
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "terminal" => Ok(Renderer::Terminal),
            "opengl" => Ok(Renderer::OpenGL),
            _ => Err("must be 'opengl' or 'terminal'"),
        }
    }
}

/// run-time config derived from the command line
#[derive(Debug, PartialEq)]
pub struct Config {
    // which backend to use for visualisation
    pub renderer: Renderer,
}

/// what the command line asked for
#[derive(Debug, PartialEq)]
pub enum Command {
    /// run the simulation and show it
    Run(Config),
    /// print [`USAGE`] and exit
    Help,
}

/// parses the process arguments (`std::env::args_os`) into a [`Command`]
///
/// see [`parse_from`] for the accepted options
pub fn parse_args() -> Result<Command, lexopt::Error> {
    parse_from(std::env::args_os().skip(1))
}

/// parses `args` (without the program name) into a [`Command`]
///
/// Options:
/// * `-r <value>` / `--renderer <value>` -- selects the [`Renderer`]
/// * `-h` / `--help` -- yields [`Command::Help`]
///
/// # Errors
///
/// returns a [`lexopt::Error`] if an unknown option or a positional argument
/// is encountered, if `--renderer` is missing its value, or if the value
/// is not a valid [`Renderer`]
pub fn parse_from<I>(args: I) -> Result<Command, lexopt::Error>
where
    I: IntoIterator,
    I::Item: Into<OsString>,
{
    let mut renderer = Renderer::Terminal;

    let mut parser = lexopt::Parser::from_args(args);

    while let Some(arg) = parser.next()? {
        match arg {
            Long("renderer") | Short('r') => {
                renderer = parser.value()?.parse()?;
            }
            Long("help") | Short('h') => return Ok(Command::Help),
            _ => return Err(arg.unexpected()),
        }
    }
    Ok(Command::Run(Config { renderer }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> Result<Command, lexopt::Error> {
        parse_from(args.iter().copied())
    }

    #[test]
    fn defaults_to_the_terminal() {
        assert_eq!(
            run(&[]).unwrap(),
            Command::Run(Config {
                renderer: Renderer::Terminal
            })
        );
    }

    #[test]
    fn long_and_short_renderer_options() {
        for args in [
            &["--renderer", "opengl"][..],
            &["-r", "opengl"],
            &["--renderer=opengl"],
        ] {
            assert_eq!(
                run(args).unwrap(),
                Command::Run(Config {
                    renderer: Renderer::OpenGL
                })
            );
        }
    }

    #[test]
    fn help_wins() {
        assert_eq!(run(&["-h"]).unwrap(), Command::Help);
        assert_eq!(run(&["--help"]).unwrap(), Command::Help);
    }

    #[test]
    fn rejects_garbage() {
        assert!(run(&["--renderer", "vulkan"]).is_err());
        assert!(run(&["--renderer"]).is_err());
        assert!(run(&["--nope"]).is_err());
        assert!(run(&["positional"]).is_err());
    }

    #[test]
    fn renderer_round_trips_through_its_spelling() {
        for r in [Renderer::Terminal, Renderer::OpenGL] {
            assert_eq!(r.to_string().parse::<Renderer>().unwrap(), r);
        }
    }
}
