// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Shared Item Vault inventory (ItemVaultBlockEntity capability). Like a double
// chest, the container view lists every member block's slots in controller
// order, so hoppers, Chutes and comparators see one inventory.
use super::{Containers, Kind, Pos, View};
use crate::containers::{BlockEntity, ModContainer};
use create_engine::vault::{self, Long, SLOTS_PER_BLOCK};
use std::{cell::RefCell, collections::HashMap, sync::Arc};

const ID: &str = "create:item_vault";

thread_local! {
    // Face tables for each multiblock size; hoppers ask every tick.
    static SHARED: RefCell<HashMap<usize, Arc<ModContainer>>> = RefCell::new(HashMap::new());
}

fn shared(slots: usize) -> Arc<ModContainer> {
    SHARED.with(|cache| {
        Arc::clone(cache.borrow_mut().entry(slots).or_insert_with(|| {
            Arc::new(ModContainer {
                slots,
                faces: std::array::from_fn(|_| (0..slots).collect()),
                inputs: (0..slots).collect(),
                outputs: (0..slots).collect(),
                menu: None,
            })
        }))
    })
}

pub(super) fn view(containers: &Containers<'_>, pos: Pos) -> Option<View> {
    let axis_at = |p: [i32; 3]| {
        let state = containers.world.block(p[0], p[1], p[2]);
        if containers.name(state) != ID {
            return None;
        }
        containers.prop(state, "axis").and_then(Long::parse)
    };
    let cuboid = vault::resolve([pos.0, pos.1, pos.2], axis_at)?;
    let members = cuboid.members();
    let ready = members.iter().all(|m| {
        matches!(
            containers.world.block_entities.get(&(m[0], m[1], m[2])),
            Some(BlockEntity::Mod(entity)) if entity.id == ID
        )
    });
    if !ready {
        return None;
    }
    Some(View {
        parts: members.iter().map(|m| ((m[0], m[1], m[2]), SLOTS_PER_BLOCK)).collect(),
        kind: Kind::Mod,
        container: Some(shared(members.len() * SLOTS_PER_BLOCK)),
    })
}
