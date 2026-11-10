# yee-algo (now in rust!)
- uses the minisquad crate for opengl rendering
- uses lexopt for argument parsing

# Dependencies
Requires a modern (2024+) rust toolchain to compile.
Third-party crates will be downloaded automatically, and are
listed under `[dependencies]` in `Cargo.toml`

# Building & running
```
git clone [url] && cd yee-algo-rs
cargo run --release -- --renderer opengl 
```

Note: When no renderer argument is passed, the program defaults to rendering in the terminal
Renderer options are:
- opengl
- terminal
