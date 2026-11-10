use std::{fmt, str::FromStr};

use lexopt::{
    Arg::{Long, Short},
    ValueExt,
};

/// the backend used to display the simulated `E_z` field
/// once the simulation has finished
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Renderer {
    /// draw each frame as a grid of 24bit-color ANSI background cells
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
#[derive(Debug)]
pub struct Config {
    // which backend to use for visualisation
    pub renderer: Renderer,
}

/// parses the process arguments (`std::env::arg_os`) into a [`Config`]
///
/// Options:
/// * `-r <value>` / `--renderer <value>` -- selects the [`Renderer`]
///
/// # Errors
///
/// returns a [`lexopt::Error`] if an unknown option or a positional argument
/// is encountered, if `--renderer` is missing its value, or if the value
/// is not a valid [`Renderer`]
pub fn parse_args() -> Result<Config, lexopt::Error> {
    let mut renderer = Renderer::Terminal;

    let mut parser = lexopt::Parser::from_env();

    while let Some(arg) = parser.next()? {
        match arg {
            Long("renderer") | Short('r') => {
                renderer = parser.value()?.parse()?;
            }
            _ => return Err(arg.unexpected()),
        }
    }
    Ok(Config { renderer })
}
