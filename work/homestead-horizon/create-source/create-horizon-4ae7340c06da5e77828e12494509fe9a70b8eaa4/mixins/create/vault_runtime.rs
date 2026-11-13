// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// ItemVaultBlock / ItemVaultBlockEntity declaration: one block entity of
// SLOTS_PER_BLOCK slots per vault block. The shared multiblock inventory is
// assembled by `create_vault_view` from the members' own slots.
use super::GameLevel;
use crate::containers::{BlockEntity, ModContainer};
use create_engine::vault::{self, Long, SLOTS_PER_BLOCK};
use horizon_core::position::BlockPos;
use horizon_world::{nbt::Compound, registry::BlockStateId};

pub(super) const ID: &str = "create:item_vault";

pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    if block == ID && container.is_none() {
        *container = Some(ModContainer {
            slots: SLOTS_PER_BLOCK,
            faces: std::array::from_fn(|_| (0..SLOTS_PER_BLOCK).collect()),
            inputs: (0..SLOTS_PER_BLOCK).collect(),
            outputs: (0..SLOTS_PER_BLOCK).collect(),
            menu: None,
        });
    }
}

pub(super) fn entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if block == ID && entity.is_none() {
        *entity = Some((block.into(), Compound::new()));
    }
}

pub(super) fn has_analog(game: &GameLevel<'_>, state: BlockStateId, output: &mut bool) {
    if game.native_state_block(state) == Some(ID) {
        *output = true;
    }
}

/// `ItemHandlerHelper.calcRedstoneFromInventory` over the shared inventory:
/// the sum of count / stack limit, divided by the slot count (vanilla's
/// `AbstractContainerMenu.getRedstoneSignalFromContainer` arithmetic).
pub(super) fn analog(game: &GameLevel<'_>, pos: BlockPos, state: BlockStateId, output: &mut i32) {
    if game.native_state_block(state) != Some(ID) {
        return;
    }
    let axis_at = |p: [i32; 3]| {
        let block = game.registry.state(game.world.block(p[0], p[1], p[2]))?;
        if block.name != ID {
            return None;
        }
        block.properties.get("axis").and_then(|axis| Long::parse(axis))
    };
    let Some(cuboid) = vault::resolve([pos.x, pos.y, pos.z], axis_at) else {
        return;
    };
    let mut total = 0.0f32;
    for member in cuboid.members() {
        if let Some(BlockEntity::Mod(entity)) =
            game.world.block_entities.get(&(member[0], member[1], member[2]))
        {
            for stack in entity.items.iter().flatten() {
                let limit = game.items.get(&stack.item).map_or(64, |item| item.max_stack).clamp(1, 64);
                total += stack.count as f32 / limit as f32;
            }
        }
    }
    total /= cuboid.slots() as f32;
    *output = (total * 14.0).floor() as i32 + i32::from(total > 0.0);
}