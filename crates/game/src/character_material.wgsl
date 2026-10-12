// The character shader: AC1's character templates' lighting on top of Bevy's StandardMaterial inputs, keeping
// Bevy's lights, shadow maps, ambient light, fog and colour management.
//
// From Banned445's AC1-Movement-Rewritten (src/character_material.wgsl), adapted to ac1-rs (the eye cube's lookup
// for our Y-up world; parts that receive no shadows keep out of the shadow maps).
// Copyright (c) 2026 Banned445. MIT License:
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the Software without restriction, including without limitation the
// rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to the following conditions:
// The above copyright notice and this permission notice shall be included in all copies or substantial portions of
// the Software.
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE
// WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
// COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
//
// Styles (`options.x`): 0 cloth, 1 inside cloth, 2 skin, 3 weapon / mouth (Fresnel), 4 eyes.
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, main_pass_post_lighting_processing},
    mesh_view_bindings as view_bindings,
    clustered_forward as clustering,
    shadows,
    mesh_types::MESH_FLAGS_SHADOW_RECEIVER_BIT,
}

struct LayerControls {
    multiply: vec4<f32>,
    specular: vec4<f32>,
    // power, factor, rim power, Fresnel baseline
    highlight: vec4<f32>,
    // style, skin specular tiling, eye normal blend, eye colour blend
    options: vec4<f32>,
    eye_color: vec4<f32>,
}
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> layers: LayerControls;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var specular_map: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var specular_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var ramp: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var ramp_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(105) var multiply_map: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(106) var multiply_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(107) var eye_cube: texture_cube<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(108) var eye_sampler: sampler;

// One light: the diffuse from the template's ramp at (N.L + 1) / 2 (plain N.L for the eyes) and a Phong highlight.
fn light_response(style: u32, diffuse: vec3<f32>, specular: vec3<f32>, power: f32,
    strength: f32, N: vec3<f32>, V: vec3<f32>, L: vec3<f32>) -> vec3<f32> {
    let nl = dot(N, L);
    var diffuse_light = vec3<f32>(max(nl, 0.0));
    if style != 4u {
        diffuse_light = textureSampleLevel(ramp, ramp_sampler, vec2<f32>((nl + 1.0) * 0.5, 0.5), 0.0).rgb;
    }
    let highlight = pow(max(dot(reflect(-L, N), V), 0.0), power) * strength * max(nl, 0.0);
    return diffuse * diffuse_light + specular * highlight;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var p = pbr_input_from_standard_material(in, is_front);
    let style = u32(layers.options.x);
    // The eyes' diffuse alpha is the iris tint's mask: it must not cut holes in the eye.
    var diffuse = p.material.base_color.rgb * layers.multiply.rgb;
    if style == 4u {
        let tinted = mix(p.material.base_color.rgb, layers.eye_color.rgb, p.material.base_color.a);
        diffuse = mix(tinted, p.material.base_color.rgb, layers.options.w);
        p.material.base_color.a = 1.0;
    }
    p.material.base_color = alpha_discard(p.material, p.material.base_color);
    var N = p.N;
    if style == 4u { N = normalize(mix(p.world_normal, N, layers.options.z)); }
    let V = p.V;
    let nv = dot(N, V);
    var power = layers.highlight.x;
    var strength = layers.highlight.y;
    var specular = layers.specular.rgb;
    var spec_uv = in.uv;
    if style == 2u { spec_uv *= layers.options.y; }
    let mask = textureSample(specular_map, specular_sampler, spec_uv);
    if style == 2u { strength *= mask.r; diffuse *= textureSample(multiply_map, multiply_sampler, in.uv).r; }
    else if style != 4u { specular *= mask.rgb; power += mask.b; }
    if style == 3u { strength *= mix(layers.highlight.w, 1.0, pow(abs(1.0 - nv), 5.0)); }
    if style <= 1u {
        // Cloth's rim lightens the diffuse towards the silhouette before lighting.
        diffuse += (0.7 * diffuse + vec3<f32>(0.1)) * (1.0 - pow(abs(nv), layers.highlight.z));
    }
    if style == 4u {
        // The cube is in the game's Z-up world, looked up along (x, z, y) of the reflection there; our world is the
        // game's turned -90 degrees about X (game (x, y, z) = ours (x, -z, y)), so that is ours (x, y, -z).
        let R = reflect(-V, N);
        diffuse += textureSample(eye_cube, eye_sampler, vec3<f32>(R.x, R.y, -R.z)).rgb;
        diffuse *= 2.0;
        strength *= mix(0.2 + 0.8 * pow(abs(1.0 - nv), 5.0), 1.0, pow(abs(1.0 - nv), 5.0));
    }
    // Bevy's ambient light stands in for AC1's environment lighting.
    var color = diffuse * view_bindings::lights.ambient_color.rgb;
    // (Parts marked `NotShadowReceiver`, the faces under the hood, take no shadow-map shadows.)
    let receives = (p.flags & MESH_FLAGS_SHADOW_RECEIVER_BIT) != 0u;
    let view_z = dot(vec4<f32>(view_bindings::view.view_from_world[0].z,
        view_bindings::view.view_from_world[1].z, view_bindings::view.view_from_world[2].z,
        view_bindings::view.view_from_world[3].z), p.world_position);
    for (var i = 0u; i < view_bindings::lights.n_directional_lights; i++) {
        let light = view_bindings::lights.directional_lights[i];
        let L = light.direction_to_light;
        var shade = 1.0;
        if receives && (light.flags & 1u) != 0u {
            shade = shadows::fetch_directional_shadow(i, p.world_position, p.world_normal, view_z, in.position.xy);
        }
        color += light_response(style, diffuse, specular, power, strength, N, V, L) * light.color.rgb * shade;
    }
    let cluster_index = clustering::view_fragment_cluster_index(in.position.xy, view_z, p.is_orthographic);
    let ranges = clustering::unpack_clusterable_object_index_ranges(cluster_index);
    for (var index = ranges.first_point_light_index_offset; index < ranges.first_reflection_probe_index_offset; index++) {
        let id = clustering::get_clusterable_object_id(index);
        let light = view_bindings::clustered_lights.data[id];
        let to_light = light.position_radius.xyz - p.world_position.xyz;
        let distance_squared = dot(to_light, to_light);
        let L = to_light * inverseSqrt(max(distance_squared, 0.000001));
        // A linear fade in squared distance, rather than an inverse-square falloff.
        var attenuation = clamp(1.0 - distance_squared * light.color_inverse_square_range.w, 0.0, 1.0);
        var shade = 1.0;
        if index >= ranges.first_spot_light_index_offset {
            var direction = vec3<f32>(light.light_custom_data.x, 0.0, light.light_custom_data.y);
            direction.y = sqrt(max(0.0, 1.0 - dot(direction, direction)));
            if (light.flags & 2u) != 0u { direction.y = -direction.y; }
            attenuation *= clamp(dot(direction, L) * light.light_custom_data.z + light.light_custom_data.w, 0.0, 1.0);
            if receives && (light.flags & 1u) != 0u {
                shade = shadows::fetch_spot_shadow(id, p.world_position, p.world_normal, light.shadow_map_near_z, in.position.xy);
            }
        } else if receives && (light.flags & 1u) != 0u {
            shade = shadows::fetch_point_shadow(id, p.world_position, p.world_normal, in.position.xy);
        }
        color += light_response(style, diffuse, specular, power, strength, N, V, L)
            * light.color_inverse_square_range.rgb * attenuation * shade;
    }
    var out: FragmentOutput;
    out.color = main_pass_post_lighting_processing(p, vec4<f32>(color * view_bindings::view.exposure, p.material.base_color.a));
    return out;
}
