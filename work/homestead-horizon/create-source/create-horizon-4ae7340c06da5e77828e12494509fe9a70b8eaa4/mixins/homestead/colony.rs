// Copyright (c) vectorwing / Farmer's Delight contributors (MIT)
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Farmer's Delight mushroom colonies (`MushroomColonyBlock`): the growth of the colony `age` 0-3.
//
// The colony blocks are declared with a vanilla mushroom as template and its random tick (which
// spreads plain mushrooms) switched off (fd/stage23.py), so nothing grows them by itself. In the
// original the colony grows on its own random tick when the block below is tagged
// `farmersdelight:mushroom_colony_growable_on` (only rich soil). Every block is random ticked at
// the same rate, so the same step is taken here on the random tick of the rich soil below the
// colony. Shearing a ripe colony (loot table) and breaking are the carried loot tables.
//
// The odds are an estimate: the Java was not read (docs/fd-stage23.md).
use super::GameLevel;
use horizon_core::{position::BlockPos, random::RandomSource};
use horizon_world::{
    level::{flags, LevelAccess, Ticks},
    registry::BlockStateId,
};

const RICH_SOIL: &str = "farmersdelight:rich_soil";
const COLONIES: [&str; 2] = [
    "farmersdelight:brown_mushroom_colony",
    "farmersdelight:red_mushroom_colony",
];
/// One growth step per this many random ticks of the soil.
const ODDS: i32 = 4;
const MAX_AGE: i32 = 3;

fn roll(level: &mut GameLevel<'_>, odds: i32) -> bool {
    level
        .native_level_random()
        .is_some_and(|random| random.next_int_bound(odds).ok() == Some(0))
}

fn name_at(level: &GameLevel<'_>, pos: BlockPos) -> String {
    level
        .registry
        .state(level.block_state(pos))
        .map(|info| info.name.to_string())
        .unwrap_or_default()
}

fn age_at(level: &GameLevel<'_>, pos: BlockPos) -> Option<i32> {
    level
        .registry
        .state(level.block_state(pos))?
        .properties
        .get("age")?
        .parse()
        .ok()
}

/// The `random_tick` hook: rich soil with a colony above it that is not ripe yet.
pub(super) fn random_tick(
    level: &mut GameLevel<'_>,
    ticks: &mut Ticks,
    pos: BlockPos,
    state: BlockStateId,
) {
    if !level.native_authoritative {
        return;
    }
    let is_soil = level
        .registry
        .state(state)
        .is_some_and(|info| &*info.name == RICH_SOIL);
    if !is_soil {
        return;
    }
    let above = pos.offset(0, 1, 0);
    if !COLONIES.contains(&name_at(level, above).as_str()) {
        return;
    }
    let Some(age) = age_at(level, above) else {
        return;
    };
    if age < MAX_AGE && roll(level, ODDS) {
        level.native_set_block_property(
            ticks,
            [above.x, above.y, above.z],
            "age",
            &(age + 1).to_string(),
            flags::UPDATE_CLIENTS,
        );
    }
}
