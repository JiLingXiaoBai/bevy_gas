//! Deterministic tag and attribute registration and tag reference resolution.

use super::{ConfigError, Tables, data};
use crate::{
    AttributeId, AttributeIdManager, AttributeRegion, GameplayTag, GameplayTagBits,
    GameplayTagManager, UniqueNamePool, add_bit_with_tag,
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn register_tags(
    tables: &Tables,
    names: &mut UniqueNamePool,
    manager: &mut GameplayTagManager,
) -> Result<BTreeMap<String, GameplayTag>, ConfigError> {
    let mut ordered = BTreeSet::new();
    for row in tables.tb_tag.iter() {
        let mut name = row.name.as_str();
        loop {
            ordered.insert(name);
            let Some((parent, _)) = name.rsplit_once('.') else {
                break;
            };
            name = parent;
        }
    }
    let mut tags: BTreeMap<String, GameplayTag> = BTreeMap::new();
    for name in ordered {
        let context = format!("Tag[{name}]");
        let unique = names
            .new_name(name)
            .map_err(|error| ConfigError::new(&context, error.to_string()))?;
        let parent = if let Some((parent, _)) = name.rsplit_once('.') {
            Some(
                *tags
                    .get(parent)
                    .ok_or_else(|| ConfigError::new(&context, "parent tag was not registered"))?,
            )
        } else {
            None
        };
        let mut expected_bits = match parent {
            Some(parent) => *manager
                .get_inherited_bits(&parent)
                .map_err(|error| ConfigError::new(&context, error.to_string()))?,
            None => GameplayTagBits::default(),
        };
        let tag = manager
            .register_tag_internal(unique, parent.map(|tag| tag.get_bit_index_u16()))
            .map_err(|error| ConfigError::new(&context, error.to_string()))?;
        add_bit_with_tag(&mut expected_bits, &tag)
            .map_err(|error| ConfigError::new(&context, error.to_string()))?;
        let actual_bits = manager
            .get_inherited_bits(&tag)
            .map_err(|error| ConfigError::new(&context, error.to_string()))?;
        if *actual_bits != expected_bits {
            return Err(ConfigError::new(
                &context,
                "existing tag inheritance conflicts with the configured hierarchy",
            ));
        }
        tags.insert(name.to_owned(), tag);
    }
    Ok(tags)
}

pub(super) fn register_attributes(
    tables: &Tables,
    names: &mut UniqueNamePool,
    manager: &mut AttributeIdManager,
) -> Result<BTreeMap<String, AttributeId>, ConfigError> {
    let mut rows: Vec<_> = tables.tb_attribute.iter().collect();
    rows.sort_by(|left, right| left.name.cmp(&right.name));
    let mut attributes = BTreeMap::new();
    for row in rows {
        let context = format!("Attribute[{}]", row.name);
        let unique = names
            .new_name(&row.name)
            .map_err(|error| ConfigError::new(&context, error.to_string()))?;
        let region = match row.region {
            data::AttributeRegion::Hot => AttributeRegion::Hot,
            data::AttributeRegion::Cold => AttributeRegion::Cold,
        };
        let id = manager
            .register_id_internal(unique, region)
            .map_err(|error| ConfigError::new(&context, error.to_string()))?;
        attributes.insert(row.name.clone(), id);
    }
    Ok(attributes)
}

pub(super) fn resolve_tags(
    names: &[String],
    tags: &BTreeMap<String, GameplayTag>,
) -> Result<Vec<GameplayTag>, ConfigError> {
    names
        .iter()
        .map(|name| {
            tags.get(name).copied().ok_or_else(|| {
                ConfigError::new(format!("Tag[{name}]"), "unresolved registered tag")
            })
        })
        .collect()
}
