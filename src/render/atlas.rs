//! Glyph atlas: font discovery, rasterisation, and shelf packing.
//!
//! Backed by `fontdb` (system font discovery) and `swash`/`zeno` (rasterisation),
//! per the D6 decision. No C library, no FreeType, no DirectWrite.
//!
//! # Why an atlas at all
//!
//! Rasterising a full 150x45 grid is ~6750 glyphs. At the measured 2.83 us/glyph
//! (`DECISIONS.md` 6.8) that is ~19 ms *per frame* if done naively, which would
//! cap the terminal at ~50 fps for no reason. Caching means each glyph is
//! rasterised exactly once, making a steady-state frame cost a texture lookup.
//!
//! # Why the fallback chain is here and not "later"
//!
//! `DECISIONS.md` 6.14 is explicit: cross-font fallback is a **mandatory
//! correctness requirement**, not a polish item. No installed monospace font
//! covers emoji or Devanagari, and only two cover CJK. Resolving against a
//! single font would render tofu for most non-Latin content.

use std::collections::HashMap;
use std::sync::Arc;
use swash::scale::{Render, ScaleContext, Source};
use swash::zeno::{Format, Vector};

/// Preference order. Cascadia is the intended default but is **not installed on
/// this machine**, so Consolas is what actually gets used — and every metric
/// observation in the register is therefore a Consolas observation.
const PREFERRED: &[&str] = &[
    // **Chosen by the design lead, 2026-10-11.** IBM Plex Mono is on this
    // machine and is explicitly the intended face for Juna. It precedes
    // Consolas, which was only ever the spike's fallback because Cascadia was
    // absent. Every Consolas-specific metric observation in the register is
    // labelled as such and does not transfer.
    "IBM Plex Mono",
    "Cascadia Mono",
    "Cascadia Code",
    "Consolas",
    "DejaVu Sans Mono",
    "Liberation Mono",
    "Courier New",
    "Lucida Console",
];

/// Consulted only when the primary font has no glyph. Order matters: the first
/// family that actually covers the character wins.
const FALLBACKS: &[&str] = &[
    "Segoe UI Symbol",
    "Segoe UI Emoji",
    "MS Gothic",
    "NSimSun",
    "Arial Unicode MS",
    "Segoe UI",
    "Arial",
];

const ATLAS: u32 = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Key {
    cp: u32,
    face: u8,
}

/// Outcome of resolving one codepoint in one face.
enum Hit {
    /// The face has this glyph and it was rasterised.
    Glyph(Glyph),
    /// The face has this codepoint but it draws nothing (space).
    Blank,
    /// The face has no glyph for this codepoint.
    Absent,
}

#[derive(Clone, Copy, Debug)]
pub struct Glyph {
    /// Atlas texture coordinates, in pixels.
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// Offset from the cell origin to the bitmap's left edge, and from the
    /// baseline to the bitmap's top edge. Both may be negative.
    pub left: i32,
    pub top: i32,
}

/// One loaded face.
///
/// `swash` 0.2 has no owned font type — `FontRef<'a>` borrows the font bytes — so
/// the bytes are kept and the ref is rebuilt on demand. Rebuilding is a pointer
/// cast plus a bounds check, which is free next to rasterisation, and it avoids a
/// self-referential struct.
struct Face {
    data: Arc<Vec<u8>>,
    index: usize,
    id: u8,
    ascent_px: f32,
    descent_px: f32,
    px: f32,
}

impl Face {
    fn font(&self) -> Option<swash::FontRef<'_>> {
        swash::FontRef::from_index(&self.data, self.index)
    }
}

pub struct Atlas {
    size: u32,
    pub pixels: Vec<u8>,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    faces: Vec<Face>,
    cache: HashMap<Key, Option<Glyph>>,
    pen_x: u32,
    pen_y: u32,
    shelf_h: u32,
    /// True once the atlas is full; further glyphs are skipped rather than
    /// silently overwriting existing ones.
    pub full: bool,
    /// Set whenever a glyph is written into `pixels`. The atlas tracks this
    /// itself rather than exposing a `mark_dirty` the caller must remember: an
    /// earlier version relied on the caller, the caller never did it, and the
    /// texture stayed blank — every glyph rendered invisible.
    dirty: bool,
    pub rasterized: u32,
    pub blank: u32,
    /// Codepoints no loaded face could supply. Counted **per distinct
    /// codepoint**, not per draw call, so the number means something.
    pub missing: std::collections::BTreeSet<u32>,
}

impl Atlas {
    pub fn new(device: &wgpu::Device, size_px: f32) -> Option<Self> {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();

        let mut faces: Vec<Face> = Vec::new();
        let mut wanted: Vec<String> = Vec::new();
        wanted.extend(PREFERRED.iter().map(|s| s.to_string()));
        wanted.extend(FALLBACKS.iter().map(|s| s.to_string()));

for family in wanted {
            let Some(id) = db.query(&fontdb::Query {
                families: &[fontdb::Family::Name(family.as_str())],
                ..Default::default()
            }) else {
                continue;
            };
            let Some(face) = db.face(id) else { continue };
            if faces.iter().any(|f| f.index == face.index as usize) {
                continue;
            }
            let Some(Some(data)): Option<Option<Vec<u8>>> =
                db.with_face_data(id, |data: &[u8], _index: u32| Some(data.to_vec()))
            else {
                continue;
            };
            let Some(font) = swash::FontRef::from_index(&data, face.index as usize) else {
                continue;
            };
            let m = font.metrics(&[]);
            let upem = m.units_per_em.max(1) as f32;
            let scale = size_px / upem;
faces.push(Face {
                data: Arc::new(data),
                index: face.index as usize,
                id: faces.len() as u8,
                ascent_px: m.ascent * scale,
                descent_px: m.descent * scale,
                px: size_px,
            });
            if faces.len() as u8 == 3 {
                // Primary plus two fallbacks is ample for now; loading every
                // system face would cost startup time for no benefit.
                break;
            }
        }
        faces.sort_by_key(|f| f.id);
        if faces.is_empty() {
            return None;
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("juna-atlas"),
            size: wgpu::Extent3d {
                width: ATLAS,
                height: ATLAS,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Some(Self {
            size: ATLAS,
            pixels: vec![0u8; (ATLAS * ATLAS) as usize],
            texture,
            view,
            faces,
            cache: HashMap::new(),
            pen_x: 1,
            pen_y: 1,
            shelf_h: 0,
            full: false,
            dirty: false,
            rasterized: 0,
            blank: 0,
            missing: std::collections::BTreeSet::new(),
        })
    }

    pub fn size_px(&self) -> u32 {
        self.size
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn ascent(&self) -> f32 {
        self.faces[0].ascent_px
    }

    pub fn descent(&self) -> f32 {
        self.faces[0].descent_px
    }

/// Resolve a codepoint to a rasterised glyph, trying each face in turn.
    /// Resolve a codepoint to a rasterised glyph, trying each face in turn.
    ///
    /// A space is **not** a miss: it rasterises to nothing, so it lands in
    /// `blank`, not `missed`. An earlier version conflated the two and reported
    /// thousands of "misses" that were almost entirely ordinary whitespace,
    /// which made the counter useless for spotting real fallback failures.
    pub fn get(&mut self, cp: u32) -> Option<Glyph> {
        for face in 0..self.faces.len() as u8 {
            match self.get_in_face(cp, face) {
                Hit::Glyph(g) => return Some(g),
                Hit::Blank => {
                    self.blank += 1;
                    return None;
                }
                Hit::Absent => {}
            }
        }
        self.missing.insert(cp);
        None
    }

    fn get_in_face(&mut self, cp: u32, face_id: u8) -> Hit {
        let key = Key { cp, face: face_id };
        if let Some(hit) = self.cache.get(&key) {
            return match hit {
                Some(g) => Hit::Glyph(*g),
                None => Hit::Absent,
            };
        }

        let Some(ch) = char::from_u32(cp) else {
            self.cache.insert(key, None);
            return Hit::Absent;
        };
        let Some(font) = self.faces[face_id as usize].font() else {
            return Hit::Absent;
        };
        let gid = font.charmap().map(ch);
        if gid == 0 {
            self.cache.insert(key, None);
            return Hit::Absent;
        }

        let px = self.faces[face_id as usize].px;
        let mut ctx = ScaleContext::new();
        let mut scaler = ctx.builder(font).size(px).hint(false).build();
        let img = Render::new(&[Source::Outline])
            .format(Format::Alpha)
            .offset(Vector::new(0.0, 0.0))
            .render(&mut scaler, gid);

        let glyph = match img {
            Some(i) if i.placement.width > 0 && i.placement.height > 0 => {
                let p = i.placement;
                // The atlas being full is not a miss; the glyph simply does not
                // get drawn, and that is reported through `full`.
                self.insert(p.width, p.height, &i.data, p.left, p.top)
            }
            // The font HAS this character but it rasterises to nothing: a space,
            // or a zero-ink control. Not a fallback failure.
            _ => {
                self.blank += 1;
                self.cache.insert(key, None);
                return Hit::Blank;
            }
        };
        self.rasterized += 1;
        self.cache.insert(key, glyph);
        match glyph {
            Some(g) => Hit::Glyph(g),
            None => Hit::Blank,
        }
    }

    /// Shelf packing: place a `w` x `h` bitmap, returning its atlas position.
    fn insert(&mut self, w: u32, h: u32, data: &[u8], left: i32, top: i32) -> Option<Glyph> {
        if self.full || w == 0 || h == 0 {
            return None;
        }
        if w > self.size || h > self.size {
            return None;
        }
        if self.pen_x + w + 1 > self.size {
            self.pen_x = 1;
            self.pen_y += self.shelf_h + 1;
            self.shelf_h = 0;
        }
        if self.pen_y + h + 1 > self.size {
            self.full = true;
            return None;
        }

        let x = self.pen_x;
        let y = self.pen_y;
        for row in 0..h {
            let src = &data[row as usize * w as usize..(row as usize + 1) * w as usize];
            let dst = ((y + row) * self.size + x) as usize;
            self.pixels[dst..dst + w as usize].copy_from_slice(src);
        }
        self.pen_x += w + 1;
        self.shelf_h = self.shelf_h.max(h);
        self.dirty = true;

        Some(Glyph {
            x,
            y,
            w,
            h,
            left,
            top,
        })
    }

    /// Upload pixels if any glyph was added since the last call. Cheap to call every
    /// frame: it returns immediately unless `dirty` is set.
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
                bytes_per_row: Some(self.size),
                rows_per_image: Some(self.size),
            },
            wgpu::Extent3d {
                width: self.size,
                height: self.size,
                depth_or_array_layers: 1,
            },
        );
        self.dirty = false;
    }
}