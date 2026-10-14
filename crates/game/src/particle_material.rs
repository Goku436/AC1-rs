//! The particles' shader (`particle_material.wgsl`): AC1's sprites drawn with the level's light (`look`), as an
//! `ExtendedMaterial` over Bevy's `StandardMaterial` (its texture, and the mesh's vertex colours and UVs).
//!
//! An emitter's sprite is its texture times its colour key (the vertex colour, as stored: display-space light as AC1's
//! shaders, `look`), then the level's grade (AC1 grades the whole frame). AC1's DX9 particle shaders (the `Default
//! Particle Material Template`'s 31 variants) have no fog and light only with the emitter's sprite light flag
//! (`Emitter::sprite_light`, set in no emitter of DataPC, Common or Damascus); where it is set ours lights by the
//! level's ambient cube toward the camera plus the sun's colour by N.L (AC1's: `LightColor` by the direction to
//! `LightPos`, plus `LightAmbient`; not followed, no data to check it on).
//! `GlowEnabled`: AC1 draws the sprite's alpha x `GlowIntensity` into its glow mask (the frame under the mask is blurred
//! and added back); ours adds the sprite's colour x `GlowIntensity` in it and draws a halo (`particles::HALO`),
//! additive, its colour x `glow`. The `BlendMode`s: alpha and copy blended over (premultiplied: one pipeline), additive
//! added (premultiplied, alpha 0), modulate multiplied (`AlphaMode::Multiply`); subtractive drawn as alpha (none of
//! Damascus' emitters has it).

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
    /// x: lit (1), y: a glow's halo (1), z: the texture is sRGB (decoded when sampled), w: additive (1).
    pub options: Vec4,
    /// x: `GlowIntensity` where `GlowEnabled` (else 0), y: a halo's brightness (1 / `particles::HALO`²).
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

/// A batch's material: the sprite's texture, the blend and lighting.
/// `halo`: a glow's halo, its brightness (0: not one).
pub fn surface(texture: Handle<Image>, srgb: bool, blend: Blend, lit: bool, glow: f32, halo: f32, look: crate::look::WorldLook) -> ParticleSurface {
    let (alpha_mode, add) = alpha_mode(blend);
    let flag = |b: bool| if b { 1.0 } else { 0.0 };
    ParticleSurface {
        base: StandardMaterial { base_color_texture: Some(texture), alpha_mode, unlit: true, cull_mode: None, double_sided: true, ..default() },
        extension: ParticleLook {
            controls: ParticleControls { options: Vec4::new(flag(lit), flag(halo > 0.0), flag(srgb), flag(add)), glow: Vec4::new(glow, halo, 0.0, 0.0) },
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
