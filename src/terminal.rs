//! ansi truecolor renderer
//!
//! every text cell shows *two* vertically stacked pixels using the upper
//! half block `▀` (foreground = upper pixel, background = lower pixel).
//! a terminal cell is roughly twice as tall as it is wide, so this gives
//! square pixels and doubles the vertical resolution
//!
//! the picture is oriented like the opengl window: `x` runs to the right,
//! `y` runs upwards (the pulse travels from the bottom to the top). the field
//! is shrunk to fit the terminal; when several cells map to one pixel the
//! one with the largest `|E_z|` wins, so thin wave fronts do not average away

use std::io::{self, BufWriter, Write};
use std::thread;
use std::time::{Duration, Instant};

use crate::colormap::{self, OBSTACLE_RGB, Scale};
use crate::frames::Frames;
use crate::geometry::{Constants, Obstacle};

/// the upper half block, drawn with the upper pixel as foreground
const UPPER_HALF: char = '\u{2580}';
/// ANSI "reset all attributes" sequence.
const RESET: &str = "\x1b[0m";
/// ANSI "move cursor to top-left" sequence. frames overwrite each other
/// in place instead of clearing the screen, which avoids flicker
const HOME: &str = "\x1b[H";
/// ANSI "erase display" sequence, sent once before the first frame
const CLEAR: &str = "\x1b[2J";
/// time between two frames
const FRAME_PERIOD: Duration = Duration::from_millis(33);
/// fallback when the terminal size cannot be determined
const DEFAULT_SIZE: (usize, usize) = (80, 24);

/// 24-bit ANSI foreground / background escape sequence writers
fn fg(out: &mut impl Write, c: [u8; 3]) -> io::Result<()> {
    write!(out, "\x1b[38;2;{};{};{}m", c[0], c[1], c[2])
}
fn bg(out: &mut impl Write, c: [u8; 3]) -> io::Result<()> {
    write!(out, "\x1b[48;2;{};{};{}m", c[0], c[1], c[2])
}

/// size of the picture in pixels. `h` is always even (two pixels per row)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Raster {
    pub w: usize,
    pub h: usize,
}

/// picks the largest picture of the field's aspect ratio that fits in
/// `cols x rows` terminal cells, never magnifying beyond one pixel per cell
///
/// the last text row is reserved for the status line
pub fn fit(nx: usize, ny: usize, cols: usize, rows: usize) -> Raster {
    let avail_w = cols.max(1);
    let avail_h = (2 * rows.saturating_sub(1)).max(2);
    // integer arithmetic, so the result never depends on float rounding.
    // compare avail_w / nx with avail_h / ny to find the limiting side
    let (w, h) = if avail_w * ny <= avail_h * nx {
        let w = nx.min(avail_w);
        (w, ny * w / nx)
    } else {
        let h = ny.min(avail_h);
        (nx * h / ny, h)
    };
    Raster {
        w: w.max(1),
        h: (h & !1).max(2),
    }
}

/// maps picture pixels to the rectangle of field cells they summarise
pub struct Sampler {
    raster: Raster,
    /// for every pixel column, the half open range of cell columns
    xs: Vec<(usize, usize)>,
    /// for every pixel row (counted from the top), the range of cell rows
    ys: Vec<(usize, usize)>,
    nx: usize,
}

impl Sampler {
    pub fn new(nx: usize, ny: usize, raster: Raster) -> Self {
        let span = |n: usize, m: usize, k: usize| {
            let a = k * n / m;
            let b = ((k + 1) * n / m).max(a + 1);
            (a, b.min(n))
        };
        Self {
            raster,
            xs: (0..raster.w).map(|k| span(nx, raster.w, k)).collect(),
            // pixel row 0 is the top of the picture, the largest y
            ys: (0..raster.h)
                .map(|k| {
                    let (a, b) = span(ny, raster.h, raster.h - 1 - k);
                    (a, b)
                })
                .collect(),
            nx,
        }
    }

    /// the colour of pixel `(px, py)`, `py` counted from the top
    fn pixel(&self, frame: &[f32], obstacle: &Obstacle, px: usize, py: usize) -> [u8; 3] {
        let (x0, x1) = self.xs[px];
        let (y0, y1) = self.ys[py];
        // the obstacle is a rectangle, so "any cell inside" is an overlap test
        let hits =
            x0 <= obstacle.x.1 && x1 > obstacle.x.0 && y0 <= obstacle.y.1 && y1 > obstacle.y.0;
        if hits {
            return OBSTACLE_RGB;
        }
        let mut best = 0.0f32;
        for j in y0..y1 {
            for &v in &frame[j * self.nx + x0..j * self.nx + x1] {
                if v.abs() > best.abs() {
                    best = v;
                }
            }
        }
        colormap::signed_rgb(best, 1.0, Scale::Linear)
    }
}

/// writes frame `k` to `out`: the cursor-home sequence, the picture,
/// and a status line
///
/// colour escapes are only emitted when the colour changes, and every
/// line ends with a reset, so an interrupted run leaves the terminal sane
pub fn write_frame<W: Write>(
    out: &mut W,
    frames: &Frames,
    k: usize,
    obstacle: &Obstacle,
    sampler: &Sampler,
) -> io::Result<()> {
    let frame = frames.frame(k);
    let Raster { w, h } = sampler.raster;
    out.write_all(HOME.as_bytes())?;
    let mut buf = [0u8; 4];
    for row in 0..h / 2 {
        let mut last: (Option<[u8; 3]>, Option<[u8; 3]>) = (None, None);
        for px in 0..w {
            let up = sampler.pixel(frame, obstacle, px, 2 * row);
            let dn = sampler.pixel(frame, obstacle, px, 2 * row + 1);
            if last.0 != Some(up) {
                fg(out, up)?;
            }
            if last.1 != Some(dn) {
                bg(out, dn)?;
            }
            last = (Some(up), Some(dn));
            out.write_all(UPPER_HALF.encode_utf8(&mut buf).as_bytes())?;
        }
        writeln!(out, "{RESET}")?;
    }
    write!(
        out,
        "t = {:7.2}   frame {}/{}\x1b[K",
        k as f32 * Constants::DELTA_T,
        k + 1,
        frames.count()
    )?;
    out.flush()
}

/// plays all frames once, at a fixed pace, fitted to the terminal
///
/// returns early with the error if stdout goes away (e.g. a closed pipe)
pub fn play(frames: &Frames, obstacle: &Obstacle) -> io::Result<()> {
    let (cols, rows) = terminal_size();
    let raster = fit(frames.nx(), frames.ny(), cols, rows);
    let sampler = Sampler::new(frames.nx(), frames.ny(), raster);

    let mut out = BufWriter::with_capacity(1 << 16, io::stdout().lock());
    out.write_all(CLEAR.as_bytes())?;
    for k in 0..frames.count() {
        let started = Instant::now();
        write_frame(&mut out, frames, k, obstacle, &sampler)?;
        if let Some(rest) = FRAME_PERIOD.checked_sub(started.elapsed()) {
            thread::sleep(rest);
        }
    }
    writeln!(out)
}

/// `(columns, rows)` of the terminal
///
/// asks the kernel (`TIOCGWINSZ`) for stdout, stderr and stdin in turn, then
/// falls back to the `COLUMNS` / `LINES` environment variables, then to 80x24
pub fn terminal_size() -> (usize, usize) {
    #[cfg(unix)]
    for fd in [1, 2, 0] {
        let mut ws = libc::winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: TIOCGWINSZ only writes a `winsize` through the pointer,
        // which is valid for the duration of the call
        let ok = unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut ws) } == 0;
        if ok && ws.ws_col > 0 && ws.ws_row > 0 {
            return (ws.ws_col as usize, ws.ws_row as usize);
        }
    }
    let env = |name: &str| {
        std::env::var(name)
            .ok()?
            .parse::<usize>()
            .ok()
            .filter(|&v| v > 0)
    };
    (
        env("COLUMNS").unwrap_or(DEFAULT_SIZE.0),
        env("LINES").unwrap_or(DEFAULT_SIZE.1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_never_magnifies_and_keeps_aspect() {
        assert_eq!(fit(200, 200, 1000, 1000), Raster { w: 200, h: 200 });
        // 80x24: 23 usable rows -> 46 pixel rows; limited by height
        assert_eq!(fit(200, 200, 80, 24), Raster { w: 46, h: 46 });
        // limited by width
        assert_eq!(fit(200, 200, 40, 100), Raster { w: 40, h: 40 });
        // non-square field
        let r = fit(200, 100, 80, 50);
        assert_eq!((r.w, r.h), (80, 40));
    }

    #[test]
    fn fit_survives_tiny_terminals() {
        let r = fit(200, 200, 1, 1);
        assert!(r.w >= 1 && r.h >= 2 && r.h % 2 == 0);
    }

    #[test]
    fn sampler_covers_every_cell_exactly_when_not_scaling() {
        let s = Sampler::new(4, 4, Raster { w: 4, h: 4 });
        assert_eq!(s.xs, vec![(0, 1), (1, 2), (2, 3), (3, 4)]);
        // pixel row 0 is the top == largest y
        assert_eq!(s.ys, vec![(3, 4), (2, 3), (1, 2), (0, 1)]);
    }

    #[test]
    fn strongest_cell_wins_when_shrinking() {
        // 4 cells -> 2 pixels; the +1 must survive next to a -0.5 and zeros
        let mut frames = Frames::with_capacity(4, 2, 1);
        frames.push(&[0.0, 1.0, -0.5, 0.0, 0.0, 0.0, 0.0, 0.0]);
        let far = Obstacle {
            x: (9, 9),
            y: (9, 9),
        };
        let s = Sampler::new(4, 2, Raster { w: 2, h: 2 });
        assert_eq!(s.pixel(frames.frame(0), &far, 0, 1), [255, 0, 0]);
        assert_eq!(s.pixel(frames.frame(0), &far, 1, 1), [0, 0, 128]);
    }

    #[test]
    fn y_axis_points_up_and_obstacle_is_grey() {
        // single hot cell at x=0, y=3 (the top-left of an upright picture)
        let mut f = vec![0.0f32; 16];
        f[3 * 4] = 1.0;
        let mut frames = Frames::with_capacity(4, 4, 1);
        frames.push(&f);
        let obstacle = Obstacle {
            x: (3, 3),
            y: (0, 0),
        }; // bottom-right cell
        let s = Sampler::new(4, 4, Raster { w: 4, h: 4 });
        let mut out = Vec::new();
        write_frame(&mut out, &frames, 0, &obstacle, &s).unwrap();
        let text = String::from_utf8(out).unwrap();
        let lines: Vec<&str> = text.split('\n').collect();
        // first text row: hot cell is the foreground (upper pixel) of column 0
        assert!(
            lines[0].starts_with(&format!("{HOME}\x1b[38;2;255;0;0m")),
            "{:?}",
            lines[0]
        );
        // last picture row: the grey obstacle is the background (lower pixel)
        assert!(
            lines[1].contains("\x1b[48;2;110;110;110m"),
            "{:?}",
            lines[1]
        );
        assert!(text.contains("frame 1/1"));
    }

    #[test]
    fn colour_escapes_are_run_length_coded() {
        let mut frames = Frames::with_capacity(6, 2, 1);
        frames.push(&[0.0; 12]);
        let far = Obstacle {
            x: (9, 9),
            y: (9, 9),
        };
        let s = Sampler::new(6, 2, Raster { w: 6, h: 2 });
        let mut out = Vec::new();
        write_frame(&mut out, &frames, 0, &far, &s).unwrap();
        let text = String::from_utf8(out).unwrap();
        // a uniform black row: one fg + one bg escape, six blocks, one reset
        assert_eq!(text.matches("\x1b[38;2;").count(), 1);
        assert_eq!(text.matches("\x1b[48;2;").count(), 1);
        assert_eq!(text.matches(UPPER_HALF).count(), 6);
    }
}
