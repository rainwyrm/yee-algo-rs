# yee-algo (now in rust!)
- uses the miniquad crate for opengl rendering
- uses lexopt for argument parsing

# Dependencies
Requires a modern (>=1.85) rust toolchain to compile.
Third-party crates will be downloaded automatically, and are
listed under `[dependencies]` in `Cargo.toml`

# Building & running
```
git clone https://gitlab.dwd.de/sgeissle/yee-algo-rs && cd yee-algo-rs
cargo run --release -- --renderer opengl 
```

Note: When no renderer argument is passed, the program defaults to rendering in the terminal
Renderer options are:
- `opengl`
- `terminal`

# Window Keybinds
|Key         | Function                                 |
|------------|-------------------------------------------|
|Space       | Pause/Resume                             |
|Left/Right  | Step one frame                           |
|Up/Down     | Playback speed x2/÷2                     |
|PgUp/PgDn   | color gain x1.5/÷1.5                     |
|F           | toggle between nearest/bilinear filtering|
|R           | restart                                  |
|Esc         | quit                                     |
