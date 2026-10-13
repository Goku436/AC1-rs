//! Loads started on threads as the game launches (`start`), while Bevy brings up the window and the GPU, and taken
//! where they are used: Altaïr's model, the clip library, a city's cached gameplay data and drawn world (`level_cache`,
//! `render_cache`) and the scholars of its crowd. Each is the same call the user would make then, with the same
//! arguments: what is taken is what would have been loaded. Whatever was not started (or failed to run) is loaded where
//! it is used, as before.

use crate::animation::AnimLib;
use crate::assets::CharacterData;
use crate::level::Level;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread::JoinHandle;

/// Altaïr's entity: forge, data file, entity name.
pub const ALTAIR: (&str, &str, &str) = ("DataPC.forge", "Rank 9", "UCMA_Altair_Rank_9");
/// The clip library: forge, data file.
pub const LIBRARY: (&str, &str) = ("DataPC.forge", "Game Fix");
/// The crowd's scholars: how many (each its own, `scholar_seed`).
pub const SCHOLARS: usize = 4;

/// A load running on a thread of its own, taken once.
struct Pending<T>(Mutex<Option<JoinHandle<T>>>);

impl<T: Send + 'static> Pending<T> {
    const fn new() -> Self {
        Self(Mutex::new(None))
    }

    fn start(&self, f: impl FnOnce() -> T + Send + 'static) {
        if let Ok(mut j) = self.0.lock() {
            *j = Some(std::thread::spawn(f));
        }
    }

    /// Its result, waited for (None: never started, or the thread failed).
    fn take(&self) -> Option<T> {
        self.0.lock().ok()?.take()?.join().ok()
    }
}

/// A city's cached gameplay data: the file and key it was read under, and the level (or why it was not used).
type CachedLevel = (PathBuf, u64, Result<Level>);

static ALTAIR_JOB: Pending<Result<CharacterData>> = Pending::new();
static LIBRARY_JOB: Pending<Result<AnimLib>> = Pending::new();
static LEVEL_JOB: Pending<Option<CachedLevel>> = Pending::new();
static DRAW_JOB: Pending<Option<crate::render_cache::CityDraw>> = Pending::new();
static SCHOLARS_JOB: Pending<Vec<Result<CharacterData>>> = Pending::new();

/// Start the loads a run will want: `city` (`AC1_LEVEL`), `drawn` (not headless).
pub fn start(game_dir: &Path, city: Option<&str>, drawn: bool) {
    let dir = game_dir.to_path_buf();
    ALTAIR_JOB.start(move || crate::assets::load_character(&dir, ALTAIR.0, ALTAIR.1, ALTAIR.2));
    if std::env::var("AC1_NO_ANIM").is_err() {
        let dir = game_dir.to_path_buf();
        LIBRARY_JOB.start(move || AnimLib::load_unbound(&dir, LIBRARY.0, LIBRARY.1));
    }
    let crowd = std::env::var("AC1_NO_CROWD").is_err();
    let Some(city) = city else {
        // (The test level always has its crowd.)
        if crowd {
            start_scholars(game_dir);
        }
        return;
    };
    use crate::level_cache as cache;
    if cache::enabled() && !cache::check() {
        let (dir, name) = (game_dir.to_path_buf(), city.to_string());
        LEVEL_JOB.start(move || {
            let (path, key) = cache::locate(&dir, &name)?;
            let level = cache::read_payload(&path, key).and_then(|p| cache::read_level(&p));
            // (A city's crowd walks only where its start has room for its loop.)
            if crowd && level.as_ref().is_ok_and(|l| l.crowd_path.is_some()) {
                start_scholars(&dir);
            }
            Some((path, key, level))
        });
        if drawn {
            let (dir, name) = (game_dir.to_path_buf(), city.to_string());
            DRAW_JOB.start(move || crate::render_cache::load(&dir, &name));
        }
    }
}

fn start_scholars(game_dir: &Path) {
    let dir = game_dir.to_path_buf();
    SCHOLARS_JOB.start(move || (0..SCHOLARS).map(|k| scholar(&dir, k)).collect());
}

/// Scholar `k` of the crowd: AC1's hooded scholar (the `CNMA_Scholar` builder), each its own; the hand-made one if
/// the builder fails.
pub fn scholar(game_dir: &Path, k: usize) -> Result<CharacterData> {
    let built = crate::npc::build(game_dir, crate::SCHOLAR_FORGE, crate::SCHOLAR_BLOCK, "CNMA_Scholar", 0x5c + k as u64 * 104_729)
        .map_err(|e| bevy::log::warn!("scholar builder: {e:#}"))
        .ok();
    built.map(|b| b.0).map_or_else(
        || {
            let (skeletons, meshes_list) = crate::crowd::scholar_recipe();
            crate::assets::load_assembled_from(game_dir, &skeletons, &meshes_list)
        },
        Ok,
    )
}

/// Altaïr's model.
pub fn altair(game_dir: &Path) -> Result<CharacterData> {
    ALTAIR_JOB.take().unwrap_or_else(|| crate::assets::load_character(game_dir, ALTAIR.0, ALTAIR.1, ALTAIR.2))
}

/// The clip library, unbound (`AnimLib::bind_player`).
pub fn library(game_dir: &Path) -> Result<AnimLib> {
    LIBRARY_JOB.take().unwrap_or_else(|| AnimLib::load_unbound(game_dir, LIBRARY.0, LIBRARY.1))
}

/// The crowd's scholars, if they were loaded ahead (else `scholar` each).
pub fn scholars() -> Option<Vec<Result<CharacterData>>> {
    SCHOLARS_JOB.take()
}

/// The city's gameplay data from its cache file at `path` under `key`.
pub fn cached_level(path: &Path, key: u64) -> Result<Level> {
    use crate::level_cache as cache;
    match LEVEL_JOB.take().flatten() {
        Some((p, k, level)) if p == path && k == key => level,
        _ => cache::read_payload(path, key).and_then(|p| cache::read_level(&p)),
    }
}

/// The city's drawn world from its cache file, if it holds it (`render_cache::load`).
pub fn draw(game_dir: &Path, city: &str) -> Option<crate::render_cache::CityDraw> {
    match DRAW_JOB.take() {
        Some(d) => d,
        None => crate::render_cache::load(game_dir, city),
    }
}
