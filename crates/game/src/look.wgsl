// AC1's world lighting and colour grade, shared by the character and city shaders (`look.rs`): the ambient cube,
// the fog and the post effects' colour table, written from what AC1's own shaders and grade do (docs/NOTES.md "How AC1
// lights its world").
#define_import_path ac1::look

struct WorldLook {
    // The ambient cubes' faces as the shaders get them: +X, -X, +Y, -Y, +Z, -Z of the game's Z-up world.
    env_cube: array<vec4<f32>, 6>,
    char_cube: array<vec4<f32>, 6>,
    // The world's sun and the characters' own sun: colour times intensity (rgb).
    sun: vec4<f32>,
    char_sun: vec4<f32>,
    // Toward the characters' sun (Bevy space; xyz).
    char_sun_dir: vec4<f32>,
    fog_color: vec4<f32>,
    // near, 1 / (far - near), intensity, on
    fog: vec4<f32>,
    // The normal's Z scale before the cube lookup: environment, characters.
    bend: vec4<f32>,
    // The baked occlusion: scale, bias, floor (`saturate(max(v * scale + bias, floor))`), and the value where a mesh
    // has none.
    ao: vec4<f32>,
    // Colour balance per channel: highlights, midtones, shadows.
    balance_r: vec4<f32>,
    balance_g: vec4<f32>,
    balance_b: vec4<f32>,
    // black, white, gamma, opacity
    levels: vec4<f32>,
    // saturation, brightness, contrast, on
    control: vec4<f32>,
    // x: light in display (gamma) space as AC1's D3D9 shaders do (1) or linear (0); y: grade on; z: balance on; w: the
    // city shows its layer weights (`AC1_SHOW_LAYERS`)
    options: vec4<f32>,
    // Screen-space ambient occlusion on the ambient (the setting, F8): strength x the world's, y the characters'; z the
    // value an open flat surface gets, read as none.
    ssao: vec4<f32>,
}

fn to_srgb(c: vec3<f32>) -> vec3<f32> {
    let x = max(c, vec3<f32>(0.0));
    return select(1.055 * pow(x, vec3<f32>(1.0 / 2.4)) - 0.055, x * 12.92, x <= vec3<f32>(0.0031308));
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    let x = max(c, vec3<f32>(0.0));
    return select(pow((x + 0.055) / 1.055, vec3<f32>(2.4)), x / 12.92, x <= vec3<f32>(0.04045));
}

// A colour of the look's (authored for display) in the space the lighting runs in.
fn space(look: WorldLook, c: vec3<f32>) -> vec3<f32> {
    if look.options.x > 0.5 {
        return c;
    }
    return to_linear(c);
}

// A Bevy (Y-up) direction in the game's Z-up world: game (x, y, z) = Bevy (x, -z, y).
fn to_game(n: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(n.x, -n.z, n.y);
}

// The ambient cube at a normal (game space): each axis's face on the normal's side, weighted by the squared component,
// the normal's Z scaled by `bend` first.
fn ambient_cube(f: array<vec4<f32>, 6>, n: vec3<f32>, bend: f32) -> vec3<f32> {
    let b = normalize(vec3<f32>(n.x, n.y, n.z * bend));
    let s = b * b;
    let x = select(f[1].rgb, f[0].rgb, b.x >= 0.0);
    let y = select(f[3].rgb, f[2].rgb, b.y >= 0.0);
    let z = select(f[5].rgb, f[4].rgb, b.z >= 0.0);
    return s.x * x + s.y * y + s.z * z;
}

// The baked occlusion's factor on the ambient (`raw` < 0: the mesh has none).
fn occlusion(look: WorldLook, raw: f32) -> f32 {
    if raw < 0.0 {
        return look.ao.w;
    }
    return clamp(max(raw * look.ao.x + look.ao.y, look.ao.z), 0.0, 1.0);
}

// AC1's distance fog: toward the fog colour by saturate((distance - near) / (far - near)) * intensity.
fn fog(look: WorldLook, c: vec3<f32>, distance: f32) -> vec3<f32> {
    let f = clamp((distance - look.fog.x) * look.fog.y, 0.0, 1.0) * look.fog.z * look.fog.w;
    return mix(c, space(look, look.fog_color.rgb), f);
}

// The colour balance's weights (GIMP's transfer curves): midtones, and the light or dark end by the shift's sign.
fn mid_weight(x: f32) -> f32 {
    let t = x * 2.0 - 1.0;
    return (1.0 - t * t) * 0.667;
}

fn balance_channel(x0: f32, s: vec4<f32>) -> f32 {
    // shadows
    var x = x0;
    var w = select(1.075 - 1.0 / ((1.0 - x) * 15.93 + 1.0), mid_weight(x), s.z > 0.0);
    x = clamp(x + w * s.z, 0.0, 1.0);
    x = clamp(x + mid_weight(x) * s.y, 0.0, 1.0);
    // highlights
    w = select(mid_weight(x), 1.075 - 1.0 / (x * 15.93 + 1.0), s.x > 0.0);
    return clamp(x + w * s.x, 0.0, 1.0);
}

// The level's post effects on a display colour: per channel levels and colour balance, mixed by opacity; then
// brightness, contrast about 0.5 and saturation (the colour control).
fn grade(look: WorldLook, d: vec3<f32>) -> vec3<f32> {
    var c = clamp(d, vec3<f32>(0.0), vec3<f32>(1.0));
    if look.options.z > 0.5 {
        let range = max(look.levels.y - look.levels.x, 1e-4);
        let lv = pow(max(c - vec3<f32>(look.levels.x), vec3<f32>(0.0)) / range, vec3<f32>(1.0 / max(look.levels.z, 1e-4)));
        let l = clamp(lv, vec3<f32>(0.0), vec3<f32>(1.0));
        let b = vec3<f32>(balance_channel(l.r, look.balance_r), balance_channel(l.g, look.balance_g), balance_channel(l.b, look.balance_b));
        c = c + (b - c) * look.levels.w;
    }
    if look.control.w > 0.5 {
        c = (c * look.control.y - vec3<f32>(0.5)) * look.control.z + vec3<f32>(0.5);
        let lum = dot(c, vec3<f32>(0.3086, 0.6094, 0.082));
        c = vec3<f32>(lum) + (c - vec3<f32>(lum)) * look.control.x;
    }
    return clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
}

// The lit colour (in the lighting's space) to what the shader writes: graded in display space, out as linear.
fn finish(look: WorldLook, c: vec3<f32>) -> vec3<f32> {
    var d = c;
    if look.options.x < 0.5 {
        d = to_srgb(c);
    }
    if look.options.y > 0.5 {
        d = grade(look, d);
    }
    return to_linear(d);
}
