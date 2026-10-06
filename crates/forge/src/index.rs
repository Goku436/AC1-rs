//! Where every object of a forge is: object id -> (data file, class, name). Entities link objects in other
//! data files by id (an NPC's base entity names the universal legs, its trousers, its skeletons, all in
//! their own data files of `DataPC_Common.forge`), and a forge's index gives no object ids, so every data
//! file is read once; the result is cached (in the system temp directory, keyed by the forge's size and
//! modification time).

use crate::Forge;
use anyhow::Result;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Place {
    /// Data file (entry) name.
    pub datafile: String,
    pub class: u32,
    pub name: String,
}

pub struct ForgeIndex {
    pub objects: HashMap<u32, Place>,
}

fn cache_path(path: &Path) -> Option<PathBuf> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    let name = path.file_stem()?.to_string_lossy().into_owned();
    Some(std::env::temp_dir().join("ac1-rs").join(format!("{name}-{}-{modified}.idx", meta.len())))
}

impl ForgeIndex {
    /// The index of the forge at `path`, from the cache when it is current.
    pub fn load(path: &Path) -> Result<Self> {
        if let Some(c) = cache_path(path)
            && let Ok(idx) = Self::read_cache(&c)
        {
            return Ok(idx);
        }
        let idx = Self::build(path)?;
        if let Some(c) = cache_path(path) {
            let _ = idx.write_cache(&c);
        }
        Ok(idx)
    }

    /// Read every data file of the forge (data files that do not decode are skipped).
    pub fn build(path: &Path) -> Result<Self> {
        let f = Forge::open(path)?;
        let mut objects = HashMap::new();
        for e in &f.entries {
            let Ok(data) = f.read(e) else { continue };
            let Ok(objs) = crate::parse_objects(&data) else { continue };
            for o in objs {
                objects.entry(o.id).or_insert(Place { datafile: e.name.clone(), class: o.class, name: o.name });
            }
        }
        Ok(ForgeIndex { objects })
    }

    fn write_cache(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut out = Vec::new();
        out.extend((self.objects.len() as u32).to_le_bytes());
        for (id, p) in &self.objects {
            out.extend(id.to_le_bytes());
            out.extend(p.class.to_le_bytes());
            for s in [&p.datafile, &p.name] {
                out.extend((s.len() as u16).to_le_bytes());
                out.extend(s.as_bytes());
            }
        }
        std::fs::File::create(path)?.write_all(&out)?;
        Ok(())
    }

    fn read_cache(path: &Path) -> Result<Self> {
        let mut d = Vec::new();
        std::fs::File::open(path)?.read_to_end(&mut d)?;
        let mut p = 0usize;
        let mut take = |n: usize| -> Result<&[u8]> {
            anyhow::ensure!(p + n <= d.len(), "index cache ends early");
            p += n;
            Ok(&d[p - n..p])
        };
        let n = u32::from_le_bytes(take(4)?.try_into()?) as usize;
        let mut objects = HashMap::with_capacity(n);
        for _ in 0..n {
            let id = u32::from_le_bytes(take(4)?.try_into()?);
            let class = u32::from_le_bytes(take(4)?.try_into()?);
            let mut strs = [String::new(), String::new()];
            for s in strs.iter_mut() {
                let l = u16::from_le_bytes(take(2)?.try_into()?) as usize;
                *s = String::from_utf8_lossy(take(l)?).into_owned();
            }
            let [datafile, name] = strs;
            objects.insert(id, Place { datafile, class, name });
        }
        Ok(ForgeIndex { objects })
    }
}
