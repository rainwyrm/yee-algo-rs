use miniquad::*;

use crate::{dimensions::Array3D, geometry::Obstacle};

const WINDOW_PX: i32 = 720;
const OBSTACLE_RGBA: [u8; 4] = [110, 110, 110, 255];

#[repr(C)]
struct Vertex {
    pos: [f32; 2],
    uv: [f32; 2],
}

// ---- playback state --------------------------------- {{{
struct PlaybackState {
    frame: usize,
    paused: bool,
    frames_per_sec: f64,
    accumulation: f64,
    last_time: f64,
    gain: f32,
    smooth: bool,
    dirty: bool,
}

impl PlaybackState {
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
struct Viewer {
    ctx: Box<dyn RenderingBackend>,
    pipeline: Pipeline,
    bindings: Bindings,
    texture: TextureId,

    // data
    e_z: Array3D<f32>,
    obstacle: Obstacle,
    tex_w: usize,
    tex_h: usize,
    frames: usize,
    pixels: Vec<u8>,

    playback: PlaybackState,
}

impl Viewer {
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

    fn create_texture(ctx: &mut dyn RenderingBackend, w: usize, h: usize) -> (TextureId, Vec<u8>) {
        let pixels = vec![0u8; w * h * 4];
        let texture = ctx.new_texture_from_rgba8(w as u16, h as u16, &pixels);
        ctx.texture_set_filter(texture, FilterMode::Nearest, MipmapFilterMode::None);
        (texture, pixels)
    }

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

    #[rustfmt::skip]
    fn get_vertices() -> [Vertex; 4] {
        [
            Vertex { pos: [-1.0, -1.0], uv: [0.0, 0.0] },
            Vertex { pos: [ 1.0, -1.0], uv: [1.0, 0.0] },
            Vertex { pos: [ 1.0,  1.0], uv: [1.0, 1.0] },
            Vertex { pos: [-1.0,  1.0], uv: [0.0, 1.0] },
        ]
    }

    fn colormap(&self, v: f32) -> [u8; 4] {
        let v = (v * self.playback.gain).clamp(-1.0, 1.0);
        let c = (v.abs() * 255.0).round() as u8;
        if v >= 0.0 {
            [c, 0, 0, 255]
        } else {
            [0, 0, c, 255]
        }
    }

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

    fn step(&mut self, delta: isize) {
        let f = self.playback.frame as isize + delta;
        self.playback.frame = f.rem_euclid(self.frames as isize) as usize;
        self.playback.dirty = true;
    }
}
// ------------------------------------------------------ }}}

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

const VERTEX: &str = r#"#version 100
attribute vec2 in_pos;
attribute vec2 in_uv;
varying lowp vec2 uv;
void main() {
    gl_Position = vec4(in_pos, 0.0, 1.0);
    uv = in_uv;
}"#;

const FRAGMENT: &str = r#"#version 100
varying lowp vec2 uv;
uniform sampler2D tex;
void main() {
    gl_FragColor = texture2D(tex, uv);
}"#;

pub fn run(e_z: Array3D<f32>, obstacle: Obstacle) {
    let conf = conf::Conf {
        window_title: "2D FDTD (TMz) - E_z".to_string(),
        window_width: WINDOW_PX,
        window_height: WINDOW_PX,
        ..Default::default()
    };
    miniquad::start(conf, move || Box::new(Viewer::new(e_z, obstacle)));
}
