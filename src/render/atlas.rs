//! Juna-owned shelf atlas; font shaping and rasterization are infrastructure.
use std::collections::{BTreeSet, HashMap};
use swash::zeno::{Format, Vector};
use swash::{
    scale::{image::Content, Render, ScaleContext, Source, StrikeWith},
    shape::ShapeContext,
    text::Codepoint,
};
const PRIMARY: &[&str] = &[
    "IBM Plex Mono",
    "Cascadia Mono",
    "Consolas",
    "DejaVu Sans Mono",
    "Liberation Mono",
    "Courier New",
];
const FALLBACK: &[&str] = &[
    "Segoe UI Emoji",
    "Segoe UI Symbol",
    "MS Gothic",
    "NSimSun",
    "Arial Unicode MS",
    "Segoe UI",
    "Arial",
];
const SIZE: u32 = 2048;
#[derive(Clone, Copy, Debug)]
pub struct Glyph {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub left: i32,
    pub top: i32,
}
#[derive(Clone, Copy, Debug)]
pub struct PositionedGlyph {
    pub glyph: Glyph,
    pub x: f32,
    pub y: f32,
}
struct Face {
    data: Vec<u8>,
    index: usize,
    key: swash::CacheKey,
}
impl Face {
    fn font(&self) -> Option<swash::FontRef<'_>> {
        let mut f = swash::FontRef::from_index(&self.data, self.index)?;
        f.key = self.key;
        Some(f)
    }
}
pub struct Atlas {
    pub pixels: Vec<u8>,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    faces: Vec<Face>,
    cache: HashMap<(usize, u16), Option<Glyph>>,
    shape_cache: HashMap<String, Vec<PositionedGlyph>>,
    shape: ShapeContext,
    scale: ScaleContext,
    pen_x: u32,
    pen_y: u32,
    shelf_h: u32,
    dirty: bool,
    px: f32,
    pub full: bool,
    pub rasterized: u32,
    pub blank: u32,
    pub missing: BTreeSet<u32>,
    ascent: f32,
    descent: f32,
}
impl Atlas {
    pub fn new(device: &wgpu::Device, px: f32) -> Option<Self> {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        let primary = PRIMARY.iter().find_map(|family| {
            db.query(&fontdb::Query {
                families: &[fontdb::Family::Name(family)],
                ..Default::default()
            })
        })?;
        let mut ids = vec![primary];
        for family in FALLBACK {
            if let Some(id) = db.query(&fontdb::Query {
                families: &[fontdb::Family::Name(family)],
                ..Default::default()
            }) {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
        let mut faces = Vec::new();
        for id in ids {
            let (data, index) = db.with_face_data(id, |d, i| (d.to_vec(), i as usize))?;
            if swash::FontRef::from_index(&data, index).is_some() {
                faces.push(Face {
                    data,
                    index,
                    key: swash::CacheKey::new(),
                });
            }
        }
        let m = faces.first()?.font()?.metrics(&[]);
        let k = px / m.units_per_em.max(1) as f32;
        let ascent = m.ascent * k;
        let descent = m.descent.abs() * k;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("juna-atlas"),
            size: wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Some(Self {
            pixels: vec![0; (SIZE * SIZE * 4) as usize],
            texture,
            view,
            faces,
            cache: HashMap::new(),
            shape_cache: HashMap::new(),
            shape: ShapeContext::new(),
            scale: ScaleContext::new(),
            pen_x: 1,
            pen_y: 1,
            shelf_h: 0,
            dirty: true,
            px,
            full: false,
            rasterized: 0,
            blank: 0,
            missing: BTreeSet::new(),
            ascent,
            descent,
        })
    }
    pub fn size_px(&self) -> u32 {
        SIZE
    }
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
    pub fn ascent(&self) -> f32 {
        self.ascent
    }
    pub fn descent(&self) -> f32 {
        self.descent
    }
    /// Recycle only between frames, so no current instance keeps a stale UV.
    pub fn begin_frame(&mut self) {
        if self.full {
            self.pixels.fill(0);
            self.cache.clear();
            self.shape_cache.clear();
            self.pen_x = 1;
            self.pen_y = 1;
            self.shelf_h = 0;
            self.full = false;
            self.dirty = true;
            log::warn!("glyph atlas recycled at frame boundary");
        }
    }
    pub fn get(&mut self, cp: u32) -> Option<Glyph> {
        self.text(&char::from_u32(cp)?.to_string())
            .first()
            .map(|g| g.glyph)
    }
    pub fn text(&mut self, text: &str) -> Vec<PositionedGlyph> {
        if let Some(found) = self.shape_cache.get(text) {
            return found.clone();
        }
        let face = self.faces.iter().position(|f| {
            f.font().is_some_and(|font| {
                text.chars()
                    .all(|ch| crate::vt::width::char_width(ch) == 0 || font.charmap().map(ch) != 0)
            })
        });
        let face = match face {
            Some(f) => f,
            None => {
                for c in text.chars() {
                    self.missing.insert(c as u32);
                }
                0
            }
        };
        let font = self.faces[face].font().unwrap();
        let script = text
            .chars()
            .find(|c| crate::vt::width::char_width(*c) > 0)
            .map(|c| c.script())
            .unwrap_or(swash::text::Script::Latin);
        let mut shaper = self
            .shape
            .builder(font)
            .script(script)
            .size(self.px)
            .features(&[("liga", 0), ("dlig", 0)])
            .build();
        shaper.add_str(text);
        let mut shaped = Vec::new();
        let mut advance = 0.;
        shaper.shape_with(|cluster| {
            for g in cluster.glyphs {
                shaped.push((g.id, advance + g.x, g.y));
                advance += g.advance;
            }
        });
        let mut result = Vec::new();
        for (id, x, y) in shaped {
            if let Some(glyph) = self.raster(face, id) {
                result.push(PositionedGlyph { glyph, x, y });
            }
        }
        if self.shape_cache.len() >= 16384 {
            self.shape_cache.clear();
        }
        if !self.full {
            self.shape_cache.insert(text.to_owned(), result.clone());
        }
        result
    }
    fn raster(&mut self, face: usize, id: u16) -> Option<Glyph> {
        if let Some(hit) = self.cache.get(&(face, id)) {
            return *hit;
        }
        let font = self.faces[face].font()?;
        let mut scaler = self.scale.builder(font).size(self.px).hint(true).build();
        let image = Render::new(&[
            Source::ColorOutline(0),
            Source::ColorBitmap(StrikeWith::BestFit),
            Source::Outline,
        ])
        .format(Format::Alpha)
        .offset(Vector::new(0., 0.))
        .render(&mut scaler, id);
        let image = match image {
            Some(i) if i.placement.width > 0 && i.placement.height > 0 => i,
            _ => {
                self.blank += 1;
                self.cache.insert((face, id), None);
                return None;
            }
        };
        let p = image.placement;
        if self.pen_x + p.width + 1 >= SIZE {
            self.pen_x = 1;
            self.pen_y += self.shelf_h + 1;
            self.shelf_h = 0;
        }
        if self.pen_y + p.height + 1 >= SIZE {
            self.full = true;
            return None;
        }
        let glyph = Glyph {
            x: self.pen_x,
            y: self.pen_y,
            w: p.width,
            h: p.height,
            left: p.left,
            top: p.top,
        };
        for row in 0..p.height {
            for col in 0..p.width {
                let src = (row * p.width + col) as usize;
                let dst = (((glyph.y + row) * SIZE + glyph.x + col) * 4) as usize;
                let rgba = match image.content {
                    Content::Mask => [255, 255, 255, image.data[src]],
                    _ => {
                        let i = src * 4;
                        [
                            image.data[i],
                            image.data[i + 1],
                            image.data[i + 2],
                            image.data[i + 3],
                        ]
                    }
                };
                self.pixels[dst..dst + 4].copy_from_slice(&rgba);
            }
        }
        self.pen_x += p.width + 1;
        self.shelf_h = self.shelf_h.max(p.height);
        self.dirty = true;
        self.rasterized += 1;
        self.cache.insert((face, id), Some(glyph));
        Some(glyph)
    }
    pub fn upload(&mut self, queue: &wgpu::Queue) {
        if !self.dirty {
            return;
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &self.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(SIZE * 4),
                rows_per_image: Some(SIZE),
            },
            wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: 1,
            },
        );
        self.dirty = false;
    }
}
