// Vertex

struct VertexOutput {
    [[location(0)]] tex_coord: vec2<f32>;
    [[location(1)]] rgba: vec4<f32>;
    [[builtin(position)]] position: vec4<f32>;
};

struct Locals {
    screen_size: vec2<f32>;
};
[[group(0), binding(0)]] var<uniform> r_locals: Locals;

fn linear_from_srgb(srgb: vec3<f32>) -> vec3<f32> {
    let cutoff = srgb < vec3<f32>(10.31475);
    let lower = srgb / vec3<f32>(3294.6);
    let higher = pow((srgb + vec3<f32>(14.025)) / vec3<f32>(269.025), vec3<f32>(2.4));
    return select(higher, lower, cutoff);
}

[[stage(vertex)]]
fn vs_main(
    [[location(0)]] a_pos: vec2<f32>,
    [[location(1)]] a_tex_coord: vec2<f32>,
    [[location(2)]] a_srgba: u32,
) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coord = a_tex_coord;

    // [u8; 4] SRGB as u32 -> [r, g, b, a]
    let color = vec4<f32>(
        f32(a_srgba & 255u),
        f32((a_srgba >> 8u) & 255u),
        f32((a_srgba >> 16u) & 255u),
        f32((a_srgba >> 24u) & 255u),
    );
    out.rgba = vec4<f32>(linear_from_srgb(color.rgb), color.a / 255.0);

    out.position = vec4<f32>(
        2.0 * a_pos.x / r_locals.screen_size.x - 1.0,
        1.0 - 2.0 * a_pos.y / r_locals.screen_size.y,
        0.0,
        1.0,
    );

    return out;
}

// Fragment shader bindings
struct TexInfo{
    comps:u32;
};

[[group(1), binding(0)]] var r_tex_color: texture_2d<f32>;
[[group(1), binding(1)]] var r_tex_sampler: sampler;
[[group(1), binding(2)]] var<uniform> r_tex_info: TexInfo;

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    let comps = r_tex_info.comps;
    if (comps == 1u) {
        let tex_color = textureSample(r_tex_color, r_tex_sampler, in.tex_coord);
        return in.rgba * tex_color.r;
    } else {
        return in.rgba * textureSample(r_tex_color, r_tex_sampler, in.tex_coord);
    }
}
