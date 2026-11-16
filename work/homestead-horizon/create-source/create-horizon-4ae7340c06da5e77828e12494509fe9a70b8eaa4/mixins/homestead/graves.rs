// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Universal Graves 3.0.3 (LGPLv3, Fabric, server side) with the Homestead pack's
// `config/universal-graves/config.json`, as far as the native hooks reach. When a player dies without
// keepInventory the whole inventory (what `native_loot_drops` offers for the dying player: main slots,
// armour, off hand, anything a mod added) is put into a grave block at the place of death instead of being
// thrown:
//
// * the grave is the block `universal_graves:grave` of the small content mod built by
//   tools/graves_content.py (a block entity with 54 item slots opened by the generic 9x6 menu),
// * the spot is the nearest replaceable block within 16 blocks of the death position (`placement`:
//   `max_distance_from_source_location` 16, `replace_any_block` is narrowed to air, plants and fluids),
// * the grave remembers its owner and the world time of its creation,
// * for 900 s (18000 ticks; `non_owner_protection_time`) only the owner and an operator (level 2 or more)
//   may open or break it; afterwards anyone can. It never destroys itself (`self_destruction_time` -1),
// * breaking it drops what is left (the engine drops a mod container's items).
//
// Not ported: experience storage (`experience_type` percent_points), the death compass, the /graves
// commands and GUI lists (remote teleport, remote break, unlock cost), the hologram and model, the
// shift-click quick pickup, the grave limit, blocked areas and worlds, restoring the replaced block, and
// the world-time vs real-time switch (world time is used). A grave that cannot be placed or filled leaves
// the drops untouched, as vanilla throws them.

use super::Game;
use crate::{containers::ModContainer, items::ItemStack};
use horizon_client::mod_api::NativeLootDrops;
use horizon_world::{
    level::flags,
    nbt::{Compound, Tag},
};

pub(super) const BLOCK: &str = "universal_graves:grave";
/// The vanilla 9x6 chest menu: 54 slots, enough for the 41 of a player.
const MENU: &str = "minecraft:generic_9x6";
const SLOTS: usize = 54;
/// `non_owner_protection_time` 900 s in ticks.
const PROTECTION_TICKS: i64 = 900 * 20;
/// `max_distance_from_source_location`.
const RADIUS: i32 = 16;
/// The permission level that may open anyone's grave (the original's `universal_graves.modify`).
const ADMIN_LEVEL: u8 = 2;
const OWNER_MOST: &str = "OwnerMost";
const OWNER_LEAST: &str = "OwnerLeast";
const CREATED: &str = "Created";

type Pos = [i32; 3];

// ------------------------------------------------------------------ block entity

pub(super) fn entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if block == BLOCK && entity.is_none() {
        *entity = Some((block.into(), Compound::new()));
    }
}

pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    if block == BLOCK && container.is_none() {
        *container = Some(ModContainer {
            slots: SLOTS,
            faces: std::array::from_fn(|_| Vec::new()),
            inputs: Vec::new(),
            outputs: Vec::new(),
            menu: Some(MENU.into()),
        });
    }
}

fn long(nbt: &Compound, key: &str) -> Option<i64> {
    match nbt.get(&key.into()) {
        Some(Tag::Long(v)) => Some(*v),
        _ => None,
    }
}

fn owner(nbt: &Compound) -> Option<u128> {
    let most = long(nbt, OWNER_MOST)?;
    let least = long(nbt, OWNER_LEAST)?;
    Some((u128::from(most as u64) << 64) | u128::from(least as u64))
}

// ------------------------------------------------------------------------ death

/// What a grave may replace: air, plants and fluids, and nothing that has a block entity.
fn replaceable(game: &Game<'_>, pos: Pos) -> bool {
    if game.world.block_entities.contains_key(&(pos[0], pos[1], pos[2])) {
        return false;
    }
    matches!(
        game.native_block(pos).as_str(),
        "minecraft:air"
            | "minecraft:cave_air"
            | "minecraft:void_air"
            | "minecraft:short_grass"
            | "minecraft:tall_grass"
            | "minecraft:fern"
            | "minecraft:water"
            | "minecraft:lava"
            | "minecraft:snow"
    )
}

/// The nearest replaceable block to `origin` within `RADIUS`, inside the world's height.
fn find_spot(game: &Game<'_>, origin: [f64; 3]) -> Option<Pos> {
    let (min_y, height) = game.native_world_height();
    let base = [origin[0].floor() as i32, origin[1].floor() as i32, origin[2].floor() as i32];
    let mut best: Option<(i32, Pos)> = None;
    for dy in -RADIUS..=RADIUS {
        for dx in -RADIUS..=RADIUS {
            for dz in -RADIUS..=RADIUS {
                let pos = [base[0] + dx, base[1] + dy, base[2] + dz];
                if pos[1] < min_y || pos[1] >= min_y + height {
                    continue;
                }
                let distance = dx * dx + dy * dy + dz * dz;
                if best.is_some_and(|(nearest, _)| nearest <= distance) {
                    continue;
                }
                if replaceable(game, pos) {
                    best = Some((distance, pos));
                }
            }
        }
    }
    best.map(|(_, pos)| pos)
}

/// `native_loot_drops`: the dying player's inventory goes into a grave. `drops` keeps what did not fit
/// (never in vanilla: a player has at most 41 stacks) or everything when no grave could be made.
pub(super) fn death_drops(game: &mut Game<'_>, context: &NativeLootDrops<'_>, drops: &mut Vec<ItemStack>) {
    let Some(player) = context.player else {
        return;
    };
    if context.used
        || !context.table.is_empty()
        || !matches!(context.entity, Some((_, "minecraft:player")))
        || drops.is_empty()
    {
        return;
    }
    let dimension = context.dimension.to_owned();
    if dimension != game.dim.id() {
        return;
    }
    let Some(pos) = find_spot(game, context.origin) else {
        return;
    };
    let Some(grave) = game.pack.assets.registry.default_state(BLOCK) else {
        return;
    };
    if !game.native_set_block(&dimension, pos, grave, flags::UPDATE_ALL) {
        return;
    }
    let mut slots: Vec<Option<ItemStack>> = vec![None; SLOTS];
    let mut overflow = Vec::new();
    for (index, stack) in drops.iter().cloned().enumerate() {
        if index < SLOTS {
            slots[index] = Some(stack);
        } else {
            overflow.push(stack);
        }
    }
    let created = game.world.game_time as i64;
    let filled = game
        .native_with_mod_block_entity(pos, |entity| {
            if entity.id != BLOCK {
                return false;
            }
            entity.items = slots;
            entity.nbt.insert(OWNER_MOST.into(), Tag::Long((player >> 64) as u64 as i64));
            entity.nbt.insert(OWNER_LEAST.into(), Tag::Long(player as u64 as i64));
            entity.nbt.insert(CREATED.into(), Tag::Long(created));
            true
        })
        .unwrap_or(false);
    if !filled {
        // No block entity to hold the items: take the grave back and let the drops fall as in vanilla.
        if let Some(air) = game.pack.assets.registry.default_state("minecraft:air") {
            game.native_set_block(&dimension, pos, air, flags::UPDATE_ALL);
        }
        return;
    }
    *drops = overflow;
}

// ------------------------------------------------------------------- protection

/// Whether the acting player may not use the grave at `pos` yet: it is protected (younger than 900 s), has
/// an owner, and they are neither the owner nor an operator.
fn guarded(game: &mut Game<'_>, pos: Pos) -> bool {
    if game.native_block(pos) != BLOCK {
        return false;
    }
    let Some(player) = game.native_acting_player() else {
        return false;
    };
    if player.permission >= ADMIN_LEVEL {
        return false;
    }
    let now = game.world.game_time as i64;
    game.native_with_mod_block_entity(pos, |entity| {
        let Some(owner) = owner(&entity.nbt) else {
            return false;
        };
        let created = long(&entity.nbt, CREATED).unwrap_or(now);
        owner != player.uuid && now - created < PROTECTION_TICKS
    })
    .unwrap_or(false)
}

/// `right_click_block`: a protected grave refuses everyone but its owner and operators. Anything else falls
/// through to the block's own use, which opens its menu.
pub(super) fn right_click(game: &mut Game<'_>, pos: Pos, canceled: &mut bool) {
    if !*canceled && guarded(game, pos) {
        game.swing_arm();
        *canceled = true;
    }
}

/// `destroy_block`: a protected grave cannot be broken by others either.
pub(super) fn destroyed(game: &mut Game<'_>, pos: Pos, canceled: &mut bool) {
    if !*canceled && guarded(game, pos) {
        *canceled = true;
    }
}
