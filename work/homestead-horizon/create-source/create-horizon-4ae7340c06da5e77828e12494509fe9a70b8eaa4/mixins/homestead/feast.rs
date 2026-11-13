// Copyright (c) vectorwing / Farmer's Delight contributors (MIT)
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Farmer's Delight feast blocks (`FeastBlock`, `RiceRollMedleyBlock`), pies (`PieBlock`) and the
// knife cut of the vanilla cake. The blocks, their items, models, blockstates and loot tables come
// from the content mod (see fd/feast.py); this module is the right click:
//
// * a feast with servings left hands out its serving item. A serving that leaves a bowl behind
//   needs that bowl in hand (the bowl is used up unless the player is in creative mode); a feast
//   whose serving has no remainder (the rice roll medley) is served by any click;
// * a feast without servings is broken (`FeastBlock.takeServing`, servings == 0) and drops what
//   its loot table gives for an eaten block: the bowl and, for the chicken and the ham, a bone;
// * the last serving of a feast without leftovers (the stuffed pumpkin) removes the block;
// * a pie is cut into slices with a knife (the slice item pops out of the block) or eaten bite by
//   bite with any other item/empty hand; the fourth bite or cut removes the pie;
// * a knife cuts the vanilla cake into `farmersdelight:cake_slice` items.
//
// Written from the 1.20 sources of vectorwing/FarmersDelight as read through a summarising
// fetch (see docs/farmersdelight-feasts.md for what is unverified or missing).
use super::{Game, native_player_state::NativeEffect};
use crate::items::ItemStack;
use horizon_client::text::Text;
use horizon_world::level::flags;

type Pos = [i32; 3];

const BOWL: &str = "minecraft:bowl";
/// Items a knife cut accepts (`ItemUtils.isKnife`, the `c:tools/knives` tag of the content mod).
const KNIVES: [&str; 5] = [
    "farmersdelight:flint_knife",
    "farmersdelight:iron_knife",
    "farmersdelight:golden_knife",
    "farmersdelight:diamond_knife",
    "farmersdelight:netherite_knife",
];

struct Feast {
    block: &'static str,
    /// `getMaxServings()`: 4 for the plated feasts, 8 for the rice roll medley.
    max: i32,
    /// The item a serving needs in hand (its `getCraftingRemainingItem`).
    container: Option<&'static str>,
    /// `hasLeftovers`: the block stays at 0 servings until it is clicked once more.
    leftovers: bool,
    /// The loot table's drops for a block with fewer than the full servings.
    leftover_drops: &'static [&'static str],
}

static FEASTS: [Feast; 5] = [
    Feast {
        block: "farmersdelight:roast_chicken_block",
        max: 4,
        container: Some(BOWL),
        leftovers: true,
        leftover_drops: &[BOWL, "minecraft:bone_meal"],
    },
    Feast {
        block: "farmersdelight:stuffed_pumpkin_block",
        max: 4,
        container: Some(BOWL),
        leftovers: false,
        leftover_drops: &[],
    },
    Feast {
        block: "farmersdelight:honey_glazed_ham_block",
        max: 4,
        container: Some(BOWL),
        leftovers: true,
        leftover_drops: &[BOWL, "minecraft:bone"],
    },
    Feast {
        block: "farmersdelight:shepherds_pie_block",
        max: 4,
        container: Some(BOWL),
        leftovers: true,
        leftover_drops: &[BOWL],
    },
    Feast {
        block: "farmersdelight:rice_roll_medley_block",
        max: 8,
        container: None,
        leftovers: true,
        leftover_drops: &[BOWL],
    },
];

/// `RiceRollMedleyBlock.riceRollServings.get(servings - 1)`.
static MEDLEY: [&str; 8] = [
    "farmersdelight:cod_roll",
    "farmersdelight:cod_roll",
    "farmersdelight:salmon_roll",
    "farmersdelight:salmon_roll",
    "farmersdelight:salmon_roll",
    "farmersdelight:kelp_roll_slice",
    "farmersdelight:kelp_roll_slice",
    "farmersdelight:kelp_roll_slice",
];

struct Pie {
    block: &'static str,
    slice: &'static str,
    /// The slice's food value: nutrition and saturation modifier.
    nutrition: i32,
    saturation: f32,
}

static PIES: [Pie; 3] = [
    Pie { block: "farmersdelight:apple_pie", slice: "farmersdelight:apple_pie_slice", nutrition: 3, saturation: 0.3 },
    Pie {
        block: "farmersdelight:chocolate_pie",
        slice: "farmersdelight:chocolate_pie_slice",
        nutrition: 3,
        saturation: 0.3,
    },
    Pie {
        block: "farmersdelight:sweet_berry_cheesecake",
        slice: "farmersdelight:sweet_berry_cheesecake_slice",
        nutrition: 3,
        saturation: 0.3,
    },
];

fn feast(block: &str) -> Option<&'static Feast> {
    FEASTS.iter().find(|feast| feast.block == block)
}

fn pie(block: &str) -> Option<&'static Pie> {
    PIES.iter().find(|pie| pie.block == block)
}

fn serving_item(feast: &Feast, servings: i32) -> &'static str {
    match feast.block {
        "farmersdelight:roast_chicken_block" => "farmersdelight:roast_chicken",
        "farmersdelight:stuffed_pumpkin_block" => "farmersdelight:stuffed_pumpkin",
        "farmersdelight:honey_glazed_ham_block" => "farmersdelight:honey_glazed_ham",
        "farmersdelight:shepherds_pie_block" => "farmersdelight:shepherds_pie",
        _ => MEDLEY[(servings.clamp(1, 8) - 1) as usize],
    }
}

fn int_property(game: &Game<'_>, pos: Pos, name: &str) -> Option<i32> {
    game.native_block_property(pos, name).and_then(|value| value.parse().ok())
}

fn centre(pos: Pos) -> [f64; 3] {
    pos.map(|c| f64::from(c) + 0.5)
}

fn remove_block(game: &mut Game<'_>, pos: Pos) -> bool {
    let Some(air) = game.pack.assets.registry.default_state("minecraft:air") else {
        return false;
    };
    let dimension = game.dim.id().to_owned();
    game.native_set_block(&dimension, pos, air, flags::UPDATE_ALL)
}

fn pop(game: &mut Game<'_>, pos: Pos, stack: ItemStack) {
    let at = centre(pos);
    game.spawn_item(stack, [at[0], at[1] + 0.2, at[2]], [0.0, 0.2, 0.0], 10);
}

fn hand_over(game: &mut Game<'_>, stack: ItemStack, at: [f64; 3]) {
    let uuid = game.native_acting_id();
    if let Some(rest) = game.native_give_stack(uuid, stack) {
        game.spawn_item(rest, [at[0], at[1] + 0.3, at[2]], [0.0, 0.1, 0.0], 10);
    }
}

fn message(game: &mut Game<'_>, key: &str) {
    let player = game.native_acting_id();
    game.native_action_bar(player, Text::translate(key, &[]));
}

fn is_knife(item: &str) -> bool {
    KNIVES.contains(&item)
}

/// `FeastBlock.takeServing`. True when the click was used.
fn take_serving(game: &mut Game<'_>, pos: Pos, feast: &Feast, held: Option<&ItemStack>) -> bool {
    let servings = int_property(game, pos, "servings").unwrap_or(feast.max).clamp(0, feast.max);
    let at = centre(pos);
    if servings == 0 {
        game.native_play_sound_in_place("minecraft:block.wood.break", at, 4, 0.8, 0.8);
        if remove_block(game, pos) {
            for id in feast.leftover_drops {
                pop(game, pos, ItemStack::new(id, 1));
            }
        }
        return true;
    }
    if let Some(container) = feast.container {
        if held.is_none_or(|stack| &*stack.item != container) {
            message(game, "farmersdelight.block.feast.use_bowl");
            return false;
        }
    }
    let rest = servings - 1;
    if !game.native_set_block_property(pos, "servings", &rest.to_string()) {
        return false;
    }
    if feast.container.is_some() && !game.creative {
        let cell = game.held_mut();
        if let Some(stack) = cell.as_mut() {
            stack.count = stack.count.saturating_sub(1);
            if stack.count == 0 {
                *cell = None;
            }
        }
    }
    hand_over(game, ItemStack::new(serving_item(feast, servings), 1), at);
    if rest == 0 && !feast.leftovers {
        remove_block(game, pos);
    }
    // FD's `BLOCK_FOOD_TAKE_PORTION` event was not read; this is a stand-in (as for the pot).
    game.native_play_sound_in_place("minecraft:item.armor.equip_generic", at, 4, 1.0, 1.0);
    true
}

/// `PieBlock.cutSlice`: the next bite, or the block goes with the last one.
fn advance(game: &mut Game<'_>, pos: Pos, bites: i32, last: i32) {
    if bites >= last {
        remove_block(game, pos);
    } else {
        game.native_set_block_property(pos, "bites", &(bites + 1).to_string());
    }
}

fn cut_pie(game: &mut Game<'_>, pos: Pos, pie: &Pie, bites: i32) {
    advance(game, pos, bites, 3);
    pop(game, pos, ItemStack::new(pie.slice, 1));
    // FD's `BLOCK_FOOD_SLICE` event was not read; wool is a stand-in.
    game.native_play_sound_in_place("minecraft:block.wool.break", centre(pos), 4, 0.8, 1.0);
}

/// `PieBlock.consumeBite`: only a player who can eat; the slice's food value and its speed effect.
fn eat_pie(game: &mut Game<'_>, pos: Pos, pie: &Pie, bites: i32) -> bool {
    let Some(player) = game.player.as_mut() else {
        return false;
    };
    if !player.health.food.needs_food() {
        return false;
    }
    player.health.food.eat(pie.nutrition, pie.saturation);
    let uuid = game.native_acting_id();
    game.native_add_effect(
        uuid,
        &NativeEffect {
            id: "minecraft:speed".into(),
            amplifier: 0,
            duration: 600,
            ambient: false,
            particles: true,
        },
    );
    advance(game, pos, bites, 3);
    game.native_play_sound_in_place("minecraft:entity.generic.eat", centre(pos), 4, 0.8, 0.8);
    true
}

/// The right click on a feast, pie or cake. True when it was used.
pub(super) fn right_click(game: &mut Game<'_>, pos: Pos) -> bool {
    let block = game.native_block(pos);
    let (is_feast, is_pie, is_cake) = (feast(&block), pie(&block), block == "minecraft:cake");
    if is_feast.is_none() && is_pie.is_none() && !is_cake {
        return false;
    }
    if game.held_slot() != game.selected || !game.may_build() {
        return false;
    }
    let held = game.held().filter(|stack| stack.count > 0).cloned();
    if game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft) && held.is_some() {
        return false;
    }
    let knife = held.as_ref().is_some_and(|stack| is_knife(&stack.item));
    if let Some(feast) = is_feast {
        return take_serving(game, pos, feast, held.as_ref());
    }
    let bites = int_property(game, pos, "bites").unwrap_or(0);
    if let Some(pie) = is_pie {
        if knife {
            cut_pie(game, pos, pie, bites.clamp(0, 3));
            return true;
        }
        return eat_pie(game, pos, pie, bites.clamp(0, 3));
    }
    // Vanilla cake: a knife cuts a slice (seven slices, `bites` 0..=6); other items eat it as usual.
    if knife {
        advance(game, pos, bites.clamp(0, 6), 6);
        pop(game, pos, ItemStack::new("farmersdelight:cake_slice", 1));
        game.native_play_sound_in_place("minecraft:block.wool.break", centre(pos), 4, 0.8, 1.0);
        return true;
    }
    false
}
