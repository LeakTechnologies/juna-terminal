//! Text rendering: a glyph atlas plus one instanced quad per cell.
//!
//! The renderer **consumes an owned, read-only view** of the engine's screen and
//! never parses VT, never holds engine state, and never mutates the grid
//! (`ARCHITECTURE.md` 3.1). Everything it needs is copied out of `TerminalState`
//! per frame; if this struct outlived a frame it could not show stale cells.

mod atlas;

pub use atlas::Atlas;

use bytemuck::{Pod, Zeroable};
use crate::vt::{CellContent, Color, TerminalState};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct Uniforms {
    pub viewport: [f32; 2],
    pub atlas: [f32; 2],
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
        0 => [40.0/255.0, 42.0/255.0, 54.0/255.0],    // #282a36 background
        1 => [255.0/255.0, 85.0/255.0, 85.0/255.0],    // #ff5555 red
        2 => [80.0/255.0, 250.0/255.0, 123.0/255.0],   // #50fa7b green
        3 => [241.0/255.0, 250.0/255.0, 140.0/255.0],  // #f1fa8c yellow
        4 => [189.0/255.0, 147.0/255.0, 249.0/255.0],  // #bd93f9 purple/blue
        5 => [255.0/255.0, 121.0/255.0, 198.0/255.0],  // #ff79c6 pink/magenta
        6 => [139.0/255.0, 233.0/255.0, 253.0/255.0],  // #8be9fd cyan
        7 => [248.0/255.0, 248.0/255.0, 242.0/255.0],  // #f8f8f2 white/foreground
        8 => [98.0/255.0, 114.0/255.0, 164.0/255.0],   // #6272a4 bright black (comment)
        9 => [255.0/255.0, 85.0/255.0, 85.0/255.0],    // #ff5555 bright red
        10 => [80.0/255.0, 250.0/255.0, 123.0/255.0],   // #50fa7b bright green
        11 => [241.0/255.0, 250.0/255.0, 140.0/255.0],  // #f1fa8c bright yellow
        12 => [189.0/255.0, 147.0/255.0, 249.0/255.0],  // #bd93f9 bright blue
        13 => [255.0/255.0, 121.0/255.0, 198.0/255.0],  // #ff79c6 bright pink
        14 => [139.0/255.0, 233.0/255.0, 253.0/255.0],  // #8be9fd bright cyan
        15 => [248.0/255.0, 248.0/255.0, 242.0/255.0],  // #f8f8f2 bright white
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

fn luminance(rgb: &[f32; 3]) -> f32 {
    0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2]
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
}

impl Renderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        atlas_size: u32,
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
        let bind_group = build_bind_group(device, &layout, &uniform, &sampler, atlas.view(), &instances);

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("juna-pipeline"),
            layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("juna-pl"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            })),
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
                atlas: [atlas_size as f32, atlas_size as f32],
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
        })
    }

    /// Rebuild every glyph instance from an owned read of the screen.
    ///
    /// Reads only; holds no reference past the call.
    pub fn build(&mut self, terminal: &TerminalState, width: u32, height: u32) {
        self.debug_frame += 1;
        self.staging.clear();

        let grid = terminal.grid();
        let ascent = self.atlas.ascent();
        let descent = self.atlas.descent();
        // Centre the em box vertically inside the cell.
        let text_h = ascent + descent;
        let baseline_pad = ((self.cell_h - text_h) * 0.5).max(0.0);

let default_fg = [0.9725, 0.9725, 0.9490];  // Dracula foreground: #f8f8f2
let default_bg = [0.1569, 0.1647, 0.2118];  // Dracula background: #282a36
        let atlas_px = self.atlas.size_px() as f32;
        for row in 0..grid.rows() {
            let Some(cells) = grid.row_slice(row) else { continue };
            let cell_y = self.pad + row as f32 * self.cell_h;
            for (col, cell) in cells.iter().enumerate() {
                let base_x = self.pad + col as f32 * self.cell_w;

                // Background first: an empty cell still paints its colour, which
                // is what makes SGR 44/100 visible.
                let bg = resolve(cell.bg, default_bg);
                if std::env::var("JUNA_NOBG").is_err() {
                    self.staging.push(Instance::fill(
                        [base_x, cell_y],
                        [self.cell_w, self.cell_h],
                        [bg[0], bg[1], bg[2], 1.0],
                    ));
                }

                let cp = match &cell.content {
                    CellContent::Empty | CellContent::WideCharSpacer => continue,
                    CellContent::Char(c) | CellContent::WideChar(c) => *c,
                    CellContent::Cluster(s) => match s.chars().next() {
                        Some(c) => c as u32,
                        None => continue,
                    },
                };
                let fg = resolve(cell.fg, default_fg);
                let alpha = if cell.attrs.is(crate::vt::Attrs::INVISIBLE) {
                    0.0
                } else {
                    1.0
                };
                if alpha == 0.0 {
                    continue;
                }
                let Some(g) = self.atlas.get(cp) else { continue };
                let x = base_x + g.left as f32;
                let y = cell_y + baseline_pad + ascent - g.top as f32;
                // Actual glyph rendering with atlas sampling and proper color.
                let uv0 = [g.x as f32 / atlas_px, g.y as f32 / atlas_px];
                let uv1 = [(g.x + g.w) as f32 / atlas_px, (g.y + g.h) as f32 / atlas_px];
                self.staging.push(Instance::glyph(
                    [x, y],
                    [g.w as f32, g.h as f32],
                    uv0,
                    uv1,
                    [fg[0], fg[1], fg[2], alpha],
                ));
            }
        }

        // The cursor, drawn as an underline block. A real terminal shows a hollow
        // block when unfocused and a solid one when focused; only the unfocused
        // form is drawn here.
        let cur = grid.cursor;
        self.staging.push(Instance::fill(
            [
                self.pad + cur.col as f32 * self.cell_w,
                self.pad + (cur.row as f32 + 1.0) * self.cell_h - 2.0,
            ],
            [self.cell_w, 2.0],
            [default_fg[0], default_fg[1], default_fg[2], 0.85],
        ));

        self.count = self.staging.len();

        if false {
            self.debug_frame = 0;
            let mut shown = 0;
            for inst in &self.staging {
                if inst.solid > 0.5 {
                    continue;
                }
                eprintln!(
                    "[glyph] pos={:?} size={:?} uv0={:?} uv1={:?} color={:?} solid={}",
                    inst.pos, inst.size, inst.uv0, inst.uv1, inst.color, inst.solid
                );
                shown += 1;
                if shown >= 5 {
                    break;
                }
            }
            eprintln!("[glyph] total instances={}", self.staging.len());
            let nz = self.atlas.pixels.iter().filter(|b| **b != 0).count();
            let maxv = self.atlas.pixels.iter().copied().max().unwrap_or(0);
            eprintln!("[atlas] non-zero bytes={nz} max={maxv} full={}", self.atlas.full);
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
                atlas: [
                    self.atlas.size_px() as f32,
                    self.atlas.size_px() as f32,
                ],
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
                eprintln!("[draw] inst{i}: pos={:?} size={:?} color={:?} solid={}", inst.pos, inst.size, inst.color, inst.solid);
            }
        }
    }

    /// (quads, glyphs rasterised, blank glyphs, true fallback misses)
    pub fn stats(&self) -> (usize, u32, u32, u32) {
        (self.count, self.atlas.rasterized, self.atlas.blank, self.atlas.missing.len() as u32)
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