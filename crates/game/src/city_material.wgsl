// The city shader (city_material.rs): AC1's world lighting. The vertex shader is Bevy's mesh vertex shader cut to what
// the city's meshes carry, plus the vertex's baked occlusion byte (carried to the fragment in world_position.w), its
// colour and its layer weights; the fragment lights the material as AC1's world templates do (city_material.rs): the
// diffuse (one map, or three layers blended by the vertex's weights) times the vertex colour and the template's colour,
// lit by the ambient cube times the occlusion plus the sun, then the fog and the level's grade; water: its scrolling
// normal maps, the cube map's reflection, the sun's highlight.
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

struct CityParams {
    // kind (0 plain, 1 layered, 2 water), saturation (1 none), alpha scale, vertex colour used (1)
    mode: vec4<f32>,
    tint: vec4<f32>,
    color1: vec4<f32>,
    color2: vec4<f32>,
    // per layer: tile factor (u, v), source (0 the mesh's UVs, 1 the world's x, y)
    tile0: vec4<f32>,
    tile1: vec4<f32>,
    tile2: vec4<f32>,
    // panners: scale, translation; translation speed, rotation, rotation speed
    wave1a: vec4<f32>,
    wave1b: vec4<f32>,
    wave2a: vec4<f32>,
    wave2b: vec4<f32>,
    // opacity, bump factor, cube intensity, specular power
    water: vec4<f32>,
    water_color: vec4<f32>,
    // `Env_Standard_WaterBlend`: the water's level (game z), falloff, on
    wet: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> look: WorldLook;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<storage, read> ao_words: array<u32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var<uniform> params: CityParams;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var base_tex: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var base_smp: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(105) var base_nrm: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(106) var base_nrm_smp: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(107) var layer1_tex: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(108) var layer1_smp: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(109) var layer1_nrm: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(110) var layer1_nrm_smp: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(111) var layer2_tex: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(112) var layer2_smp: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(113) var layer2_nrm: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(114) var layer2_nrm_smp: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(115) var cube_tex: texture_cube<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(116) var cube_smp: sampler;

struct CityVertex {
    @builtin(instance_index) instance_index: u32,
    @builtin(vertex_index) index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
#ifdef VERTEX_UVS_A
    @location(2) uv: vec2<f32>,
#endif
#ifdef VERTEX_UVS_B
    @location(3) uv_b: vec2<f32>,
#endif
#ifdef VERTEX_TANGENTS
    @location(4) tangent: vec4<f32>,
#endif
#ifdef VERTEX_COLORS
    @location(5) color: vec4<f32>,
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
#ifdef VERTEX_UVS_B
    out.uv_b = v.uv_b;
#endif
#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(world_from_local, v.tangent, v.instance_index);
#endif
#ifdef VERTEX_COLORS
    out.color = v.color;
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

// A texture's sample (linear, as sampled) in the space the lighting runs in.
fn lit(c: vec3<f32>) -> vec3<f32> {
    if look.options.x > 0.5 {
        return to_srgb(c);
    }
    return c;
}

// The game's x, y of a Bevy position (game y = -Bevy z).
fn world_xy(p: vec4<f32>) -> vec2<f32> {
    return vec2<f32>(p.x, -p.z);
}

// A layer's coordinates: the mesh's UVs or the world's x, y, times its tile factor.
fn layer_uv(t: vec4<f32>, uv: vec2<f32>, xy: vec2<f32>) -> vec2<f32> {
    return select(uv, xy, t.z > 0.5) * t.xy;
}

// x - trunc(x): the game keeps a panner's translation in -1..1.
fn wrap(x: vec2<f32>) -> vec2<f32> {
    return x - trunc(x);
}

// A panner (`UVTransform`) at time t on the world's x, y.
fn panner(a: vec4<f32>, b: vec4<f32>, xy: vec2<f32>, t: f32) -> vec2<f32> {
    let off = a.zw + wrap(b.xy * t);
    let r = b.z + b.w * t;
    let c = cos(r);
    let s = sin(r);
    return vec2<f32>(c * a.x * xy.x - s * a.y * xy.y + off.x, s * a.x * xy.x + c * a.y * xy.y + off.y);
}

// A tangent-space normal (as stored, 0..1, DirectX's green) on the surface: N = x T + y B + z N.
fn perturb(stored: vec3<f32>, N: vec3<f32>, T: vec4<f32>) -> vec3<f32> {
    var nt = stored * 2.0 - 1.0;
    nt.y = -nt.y;
    let B = T.w * cross(N, T.xyz);
    return normalize(nt.x * T.xyz + nt.y * B + nt.z * N);
}

// The sun (Bevy's first directional light: its direction and shadow maps): toward it (xyz) and its shadow (w).
fn sun_shade(world_position: vec4<f32>, world_normal: vec3<f32>, receives: bool, frag: vec2<f32>) -> vec4<f32> {
    if view_bindings::lights.n_directional_lights == 0u {
        return vec4<f32>(0.0);
    }
    let light = view_bindings::lights.directional_lights[0];
    var shade = 1.0;
    if receives && (light.flags & 1u) != 0u {
        let view_z = dot(vec4<f32>(view_bindings::view.view_from_world[0].z,
            view_bindings::view.view_from_world[1].z, view_bindings::view.view_from_world[2].z,
            view_bindings::view.view_from_world[3].z), world_position);
        shade = shadows::fetch_directional_shadow(0u, world_position, world_normal, view_z, frag);
    }
    return vec4<f32>(light.direction_to_light, shade);
}

// AC1's water (`GFX_Water_Puddle`): see city_material.rs.
fn water(in: VertexOutput, vc: vec4<f32>, ao_raw: f32, receives: bool) -> vec4<f32> {
    let xy = world_xy(in.world_position);
    let t = view_bindings::globals.time;
    let n1 = textureSample(base_nrm, base_nrm_smp, panner(params.wave1a, params.wave1b, xy, t)).rgb;
    let n2 = textureSample(base_nrm, base_nrm_smp, panner(params.wave2a, params.wave2b, xy, t)).rgb;
    let avg = (n1 + n2) * 0.5;
    let bump = params.water.y;
    let nt = vec3<f32>((avg.xy * 2.0 - 1.0) * bump, 1.0 + 2.0 * bump * (avg.z - 1.0));
    // (The panners run along the world's x and y: that is the tangent frame; game +x, +y = Bevy +x, -z.)
    let Nv = normalize(in.world_normal);
    let T = normalize(vec3<f32>(1.0, 0.0, 0.0) - Nv * Nv.x);
    let B = cross(Nv, T);
    let N = normalize(nt.x * T + nt.y * B + nt.z * Nv);
    let eye = view_bindings::view.world_position.xyz;
    let V = normalize(in.world_position.xyz - eye);
    let R = reflect(V, N);
    // (The cube is in the game's Z-up world, looked up along (x, z, y): Bevy (x, y, -z).)
    let refl = lit(textureSample(cube_tex, cube_smp, vec3<f32>(R.x, R.y, -R.z)).rgb) * params.water.z * space(look, params.water_color.rgb);
    var color = refl * space(look, ambient_cube(look.env_cube, to_game(N), look.bend.x)) * occlusion(look, ao_raw);
    let sun = sun_shade(in.world_position, Nv, receives, in.position.xy);
    let nl = max(dot(N, sun.xyz), 0.0) * sun.w;
    let spec = pow(max(dot(reflect(-sun.xyz, N), -V), 0.0), params.water.w);
    color += space(look, look.sun.rgb) * nl * (refl + vec3<f32>(spec));
    color = fog(look, color, distance(eye, in.world_position.xyz));
    return vec4<f32>(finish(look, color), clamp(params.water.x * vc.a, 0.0, 1.0));
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var vin = in;
    let ao_raw = in.world_position.w;
    vin.world_position.w = 1.0;
    // The vertex colour and layer weights (white and none where the mesh has none). (Bevy's own use of the colour is
    // left out: it is applied here, in display space, as AC1's shaders do.)
    var vc = vec4<f32>(1.0);
#ifdef VERTEX_COLORS
    if params.mode.w > 0.5 {
        vc = in.color;
    }
    vin.color = vec4<f32>(1.0);
#endif
    var lw = vec2<f32>(0.0);
#ifdef VERTEX_UVS_B
    lw = in.uv_b;
#endif
    var out: FragmentOutput;
    if look.options.w > 0.5 {
        // (`AC1_SHOW_LAYERS`: the first layer's weight red, the second's green, layered materials blue, water white.)
        out.color = vec4<f32>(lw.x, lw.y, select(0.0, 1.0, params.mode.x > 0.5), 1.0);
        if params.mode.x > 1.5 {
            out.color = vec4<f32>(1.0);
        }
        return out;
    }
    var p = pbr_input_from_standard_material(vin, is_front);
    let receives = (p.flags & MESH_FLAGS_SHADOW_RECEIVER_BIT) != 0u;
    if params.mode.x > 1.5 {
        out.color = water(vin, vc, ao_raw, receives);
        return out;
    }
    var albedo = lit(p.material.base_color.rgb);
    var alpha = p.material.base_color.a * params.mode.z;
    var N = p.N;
    if params.mode.x > 0.5 {
        // Layered: the base and two layers by the vertex's weights.
#ifdef VERTEX_UVS_A
        let xy = world_xy(in.world_position);
        let uv0 = layer_uv(params.tile0, in.uv, xy);
        let uv1 = layer_uv(params.tile1, in.uv, xy);
        let uv2 = layer_uv(params.tile2, in.uv, xy);
        let d0 = lit(textureSample(base_tex, base_smp, uv0).rgb) * space(look, params.tint.rgb);
        let d1 = lit(textureSample(layer1_tex, layer1_smp, uv1).rgb) * space(look, params.color1.rgb);
        let d2 = lit(textureSample(layer2_tex, layer2_smp, uv2).rgb) * space(look, params.color2.rgb);
        albedo = mix(mix(d0, d1, lw.x), d2, lw.y);
#ifdef VERTEX_TANGENTS
        let n0 = textureSample(base_nrm, base_nrm_smp, uv0).rgb;
        let n1 = textureSample(layer1_nrm, layer1_nrm_smp, uv1).rgb;
        let n2 = textureSample(layer2_nrm, layer2_nrm_smp, uv2).rgb;
        N = perturb(mix(mix(n0, n1, lw.x), n2, lw.y), p.world_normal, in.world_tangent);
#endif
#endif
        alpha = 1.0;
    }
    albedo *= space(look, vc.rgb);
    alpha *= vc.a;
    if params.mode.x < 0.5 {
        // `Env_Standard_Saturation`: toward the grey of the vertex-coloured diffuse; then the template's colour.
        let grey = dot(albedo, vec3<f32>(1.0 / 3.0));
        albedo = mix(vec3<f32>(grey), albedo, params.mode.y) * space(look, params.tint.rgb);
    }
    let cut = alpha_discard(p.material, vec4<f32>(albedo, alpha));
    // `Env_Standard_WaterBlend`: under the water's level, toward its vertical colour (lit), plus its horizontal colour by
    // the view's vertical component (unlit). (Game z = Bevy y.)
    var under = 0.0;
    if params.wet.z > 0.5 {
        under = clamp((params.wet.x - p.world_position.y) * params.wet.y, 0.0, 1.0);
        albedo = mix(albedo, space(look, params.water_color.rgb), under);
    }
    var color = albedo * space(look, ambient_cube(look.env_cube, to_game(N), look.bend.x)) * occlusion(look, ao_raw);
#ifdef SCREEN_SPACE_AMBIENT_OCCLUSION
    // (The settings' screen-space occlusion on the ambient, at the look's strength.)
    color *= mix(1.0, saturate(textureLoad(view_bindings::screen_space_ambient_occlusion_texture, vec2<i32>(in.position.xy), 0i).r / look.ssao.z), look.ssao.x);
#endif
    let sun = sun_shade(p.world_position, p.world_normal, receives, in.position.xy);
    color += albedo * space(look, look.sun.rgb) * max(dot(N, sun.xyz), 0.0) * sun.w;
    if params.wet.z > 0.5 {
        let vz = normalize(p.world_position.xyz - view_bindings::view.world_position.xyz).y;
        color += space(look, params.color1.rgb) * (1.0 + vz) * under;
    }
    color = fog(look, color, distance(view_bindings::view.world_position.xyz, p.world_position.xyz));
    out.color = vec4<f32>(finish(look, color), cut.a);
    return out;
}
