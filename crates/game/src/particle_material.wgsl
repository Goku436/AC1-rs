// The particles' shader (particle_material.rs): a sprite's texture times its colour key (the vertex colour, as stored),
// lit where its emitter has AC1's sprite light (the level's ambient cube toward the camera plus the sun by N.L), then
// the level's grade; no fog (AC1's particle shaders have none); out premultiplied (additive: alpha 0, so it adds).
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    mesh_view_bindings as view_bindings,
}
#import ac1::look::{WorldLook, space, to_srgb, to_game, ambient_cube, finish}

struct ParticleControls {
    // lit, a glow's halo, the texture is sRGB, additive
    options: vec4<f32>,
    // the glow's intensity
    glow: vec4<f32>,
}
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> controls: ParticleControls;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<uniform> look: WorldLook;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var vin = in;
    var vc = vec4<f32>(1.0);
#ifdef VERTEX_COLORS
    vc = in.color;
    vin.color = vec4<f32>(1.0);
#endif
    let p = pbr_input_from_standard_material(vin, is_front);
    var tex = p.material.base_color.rgb;
    // (The texture's value as stored, in display-space light: an sRGB texture is decoded when sampled.)
    if look.options.x > 0.5 && controls.options.z > 0.5 {
        tex = to_srgb(tex);
    } else if look.options.x < 0.5 && controls.options.z < 0.5 {
        tex = space(look, tex);
    }
    var color = tex * space(look, vc.rgb);
    let alpha = clamp(p.material.base_color.a * vc.a, 0.0, 1.0);
    let to_eye = view_bindings::view.world_position.xyz - in.world_position.xyz;
    if controls.options.x > 0.5 {
        let N = normalize(to_eye);
        var light = space(look, ambient_cube(look.env_cube, to_game(N), look.bend.x));
        if view_bindings::lights.n_directional_lights > 0u {
            let sun = view_bindings::lights.directional_lights[0];
            light += space(look, look.sun.rgb) * max(dot(N, sun.direction_to_light), 0.0);
        }
        color *= light;
    }
    var out: FragmentOutput;
    let c = finish(look, color);
    // (AC1's glow: the mask, alpha x `GlowIntensity` at most 1 (an 8-bit target), x the colour added; a halo,
    // `particles::HALO`, adds only that, x 1 / HALO².)
    let mask = min(alpha * controls.glow.x, 1.0);
    let weight = select(alpha + mask, mask * controls.glow.y, controls.options.y > 0.5);
    out.color = vec4<f32>(c * weight, select(alpha, 0.0, controls.options.w > 0.5 || controls.options.y > 0.5));
    return out;
}
