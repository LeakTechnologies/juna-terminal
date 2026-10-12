// One instanced quad per cell. Vertex positions come from `vertex_index`, so the
// pipeline needs no vertex buffer at all; per-glyph data arrives through a
// read-only storage array indexed by `instance_index`.

struct Uniforms {
    viewport: vec2<f32>,
    encoding: vec2<f32>,
};

struct Instance {
    // Top-left of the glyph bitmap, in pixels.
    pos:   vec2<f32>,
    size:  vec2<f32>,
    // Atlas rectangle, normalised.
    uv0:   vec2<f32>,
    uv1:   vec2<f32>,
    color: vec4<f32>,
    // 1 for a flat fill (background cell, cursor bar), 0 for a glyph sample.
    //
    // This MUST be explicit. An earlier version inferred "solid" from the UVs --
    // `uv.y <= uv.x` -- which is wrong: with shelf packing almost every glyph on
    // the first shelf has atlas y = 1 and any x > 1, so the test was true for
    // them and they rendered as solid blocks. That is what made the text look
    // like a glowing outline, and what blanked the screen on resize.
    solid: f32,
    italic: f32,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var atlas_tex: texture_2d<f32>;
@group(0) @binding(2) var atlas_smp: sampler;
@group(0) @binding(3) var<storage, read> instances: array<Instance>;

struct VSOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) solid: f32,
    @location(3) idx: u32,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VSOut {
    let inst = instances[ii];

    // Two triangles, listed explicitly rather than via an index buffer.
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let c = corners[vi];

    let px = inst.pos + c * inst.size + vec2<f32>((1.0 - c.y) * inst.italic * inst.size.y,0.0);
    // Screen space is y-down; clip space is y-up.
    let ndc = vec2<f32>(
        px.x / max(u.viewport.x, 1.0) * 2.0 - 1.0,
        1.0 - px.y / max(u.viewport.y, 1.0) * 2.0,
    );

    var out: VSOut;
    out.clip = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = mix(inst.uv0, inst.uv1, c);
    out.color = inst.color;
    out.solid = inst.solid;
    out.idx = ii;
    return out;
}

@fragment
fn fs(in: VSOut) -> @location(0) vec4<f32> {
    if (in.solid > 0.5) {
        return in.color;
    }
    let tex = textureSample(atlas_tex, atlas_smp, in.uv);
    var rgb=tex.rgb;
    if (u.encoding.x > 0.5) {
        rgb=select(pow((rgb+vec3<f32>(0.055))/1.055,vec3<f32>(2.4)),rgb/12.92,rgb<=vec3<f32>(0.04045));
    }
    // Straight (non-premultiplied) alpha, matching ALPHA_BLENDING below.
    return vec4<f32>(in.color.rgb * rgb, in.color.a * tex.a);
}
