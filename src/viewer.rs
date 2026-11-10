use miniquad::*;

use crate::{dimensions::Array3D, geometry::Obstacle};

/// initial window width and height in pixels (square window)
const WINDOW_PX: i32 = 720;
/// color of obstacle cells
const OBSTACLE_RGBA: [u8; 4] = [110, 110, 110, 255];

/// a vertex of the fullscreen quad
#[repr(C)] // guarantees exact element ordering in memory, like in C
struct Vertex {
    /// position in normalised coordinates (`-1..=1`)
    pos: [f32; 2],
    /// texture coordinate (`0..=1`)
    uv: [f32; 2],
}

// ---- playback state --------------------------------- {{{
/// mutable state controlling which frame is shown and how fast it advances
#[allow(dead_code)]
struct PlaybackState {
    /// index of the frame currently shown (`0..frames`).
    /// the corresponding doubled-grid time index is `2 * frame`
    frame: usize,
    /// when `true`, [`EventHandler::update`] does advance the frame
    paused: bool,
    /// target playback speed in simulation frames per second of wall time
    frames_per_sec: f64,
    /// fractional frames carried over between updates, so that speeds
    /// that are not a multiple of the display refresh rate stay accurate
    accumulation: f64,
    /// timestamp (from [`date::now`]) of the previous update (in seconds)
    last_time: f64,
    /// display gain multiplied onto the field before color mapping
    gain: f32,
    /// reserved for future stuff & tomfoolery
    smooth: bool,
    /// set whenever the pixel buffer must be regenerated.
    /// cleared after the texture has been re-uploaded
    dirty: bool,
}

impl PlaybackState {
    /// 0..n@120fps
    fn new() -> Self {
        Self {
            frame: 0,
            paused: false,
            frames_per_sec: 120.0,
            accumulation: 0.0,
            last_time: date::now(),
            gain: 1.0,
            smooth: false,
            dirty: true,
        }
    }
}
// ------------------------------------------------------ }}}

// ---- viewer ----------------------------------------- {{{
/// the application (gpu resources, simulation data, playback state)
struct Viewer {
    // c/gpu resources
    ctx: Box<dyn RenderingBackend>,
    pipeline: Pipeline,
    bindings: Bindings,
    texture: TextureId,

    // data
    /// the complete `E_z` history, in doubled-grid indices
    e_z: Array3D<f32>,
    obstacle: Obstacle,
    /// texture width in pixels (= number of cells along x)
    tex_w: usize,
    /// texture height in pixels (= number of cells along y)
    tex_h: usize,
    /// number of playable frames (= number of `E_z` time steps)
    frames: usize,
    /// cpu-side RGBA staging buffer, `tex_w * tex_h * 4` bytes
    pixels: Vec<u8>,

    playback: PlaybackState,
}

impl Viewer {
    /// creates the rendering backend, texture, quad and shader
    /// for the given field data
    ///
    /// the texture has one pixel per physical cell, i.e. half the array
    /// resolution in each of `x` and `y`, and
    /// there is one frame per physical time step
    ///
    /// must be called after the window exists.
    fn new(e_z: Array3D<f32>, obstacle: Obstacle) -> Self {
        let (nx, ny, nt) = e_z.dims();
        let (tex_w, tex_h, frames) = (nx / 2, ny / 2, nt / 2);

        let mut ctx = window::new_rendering_backend();

        let (texture, pixels) = Self::create_texture(&mut *ctx, tex_w, tex_h);
        let bindings = Self::create_quad_bindings(&mut *ctx, texture);
        let pipeline = Self::create_pipeline(&mut *ctx);

        Self {
            ctx,
            pipeline,
            bindings,
            texture,
            e_z,
            obstacle,
            tex_w,
            tex_h,
            frames,
            pixels,
            playback: PlaybackState::new(),
        }
    }

    /// creates a black `w * h` RGBA texture with the nearest neighbour
    /// filtering and returns it together with a matching cpu pixel buffer
    fn create_texture(ctx: &mut dyn RenderingBackend, w: usize, h: usize) -> (TextureId, Vec<u8>) {
        let pixels = vec![0u8; w * h * 4];
        let texture = ctx.new_texture_from_rgba8(w as u16, h as u16, &pixels);
        ctx.texture_set_filter(texture, FilterMode::Nearest, MipmapFilterMode::None);
        (texture, pixels)
    }

    /// builds the vertex/index buffers for a fullscreen quad and binds `texture` to it
    fn create_quad_bindings(ctx: &mut dyn RenderingBackend, texture: TextureId) -> Bindings {
        let vertices = Self::get_vertices();
        let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];

        let vertex_buffer = ctx.new_buffer(
            BufferType::VertexBuffer,
            BufferUsage::Immutable,
            BufferSource::slice(&vertices),
        );

        let index_buffer = ctx.new_buffer(
            BufferType::IndexBuffer,
            BufferUsage::Immutable,
            BufferSource::slice(&indices),
        );

        Bindings {
            vertex_buffers: vec![vertex_buffer],
            index_buffer,
            images: vec![texture],
        }
    }

    /// compiles the [`VERTEX`] / [`FRAGMENT`] shaders and creates the render
    /// pipeline (attributes `in_pos`, `in_uv`; one sampler `tex`)
    fn create_pipeline(ctx: &mut dyn RenderingBackend) -> Pipeline {
        let shader = ctx
            .new_shader(
                ShaderSource::Glsl {
                    vertex: VERTEX,
                    fragment: FRAGMENT,
                },
                ShaderMeta {
                    uniforms: UniformBlockLayout { uniforms: vec![] },
                    images: vec!["tex".to_string()],
                },
            )
            .expect("shader compilation failed");
        ctx.new_pipeline(
            &[BufferLayout::default()],
            &[
                VertexAttribute::new("in_pos", VertexFormat::Float2),
                VertexAttribute::new("in_uv", VertexFormat::Float2),
            ],
            shader,
            PipelineParams::default(),
        )
    }

    /// the four corners of the fullscreen quad, counter-clockwise from
    /// the bottom-left. texture row 0 maps to the bottom edge of the window.
    #[rustfmt::skip]
    fn get_vertices() -> [Vertex; 4] {
        [
            Vertex { pos: [-1.0, -1.0], uv: [0.0, 0.0] },
            Vertex { pos: [ 1.0, -1.0], uv: [1.0, 0.0] },
            Vertex { pos: [ 1.0,  1.0], uv: [1.0, 1.0] },
            Vertex { pos: [-1.0,  1.0], uv: [0.0, 1.0] },
        ]
    }

    /// maps a field value to an opaque RGBA color
    ///
    /// the value is scaled by the current display gain and clamped to
    /// `[-1, 1]`. positive values become red, negative values blue,
    /// with intensity proportional to magnitude
    fn colormap(&self, v: f32) -> [u8; 4] {
        let v = (v * self.playback.gain).clamp(-1.0, 1.0);
        let c = (v.abs() * 255.0).round() as u8;
        if v >= 0.0 {
            [c, 0, 0, 255]
        } else {
            [0, 0, c, 255]
        }
    }

    /// Regenerates the cpu pixel buffer for the current frame
    ///
    /// pixel `(i, j)` samples the array at doubled indices
    /// `(2i, 2j, 2*frame)`, i.e. only the true `E_z` nodes.
    /// pixels inside the obstacle are painted grey.
    fn fill_pixels(&mut self) {
        let n = 2 * self.playback.frame;
        for j in 0..self.tex_h {
            for i in 0..self.tex_w {
                let (gi, gj) = (2 * i, 2 * j);
                let rgba = if self.obstacle.contains(gi, gj) {
                    OBSTACLE_RGBA
                } else {
                    self.colormap(self.e_z[(gi, gj, n)])
                };
                let p = 4 * (j * self.tex_w + i);
                self.pixels[p..p + 4].copy_from_slice(&rgba);
            }
        }
    }

    /// moves the current frame by `delta`, wrapping around at both ends,
    /// and marks the display as needing a redraw
    fn step(&mut self, delta: isize) {
        let f = self.playback.frame as isize + delta;
        self.playback.frame = f.rem_euclid(self.frames as isize) as usize;
        self.playback.dirty = true;
    }
}
// ------------------------------------------------------ }}}

// ---- event handler ---------------------------------- {{{

/// advances playback according to wall-clock time
///
/// the elapsed time since the last call (capped at 0.1s so that a stall
/// or window drag does not cause a huge jump) is converted into frames
/// at `frames_per_sec`. only whole frames are applied, the fractional
/// remainder is kept in `accumulation`
impl EventHandler for Viewer {
    fn update(&mut self) {
        let now = date::now();
        let dt = (now - self.playback.last_time).min(0.1);
        self.playback.last_time = now;
        if self.playback.paused {
            return;
        }
        self.playback.accumulation += dt * self.playback.frames_per_sec;
        let adv = self.playback.accumulation as usize;
        if adv > 0 {
            self.playback.accumulation -= adv as f64;
            self.step(adv as isize);
        }
    }

    /// re-uploads the texture if the frame or gain changed,
    /// then draws the textured quad
    fn draw(&mut self) {
        if self.playback.dirty {
            self.fill_pixels();
            self.ctx.texture_update(self.texture, &self.pixels);
            self.playback.dirty = false;
        }

        self.ctx
            .begin_default_pass(PassAction::clear_color(0.0, 0.0, 0.0, 1.0));
        self.ctx.apply_pipeline(&self.pipeline);
        self.ctx.apply_bindings(&self.bindings);
        self.ctx.draw(0, 6, 1);
        self.ctx.end_render_pass();
        self.ctx.commit_frame();
    }

    /// handles the key bindings listed in the README
    fn key_down_event(&mut self, _keycode: KeyCode, _keymods: KeyMods, _repeat: bool) {
        match _keycode {
            KeyCode::Escape => window::order_quit(),
            KeyCode::Space => self.playback.paused = !self.playback.paused,
            KeyCode::Right => {
                self.playback.paused = true;
                self.step(1);
            }
            KeyCode::Left => {
                self.playback.paused = true;
                self.step(-1);
            }
            KeyCode::Up => {
                self.playback.frames_per_sec = (self.playback.frames_per_sec * 2.0).min(4000.0)
            }
            KeyCode::Down => {
                self.playback.frames_per_sec = (self.playback.frames_per_sec / 2.0).max(1.0)
            }
            KeyCode::PageUp => {
                self.playback.gain *= 1.5;
                self.playback.dirty = true;
            }
            KeyCode::PageDown => {
                self.playback.gain /= 1.5;
                self.playback.dirty = true;
            }
            KeyCode::F => {
                self.playback.frame = 0;
                self.playback.accumulation = 0.0;
                self.playback.dirty = true;
            }
            _ => {}
        }
    }
}
// ------------------------------------------------------ }}}

/// Vertex shader (GLSL ES 1.00): passes the quad position through and
/// forwards the texture coordinate.
const VERTEX: &str = r#"#version 100
attribute vec2 in_pos;
attribute vec2 in_uv;
varying lowp vec2 uv;
void main() {
    gl_Position = vec4(in_pos, 0.0, 1.0);
    uv = in_uv;
}"#;

/// Fragment shader (GLSL ES 1.00): samples the field texture unmodified.
const FRAGMENT: &str = r#"#version 100
varying lowp vec2 uv;
uniform sampler2D tex;
void main() {
    gl_FragColor = texture2D(tex, uv);
}"#;

/// opens a window and plays back the `E_z` history until the user quits.
///
/// takes ownership of the data. This call blocks until the window is
/// closed (or `Esc` is pressed).
///
/// `e_z` must be in doubled-grid indices with even `x`, `y` and time size,
/// as produced by the solver in `main.rs`.
pub fn run(e_z: Array3D<f32>, obstacle: Obstacle) {
    let conf = conf::Conf {
        window_title: "2D FDTD (TMz) - E_z".to_string(),
        window_width: WINDOW_PX,
        window_height: WINDOW_PX,
        ..Default::default()
    };
    miniquad::start(conf, move || Box::new(Viewer::new(e_z, obstacle)));
}
