// The particles' shader (particle_material.rs): a sprite's texture times its colour key (the vertex colour, as stored),
// lit per vertex where its emitter is (the level's ambient cube toward the camera plus the sun by N.L), fogged where
// it is, then the level's grade; out premultiplied (additive: alpha 0, so it adds).
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    mesh_view_bindings as view_bindings,
}
#import ac1::look::{WorldLook, space, to_srgb, to_game, ambient_cube, fog, finish}

struct ParticleControls {
    // lit, fogged, the texture is sRGB, additive
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
    // (AC1's glow pass: the sprite's own colour, unlit, added; not blurred here.)
    let glow = color * controls.glow.x;
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
    let d = length(to_eye);
    if controls.options.y > 0.5 {
        if controls.options.w > 0.5 {
            // (Added light fades out in the fog rather than turning its colour.)
            color *= 1.0 - clamp((d - look.fog.x) * look.fog.y, 0.0, 1.0) * look.fog.z * look.fog.w;
        } else {
            color = fog(look, color, d);
        }
    }
    var out: FragmentOutput;
    let c = finish(look, color + glow);
    out.color = vec4<f32>(c * alpha, select(alpha, 0.0, controls.options.w > 0.5));
    return out;
}
