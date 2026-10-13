//! The particles' shader (`particle_material.wgsl`): AC1's sprites drawn with the level's light (`look`), as an
//! `ExtendedMaterial` over Bevy's `StandardMaterial` (its texture, and the mesh's vertex colours and UVs).
//!
//! An emitter's sprite is its texture times its colour key (the vertex colour, as stored: display-space light as AC1's
//! shaders, `look`). `LightingMode` per vertex (`lit`): times the level's ambient cube toward the camera plus the sun's
//! colour by N.L (the sprite facing the camera); `FogEnabled`: AC1's fog by distance; then the level's grade.
//! `GlowEnabled`: AC1 draws those into its glow pass (a blurred bloom added over the frame); ours adds the sprite's own
//! unlit colour times `GlowIntensity` (no blur). The `BlendMode`s: alpha and copy blended over (premultiplied: one
//! pipeline), additive added (premultiplied, alpha 0), modulate multiplied (`AlphaMode::Multiply`); subtractive drawn as
//! alpha (none of Damascus' emitters has it).

use bevy::asset::embedded_asset;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use forge::fx::Blend;

pub type ParticleSurface = ExtendedMaterial<StandardMaterial, ParticleLook>;

/// The shader's per-batch values.
#[derive(Clone, Copy, Debug, Default, ShaderType, Reflect)]
pub struct ParticleControls {
    /// x: lit (1), y: fogged (1), z: the texture is sRGB (decoded when sampled), w: additive (1).
    pub options: Vec4,
    /// x: the glow's intensity (`GlowIntensity` where `GlowEnabled`, else 0).
    pub glow: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct ParticleLook {
    #[uniform(100)]
    pub controls: ParticleControls,
    /// The level's light (`look`), kept current by `ParticlePlugin`.
    #[uniform(101)]
    pub look: crate::look::WorldLook,
}

impl MaterialExtension for ParticleLook {
    fn fragment_shader() -> ShaderRef {
        "embedded://ac1/particle_material.wgsl".into()
    }
}

pub struct ParticleMaterialPlugin;

impl Plugin for ParticleMaterialPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "particle_material.wgsl");
        app.add_plugins(MaterialPlugin::<ParticleSurface>::default());
    }
}

/// How a batch of a blend mode is drawn: Bevy's alpha mode and whether the shader adds (alpha 0 out).
pub fn alpha_mode(blend: Blend) -> (AlphaMode, bool) {
    match blend {
        Blend::Additive => (AlphaMode::Premultiplied, true),
        Blend::Modulate => (AlphaMode::Multiply, false),
        _ => (AlphaMode::Premultiplied, false),
    }
}

/// A batch's material: the sprite's texture, the blend, lighting and fog.
pub fn surface(texture: Handle<Image>, srgb: bool, blend: Blend, lit: bool, fog: bool, glow: f32, look: crate::look::WorldLook) -> ParticleSurface {
    let (alpha_mode, add) = alpha_mode(blend);
    let flag = |b: bool| if b { 1.0 } else { 0.0 };
    ParticleSurface {
        base: StandardMaterial { base_color_texture: Some(texture), alpha_mode, unlit: true, cull_mode: None, double_sided: true, ..default() },
        extension: ParticleLook {
            controls: ParticleControls { options: Vec4::new(flag(lit), flag(fog), flag(srgb), flag(add)), glow: Vec4::new(glow, 0.0, 0.0, 0.0) },
            look,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn additive_adds_and_alpha_blends_premultiplied() {
        assert_eq!(alpha_mode(Blend::Additive), (AlphaMode::Premultiplied, true));
        assert_eq!(alpha_mode(Blend::Alpha), (AlphaMode::Premultiplied, false));
        assert_eq!(alpha_mode(Blend::Modulate), (AlphaMode::Multiply, false));
    }
}
