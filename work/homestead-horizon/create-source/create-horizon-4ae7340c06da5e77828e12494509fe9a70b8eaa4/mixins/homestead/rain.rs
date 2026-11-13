// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Crops Love Rain 1.4.0 (thepoultryman, MIT) with the Homestead pack's config:
// general speed 3, individual speeds off. Rain on a crop or sugar cane gives each
// random tick a 1 in 3 chance of one extra growth step.

use super::GameLevel;
use horizon_core::position::BlockPos;
use horizon_core::random::RandomSource;
use horizon_world::level::{LevelAccess, Ticks};
use horizon_world::registry::BlockStateId;

/// `rainGrowthSpeed`: `random.nextInt(speed) == 0` decides the extra step.
const SPEED: i32 = 3;

fn extra(level: &mut GameLevel<'_>, pos: BlockPos) -> bool {
    // `Level.isRainingAt(pos)` first, as the mod does, then the draw.
    level.is_raining_at(pos)
        && level
            .native_level_random()
            .is_some_and(|random| random.next_int_bound(SPEED).ok() == Some(0))
}

/// `getRawBrightness(pos, 0)`.
fn brightness(level: &GameLevel<'_>, pos: BlockPos) -> i32 {
    level.block_light(pos).max(level.sky_light(pos) - level.sky_darken())
}

fn max_age(block: &str) -> Option<i32> {
    match block {
        "minecraft:wheat" | "minecraft:carrots" | "minecraft:potatoes" => Some(7),
        "minecraft:beetroots" => Some(3),
        "farmersdelight:cabbages" | "farmersdelight:onions" => Some(7),
        "farmersdelight:budding_tomatoes" | "farmersdelight:tomatoes" | "farmersdelight:rice_panicles" => Some(3),
        _ => None,
    }
}

pub(super) fn grow(
    level: &mut GameLevel<'_>,
    ticks: &mut Ticks,
    pos: BlockPos,
    state: BlockStateId,
) {
    if !level.native_authoritative {
        return;
    }
    let Some(info) = level.registry.state(state) else {
        return;
    };
    let block = info.name.clone();
    let Some(age) = info.properties.get("age").and_then(|v| v.parse::<i32>().ok()) else {
        return;
    };
    if let Some(max) = max_age(&block) {
        // CropBlock: lit enough, not yet ripe (the mod tests `< 7`; a beetroot stops at its 3).
        if brightness(level, pos) >= 9 && age < max && extra(level, pos) {
            level.native_set_block_property(
                ticks,
                [pos.x, pos.y, pos.z],
                "age",
                &(age + 1).to_string(),
                horizon_world::level::flags::UPDATE_CLIENTS,
            );
        }
    } else if block == "minecraft:sugar_cane" {
        let above = pos.offset(0, 1, 0);
        if !level.facts.is_air(level.block_state(above)) {
            return;
        }
        let mut height = 1;
        while level.registry.state(level.block_state(pos.offset(0, -height, 0)))
            .is_some_and(|s| &*s.name == "minecraft:sugar_cane")
        {
            height += 1;
        }
        if height < 3 && extra(level, pos) {
            if age == 15 {
                if let Some(cane) = level.registry.default_state("minecraft:sugar_cane") {
                    let b = level.behaviours;
                    b.set_block(level, ticks, above, cane, horizon_world::level::flags::UPDATE_ALL);
                }
                level.native_set_block_property(ticks, [pos.x, pos.y, pos.z], "age", "0", 4);
            } else {
                level.native_set_block_property(
                    ticks,
                    [pos.x, pos.y, pos.z],
                    "age",
                    &(age + 1).to_string(),
                    4,
                );
            }
        }
    }
}