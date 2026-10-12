//! The level's sky dome: its `LayeredSky`'s first layer (`forge::ambiance::Sky::layers`), a mesh (`LAYERED_SKY_SPHERE`
//! in DataPC's `Game Bootstrap Settings`, or the level's own) with a `GFX_Sky` material. AC1's sky shader is the
//! material's diffuse map times its `DiffColor`, unlit and unfogged; the frame's grade then applies, so here the texture
//! is graded once on the CPU (`look::Look::graded`) and drawn unlit, centred on the camera, past everything else.
//! `AC1_NO_SKY=1` leaves it out (the clear colour shows).

use bevy::asset::RenderAssetUsages;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

/// The sky as read: the mesh (game space) and its texture, graded.
#[derive(Resource)]
pub struct SkyData {
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
    texture: Option<forge::Texture>,
}

#[derive(Component)]
pub struct SkyDome;

/// How far out the dome is drawn (m; inside the camera's far plane).
const RADIUS: f32 = 4000.0;

/// The level's sky (`level`: its name; the test level has Masyaf's), or None when it is not found.
pub fn load(game_dir: &std::path::Path, level: Option<&str>, look: &crate::look::Look) -> Option<SkyData> {
    let name = level.unwrap_or("masyaf");
    let f = forge::Forge::open(game_dir.join(crate::city::forge_of(name))).ok()?;
    let e = f.entries.iter().find(|e| e.name.eq_ignore_ascii_case(name))?;
    let data = f.read(e).ok()?;
    let df = forge::datafile::DataFile::parse(&data).ok()?;
    let pair = forge::ambiance::find_in(df.objects.iter()).ok()?;
    let &(mesh_id, material) = pair.sky.layers.first()?;
    let tex = df.material_diffuse(material).and_then(|t| forge::decode_texture(t.body).ok())?;
    let color = df.get(material).and_then(|m| forge::material::Material::parse(m.body).ok()).and_then(|m| m.color("DiffColor")).unwrap_or([1.0; 4]);
    // The mesh: in the level's file, else DataPC's `Game Bootstrap Settings`.
    let mesh = match df.get(mesh_id) {
        Some(o) => forge::mesh::parse_mesh(o.body).ok(),
        None => {
            let d = forge::Forge::open(game_dir.join("DataPC.forge")).ok()?;
            let e = d.entries.iter().find(|e| e.name == "Game Bootstrap Settings")?;
            let boot = d.read(e).ok()?;
            let objects = forge::parse_objects(&boot).ok()?;
            objects.iter().find(|o| o.id == mesh_id).and_then(|o| forge::mesh::parse_mesh(o.body).ok())
        }
    }?;
    let mut texture = tex;
    grade(&mut texture.rgba, color, look);
    for m in texture.mips.iter_mut() {
        grade(m, color, look);
    }
    info!("sky: {}x{} texture, {} vertices, DiffColor {color:?}", texture.width, texture.height, mesh.positions.len());
    Some(SkyData { positions: mesh.positions, uvs: mesh.uvs, indices: mesh.indices, texture: Some(texture) })
}

/// The texels times the material's colour, graded as the frame is (display values, as the sky shader writes them).
fn grade(rgba: &mut [u8], color: [f32; 4], look: &crate::look::Look) {
    for px in rgba.chunks_exact_mut(4) {
        let c = [0, 1, 2].map(|k| px[k] as f32 / 255.0 * color[k]);
        let g = look.graded(c).to_srgba();
        px[0] = (g.red * 255.0).round() as u8;
        px[1] = (g.green * 255.0).round() as u8;
        px[2] = (g.blue * 255.0).round() as u8;
    }
}

/// Spawn the dome (Startup, after the camera).
pub fn spawn(
    mut commands: Commands,
    sky: Option<ResMut<SkyData>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(mut sky) = sky else { return };
    let Some(mut tex) = sky.texture.take() else { return };
    let r = sky.positions.iter().map(|p| Vec3::from(*p).length()).fold(0.0f32, f32::max).max(1e-3);
    let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, sky.positions.clone());
    m.insert_attribute(Mesh::ATTRIBUTE_UV_0, sky.uvs.clone());
    m.insert_indices(Indices::U32(sky.indices.clone()));
    // (Its texels are already display values: an sRGB texture shows them as they are.)
    tex.sampling.srgb = true;
    let image = images.add(crate::assets::to_image(tex));
    let mat = mats.add(StandardMaterial { base_color_texture: Some(image), unlit: true, fog_enabled: false, cull_mode: None, ..default() });
    commands.spawn((
        SkyDome,
        Mesh3d(meshes.add(m)),
        MeshMaterial3d(mat),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)).with_scale(Vec3::splat(RADIUS / r)),
        NotShadowCaster,
        NotShadowReceiver,
        bevy::camera::visibility::NoFrustumCulling,
    ));
    commands.remove_resource::<SkyData>();
}

/// The dome stays centred on the camera.
pub fn follow(cams: Query<&GlobalTransform, (With<Camera3d>, Without<SkyDome>)>, mut domes: Query<&mut Transform, With<SkyDome>>) {
    let Ok(cam) = cams.single() else { return };
    for mut t in &mut domes {
        t.translation = cam.translation();
    }
}
