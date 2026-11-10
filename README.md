# yee-algo (now in rust!)
2D FDTD (TMz) simulation: a plane-wave pulse travels through a perfectly
conducting box and scatters off a conducting rectangle.

- uses the miniquad crate for opengl rendering
- uses lexopt for argument parsing
- uses libc (unix only) to ask the terminal how big it is

# Dependencies
Requires a modern (>=1.85) rust toolchain to compile.
Third-party crates will be downloaded automatically, and are
listed under `[dependencies]` in `Cargo.toml`

# Building & running
```
git clone https://gitlab.dwd.de/sgeissle/yee-algo-rs && cd yee-algo-rs
cargo run --release -- --renderer opengl
```

Options:

| Option                    | Function                                        |
|---------------------------|-------------------------------------------------|
| `-r`, `--renderer <name>` | `terminal` (default) or `opengl`                |
| `-h`, `--help`            | print usage                                     |

Renderer options are:
- `opengl`: a window, one texel per cell
- `terminal`: plays once in your terminal. The picture is fitted to the
  terminal size (two pixels per text cell using `▀`, so pixels are square)
  and drawn the same way up as the window. If the size cannot be detected,
  set `COLUMNS` and `LINES`, otherwise 80x24 is assumed

# Window Keybinds
|Key         | Function                                  |
|------------|-------------------------------------------|
|Space       | Pause/Resume                              |
|Left/Right  | Step one frame                            |
|Up/Down     | Playback speed x2/÷2                      |
|PgUp/PgDn   | color gain x1.5/÷1.5                      |
|C           | toggle linear / sqrt color scale          |
|F           | toggle between nearest/bilinear filtering |
|R           | restart                                   |
|Esc         | quit                                      |

The sqrt scale lifts weak fields (the scattered and diffracted waves) out of
the black. Blue is negative `E_z`, red is positive, grey is the conductor.

# How it works
- `solver.rs` holds the whole solver state: three `nx * ny` arrays (`E_z`,
  `H_x`, `H_y`) on the staggered yee lattice, updated in place by a leapfrog
  step. The doc comment at the top of the file maps this to the classic
  "index doubling" lattice
- only `E_z` is recorded (`frames.rs`), one snapshot per time step, which is
  what makes stepping backwards and looping possible in the viewer
- all numbers live in `Constants` (`geometry.rs`). Highlights: 200 x 200 cells,
  courant number 0.5 (the stability limit in 2D is 0.707, enforced at compile
  time), a pulse of three wavelengths with 10 cells per wavelength

# Tests
```
cargo test --release
```
The suite checks, among other things, that the solver matches the original
index-doubled kernel bit for bit, that energy is conserved in the closed box,
that the pulse travels in one direction without smearing, and that the setup
stays mirror symmetric.
