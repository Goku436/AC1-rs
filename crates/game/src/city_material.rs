//! The city's shader: AC1's world lighting (`GEN_Standard` and kin, as their DX9 shaders do it) as an
//! `ExtendedMaterial` over Bevy's `StandardMaterial` (`city_material.wgsl`): the diffuse lit by the level's environment
//! ambient cube times the entity's baked occlusion, plus the sun (N.L, Bevy's shadow maps), then AC1's fog and grade
//! (`look`). No specular: the city's templates have none.
//!
//! The occlusion is a byte per vertex per placed mesh (`forge::instance`), so placements sharing a mesh differ: the
//! bytes of every placement sit in one storage buffer, and each drawn part's `MeshTag` is where its first vertex's byte
//! is (`NO_AO`: none); the vertex shader reads its own by its index in the mesh. `AC1_PLAIN_CITY=1` keeps the plain
//! `StandardMaterial`.

use bevy::asset::embedded_asset;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::render::storage::ShaderBuffer;
use bevy::shader::ShaderRef;

pub type CitySurface = ExtendedMaterial<StandardMaterial, CityLayers>;

/// A part whose mesh has no baked occlusion.
pub const NO_AO: u32 = u32::MAX;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct CityLayers {
    #[uniform(100)]
    pub look: crate::look::WorldLook,
    /// Every placement's occlusion bytes, four to a word.
    #[storage(101, read_only)]
    pub ao: Handle<ShaderBuffer>,
}

impl MaterialExtension for CityLayers {
    fn vertex_shader() -> ShaderRef {
        "embedded://ac1/city_material.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "embedded://ac1/city_material.wgsl".into()
    }
}

pub struct CityMaterialPlugin;

impl Plugin for CityMaterialPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "city_material.wgsl");
        app.add_plugins(MaterialPlugin::<CitySurface>::default());
    }
}

/// Whether the city shader is used (`AC1_PLAIN_CITY=1` turns it off).
pub fn enabled() -> bool {
    std::env::var("AC1_PLAIN_CITY").is_err()
}

/// The occlusion bytes as the storage buffer holds them (little-endian words, padded).
pub fn ao_buffer(bytes: &[u8]) -> ShaderBuffer {
    let mut b = bytes.to_vec();
    b.resize(b.len().div_ceil(4).max(1) * 4, 255);
    ShaderBuffer::new(&b, bevy::asset::RenderAssetUsages::RENDER_WORLD)
}
