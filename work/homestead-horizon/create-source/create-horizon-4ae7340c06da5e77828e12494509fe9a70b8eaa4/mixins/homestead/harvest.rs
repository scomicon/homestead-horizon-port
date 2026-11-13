// Copyright (c) JamCoreModding / Right Click Harvest contributors (MIT)
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// RightClickHarvest 4.6 (`RightClickHarvest.onBlockUse`, `completeHarvest`,
// `dropStacks`) with its default configuration: no hoe required, radius
// harvesting on, no hunger cost, no experience cost.
use super::Game;
use crate::items::ItemStack;
use horizon_world::{level::flags, player_inventory::EquipmentSlot, registry::BlockStateId};

/// Config.HungerLevel.NORMAL.modifier * 0.008: vanilla block breaking costs 0.005.
const EXHAUSTION: f32 = 0.008;

/// onBlockUse passes at food level 0 unless the player is in creative mode.
fn fed(game: &Game<'_>) -> bool {
    game.creative || game.player.as_ref().is_some_and(|p| p.health.food.food > 0)
}

fn tire(game: &mut Game<'_>) {
    if let Some(player) = game.player.as_mut() {
    player.cause_food_exhaustion(EXHAUSTION);
    }
}

struct Crop {
    max_age: i32,
    replant: &'static str,
}

fn crop(block: &str) -> Option<Crop> {
    let (max_age, replant) = match block {
        "minecraft:wheat" => (7, "minecraft:wheat_seeds"),
        "minecraft:carrots" => (7, "minecraft:carrot"),
        "minecraft:potatoes" => (7, "minecraft:potato"),
        "minecraft:beetroots" => (3, "minecraft:beetroot_seeds"),
        "minecraft:nether_wart" => (3, "minecraft:nether_wart"),
        "minecraft:cocoa" => (2, "minecraft:cocoa_beans"),
        "farmersdelight:cabbages" => (7, "farmersdelight:cabbage_seeds"),
        "farmersdelight:onions" => (7, "farmersdelight:onion"),
        "farmersdelight:tomatoes" => (3, "farmersdelight:tomato_seeds"),
        "farmersdelight:rice_panicles" => (3, "farmersdelight:rice_panicles"),
        _ => return None,
    };
    Some(Crop { max_age, replant })
}

/// `rightclickharvest:radius_harvest_blacklist`.
fn radius_blacklisted(block: &str) -> bool {
    matches!(block, "minecraft:cocoa" | "minecraft:sugar_cane" | "minecraft:cactus")
}

// `rightclickharvest:{high,mid,low}_tier_hoes`, limited to the mods in the pack.
const HIGH: &[&str] = &["minecraft:netherite_hoe", "deeperdarker:warden_hoe"];
const MID: &[&str] = &[
    "minecraft:golden_hoe",
    "minecraft:diamond_hoe",
    "deeperdarker:resonarium_hoe",
    "mythicupgrades:ametrine_hoe",
    "mythicupgrades:aquamarine_hoe",
    "mythicupgrades:jade_hoe",
    "mythicupgrades:peridot_hoe",
    "mythicupgrades:ruby_hoe",
    "mythicupgrades:sapphire_hoe",
    "mythicupgrades:topaz_hoe",
];
const LOW: &[&str] = &["minecraft:copper_hoe", "minecraft:iron_hoe"];

#[derive(Clone, Copy, PartialEq)]
enum Reach {
    None,
    /// Four orthogonal neighbours.
    Cross,
    /// The eight surrounding blocks.
    Square,
    /// Manhattan distance two: twelve neighbours.
    Diamond,
}

fn reach(item: &str) -> Reach {
    if HIGH.contains(&item) {
        Reach::Diamond
    } else if MID.contains(&item) {
        Reach::Square
    } else if LOW.contains(&item) {
        Reach::Cross
    } else {
        Reach::None
    }
}

fn is_hoe(item: &str) -> bool {
    item.ends_with("_hoe") || reach(item) != Reach::None
}

fn offsets(reach: Reach) -> Vec<[i32; 2]> {
    let radius: i32 = match reach {
        Reach::None => 0,
        Reach::Cross | Reach::Square => 1,
        Reach::Diamond => 2,
    };
    let mut out = Vec::new();
    for dx in -radius..=radius {
        for dz in -radius..=radius {
            let keep = match reach {
                Reach::Square => true,
                _ => dx.abs() + dz.abs() <= radius,
            };
            if keep && (dx, dz) != (0, 0) {
                out.push([dx, dz]);
            }
        }
    }
    out
}

fn mature(game: &Game<'_>, pos: [i32; 3]) -> Option<Crop> {
    let crop = crop(&game.native_block(pos))?;
    let age: i32 = game.native_block_property(pos, "age")?.parse().ok()?;
    (age >= crop.max_age).then_some(crop)
}

fn state_with_age_zero(game: &Game<'_>, pos: [i32; 3]) -> Option<BlockStateId> {
    let registry = game.pack.assets.registry;
    let mut descriptor = registry
        .state(game.world.block(pos[0], pos[1], pos[2]))?
        .clone();
    descriptor.properties.get("age")?;
    descriptor.properties.insert("age".to_owned(), "0".to_owned());
    registry.id(&descriptor)
}

/// `Block.popResource` for each stack.
fn pop(game: &mut Game<'_>, pos: [i32; 3], drops: Vec<ItemStack>) {
    for stack in drops.into_iter().filter(|stack| stack.count > 0) {
        game.pop_resource(pos, stack);
    }
}

/// `Block.getDrops(state, level, pos, null, player, tool)`: the block's loot
/// table rolled with the held tool.
fn loot(game: &mut Game<'_>, pos: [i32; 3]) -> Vec<ItemStack> {
    let state = game.world.block(pos[0], pos[1], pos[2]);
    let tool = game.held().cloned();
    game.block_loot_at(pos, state, tool.as_ref(), None)
}
/// `completeHarvest` for a mature crop: reset the age, then drop the loot
/// minus one replant item. False when this side cannot change the world.
fn harvest(game: &mut Game<'_>, pos: [i32; 3], crop: &Crop) -> bool {
    let Some(fresh) = state_with_age_zero(game, pos) else {
        return false;
    };
    let mut drops = loot(game, pos);
    if !game.native_set_block(game.dim.id(), pos, fresh, flags::UPDATE_ALL) {
        return false;
    }
    if let Some(index) = drops
        .iter()
        .position(|stack| stack.item.as_ref() == crop.replant && stack.count > 0)
    {
        drops[index].count -= 1;
    }
    pop(game, pos, drops);
    tire(game);
    true
}

/// Sugar cane and cactus: break the segment above the bottom of the clicked
/// column, leaving the base.
fn column(game: &mut Game<'_>, pos: [i32; 3], face: [i32; 3], block: &str) -> bool {
    let held = game.held().map(|stack| stack.item.to_string());
    if face == [0, 1, 0] && held.as_deref() == Some(block) {
        return false;
    }
    let mut bottom = pos;
    while game.native_block([bottom[0], bottom[1] - 1, bottom[2]]) == block {
        bottom[1] -= 1;
    }
    let target = [bottom[0], bottom[1] + 1, bottom[2]];
    if game.native_block(target) != block {
        return false;
    }
    let drops = loot(game, target);
    let Some(air) = game.pack.assets.registry.default_state("minecraft:air") else {
        return false;
    };
    if !game.native_set_block(game.dim.id(), target, air, flags::UPDATE_ALL) {
        return false;
    }
    pop(game, target, drops);
    tire(game);
    true
}

pub(super) fn right_click(game: &mut Game<'_>, pos: [i32; 3], face: [i32; 3]) -> bool {
    if game.native_sneaking() || !fed(game) {
        return false;
    }
    let block = game.native_block(pos);
    if matches!(block.as_str(), "minecraft:sugar_cane" | "minecraft:cactus") {
        return column(game, pos, face, &block);
    }
    let Some(found) = mature(game, pos) else {
        return false;
    };
    let hoe = game
        .held()
        .map(|stack| stack.item.to_string())
        .filter(|item| is_hoe(item));
    if !harvest(game, pos, &found) {
        return false;
    }
    if let Some(item) = hoe {
        if !radius_blacklisted(&block) {
            for [dx, dz] in offsets(reach(&item)) {
                let at = [pos[0] + dx, pos[1], pos[2] + dz];
                if radius_blacklisted(&game.native_block(at)) {
                    continue;
                }
                if let Some(neighbour) = mature(game, at) {
                    harvest(game, at, &neighbour);
                }
            }
            game.damage_held_in_slot(1, EquipmentSlot::MainHand);
        }
    }
    true
}
