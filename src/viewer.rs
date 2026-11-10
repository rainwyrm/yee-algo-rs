use miniquad::*;

use crate::colormap::{self, OBSTACLE_RGB, Scale};
use crate::frames::Frames;
use crate::geometry::{Constants, Obstacle};

/// the window edge is the smallest whole multiple of the grid
/// size that reaches this many pixels, so every cell is drawn as a
/// square of identical size
const WINDOW_PX: usize = 720;
/// bounds for the display gain
const GAIN_RANGE: (f32, f32) = (1.0 / 64.0, 64.0);
/// bounds for the playback speed in frames per second
const FPS_RANGE: (f64, f64) = (1.0, 1000.0);
/// default playback speed. one frame is `DELTA_T` simulation units, so this
/// plays 7.5 simulation units per second
const DEFAULT_FPS: f64 = 30.0;

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
struct PlaybackState {
    /// index of the frame currently shown (`0..frames`)
    frame: usize,
    /// when `true`, [`EventHandler::update`] does not advance the frame
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
    /// how magnitude maps to intensity (toggled with `C`)
    scale: Scale,
    /// bilinear instead of nearest neighbour filtering (toggled with `F`)
    smooth: bool,
    /// set whenever the pixel buffer must be regenerated.
    /// cleared after the texture has been re-uploaded
    dirty: bool,
}

impl PlaybackState {
    /// 0..n@30fps
    fn new() -> Self {
        Self {
            frame: 0,
            paused: false,
            frames_per_sec: DEFAULT_FPS,
            accumulation: 0.0,
            last_time: date::now(),
            gain: 1.0,
            scale: Scale::Linear,
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
    /// the complete `E_z` history
    frames: Frames,
    obstacle: Obstacle,
    /// cpu-side RGBA staging buffer, `nx * ny * 4` bytes
    pixels: Vec<u8>,

    playback: PlaybackState,
}

impl Viewer {
    /// creates the rendering backend, texture, quad and shader
    /// for the given field data
    ///
    /// the texture has one pixel per cell, and
    /// there is one frame per recorded time step
    ///
    /// must be called after the window exists.
    fn new(frames: Frames, obstacle: Obstacle) -> Self {
        let mut ctx = window::new_rendering_backend();

        let (texture, pixels) = Self::create_texture(&mut *ctx, frames.nx(), frames.ny());
        let bindings = Self::create_quad_bindings(&mut *ctx, texture);
        let pipeline = Self::create_pipeline(&mut *ctx);

        Self {
            ctx,
            pipeline,
            bindings,
            texture,
            frames,
            obstacle,
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

    /// Regenerates the cpu pixel buffer for the current frame
    ///
    /// the frame is already laid out as a texture (rows of `nx` values,
    /// row 0 = `y = 0`), so pixel `(i, j)` is simply element `j * nx + i`.
    /// pixels inside the obstacle are painted grey.
    fn fill_pixels(&mut self) {
        let nx = self.frames.nx();
        let frame = self.frames.frame(self.playback.frame);
        let (gain, scale) = (self.playback.gain, self.playback.scale);
        for (j, row) in frame.chunks_exact(nx).enumerate() {
            for (i, &v) in row.iter().enumerate() {
                let [r, g, b] = if self.obstacle.contains(i, j) {
                    OBSTACLE_RGB
                } else {
                    colormap::signed_rgb(v, gain, scale)
                };
                let p = 4 * (j * nx + i);
                self.pixels[p..p + 4].copy_from_slice(&[r, g, b, 255]);
            }
        }
    }

    /// moves the current frame by `delta`, wrapping around at both ends,
    /// and marks the display as needing a redraw
    fn step(&mut self, delta: isize) {
        let n = self.frames.count() as isize;
        let f = self.playback.frame as isize + delta;
        self.playback.frame = f.rem_euclid(n) as usize;
        self.playback.dirty = true;
    }
}
// ------------------------------------------------------ }}}

// ---- event handler ---------------------------------- {{{

/// advances playback according to wall-clock time
///
/// the elapsed time since the last call (capped at 0.1s so that a stall
/// or window drag does not cause a huge jump, and floored at 0 because
/// `date::now` is the wall clock, which may step backwards) is converted into
/// frames at `frames_per_sec`. only whole frames are applied, the fractional
/// remainder is kept in `accumulation`
impl EventHandler for Viewer {
    fn update(&mut self) {
        let now = date::now();
        let dt = (now - self.playback.last_time).clamp(0.0, 0.1);
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
    fn key_down_event(&mut self, keycode: KeyCode, _keymods: KeyMods, repeat: bool) {
        let p = &mut self.playback;
        match keycode {
            KeyCode::Escape => window::order_quit(),
            // a held key must not flicker between paused and playing
            KeyCode::Space if !repeat => p.paused = !p.paused,
            KeyCode::Right => {
                p.paused = true;
                self.step(1);
            }
            KeyCode::Left => {
                p.paused = true;
                self.step(-1);
            }
            KeyCode::Up => p.frames_per_sec = (p.frames_per_sec * 2.0).min(FPS_RANGE.1),
            KeyCode::Down => p.frames_per_sec = (p.frames_per_sec / 2.0).max(FPS_RANGE.0),
            KeyCode::PageUp => {
                p.gain = (p.gain * 1.5).min(GAIN_RANGE.1);
                p.dirty = true;
            }
            KeyCode::PageDown => {
                p.gain = (p.gain / 1.5).max(GAIN_RANGE.0);
                p.dirty = true;
            }
            KeyCode::C if !repeat => {
                p.scale = p.scale.toggled();
                p.dirty = true;
            }
            KeyCode::F if !repeat => {
                p.smooth = !p.smooth;
                let f = if p.smooth {
                    FilterMode::Linear
                } else {
                    FilterMode::Nearest
                };
                self.ctx
                    .texture_set_filter(self.texture, f, MipmapFilterMode::None);
            }
            KeyCode::R => {
                p.frame = 0;
                p.accumulation = 0.0;
                p.dirty = true;
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
varying highp vec2 uv;
void main() {
    gl_Position = vec4(in_pos, 0.0, 1.0);
    uv = in_uv;
}"#;

/// Fragment shader (GLSL ES 1.00): samples the field texture unmodified.
const FRAGMENT: &str = r#"#version 100
varying highp vec2 uv;
uniform sampler2D tex;
void main() {
    gl_FragColor = texture2D(tex, uv);
}"#;

/// the window edge for a grid `n` cells wide: the smallest whole number
/// of pixels per cell that reaches [`WINDOW_PX`]
fn window_edge(n: usize) -> i32 {
    (n * WINDOW_PX.div_ceil(n).max(1)) as i32
}

/// opens a window and plays back the `E_z` history until the user quits.
///
/// takes ownership of the data. This call blocks until the window is
/// closed (or `Esc` is pressed).
pub fn run(frames: Frames, obstacle: Obstacle) {
    let conf = conf::Conf {
        window_title: format!(
            "2D FDTD (TMz) - E_z - {} x {} cells, dt = {}",
            frames.nx(),
            frames.ny(),
            Constants::DELTA_T
        ),
        window_width: window_edge(frames.nx()),
        window_height: window_edge(frames.ny()),
        // the quad is stretched over the window, so a resizable window
        // would distort the cells
        window_resizable: false,
        ..Default::default()
    };
    miniquad::start(conf, move || Box::new(Viewer::new(frames, obstacle)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_edge_is_a_whole_multiple_of_the_grid() {
        assert_eq!(window_edge(200), 800);
        assert_eq!(window_edge(60), 720);
        assert_eq!(window_edge(1000), 1000);
        assert_eq!(window_edge(720), 720);
    }
}
