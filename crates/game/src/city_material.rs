//! The city's shader: AC1's world lighting (`GEN_Standard` and kin, as their DX9 shaders do it) as an
//! `ExtendedMaterial` over Bevy's `StandardMaterial` (`city_material.wgsl`): the diffuse lit by the level's environment
//! ambient cube times the entity's baked occlusion, plus the sun (N.L, Bevy's shadow maps), then AC1's fog and grade
//! (`look`). No specular on the walls and ground: their templates have none.
//!
//! The occlusion is a byte per vertex per placed mesh (`forge::instance`), so placements sharing a mesh differ: the
//! bytes of every placement sit in one storage buffer, and each drawn part's `MeshTag` is where its first vertex's byte
//! is (`NO_AO`: none); the vertex shader reads its own by its index in the mesh. `AC1_PLAIN_CITY=1` keeps the plain
//! `StandardMaterial`.
//!
//! What a material's template does with its maps and parameters (`CityParams`; learned from the templates' DX9 pixel
//! shaders in the data and their constant tables, docs/NOTES.md "City materials"):
//! - the world templates multiply the diffuse by the vertex colour (static meshes' bytes 16..19, `forge::mesh`) and
//!   the output alpha by its alpha (not `Env_Foliage`, `Plants`, `Env_DistrictColorTheme`, whose shaders take none); `DiffuseColor` / `DiffColor` tint it, `Env_Standard_Saturation` first moves it
//!   toward its grey by its saturation factor;
//! - the layered ground (`Env_MultiBlender2` and its `_UVScale`, `_WorldUV`, `_Specular` kin, `Env_MultiBlender1`):
//!   a base, a first and a second layer (diffuse and normal maps, each diffuse times its colour), blended by the vertex's
//!   layer weights (the normal's and the tangent's fourth bytes, carried as the second UV set), each layer on the mesh's
//!   UVs or the world's x, y, times its tile factor;
//! - water (`GFX_Water_Puddle`: rivers, fountains, puddles): two scrolling copies of one normal map on the world's x, y
//!   (the `UVTransform` panners, `forge::material::UvTransform`), averaged and scaled by `BumpFactor`; its cube map along
//!   the view's reflection, times `CubeMapIntensity` and `Color`, lit by the ambient cube and the sun, plus the sun's
//!   Phong highlight at `SpecularPower`; alpha `Opacity` times the vertex alpha (the puddles' and banks' faded rims);
//! - `Env_Standard_WaterBlend` (river walls and piers, the vizier's palace basins: AC1 swaps them in by the entities'
//!   material overrides): below `WorldOceanLevel` the vertex-coloured diffuse goes to `WaterColorVertical` by
//!   `saturate((level - z) * WaterFalloff)` before it is lit, and `WaterColorHorizontal` x (1 + the view's vertical
//!   component) is added unlit, by the same weight.

use bevy::asset::embedded_asset;
use bevy::mesh::VertexAttributeValues;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::render::storage::ShaderBuffer;
use bevy::shader::ShaderRef;
use forge::material::{Material as GameMaterial, Param, UvTransform};

pub type CitySurface = ExtendedMaterial<StandardMaterial, CityLayers>;

/// A part whose mesh has no baked occlusion.
pub const NO_AO: u32 = u32::MAX;

/// How a material's template draws (`CityParams::mode.x`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// One diffuse and normal map (`GEN_Standard`, `GEN_DiffuseAlpha`, `Env_Atlas`, foliage, ...).
    Plain,
    /// A base and two layers blended by the vertex's weights (`Env_MultiBlender*`).
    Layered,
    /// AC1's water (`GFX_Water_Puddle`).
    Water,
}

/// The shader's per-material values (`CityParams` in the WGSL). Colours are as authored (display space).
#[derive(Clone, Copy, Debug, PartialEq, ShaderType, Reflect)]
pub struct CityParams {
    /// Kind (0 plain, 1 layered, 2 water), saturation (1: none), alpha scale, whether the vertex colour is used (1).
    pub mode: Vec4,
    /// The diffuse's (or the base layer's) colour.
    pub tint: Vec4,
    pub color1: Vec4,
    pub color2: Vec4,
    /// Per layer (base, first, second): its tile factor (u, v) and where its coordinates come from (z: 0 the mesh's
    /// UVs, 1 the world's x, y).
    pub tile0: Vec4,
    pub tile1: Vec4,
    pub tile2: Vec4,
    /// The water's panners: scale (u, v), translation (u, v); translation speed (u, v), rotation, rotation speed.
    pub wave1a: Vec4,
    pub wave1b: Vec4,
    pub wave2a: Vec4,
    pub wave2b: Vec4,
    /// The water's opacity, bump factor, cube map intensity, specular power.
    pub water: Vec4,
    /// The water's colour; `Env_Standard_WaterBlend`'s vertical water colour.
    pub water_color: Vec4,
    /// `Env_Standard_WaterBlend` (walls and basin floors under a river's or a fountain's water): the water's level (the
    /// game's z, metres), its falloff (per metre below it), whether it is on; its horizontal colour is `color1`.
    pub wet: Vec4,
}

impl Default for CityParams {
    fn default() -> Self {
        let tile = Vec4::new(1.0, 1.0, 0.0, 0.0);
        Self {
            mode: Vec4::new(0.0, 1.0, 1.0, 1.0),
            tint: Vec4::ONE,
            color1: Vec4::ONE,
            color2: Vec4::ONE,
            tile0: tile,
            tile1: tile,
            tile2: tile,
            wave1a: Vec4::ZERO,
            wave1b: Vec4::ZERO,
            wave2a: Vec4::ZERO,
            wave2b: Vec4::ZERO,
            water: Vec4::new(1.0, 0.0, 1.0, 1.0),
            water_color: Vec4::ONE,
            wet: Vec4::ZERO,
        }
    }
}

/// `Env_Standard_Saturation`'s parameters, known by their names' CRC32 only (its shader's constants are
/// `SaturationFactor` and `DiffuseColor`; the material's names hash otherwise).
const SATURATION: u32 = 0x7da1_42d7;
const SATURATION_COLOR: u32 = 0x845f_2507;

/// The texture slots of `CityLayers`, by the material parameters that fill them.
pub struct Slots {
    pub base: &'static [&'static str],
    pub base_normal: &'static [&'static str],
    pub layer1: &'static [&'static str],
    pub layer1_normal: &'static [&'static str],
    pub layer2: &'static [&'static str],
    pub layer2_normal: &'static [&'static str],
    pub cube: &'static [&'static str],
}

impl Slots {
    pub fn of(kind: Kind) -> Option<Self> {
        match kind {
            Kind::Plain => None,
            Kind::Layered => Some(Self {
                base: &["DiffuseBase"],
                base_normal: &["NormalBase"],
                layer1: &["DiffuseLayer1"],
                layer1_normal: &["NormalLayer1"],
                layer2: &["DiffuseLayer2"],
                layer2_normal: &["NormalLayer2"],
                cube: &[],
            }),
            Kind::Water => Some(Self {
                base: &[],
                base_normal: &["Normal"],
                layer1: &[],
                layer1_normal: &[],
                layer2: &[],
                layer2_normal: &[],
                cube: &["cubemap", "CubeMap"],
            }),
        }
    }
}

/// The kind of a material, by the parameters its template gives it.
pub fn kind_of(m: &GameMaterial) -> Kind {
    if m.uv_transform("Wave01").is_some() {
        Kind::Water
    } else if m.slot(&["DiffuseLayer1"]).is_some() {
        Kind::Layered
    } else {
        Kind::Plain
    }
}

/// A colour parameter, or white.
fn color(m: &GameMaterial, names: &[&str]) -> Vec4 {
    names.iter().find_map(|n| m.color(n)).map_or(Vec4::ONE, |[r, g, b, _]| Vec4::new(r, g, b, 1.0))
}

/// A tile factor (its first two values; 1 without one) and where its coordinates come from.
fn tile(m: &GameMaterial, name: &str, world: bool) -> Vec4 {
    let [u, v, _, _] = m.color(name).unwrap_or([1.0, 1.0, 0.0, 0.0]);
    Vec4::new(u, v, if world { 1.0 } else { 0.0 }, 0.0)
}

/// A panner as the shader takes it: the translation (from zero when animated) and the speeds (zero when not).
fn panner(u: Option<UvTransform>) -> (Vec4, Vec4) {
    let Some(u) = u else { return (Vec4::ZERO, Vec4::ZERO) };
    let (t, ts) = if u.animate_translation { ([0.0; 2], u.translation_speed) } else { (u.translation, [0.0; 2]) };
    let (r, rs) = if u.animate_rotation { (0.0, u.rotation_speed) } else { (u.rotation, 0.0) };
    (Vec4::new(u.scale[0], u.scale[1], t[0], t[1]), Vec4::new(ts[0], ts[1], r, rs))
}

/// Templates whose pixel shaders take no vertex colour (`Env_Foliage`, `Plants`, `Env_DistrictColorTheme`): their meshes'
/// colour bytes are other data (the cypresses' are orange, (255, 185, 0)).
const NO_VERTEX_COLOUR: [u32; 3] = [0x2a8f_90e8, 0x14c1_2ed2, 0x28c1_a6d5];

/// `Env_Foliage_FadeOut`: moss, lichen and ground patches laid over walls and streets, faded out by their vertex alpha
/// (its pixel shader's alpha is the diffuse's times the vertex's): blended over what is behind them.
const FADE_OUT: u32 = 0x3dcd_c04d;

/// `Env_Standard_WaterBlend`: a diffuse and normal map whose colour goes to the water's under its level (see `CityParams::wet`).
const WATER_BLEND: u32 = 0x0631_6c2b;

/// Whether a material's template blends it over what is behind (see `FADE_OUT`; the water is blended by its kind).
pub fn blends(m: &GameMaterial) -> bool {
    m.template == FADE_OUT
}

/// The shader values of a material (see the module docs).
pub fn params_of(m: &GameMaterial) -> CityParams {
    let mut p = CityParams::default();
    if NO_VERTEX_COLOUR.contains(&m.template) {
        p.mode.w = 0.0;
    }
    match kind_of(m) {
        Kind::Water => {
            p.mode.x = 2.0;
            (p.wave1a, p.wave1b) = panner(m.uv_transform("Wave01"));
            (p.wave2a, p.wave2b) = panner(m.uv_transform("Wave02"));
            p.water = Vec4::new(
                m.scalar("Opacity").unwrap_or(1.0),
                m.scalar("BumpFactor").unwrap_or(0.2),
                m.scalar("CubeMapIntensity").unwrap_or(1.0),
                m.scalar("SpecularPower").unwrap_or(100.0),
            );
            p.water_color = color(m, &["Color"]);
        }
        Kind::Layered => {
            p.mode.x = 1.0;
            p.tint = color(m, &["BaseColor"]);
            p.color1 = color(m, &["Layer1Color"]);
            p.color2 = color(m, &["Layer2Color"]);
            if m.color("TileFactor").is_some() {
                // (`_WorldUV[0,2]`: the base and the second layer on the world's x, y, the first on the UVs.)
                p.tile0 = tile(m, "TileFactor", true);
                p.tile1 = tile(m, "TileFactorLayer1", false);
                p.tile2 = p.tile0;
            } else {
                // (`_UVScale`: each on the UVs times its own factor; plain `Env_MultiBlender2`: none, 1.)
                p.tile0 = tile(m, "TileFactorBase", false);
                p.tile1 = tile(m, "TileFactorLayer1", false);
                p.tile2 = tile(m, "TileFactorLayer2", false);
            }
        }
        Kind::Plain => {
            p.tint = match m.param(SATURATION_COLOR) {
                Some(Param::Color([r, g, b, _])) => Vec4::new(r, g, b, 1.0),
                _ => color(m, &["DiffuseColor", "DiffColor"]),
            };
            if let Some(Param::Scalar(s)) = m.param(SATURATION) {
                p.mode.y = s;
            }
            p.mode.z = m.scalar("AlphaValue").unwrap_or(1.0);
            if m.template == WATER_BLEND
                && let Some(level) = m.scalar("WorldOceanLevel")
            {
                p.wet = Vec4::new(level, m.scalar("WaterFalloff").unwrap_or(1.0), 1.0, 0.0);
                p.water_color = color(m, &["WaterColorVertical"]);
                p.color1 = color(m, &["WaterColorHorizontal"]);
            }
        }
    }
    p
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct CityLayers {
    #[uniform(100)]
    pub look: crate::look::WorldLook,
    /// Every placement's occlusion bytes, four to a word.
    #[storage(101, read_only)]
    pub ao: Handle<ShaderBuffer>,
    #[uniform(102)]
    pub params: CityParams,
    /// The layered materials' base, first and second layers (diffuse, normal); the water's normal map (`base_normal`)
    /// and cube map.
    #[texture(103)]
    #[sampler(104)]
    pub base: Option<Handle<Image>>,
    #[texture(105)]
    #[sampler(106)]
    pub base_normal: Option<Handle<Image>>,
    #[texture(107)]
    #[sampler(108)]
    pub layer1: Option<Handle<Image>>,
    #[texture(109)]
    #[sampler(110)]
    pub layer1_normal: Option<Handle<Image>>,
    #[texture(111)]
    #[sampler(112)]
    pub layer2: Option<Handle<Image>>,
    #[texture(113)]
    #[sampler(114)]
    pub layer2_normal: Option<Handle<Image>>,
    #[texture(115, dimension = "cube")]
    #[sampler(116)]
    pub cube: Option<Handle<Image>>,
}

/// What a city material slot draws beyond its `StandardMaterial` (made by the city's material bank; with the level's
/// look and the occlusion buffer it becomes the slot's `CityLayers`).
#[derive(Clone, Debug, Default)]
pub struct SlotLayers {
    pub params: CityParams,
    pub base: Option<Handle<Image>>,
    pub base_normal: Option<Handle<Image>>,
    pub layer1: Option<Handle<Image>>,
    pub layer1_normal: Option<Handle<Image>>,
    pub layer2: Option<Handle<Image>>,
    pub layer2_normal: Option<Handle<Image>>,
    pub cube: Option<Handle<Image>>,
}

impl SlotLayers {
    pub fn layers(&self, look: crate::look::WorldLook, ao: Handle<ShaderBuffer>) -> CityLayers {
        CityLayers {
            look,
            ao,
            params: self.params,
            base: self.base.clone(),
            base_normal: self.base_normal.clone(),
            layer1: self.layer1.clone(),
            layer1_normal: self.layer1_normal.clone(),
            layer2: self.layer2.clone(),
            layer2_normal: self.layer2_normal.clone(),
            cube: self.cube.clone(),
        }
    }
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

/// A static mesh part's vertex colours (as `ATTRIBUTE_COLOR`, the bytes / 255 as stored: the shader multiplies them in
/// display space as AC1's does) and layer weights (as `ATTRIBUTE_UV_1`), each only where it is not the default (white;
/// no weight), so most parts carry neither.
pub fn vertex_extras(mesh: &mut Mesh, colors: &[[u8; 4]], layers: &[[u8; 2]]) {
    if colors.iter().any(|c| *c != [255; 4]) {
        let c: Vec<[f32; 4]> = colors.iter().map(|c| c.map(|x| x as f32 / 255.0)).collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, VertexAttributeValues::Float32x4(c));
    }
    if layers.iter().any(|l| *l != [0; 2]) {
        let l: Vec<[f32; 2]> = layers.iter().map(|l| l.map(|x| x as f32 / 255.0)).collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, VertexAttributeValues::Float32x2(l));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn material(params: Vec<(&str, Param)>) -> GameMaterial {
        GameMaterial {
            template: 0,
            texture_set: 0,
            inside: None,
            params: params.into_iter().map(|(n, p)| (forge::material::param_id(n), p)).collect(),
            alpha_test: None,
            two_sided: false,
            hidden: false,
        }
    }

    #[test]
    fn layered_materials_take_their_tile_factors() {
        let map = Param::Map { mode: 1, channel: 0, set: 1, map: 0 };
        let world =
            material(vec![("DiffuseLayer1", map), ("TileFactor", Param::Color([0.35, 0.35, 0.0, 0.0])), ("Layer2Color", Param::Color([0.5, 0.4, 0.3, 0.0]))]);
        let p = params_of(&world);
        assert_eq!(p.mode.x, 1.0);
        assert_eq!((p.tile0, p.tile1, p.tile2), (Vec4::new(0.35, 0.35, 1.0, 0.0), Vec4::new(1.0, 1.0, 0.0, 0.0), Vec4::new(0.35, 0.35, 1.0, 0.0)));
        assert_eq!(p.color2, Vec4::new(0.5, 0.4, 0.3, 1.0));
        let scaled = material(vec![("DiffuseLayer1", map), ("TileFactorLayer1", Param::Color([2.0, 2.0, 0.0, 0.0]))]);
        assert_eq!(params_of(&scaled).tile1, Vec4::new(2.0, 2.0, 0.0, 0.0));
    }

    #[test]
    fn water_takes_its_panners() {
        let wave = UvTransform {
            scale: [0.1, 0.1],
            translation: [0.5, 0.0],
            translation_speed: [-0.02, -0.03],
            rotation: 0.0,
            rotation_speed: 0.0,
            animate_translation: true,
            animate_rotation: false,
        };
        let m = material(vec![("Wave01", Param::Uv(wave)), ("Opacity", Param::Scalar(0.8)), ("Color", Param::Color([0.3, 0.4, 0.4, 0.0]))]);
        assert_eq!(kind_of(&m), Kind::Water);
        let p = params_of(&m);
        // (Animated: from zero, at its speed.)
        assert_eq!((p.wave1a, p.wave1b), (Vec4::new(0.1, 0.1, 0.0, 0.0), Vec4::new(-0.02, -0.03, 0.0, 0.0)));
        assert_eq!(p.water.x, 0.8);
        assert_eq!(p.water_color, Vec4::new(0.3, 0.4, 0.4, 1.0));
    }

    #[test]
    fn plain_materials_take_their_tint_and_saturation() {
        let m = material(vec![("DiffuseColor", Param::Color([0.5, 0.6, 0.7, 0.0]))]);
        assert_eq!((params_of(&m).tint, params_of(&m).mode.y), (Vec4::new(0.5, 0.6, 0.7, 1.0), 1.0));
        let mut s = material(vec![]);
        s.params = vec![(SATURATION, Param::Scalar(0.49)), (SATURATION_COLOR, Param::Color([0.8, 0.8, 0.78, 1.0]))];
        assert_eq!((params_of(&s).tint, params_of(&s).mode.y), (Vec4::new(0.8, 0.8, 0.78, 1.0), 0.49));
    }

    #[test]
    fn water_blend_takes_its_level_and_colours() {
        let mut m = material(vec![
            ("WorldOceanLevel", Param::Scalar(9.8)),
            ("WaterFalloff", Param::Scalar(1.37)),
            ("WaterColorVertical", Param::Color([0.12, 0.14, 0.13, 0.0])),
            ("WaterColorHorizontal", Param::Color([0.04, 0.05, 0.04, 1.0])),
        ]);
        assert_eq!(params_of(&m).wet, Vec4::ZERO, "only its template's");
        m.template = WATER_BLEND;
        let p = params_of(&m);
        assert_eq!(p.wet, Vec4::new(9.8, 1.37, 1.0, 0.0));
        assert_eq!((p.water_color, p.color1), (Vec4::new(0.12, 0.14, 0.13, 1.0), Vec4::new(0.04, 0.05, 0.04, 1.0)));
    }

    #[test]
    fn vertex_extras_only_where_not_default() {
        let mut m = Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::RENDER_WORLD);
        vertex_extras(&mut m, &[[255u8; 4]; 3], &[[0u8; 2]; 3]);
        assert!(m.attribute(Mesh::ATTRIBUTE_COLOR).is_none() && m.attribute(Mesh::ATTRIBUTE_UV_1).is_none());
        vertex_extras(&mut m, &[[255, 255, 255, 0], [255; 4], [255; 4]], &[[0, 255], [0, 0], [0, 0]]);
        assert!(m.attribute(Mesh::ATTRIBUTE_COLOR).is_some() && m.attribute(Mesh::ATTRIBUTE_UV_1).is_some());
    }
}
