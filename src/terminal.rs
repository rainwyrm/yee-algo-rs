use std::fmt;
use std::io::{self, BufWriter, Write};

use crate::dimensions::{self, Array3D};
use crate::geometry::{self, Constants, Obstacle, View};

// ---- term helpers ----------------------------------- {{{
const FAT_BLOCK: char = '\u{2588}';
const MID_POINT: char = '\u{00B7}';
const CLEAR_COLOR: &str = "\x1b[0m";
const CLEAR_SCREEN: &str = "\x1b[2J\x1b[H";
const SEPERATOR_WIDTH: usize = 28;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// 24b ansi foreground escape sequence
struct Fg(Rgb);

/// 24b ansi background escape sequence
struct Bg(Rgb);

impl fmt::Display for Fg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Rgb(r, g, b) = self.0;
        // life-saver: https://ansi.tools
        write!(f, "\x1b[38;2;{r};{g};{b}m")
    }
}

impl fmt::Display for Bg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Rgb(r, g, b) = self.0;
        write!(f, "\x1b[48;2;{r};{g};{b}m")
    }
}
// ------------------------------------------------------ }}}

// ---- color mapping ---------------------------------- {{{

const GAIN: f64 = 1.0;

pub fn heat_color(value: impl Into<f64>) -> Rgb {
    let v = (value.into() * GAIN).clamp(-1.0, 1.0);
    let c = (v.abs() * 255.0).round() as u8;
    if v >= 0.0 { Rgb(c, 0, 0) } else { Rgb(0, 0, c) }
}

const OBSTACLE_COLOR: Rgb = Rgb(110, 110, 110);
pub fn write_frame<W, T>(
    out: &mut W,
    t: usize,
    array: &Array3D<T>,
    obstacle: &Obstacle,
) -> io::Result<()>
where
    W: Write,
    T: Copy + Default + Into<f64>,
{
    for i in (0..Constants::MAX_I * 2).step_by(2) {
        for j in (0..Constants::MAX_J * 2).step_by(2) {
            let color = if obstacle.contains(i, j) {
                OBSTACLE_COLOR
            } else {
                heat_color(array[(i, j, t)])
            };
            write!(out, "{} {CLEAR_COLOR}", Bg(color))?;
        }
        writeln!(out)?;
    }
    writeln!(out, "{}", "=".repeat(SEPERATOR_WIDTH))
}

pub fn print_time<T>(t: usize, array: &Array3D<T>, obstacle: &Obstacle) -> io::Result<()>
where
    T: Copy + Default + Into<f64>,
{
    let mut out = BufWriter::new(io::stdout().lock());
    write_frame(&mut out, t, array, obstacle)?;
    out.flush()
}

pub fn clear_screen() {
    print!("{}", CLEAR_SCREEN);
}
