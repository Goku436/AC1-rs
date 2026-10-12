//! The character shader: AC1's character templates (cloth, inside cloth, skin, weapon / mouth, eyes) as an
//! `ExtendedMaterial` over Bevy's `StandardMaterial` (`character_material.wgsl`): the diffuse lit through the template's
//! ramp, Phong highlights from the specular map, cloth's rim, skin's tiled specular and multiply map, the weapons' and
//! mouth's Fresnel, the eyes' iris tint and reflection cube. Bevy keeps the lights, shadows, ambient light and fog.
//!
//! The parts carry a `Look` (`assets`) with the material's parameters; `spawn_character` draws them with a
//! `StandardMaterial` and a `PendingLook`, which `apply_looks` turns into this material (so the spawners need no new
//! asset collections). `AC1_PLAIN_LOOK=1` keeps the plain `StandardMaterial`.
//!
//! The material's layout, the parameter use and the shader are from Banned445's AC1-Movement-Rewritten
//! (src/character_material.rs and .wgsl, MIT):
//! Copyright (c) 2026 Banned445. Permission is hereby granted, free of charge, to any person obtaining a copy of this
//! software and associated documentation files (the "Software"), to deal in the Software without restriction,
//! including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//! copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following
//! conditions: The above copyright notice and this permission notice shall be included in all copies or substantial
//! portions of the Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
//! INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
//! NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE
//! SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use bevy::asset::embedded_asset;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use forge::material::{Material as GameMaterial, Style};

pub type CharacterSurface = ExtendedMaterial<StandardMaterial, CharacterLayers>;

/// The shader's per-material values (`LayerControls` in the WGSL).
#[derive(Clone, Copy, Debug, Default, ShaderType, Reflect)]
pub struct LayerControls {
    pub multiply: Vec4,
    pub specular: Vec4,
    /// Power, factor, rim power, Fresnel baseline.
    pub highlight: Vec4,
    /// Style, skin specular tiling, eye normal blend, eye colour blend.
    pub options: Vec4,
    pub eye_color: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct CharacterLayers {
    #[uniform(100)]
    pub controls: LayerControls,
    #[texture(101)]
    #[sampler(102)]
    pub specular_map: Option<Handle<Image>>,
    #[texture(103)]
    #[sampler(104)]
    pub ramp: Option<Handle<Image>>,
    #[texture(105)]
    #[sampler(106)]
    pub multiply_map: Option<Handle<Image>>,
    #[texture(107, dimension = "cube")]
    #[sampler(108)]
    pub eye_cube: Option<Handle<Image>>,
}

impl MaterialExtension for CharacterLayers {
    fn fragment_shader() -> ShaderRef {
        "embedded://ac1/character_material.wgsl".into()
    }
}

pub struct CharacterMaterialPlugin;

impl Plugin for CharacterMaterialPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "character_material.wgsl");
        app.add_plugins(MaterialPlugin::<CharacterSurface>::default()).add_systems(PostUpdate, apply_looks);
    }
}

/// The shader's style number of a template style.
fn style_index(s: Style) -> u32 {
    match s {
        Style::Cloth => 0,
        Style::InsideCloth => 1,
        Style::Skin => 2,
        Style::Fresnel => 3,
        Style::Eye => 4,
    }
}

/// Eye parameters known only by their name's CRC32: the highlight's power and strength and the iris colour's blend.
const EYE_POWER: u32 = 799_441_376;
const EYE_STRENGTH: u32 = 2_606_792_976;
const EYE_COLOR_BLEND: u32 = 1_837_413_648;

/// The shader values of a material of style `style` (the defaults where it leaves a parameter out).
/// The colour parameters are taken as sRGB, as the textures they tint (Banned445's takes them as linear: `EyeColor`'s brown turned
/// the irises near white).
pub fn controls(m: &GameMaterial, style: Style) -> LayerControls {
    let scalar = |name: &str, default: f32| m.scalar(name).unwrap_or(default);
    let by_id = |id: u32, default: f32| match m.param(id) {
        Some(forge::material::Param::Scalar(v)) => v,
        _ => default,
    };
    let color = |name: &str| {
        let [r, g, b, a] = m.color(name).unwrap_or([1.0; 4]);
        let l = Color::srgba(r, g, b, a).to_linear();
        Vec4::new(l.red, l.green, l.blue, l.alpha)
    };
    let eye = style == Style::Eye;
    LayerControls {
        multiply: color("MultiplyColor") * scalar("MultiplyColorValue", 1.0),
        specular: color("SpecularColor"),
        highlight: Vec4::new(
            if eye { by_id(EYE_POWER, 100.0) } else { scalar("SpecularPower", 1.0) },
            match style {
                Style::Eye => by_id(EYE_STRENGTH, 10.0),
                Style::InsideCloth => 0.0,
                _ => scalar("SpecularFactor", 1.0),
            },
            m.scalar("Rim_Power").or(m.scalar("RimPower")).unwrap_or(0.0),
            scalar("Fresnel", 1.0),
        ),
        options: Vec4::new(style_index(style) as f32, scalar("SpecularTiling", 1.0), scalar("NormalMapBlend", 0.0), by_id(EYE_COLOR_BLEND, 0.0)),
        eye_color: color("EyeColor"),
    }
}

/// A part waiting for its character material: the shader values and maps, and whether it is the inside draw.
#[derive(Component)]
pub struct PendingLook {
    pub layers: CharacterLayers,
    /// The material's own diffuse and normal maps (its slots), where it names them.
    pub diffuse: Option<Handle<Image>>,
    pub normal: Option<Handle<Image>>,
    /// The second draw of two-sided cloth: its back faces, with the inside material.
    pub inside: bool,
}

/// Swap each pending part's `StandardMaterial` for the character material built on it.
fn apply_looks(
    mut commands: Commands,
    parts: Query<(Entity, &MeshMaterial3d<StandardMaterial>, &PendingLook)>,
    standard: Res<Assets<StandardMaterial>>,
    mut surfaces: ResMut<Assets<CharacterSurface>>,
) {
    use bevy::render::render_resource::Face;
    for (e, mat, look) in &parts {
        let Some(base) = standard.get(&mat.0) else { continue };
        let mut base = base.clone();
        if let Some(d) = &look.diffuse {
            base.base_color_texture = Some(d.clone());
        }
        if let Some(n) = &look.normal {
            base.normal_map_texture = Some(n.clone());
        }
        if look.layers.controls.options.x == 4.0 {
            base.alpha_mode = AlphaMode::Opaque;
        }
        if look.inside {
            (base.double_sided, base.cull_mode) = (true, Some(Face::Front));
        }
        let handle = surfaces.add(CharacterSurface { base, extension: look.layers.clone() });
        commands.entity(e).remove::<(MeshMaterial3d<StandardMaterial>, PendingLook)>().insert(MeshMaterial3d(handle));
    }
}

/// Whether the character shader is used (`AC1_PLAIN_LOOK=1` turns it off).
pub fn enabled() -> bool {
    std::env::var("AC1_PLAIN_LOOK").is_err()
}
