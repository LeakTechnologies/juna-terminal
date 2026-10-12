//! Text rendering: a glyph atlas plus one instanced quad per cell.
//!
//! The renderer **consumes an owned, read-only view** of the engine's screen and
//! never parses VT, never holds engine state, and never mutates the grid
//! (`ARCHITECTURE.md` 3.1). Everything it needs arrives in an owned `RenderSnapshot`.
//! Font and GPU resources carry no mutable terminal state.

mod atlas;
mod contrast;

pub use atlas::Atlas;

use crate::vt::{CellContent, Color, RenderSnapshot};
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct Uniforms {
    pub viewport: [f32; 2],
    /// x = 1 when the target encodes linear shader output into sRGB; y reserved.
    pub encoding: [f32; 2],
}

/// One glyph, one instance. Plain `f32` rather than a packed vertex format: a
/// frame is a few thousand of these and the buffer is rewritten every frame, so
/// a readable layout is worth more than the bytes saved.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct Instance {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub uv0: [f32; 2],
    pub uv1: [f32; 2],
    pub color: [f32; 4],
    /// 1 for a flat fill, 0 for a glyph sample.
    pub solid: f32,
    /// WGSL pads this struct to 64 bytes (vec4 alignment); match it here so
    /// the storage buffer strides align when the shader reads `instances[ii]`.
    pub pad: [f32; 3],
}

impl Instance {
    fn fill(pos: [f32; 2], size: [f32; 2], color: [f32; 4]) -> Self {
        Self {
            pos,
            size,
            uv0: [0.0, 0.0],
            uv1: [0.0, 0.0],
            color,
            solid: 1.0,
            pad: [0.0; 3],
        }
    }

    fn glyph(pos: [f32; 2], size: [f32; 2], uv0: [f32; 2], uv1: [f32; 2], color: [f32; 4]) -> Self {
        Self {
            pos,
            size,
            uv0,
            uv1,
            color,
            solid: 0.0,
            pad: [0.0; 3],
        }
    }
}

/// Dracula theme color palette (https://draculatheme.com/)
fn xterm(n: u8) -> [f32; 3] {
    match n {
        0 => [40.0 / 255.0, 42.0 / 255.0, 54.0 / 255.0], // #282a36 background
        1 => [1.0, 85.0 / 255.0, 85.0 / 255.0],          // #ff5555 red
        2 => [80.0 / 255.0, 250.0 / 255.0, 123.0 / 255.0], // #50fa7b green
        3 => [241.0 / 255.0, 250.0 / 255.0, 140.0 / 255.0], // #f1fa8c yellow
        4 => [189.0 / 255.0, 147.0 / 255.0, 249.0 / 255.0], // #bd93f9 purple/blue
        5 => [1.0, 121.0 / 255.0, 198.0 / 255.0],        // #ff79c6 pink/magenta
        6 => [139.0 / 255.0, 233.0 / 255.0, 253.0 / 255.0], // #8be9fd cyan
        7 => [248.0 / 255.0, 248.0 / 255.0, 242.0 / 255.0], // #f8f8f2 white/foreground
        8 => [98.0 / 255.0, 114.0 / 255.0, 164.0 / 255.0], // #6272a4 bright black (comment)
        9 => [1.0, 85.0 / 255.0, 85.0 / 255.0],          // #ff5555 bright red
        10 => [80.0 / 255.0, 250.0 / 255.0, 123.0 / 255.0], // #50fa7b bright green
        11 => [241.0 / 255.0, 250.0 / 255.0, 140.0 / 255.0], // #f1fa8c bright yellow
        12 => [189.0 / 255.0, 147.0 / 255.0, 249.0 / 255.0], // #bd93f9 bright blue
        13 => [1.0, 121.0 / 255.0, 198.0 / 255.0],       // #ff79c6 bright pink
        14 => [139.0 / 255.0, 233.0 / 255.0, 253.0 / 255.0], // #8be9fd bright cyan
        15 => [248.0 / 255.0, 248.0 / 255.0, 242.0 / 255.0], // #f8f8f2 bright white
        _ => [1.0, 1.0, 1.0],
    }
}

fn color256(n: u8) -> [f32; 3] {
    match n {
        0..=15 => xterm(n),
        16..=231 => {
            let i = n as u32 - 16;
            let steps = [0u8, 95, 135, 175, 215, 255];
            [
                steps[(i / 36) as usize] as f32 / 255.0,
                steps[((i % 36) / 6) as usize] as f32 / 255.0,
                steps[(i % 6) as usize] as f32 / 255.0,
            ]
        }
        _ => {
            let v = (n as u32 - 232) as f32 / 24.0;
            [v, v, v]
        }
    }
}

fn resolve(c: Color, default: [f32; 3]) -> [f32; 3] {
    match c {
        Color::Default => default,
        Color::Indexed(n) => color256(n),
        Color::Rgb(r, g, b) => [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0],
    }
}

pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    instances: wgpu::Buffer,
    capacity: usize,
    count: usize,
    staging: Vec<Instance>,
    pub atlas: Atlas,
    pub cell_w: f32,
    pub cell_h: f32,
    pad: f32,
    debug_frame: u32,
    srgb_target: bool,
    minimum_contrast: f32,
}

impl Renderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        _atlas_size: u32,
        cell: (f32, f32),
        pad: f32,
        font_px: f32,
    ) -> Option<Self> {
        let atlas = Atlas::new(device, font_px)?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("juna-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("juna-uniform"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("juna-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("juna-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let instances = make_instance_buffer(device, 8192);
        let bind_group = build_bind_group(
            device,
            &layout,
            &uniform,
            &sampler,
            atlas.view(),
            &instances,
        );

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("juna-pipeline"),
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("juna-pl"),
                    bind_group_layouts: &[Some(&layout)],
                    immediate_size: 0,
                }),
            ),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    // Alpha blending is mandatory, not cosmetic. With blending
                    // off, an anti-aliased glyph edge is written as opaque
                    // near-black over the cell background, which erases the
                    // glyph instead of shading it.
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        queue.write_buffer(
            &uniform,
            0,
            bytemuck::bytes_of(&Uniforms {
                viewport: [1.0, 1.0],
                encoding: [f32::from(format.is_srgb()), 0.],
            }),
        );

        Some(Self {
            pipeline,
            layout,
            sampler,
            uniform,
            bind_group,
            instances,
            capacity: 8192,
            count: 0,
            staging: Vec::with_capacity(8192),
            atlas,
            cell_w: cell.0,
            cell_h: cell.1,
            pad,
            debug_frame: 0,
            srgb_target: format.is_srgb(),
            minimum_contrast: std::env::var("JUNA_MIN_CONTRAST")
                .ok()
                .and_then(|s| s.parse::<f32>().ok())
                .filter(|v| v.is_finite() && (1.0..=21.0).contains(v))
                .unwrap_or(contrast::DEFAULT_MINIMUM),
        })
    }

    /// Consume only owned screen values; backgrounds precede all glyphs.
    pub fn build(&mut self, screen: &RenderSnapshot, width: u32, height: u32) {
        self.debug_frame += 1;
        self.staging.clear();
        self.atlas.begin_frame();
        let default_fg = [0.9725, 0.9725, 0.9490];
        let default_bg = [0.1569, 0.1647, 0.2118];
        let mut foreground = Vec::new();
        let atlas_px = self.atlas.size_px() as f32;
        let text_h = self.atlas.ascent() + self.atlas.descent();
        let baseline = ((self.cell_h - text_h) * 0.5).max(0.0) + self.atlas.ascent();
        for row in 0..screen.rows {
            for (col, cell) in screen.row(row).iter().enumerate() {
                let x = self.pad + col as f32 * self.cell_w;
                let y = self.pad + row as f32 * self.cell_h;
                let mut fg = resolve(cell.fg, default_fg);
                let mut bg = resolve(cell.bg, default_bg);
                if cell.attrs.is(crate::vt::Attrs::INVERSE) {
                    std::mem::swap(&mut fg, &mut bg);
                }
                fg = contrast::foreground(fg, bg, self.minimum_contrast);
                self.staging.push(Instance::fill(
                    [x, y],
                    [self.cell_w, self.cell_h],
                    [bg[0], bg[1], bg[2], 1.],
                ));
                if matches!(
                    cell.content,
                    CellContent::WideCharSpacer | CellContent::Empty
                ) || cell.attrs.is(crate::vt::Attrs::INVISIBLE)
                {
                    continue;
                }
                let text = match &cell.content {
                    CellContent::Char(c) | CellContent::WideChar(c) => {
                        char::from_u32(*c).unwrap_or('\u{fffd}').to_string()
                    }
                    CellContent::Cluster(s) => s.to_string(),
                    _ => continue,
                };
                let alpha = if cell.attrs.is(crate::vt::Attrs::DIM) {
                    0.55
                } else {
                    1.
                };
                let cell_width = cell.content.width().max(1) as f32 * self.cell_w;
                for shaped in self.atlas.text(&text) {
                    let g = shaped.glyph;
                    let mut inst = Instance::glyph(
                        [
                            x + shaped.x + g.left as f32,
                            y + baseline - g.top as f32 - shaped.y,
                        ],
                        [g.w as f32, g.h as f32],
                        [g.x as f32 / atlas_px, g.y as f32 / atlas_px],
                        [(g.x + g.w) as f32 / atlas_px, (g.y + g.h) as f32 / atlas_px],
                        [fg[0], fg[1], fg[2], alpha],
                    );
                    // Clip to the owning row and terminal cell allocation before upload.
                    if clip_glyph(&mut inst, [x, y], [x + cell_width, y + self.cell_h]) {
                        foreground.push(inst);
                        if cell.attrs.is(crate::vt::Attrs::BOLD) {
                            let mut bold = inst;
                            bold.pos[0] += 0.65;
                            if clip_glyph(&mut bold, [x, y], [x + cell_width, y + self.cell_h]) {
                                foreground.push(bold);
                            }
                        }
                        if cell.attrs.is(crate::vt::Attrs::ITALIC) {
                            foreground.last_mut().unwrap().pad[0] = 0.12;
                        }
                    }
                }
                let color = [fg[0], fg[1], fg[2], alpha];
                if cell.attrs.underline_style() > 0 {
                    foreground.push(Instance::fill(
                        [x, y + self.cell_h - 2.],
                        [cell_width, 1.],
                        color,
                    ));
                }
                if cell.attrs.is(crate::vt::Attrs::STRIKETHROUGH) {
                    foreground.push(Instance::fill(
                        [x, y + self.cell_h * 0.55],
                        [cell_width, 1.],
                        color,
                    ));
                }
                if cell.attrs.is(crate::vt::Attrs::OVERLINE) {
                    foreground.push(Instance::fill([x, y], [cell_width, 1.], color));
                }
            }
        }
        self.staging.extend(foreground);
        let cur = screen.cursor;
        if cur.visible {
            let x = self.pad + cur.col as f32 * self.cell_w;
            let y = self.pad + cur.row as f32 * self.cell_h;
            let (pos, size) = match screen.cursor_style {
                3 | 4 => ([x, y + self.cell_h - 2.], [self.cell_w, 2.]),
                5 | 6 => ([x, y], [2., self.cell_h]),
                _ => ([x, y], [self.cell_w, self.cell_h]),
            };
            self.staging.push(Instance::fill(
                pos,
                size,
                [default_fg[0], default_fg[1], default_fg[2], 0.45],
            ));
        }
        self.count = self.staging.len();
        if self.srgb_target {
            for instance in &mut self.staging {
                for channel in &mut instance.color[..3] {
                    *channel = contrast::linear(*channel);
                }
            }
        }
        let _ = (width, height);
    }

    /// Upload instances and any new glyphs, then draw into the open pass.
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        pass: &mut wgpu::RenderPass<'_>,
    ) {
        // The atlas tracks its own dirtiness, so this is a no-op on steady-state
        // frames and a single upload when new glyphs appear.
        self.atlas.upload(queue);
        queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&Uniforms {
                viewport: [width as f32, height as f32],
                encoding: [f32::from(self.srgb_target), 0.],
            }),
        );

        if self.staging.len() > self.capacity {
            let mut cap = self.capacity;
            while cap < self.staging.len() {
                cap *= 2;
            }
            self.instances = make_instance_buffer(device, cap);
            self.capacity = cap;
            self.bind_group = build_bind_group(
                device,
                &self.layout,
                &self.uniform,
                &self.sampler,
                self.atlas.view(),
                &self.instances,
            );
        }
        if !self.staging.is_empty() {
            queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&self.staging));
        }

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..6, 0..self.count as u32);

        if std::env::var("JUNA_DEBUG_GLYPHS").is_ok() && self.debug_frame == 1 {
            self.debug_frame = 0;
            eprintln!(
                "[draw] count={} staging_len={} cap={}",
                self.count,
                self.staging.len(),
                self.capacity
            );
            for (i, inst) in self.staging.iter().take(4).enumerate() {
                eprintln!(
                    "[draw] inst{i}: pos={:?} size={:?} color={:?} solid={}",
                    inst.pos, inst.size, inst.color, inst.solid
                );
            }
        }
    }

    /// (quads, glyphs rasterised, blank glyphs, true fallback misses)
    pub fn stats(&self) -> (usize, u32, u32, u32) {
        (
            self.count,
            self.atlas.rasterized,
            self.atlas.blank,
            self.atlas.missing.len() as u32,
        )
    }
    pub fn clear_color(&self) -> wgpu::Color {
        let mut rgb = [40. / 255., 42. / 255., 54. / 255.];
        if self.srgb_target {
            rgb = rgb.map(contrast::linear);
        }
        wgpu::Color {
            r: rgb[0] as f64,
            g: rgb[1] as f64,
            b: rgb[2] as f64,
            a: 1.,
        }
    }
}

fn build_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniform: &wgpu::Buffer,
    sampler: &wgpu::Sampler,
    atlas: &wgpu::TextureView,
    instances: &wgpu::Buffer,
) -> wgpu::BindGroup {
    let _ = device;
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("juna-bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(atlas),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: instances.as_entire_binding(),
            },
        ],
    })
}

fn make_instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("juna-instances"),
        size: (capacity * std::mem::size_of::<Instance>()) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
fn clip_glyph(inst: &mut Instance, low: [f32; 2], high: [f32; 2]) -> bool {
    for axis in 0..2 {
        let start = inst.pos[axis];
        let size = inst.size[axis];
        if size <= 0. {
            return false;
        }
        let end = start + size;
        let a = start.max(low[axis]);
        let b = end.min(high[axis]);
        if b <= a {
            return false;
        }
        let uv0 = inst.uv0[axis];
        let span = inst.uv1[axis] - uv0;
        inst.uv0[axis] = uv0 + span * (a - start) / size;
        inst.uv1[axis] = uv0 + span * (b - start) / size;
        inst.pos[axis] = a;
        inst.size[axis] = b - a;
    }
    true
}
