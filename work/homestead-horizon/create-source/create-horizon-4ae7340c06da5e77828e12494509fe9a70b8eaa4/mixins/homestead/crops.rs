// Copyright (c) vectorwing / Farmer's Delight contributors (MIT)
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Farmer's Delight crops, soil and compost as native behaviour on top of the blocks the
// content mod declares (see fd/crops.py). Cabbages, onions, budding tomatoes, tomato vines and
// rice panicles grow as the vanilla crop templates they copy; this module adds what the
// templates cannot express:
//
// * rice (a water plant): growth by age 0-3 and the panicles put on top once it is ripe,
//   `RiceBlock.randomTick` as far as it could be read,
// * budding tomatoes: a ripe bud becomes a tomato vine in place (the original grows the vine
//   upward and lets it climb a rope; there is no rope here),
// * rich soil and rich soil farmland: a random tick gives the crop above an extra growth step
//   (`RichSoilBlock.randomTick` / `RichSoilFarmlandBlock.randomTick` tick the plant once more);
//   mushroom spreading is not ported,
// * organic compost: the composting stage 0-7 rises on random ticks, faster beside the
//   `farmersdelight:compost_activators`, and a ripe block turns into rich soil,
// * a hoe on rich soil makes rich soil farmland (the original's `HoeItem` tillable).
//
// The rates are estimates: the Java was not read here (see docs/farmersdelight-crops.md).
use super::{Game, GameLevel};
use horizon_core::{position::BlockPos, random::RandomSource};
use horizon_world::{
    level::{flags, LevelAccess, Ticks},
    player_inventory::EquipmentSlot,
    registry::BlockStateId,
};

const RICE: &str = "farmersdelight:rice";
const PANICLES: &str = "farmersdelight:rice_panicles";
const BUD: &str = "farmersdelight:budding_tomatoes";
const VINE: &str = "farmersdelight:tomatoes";
const RICH_SOIL: &str = "farmersdelight:rich_soil";
const FARMLAND: &str = "farmersdelight:rich_soil_farmland";
const COMPOST: &str = "farmersdelight:organic_compost";

/// `RichSoilBlock` boosts a crop with about this chance per random tick (one in this many).
const BOOST_ODDS: i32 = 3;
/// A rice plant grows one stage with one chance in this many per random tick.
const RICE_ODDS: i32 = 5;
/// Compost: one chance in `FAST_ODDS` per random tick with an activator in reach, else `SLOW_ODDS`.
const FAST_ODDS: i32 = 3;
const SLOW_ODDS: i32 = 8;
/// Last composting stage; the next step turns the block into rich soil.
const COMPOST_MAX: i32 = 7;

/// `farmersdelight:compost_activators`, spelled out because native hooks see no tags.
const ACTIVATORS: [&str; 9] = [
    "minecraft:brown_mushroom",
    "minecraft:red_mushroom",
    "minecraft:podzol",
    "minecraft:mycelium",
    COMPOST,
    RICH_SOIL,
    FARMLAND,
    "farmersdelight:brown_mushroom_colony",
    "farmersdelight:red_mushroom_colony",
];

/// The age at which the crop is ripe, for the crops rich soil speeds up.
fn max_age(block: &str) -> Option<i32> {
    match block {
        "minecraft:wheat" | "minecraft:carrots" | "minecraft:potatoes" => Some(7),
        "minecraft:beetroots" => Some(3),
        "farmersdelight:cabbages" | "farmersdelight:onions" => Some(7),
        BUD | VINE | PANICLES => Some(3),
        _ => None,
    }
}

fn roll(level: &mut GameLevel<'_>, odds: i32) -> bool {
    level
        .native_level_random()
        .is_some_and(|random| random.next_int_bound(odds).ok() == Some(0))
}

/// `getRawBrightness(pos, 0)`.
fn brightness(level: &GameLevel<'_>, pos: BlockPos) -> i32 {
    level.block_light(pos).max(level.sky_light(pos) - level.sky_darken())
}

fn name_at(level: &GameLevel<'_>, pos: BlockPos) -> String {
    level
        .registry
        .state(level.block_state(pos))
        .map(|info| info.name.to_string())
        .unwrap_or_default()
}

fn int_property(level: &GameLevel<'_>, pos: BlockPos, key: &str) -> Option<i32> {
    level
        .registry
        .state(level.block_state(pos))?
        .properties
        .get(key)?
        .parse()
        .ok()
}

fn set_property(level: &mut GameLevel<'_>, ticks: &mut Ticks, pos: BlockPos, key: &str, value: i32) {
    level.native_set_block_property(
        ticks,
        [pos.x, pos.y, pos.z],
        key,
        &value.to_string(),
        flags::UPDATE_CLIENTS,
    );
}

/// Replaces the block at `pos` by the default state of `block`.
fn replace(level: &mut GameLevel<'_>, ticks: &mut Ticks, pos: BlockPos, block: &str) -> bool {
    let Some(state) = level.registry.default_state(block) else {
        return false;
    };
    let behaviours = level.behaviours;
    behaviours.set_block(level, ticks, pos, state, flags::UPDATE_ALL);
    true
}

/// `RiceBlock.randomTick`: a stage with a one in five chance when lit, and at the last stage
/// the panicles go on top if that cell is air.
fn rice(level: &mut GameLevel<'_>, ticks: &mut Ticks, pos: BlockPos) {
    let Some(age) = int_property(level, pos, "age") else {
        return;
    };
    let above = pos.offset(0, 1, 0);
    if age >= 3 {
        if level.facts.is_air(level.block_state(above)) && brightness(level, above) >= 6 {
            replace(level, ticks, above, PANICLES);
        }
    } else if brightness(level, above) >= 6 && roll(level, RICE_ODDS) {
        set_property(level, ticks, pos, "age", age + 1);
    }
}

/// A bud that is about to ripen becomes a vine (age 0) in the same place. A fully grown crop no
/// longer receives random ticks, so the step from age 2 is taken here (one chance in three, more
/// likely than the crop's own growth step); a bud that still reached age 3 turns on a right click.
fn bud(level: &mut GameLevel<'_>, ticks: &mut Ticks, pos: BlockPos) {
    if int_property(level, pos, "age").is_some_and(|age| age >= 2) && roll(level, BOOST_ODDS) {
        replace(level, ticks, pos, VINE);
    }
}

/// Rich soil and rich soil farmland: one more growth step for the crop above.
fn boost(level: &mut GameLevel<'_>, ticks: &mut Ticks, pos: BlockPos) {
    let above = pos.offset(0, 1, 0);
    let block = name_at(level, above);
    let Some(max) = max_age(&block) else {
        return;
    };
    let Some(age) = int_property(level, above, "age") else {
        return;
    };
    if age < max && brightness(level, above) >= 9 && roll(level, BOOST_ODDS) {
        set_property(level, ticks, above, "age", age + 1);
    }
}

/// Organic compost: the stage rises with activators near, and the last stage becomes rich soil.
fn compost(level: &mut GameLevel<'_>, ticks: &mut Ticks, pos: BlockPos) {
    let Some(stage) = int_property(level, pos, "composting") else {
        return;
    };
    let mut near = false;
    'scan: for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                if (dx, dy, dz) != (0, 0, 0)
                    && ACTIVATORS.contains(&name_at(level, pos.offset(dx, dy, dz)).as_str())
                {
                    near = true;
                    break 'scan;
                }
            }
        }
    }
    if !roll(level, if near { FAST_ODDS } else { SLOW_ODDS }) {
        return;
    }
    if stage >= COMPOST_MAX {
        replace(level, ticks, pos, RICH_SOIL);
    } else {
        set_property(level, ticks, pos, "composting", stage + 1);
    }
}

/// The `random_tick` hook: only the blocks of this module are touched.
pub(super) fn random_tick(
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
    let block = info.name.to_string();
    match block.as_str() {
        RICE => rice(level, ticks, pos),
        BUD => bud(level, ticks, pos),
        RICH_SOIL | FARMLAND => boost(level, ticks, pos),
        COMPOST => compost(level, ticks, pos),
        _ => {}
    }
}

/// A hoe on the top of rich soil with air above makes rich soil farmland. True when it did.
pub(super) fn right_click(game: &mut Game<'_>, pos: [i32; 3], face: [i32; 3]) -> bool {
    if game.native_block(pos) == BUD && game.native_block_property(pos, "age").is_some_and(|age| age == "3") {
        let Some(vine) = game.pack.assets.registry.default_state(VINE) else {
            return false;
        };
        return game.native_set_block(game.dim.id(), pos, vine, flags::UPDATE_ALL);
    }
    if game.native_block(pos) != RICH_SOIL || face == [0, -1, 0] {
        return false;
    }
    let hoe = game
        .held()
        .is_some_and(|stack| stack.item.to_string().ends_with("_hoe"));
    if !hoe || game.native_block([pos[0], pos[1] + 1, pos[2]]) != "minecraft:air" {
        return false;
    }
    let Some(tilled) = game.pack.assets.registry.default_state(FARMLAND) else {
        return false;
    };
    if !game.native_set_block(game.dim.id(), pos, tilled, flags::UPDATE_ALL) {
        return false;
    }
    game.damage_held_in_slot(1, EquipmentSlot::MainHand);
    true
}
