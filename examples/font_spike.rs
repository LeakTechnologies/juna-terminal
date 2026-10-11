use std::time::Instant;

use swash::scale::image::Image;
use swash::scale::{Render, ScaleContext, Source};
use swash::zeno::{Format, Vector};

struct Case {
    label: &'static str,
    text: &'static str,
    note: &'static str,
}

const CASES: &[Case] = &[
    Case { label: "latin", text: "A", note: "plain monospace latin" },
    Case { label: "latin-desc", text: "g", note: "descender + overshoot" },
    Case { label: "combining", text: "\u{65}\u{301}", note: "e + U+0301, needs mark attach" },
    Case { label: "cjk", text: "\u{6f22}", note: "CJK ideograph, wide" },
    Case { label: "emoji", text: "\u{1f600}", note: "grinning face" },
    Case { label: "emoji-zwj", text: "\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}", note: "family, needs shaping" },
    Case { label: "devanagari", text: "\u{915}", note: "ka, needs shaping for matras" },
];

fn pct_ink(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let nz = data.iter().filter(|b| **b != 0).count();
    100.0 * nz as f64 / data.len() as f64
}

fn main() {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();

    let mut monos: Vec<String> = db
        .faces()
        .filter(|f| f.monospaced)
        .filter_map(|f| f.families.first().map(|(n, _)| n.clone()))
        .collect();
    monos.sort();
    monos.dedup();
    println!("=== monospace families discovered: {} ===", monos.len());
    for m in monos.iter().take(20) {
        println!("  {m}");
    }
    if monos.len() > 20 {
        println!("  ... and {} more", monos.len() - 20);
    }

    let wanted = [
        "Cascadia Mono",
        "Cascadia Code",
        "Consolas",
        "DejaVu Sans Mono",
        "Liberation Mono",
        "Courier New",
    ];
    let chosen = wanted.iter().find_map(|w| {
        monos
            .iter()
            .find(|m| m.eq_ignore_ascii_case(w))
            .cloned()
            .or_else(|| monos.iter().find(|m| m.contains(w)).cloned())
    });

    let Some(family) = chosen else {
        println!("\nNO CANDIDATE FONT FOUND");
        return;
    };
    println!("\n=== using family: {family} ===");

    let Some(id) = db.query(&fontdb::Query {
        families: &[fontdb::Family::Name(family.as_str())],
        ..Default::default()
    }) else {
        println!("query failed for {family}");
        return;
    };
    let Some(face) = db.face(id) else {
        println!("face lookup failed for {family}");
        return;
    };
    let face_index = face.index;
    let face_families: Vec<String> = face
        .families
        .iter()
        .map(|(n, _)| n.clone())
        .collect();
    let face_mono = face.monospaced;
    println!("families     = {face_families:?}");
    println!("index        = {face_index}");
    println!("monospace    = {face_mono}");

    let Some(Some(owned)): Option<Option<Vec<u8>>> =
        db.with_face_data(id, |data, _i| Some(data.to_vec()))
    else {
        println!("no data for face");
        return;
    };
    let Some(font) = swash::FontRef::from_index(&owned, face_index as usize) else {
        println!("cannot parse font at index {face_index}");
        return;
    };

    let size = 16.0f32;
    let mut context = ScaleContext::new();
    let mut scaler = context.builder(font).size(size).hint(false).build();

    let m = font.metrics(&[]);
    println!("\n--- font metrics at size {size} (normalized units) ---");
    println!("  units_per_em = {}", m.units_per_em);
    println!("  glyph_count  = {}", m.glyph_count);
    println!("  is_monospace = {}", m.is_monospace);
    println!("  ascent       = {:.4}", m.ascent);
    println!("  descent      = {:.4}", m.descent);
    println!("  leading      = {:.4}", m.leading);
    println!("  cap_height   = {:?}", m.cap_height);

    let gm = font.glyph_metrics(&[]);

    println!("\n--- per-glyph rasterization (zeno, Alpha, hint=false) ---");
    println!(
        "{:<12} {:>5} {:>8} {:>11} {:>7} {:>9} {:>8}",
        "case", "gid", "advance", "bitmap", "ink%", "cold_us", "warm_us"
    );

    for c in CASES {
        let Some(first) = c.text.chars().next() else { continue };
        let gid = font.charmap().map(first);
        if gid == 0 {
            println!("{:<12} {:>5} {:>8}  NO GLYPH IN THIS FONT  ({})", c.label, "-", "-", c.note);
            continue;
        }
        let adv = gm.advance_width(gid);

        let t0 = Instant::now();
        let img1 = Render::new(&[Source::Outline])
            .format(Format::Alpha)
            .offset(Vector::new(0.0, 0.0))
            .render(&mut scaler, gid);
        let cold = t0.elapsed();

        let reps = 500;
        let t1 = Instant::now();
        let mut last = None;
        for _ in 0..reps {
            last = Render::new(&[Source::Outline])
                .format(Format::Alpha)
                .offset(Vector::new(0.0, 0.0))
                .render(&mut scaler, gid);
        }
        let warm = t1.elapsed() / reps;

        let Some(img) = img1.or(last) else {
            println!(
                "{:<12} {:>5} {:>8.3} {:>11}  (no image)  {}",
                c.label, gid, adv, "-", c.note
            );
            continue;
        };
        println!(
            "{:<12} {:>5} {:>8.3} {:>4}x{:<5} {:>6.2} {:>9.1} {:>8.2}   {}",
            c.label,
            gid,
            adv,
            img.placement.width,
            img.placement.height,
            pct_ink(&img.data),
            cold.as_secs_f64() * 1e6,
            warm.as_secs_f64() * 1e6,
            c.note
        );
    }

    println!("\n--- cmap lookup throughput ---");
    let sample: Vec<char> = (0x20u32..0x7f).map(|c| char::from_u32(c).unwrap()).collect();
    let iters = 50_000;
    let total = iters * sample.len();

    let t = Instant::now();
    let mut acc = 0u32;
    for _ in 0..iters {
        for ch in &sample {
            acc += font.charmap().map(*ch) as u32;
        }
    }
    let naive = t.elapsed();
    println!(
        "  NAIVE  (re-materialize Charmap per lookup): {:.1} ns/lookup  (checksum {acc})",
        naive.as_secs_f64() * 1e9 / total as f64
    );

    let cm = font.charmap();
    let t = Instant::now();
    let mut acc = 0u32;
    for _ in 0..iters {
        for ch in &sample {
            acc += cm.map(*ch) as u32;
        }
    }
    let hoisted = t.elapsed();
    println!(
        "  HOOISTED (one Charmap reused):              {:.1} ns/lookup  (checksum {acc})",
        hoisted.as_secs_f64() * 1e9 / total as f64
    );
    println!(
        "  -> naive/hoisted ratio {:.0}x; the naive figure is API misuse, not library cost",
        naive.as_secs_f64() / hoisted.as_secs_f64().max(1e-12)
    );

    println!("\n--- bulk rasterization, full ASCII set, 50 passes ---");
    let ascii: Vec<u16> = (0x20u32..0x7f)
        .filter_map(|c| {
            let g = cm.map(char::from_u32(c).unwrap());
            if g == 0 { None } else { Some(g) }
        })
        .collect();

    let sources = [Source::Outline];
    let mut render = Render::new(&sources);
    render
        .format(Format::Alpha)
        .offset(Vector::new(0.0, 0.0));

    let t = Instant::now();
    let mut rendered = 0usize;
    let mut scratch = Image::new();
    for _ in 0..50 {
        for g in &ascii {
            scratch.clear();
            if render.render_into(&mut scaler, *g, &mut scratch) {
                rendered += 1;
            }
        }
    }
    let reused = t.elapsed();
    println!(
        "  HOOISTED (Render + Image reused): {:.2} us/glyph over {} renders",
        reused.as_secs_f64() * 1e6 / rendered.max(1) as f64,
        rendered
    );

    let t = Instant::now();
    let mut rendered2 = 0usize;
    for _ in 0..50 {
        for g in &ascii {
            if Render::new(&[Source::Outline])
                .format(Format::Alpha)
                .offset(Vector::new(0.0, 0.0))
                .render(&mut scaler, *g)
                .is_some()
            {
                rendered2 += 1;
            }
        }
    }
    let naive_r = t.elapsed();
    println!(
        "  NAIVE (rebuild Render + alloc Image):   {:.2} us/glyph over {} renders",
        naive_r.as_secs_f64() * 1e6 / rendered2.max(1) as f64,
        rendered2
    );
    println!("  -> allocation overhead {:.0}x", naive_r.as_secs_f64() / reused.as_secs_f64().max(1e-12));

    println!("\n--- glyph coverage probe across ALL discovered monospace fonts ---");
    println!("  {:<10} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8}", "family", "latin", "comb", "cjk", "emoji", "zwj", "deva");
    for f in monos.iter().take(12) {
        let Some(q) = db.query(&fontdb::Query {
            families: &[fontdb::Family::Name(f.as_str())],
            ..Default::default()
        }) else {
            continue;
        };
        let Some(Some(owned)): Option<Option<Vec<u8>>> =
            db.with_face_data(q, |data, _i| Some(data.to_vec()))
        else {
            continue;
        };
        let Some(ft) = swash::FontRef::from_index(&owned, 0) else {
            continue;
        };
        let have = |s: &str| -> &'static str {
            match s.chars().next() {
                Some(ch) if ft.charmap().map(ch) != 0 => "yes",
                _ => "no",
            }
        };
        println!(
            "  {:<10} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8}",
            f.chars().take(10).collect::<String>(),
            have("A"),
            have("\u{65}\u{301}"),
            have("\u{6f22}"),
            have("\u{1f600}"),
            have("\u{1f468}"),
            have("\u{915}")
        );
    }

    println!("\nNOTE: coverage above is cmap presence only. A ZWJ family sequence");
    println!("      reports 'yes' for its first codepoint but cannot be rendered");
    println!("      without shaping - cmap lookup alone is insufficient for it.");
    println!("\nNOTE: 'comb' likewise shows only the base 'e'; the U+0301 needs mark attachment.");
}
