// The city shader (city_material.rs): AC1's world lighting. The vertex shader is Bevy's mesh vertex shader cut to what
// the city's meshes carry, plus the vertex's baked occlusion byte (carried to the fragment in world_position.w); the
// fragment lights the StandardMaterial's inputs (its diffuse map, alpha cut-out and normal map) as AC1's GEN_Standard
// shaders do: the ambient cube times the occlusion, plus the sun, then the fog and the level's grade.
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
    mesh_view_bindings as view_bindings,
    mesh_bindings::mesh,
    mesh_functions,
    shadows,
    view_transformations::position_world_to_clip,
    mesh_types::MESH_FLAGS_SHADOW_RECEIVER_BIT,
}
#import ac1::look::{WorldLook, space, to_srgb, to_game, ambient_cube, occlusion, fog, finish}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> look: WorldLook;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<storage, read> ao_words: array<u32>;

struct CityVertex {
    @builtin(instance_index) instance_index: u32,
    @builtin(vertex_index) index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
#ifdef VERTEX_UVS_A
    @location(2) uv: vec2<f32>,
#endif
#ifdef VERTEX_TANGENTS
    @location(4) tangent: vec4<f32>,
#endif
}

@vertex
fn vertex(v: CityVertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(v.instance_index);
    out.world_normal = mesh_functions::mesh_normal_local_to_world(v.normal, v.instance_index);
    out.world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(v.position, 1.0));
    out.position = position_world_to_clip(out.world_position.xyz);
#ifdef VERTEX_UVS_A
    out.uv = v.uv;
#endif
#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(world_from_local, v.tangent, v.instance_index);
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = v.instance_index;
#endif
#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(v.instance_index, world_from_local[3]);
#endif
    // The tag is where this part's first vertex's byte is; -1: none.
    let tag = mesh_functions::get_tag(v.instance_index);
    var ao = -1.0;
    if tag != 0xffffffffu {
        let i = tag + (v.index - mesh[v.instance_index].first_vertex_index);
        ao = f32((ao_words[i >> 2u] >> ((i & 3u) * 8u)) & 0xffu) / 255.0;
    }
    out.world_position.w = ao;
    return out;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var vin = in;
    let ao_raw = in.world_position.w;
    vin.world_position.w = 1.0;
    var p = pbr_input_from_standard_material(vin, is_front);
    p.material.base_color = alpha_discard(p.material, p.material.base_color);
    var albedo = p.material.base_color.rgb;
    if look.options.x > 0.5 {
        albedo = to_srgb(albedo);
    }
    let N = p.N;
    var color = albedo * space(look, ambient_cube(look.env_cube, to_game(N), look.bend.x)) * occlusion(look, ao_raw);
    let receives = (p.flags & MESH_FLAGS_SHADOW_RECEIVER_BIT) != 0u;
    let view_z = dot(vec4<f32>(view_bindings::view.view_from_world[0].z,
        view_bindings::view.view_from_world[1].z, view_bindings::view.view_from_world[2].z,
        view_bindings::view.view_from_world[3].z), p.world_position);
    // The sun: Bevy's first directional light (its direction and shadow maps), AC1's colour.
    if view_bindings::lights.n_directional_lights > 0u {
        let light = view_bindings::lights.directional_lights[0];
        var shade = 1.0;
        if receives && (light.flags & 1u) != 0u {
            shade = shadows::fetch_directional_shadow(0u, p.world_position, p.world_normal, view_z, in.position.xy);
        }
        let nl = max(dot(N, light.direction_to_light), 0.0);
        color += albedo * space(look, look.sun.rgb) * nl * shade;
    }
    color = fog(look, color, distance(view_bindings::view.world_position.xyz, p.world_position.xyz));
    var out: FragmentOutput;
    out.color = vec4<f32>(finish(look, color), p.material.base_color.a);
    return out;
}
