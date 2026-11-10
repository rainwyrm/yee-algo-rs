use std::{fmt, str::FromStr};

use lexopt::{
    Arg::{Long, Short},
    ValueExt,
};

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Renderer {
    Terminal,
    OpenGL,
}

impl Renderer {
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

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "terminal" => Ok(Renderer::Terminal),
            "opengl" => Ok(Renderer::OpenGL),
            _ => Err("must be 'opengl' or 'terminal'"),
        }
    }
}

#[derive(Debug)]
pub struct Config {
    pub renderer: Renderer,
}

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
