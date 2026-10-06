//! A decoded data file as an object graph: lookup by id and reference scanning.
//!
//! References in AC1 objects are plain u32 object ids embedded in the body, so we find them by
//! scanning for values that are ids of objects in the same file.

use crate::{Object, parse_objects, u32_at};
use anyhow::Result;
use std::collections::HashMap;

pub const CLASS_ENTITY: u32 = 0x0984415e;
pub const CLASS_MATERIAL: u32 = 0x85c817c3;
pub const CLASS_TEXTURE_SET: u32 = 0xd70e6670;
pub const CLASS_MAP_SPEC: u32 = 0x989dc6b2;

pub struct DataFile<'a> {
    pub objects: Vec<Object<'a>>,
    index: HashMap<u32, usize>,
}

impl<'a> DataFile<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        let objects = parse_objects(data)?;
        let index = objects.iter().enumerate().map(|(i, o)| (o.id, i)).collect();
        Ok(Self { objects, index })
    }

    pub fn get(&self, id: u32) -> Option<&Object<'a>> {
        self.index.get(&id).map(|&i| &self.objects[i])
    }

    pub fn by_name(&self, class: u32, name: &str) -> Option<&Object<'a>> {
        self.objects.iter().find(|o| o.class == class && o.name == name)
    }

    /// (body offset, referenced object) for every id in `o`'s body that names another object here.
    pub fn refs(&self, o: &Object<'a>) -> Vec<(usize, &Object<'a>)> {
        let b = o.body;
        (0..b.len().saturating_sub(3))
            .filter_map(|k| {
                let id = u32_at(b, k);
                (id != o.id).then(|| self.get(id)).flatten().map(|r| (k, r))
            })
            .collect()
    }

    fn first_ref(&self, o: &Object<'a>, class: u32, name_has: Option<&str>) -> Option<&Object<'a>> {
        self.refs(o).into_iter().map(|(_, r)| r).find(|r| r.class == class && name_has.is_none_or(|n| r.name.contains(n)))
    }

    /// Material -> texture set -> "Diffuse" map spec -> texture.
    pub fn material_diffuse(&self, material: u32) -> Option<&Object<'a>> {
        self.material_map(material, "Diffuse")
    }

    /// Material -> texture set -> the map spec whose name has `kind` ("Diffuse", "Normal", "Specular") -> texture.
    pub fn material_map(&self, material: u32, kind: &str) -> Option<&Object<'a>> {
        let m = self.get(material)?;
        let set = self.first_ref(m, CLASS_TEXTURE_SET, None)?;
        let spec = self.first_ref(set, CLASS_MAP_SPEC, Some(kind))?;
        self.first_ref(spec, crate::CLASS_TEXTURE, None)
    }

    /// Material overrides in an entity: adjacent (placeholder, real) material id pairs.
    pub fn material_overrides(&self, entity: &Object<'a>) -> HashMap<u32, u32> {
        let mats: Vec<(usize, u32)> = self.refs(entity).into_iter().filter(|(_, r)| r.class == CLASS_MATERIAL).map(|(k, r)| (k, r.id)).collect();
        mats.windows(2).filter(|w| w[1].0 == w[0].0 + 4).map(|w| (w[0].1, w[1].1)).collect()
    }
}
