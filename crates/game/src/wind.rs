//! The city's wind: AC1's `Wind` (read by `forge::cloth::wind`, blown by `ik::wind`), one per city, placed in a cell
//! data block. It moves the robe (`robe`) and the bones the skeleton's hinges and springs swing (`character`).
//! Every city has its own (`Wind_<City>_001`); the test level borrows Masyaf's (as Banned445's AC1-Movement-Rewritten
//! does for its test map). The data block is large and slow
//! to read, so the wind is kept in the temp directory after the first time (`ac1-rs/<forge>-<size>-<time>-<name>.wind`).
//! `AC1_WIND=0` turns it off.

use bevy::prelude::*;
use std::path::{Path, PathBuf};

/// Where each city's wind is: (forge, data block, entity).
const WINDS: [(&str, &str, &str, &str); 5] = [
    ("damascus", "DataPC_Damascus.forge", "Cell01364_DataBlock", "Wind_Damascus_001"),
    ("masyaf", "DataPC_Masyaf.forge", "Cell05460_DataBlock", "Wind_Masyaf_001"),
    ("acre", "DataPC_Acre.forge", "Cell01364_DataBlock", "Wind_Acre_001"),
    ("jerusalem", "DataPC_Jerusalem.forge", "Cell01364_DataBlock", "Wind_Jerusalem_001"),
    ("kingdom", "DataPC_Kingdom.forge", "Cell05460_DataBlock", "Wind_Kingdom_001"),
];

#[derive(Resource, Default)]
pub struct WindField {
    noise: ik::wind::Noise,
    winds: Vec<ik::wind::Wind>,
    /// Seconds since it was made (AC1's wind runs on game time from the map's load).
    pub time: f32,
}

impl WindField {
    /// The force at Bevy position `at`, in Bevy space (m/s²).
    pub fn at(&self, at: Vec3) -> Vec3 {
        let p = Vec3::new(at.x, -at.z, at.y);
        let f: Vec3 = self.winds.iter().map(|w| w.force(&self.noise, p, self.time)).sum();
        Vec3::new(f.x, f.z, -f.y)
    }
}

fn cache_path(forge: &Path, name: &str) -> Option<PathBuf> {
    let meta = std::fs::metadata(forge).ok()?;
    let modified = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    let file = forge.file_name()?.to_string_lossy();
    Some(std::env::temp_dir().join("ac1-rs").join(format!("{file}-{}-{modified}-{name}.wind", meta.len())))
}

/// A wind as cached: strength, noise (0/1), four percentages, direction.
fn parse_cached(text: &str) -> Option<ik::wind::Wind> {
    let v: Vec<f32> = text.split_whitespace().map(|s| s.parse().ok()).collect::<Option<_>>()?;
    (v.len() == 9).then(|| ik::wind::Wind { strength: v[0], noise: v[1] != 0.0, percent: [v[2], v[3], v[4], v[5]], direction: Vec3::new(v[6], v[7], v[8]) })
}

fn read_wind(game: &Path, forge_file: &str, block: &str, name: &str) -> anyhow::Result<ik::wind::Wind> {
    use anyhow::Context;
    let path = game.join(forge_file);
    let cache = cache_path(&path, name);
    if let Some(w) = cache.as_ref().and_then(|c| std::fs::read_to_string(c).ok()).and_then(|t| parse_cached(&t)) {
        return Ok(w);
    }
    let f = forge::Forge::open(&path)?;
    let e = f.entries.iter().find(|e| e.name == block).with_context(|| format!("no {block}"))?;
    let data = f.read(e)?;
    let objs = forge::parse_objects(&data)?;
    let o = objs.iter().find(|o| o.name == name).with_context(|| format!("no {name}"))?;
    let w = forge::cloth::wind(o.body)?.with_context(|| format!("{name} has no wind"))?;
    if let Some(c) = cache {
        let d = w.direction;
        let text = format!("{} {} {} {} {} {} {} {} {}", w.strength, w.noise as u8, w.percent[0], w.percent[1], w.percent[2], w.percent[3], d[0], d[1], d[2]);
        let _ = std::fs::create_dir_all(c.parent().unwrap_or(Path::new("."))).and_then(|_| std::fs::write(&c, text));
    }
    Ok(ik::wind::Wind { strength: w.strength, noise: w.noise, percent: w.percent, direction: Vec3::from_array(w.direction) })
}

/// The wind for `level` (`AC1_LEVEL`, lower case; none: the test level).
pub fn load(game: &Path, level: Option<&str>) -> WindField {
    if std::env::var("AC1_WIND").is_ok_and(|v| v == "0") {
        return WindField::default();
    }
    let key = level.unwrap_or("masyaf");
    let Some(&(_, forge_file, block, name)) = WINDS.iter().find(|w| w.0 == key) else {
        info!("wind: none known for {key}");
        return WindField::default();
    };
    match read_wind(game, forge_file, block, name) {
        Ok(w) => {
            info!("wind: {name}, {} m/s² toward {:.2}", w.strength, w.direction);
            WindField { winds: vec![w], ..default() }
        }
        Err(e) => {
            warn!("wind: {name} not read ({e:#})");
            WindField::default()
        }
    }
}

/// The wind's clock (game time).
pub fn advance(time: Res<Time>, wind: Option<ResMut<WindField>>) {
    if let Some(mut w) = wind {
        w.time += time.delta_secs();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cached_wind_reads_back() {
        let w = parse_cached("10 1 61.37931 100 33.10345 11.724138 0.852 -0.523 0").unwrap();
        assert_eq!((w.strength, w.noise, w.percent[3]), (10.0, true, 11.724138));
        assert!(parse_cached("10 1 2").is_none());
    }

    #[test]
    fn bevy_space_maps_to_the_games() {
        let f =
            WindField { winds: vec![ik::wind::Wind { strength: 10.0, noise: false, percent: [0.0; 4], direction: Vec3::new(0.6, -0.8, 0.0) }], ..default() };
        assert!(f.at(Vec3::ZERO).distance(Vec3::new(6.0, 0.0, 8.0)) < 1e-5, "game (x, y, z) is Bevy (x, z, -y)");
    }
}
