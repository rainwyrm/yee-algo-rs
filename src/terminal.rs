#![allow(dead_code)]
use std::fmt;
use std::io::{self, BufWriter, Write};

use crate::dimensions::Array3D;
use crate::geometry::{Constants, Obstacle};

// ---- term helpers ----------------------------------- {{{
const FAT_BLOCK: char = '\u{2588}';
const MID_POINT: char = '\u{00B7}';
/// ANSI "reset all attributes" sequence.
const CLEAR_COLOR: &str = "\x1b[0m";
/// ANSI "erase display, move cursor to top-left" sequence.
const CLEAR_SCREEN: &str = "\x1b[2J\x1b[H";
/// width (in characters) of the `=` line printed under each frame.
const SEPERATOR_WIDTH: usize = 28;

/// an 8-bit-per-channel RGB colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// 24-bit ANSI foreground escape sequence
///
/// formats as `ESC[38;2;R;G;Bm`.
struct Fg(Rgb);

/// 24-bit ANSI background escape sequence
///
/// formats as `ESC[48;2;R;G;Bm`.
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

/// linear display gain applied to the field before color mapping
const GAIN: f64 = 1.0;

/// maps a field value to a diverging red/black/blue color
///
/// the value is multiplied by [`GAIN`], clamped to `[-1, 1]`,
/// and its magnitude sets the cannel intensity:
///
/// * `v >= 0` -> `Rgb(255 * v, 0, 0)` (red)
/// * `v <  0` -> `Rgb(0, 0, 255 * |v|)` (blue)
///
/// so `0` is black and `+- 1` is full red/blue
pub fn heat_color(value: impl Into<f64>) -> Rgb {
    let v = (value.into() * GAIN).clamp(-1.0, 1.0);
    let c = (v.abs() * 255.0).round() as u8;
    if v >= 0.0 { Rgb(c, 0, 0) } else { Rgb(0, 0, c) }
}

/// color used for cells inside the obstacle
const OBSTACLE_COLOR: Rgb = Rgb(110, 110, 110);

/// writes on frame (time index `t`) to `out`
///
/// each physical cell is emitted as a single space with a 24-bit background
/// color (followed by a color reset), one text row per `i` and one column
/// per `j`. a line of `=` characters terminates the frame.
///
/// `t` is a doubled-grid time index (even for `E_z` data), and the
/// array is sampled at even `(i,j)` positions only
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

/// renders frame `t` to stdout in a single buffered write.
///
/// buffering (and holding the stdout lock for the whole frame) keeps the
/// output from flickering or interleaving with other writers.
pub fn print_time<T>(t: usize, array: &Array3D<T>, obstacle: &Obstacle) -> io::Result<()>
where
    T: Copy + Default + Into<f64>,
{
    let mut out = BufWriter::new(io::stdout().lock());
    write_frame(&mut out, t, array, obstacle)?;
    out.flush()
}

/// clears the terminal and moves the cursor to the top-left corner.
///
/// note that this uses [`print!`] without flushing.
/// the following [`print_time`] call flushes stdout.
pub fn clear_screen() {
    print!("{}", CLEAR_SCREEN);
}
