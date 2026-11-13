// Copyright (c) vectorwing / Farmer's Delight contributors (MIT)
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Farmer's Delight Cutting Board (`CuttingBoardBlock.use`, `CuttingBoardBlockEntity`).
// The block comes from the content mod; this module gives it its block entity,
// the right-click behaviour, the `farmersdelight:cutting` recipes, the drop on
// break and the item drawn on the board. Written from the 2.x behaviour as
// remembered, not diffed against the Java source (see docs/farmersdelight-stage8.md).
use super::{Game, native_block_entity_draw::BlockEntityDraw};
use crate::{containers::ModBlockEntity, items::ItemStack};
use horizon_assets::item_model::DisplayContext;
use horizon_items::item_nbt::{item_from_nbt, item_to_nbt};
use horizon_world::nbt::{Compound, Tag};
use serde_json::Value;

pub(super) const BLOCK: &str = "farmersdelight:cutting_board";
const RECIPES: &str = "farmersdelight:cutting";
const KNIFE_SOUND: &str = "farmersdelight:block.cutting_board.knife";

type Pos = [i32; 3];

// ------------------------------------------------------------------- state

pub(super) fn entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if block == BLOCK && entity.is_none() {
        *entity = Some((block.into(), Compound::new()));
    }
}

fn stored(nbt: &Compound) -> Option<ItemStack> {
    nbt.get(&"Item".into()).and_then(item_from_nbt)
}

fn set_stored(nbt: &mut Compound, stack: Option<ItemStack>) {
    match stack {
        Some(stack) => {
            nbt.insert("Item".into(), item_to_nbt(&stack));
        }
        None => {
            nbt.remove(&"Item".into());
        }
    }
}

pub(super) fn update_tag(entity: &ModBlockEntity) -> Compound {
    entity
        .nbt
        .get(&"Item".into())
        .map(|tag| Compound::from([("Item".into(), tag.clone())]))
        .unwrap_or_default()
}

// ----------------------------------------------------------------- recipes

fn matches_ingredient(game: &Game<'_>, ingredient: &Value, stack: &ItemStack) -> bool {
    if let Some(id) = ingredient.as_str() {
        return &*stack.item == id;
    }
    if let Some(id) = ingredient.get("item").and_then(Value::as_str) {
        return &*stack.item == id;
    }
    if let Some(tag) = ingredient.get("tag").and_then(Value::as_str) {
        return game.native_server_item_in_tag(stack, tag).unwrap_or(false);
    }
    false
}

/// `ToolActionIngredient` / tag / item tool of a recipe against the held tool.
fn matches_tool(game: &Game<'_>, tool: &Value, held: &ItemStack) -> bool {
    if let Some(action) = tool.get("action").and_then(Value::as_str) {
        let id: &str = &held.item;
        return match action {
            "axe_dig" | "axe_strip" | "axe_scrape" | "axe_wax_off" => id.ends_with("_axe"),
            "pickaxe_dig" => id.ends_with("_pickaxe"),
            "shovel_dig" => id.ends_with("_shovel"),
            "hoe_dig" => id.ends_with("_hoe"),
            "shears_dig" => id == "minecraft:shears",
            _ => false,
        };
    }
    if let Some(tag) = tool.get("tag").and_then(Value::as_str) {
        let tagged = game.native_server_item_in_tag(held, tag).unwrap_or(false);
        // The convention tag is a safety net for items the tag does not list.
        return tagged
            || (tag == "c:tools/knives" && held.item.ends_with("_knife"))
            || (tag == "c:shears" && &*held.item == "minecraft:shears");
    }
    if let Some(id) = tool.get("item").and_then(Value::as_str) {
        return &*held.item == id;
    }
    false
}

/// The first recipe whose ingredient is on the board and whose tool is in hand.
fn find_recipe(game: &Game<'_>, board: &ItemStack, held: &ItemStack) -> Option<Value> {
    game.native_recipes_of_type(RECIPES)
        .iter()
        .find(|recipe| {
            let json = &*recipe.json;
            json["ingredients"]
                .as_array()
                .is_some_and(|list| list.iter().any(|i| matches_ingredient(game, i, board)))
                && matches_tool(game, &json["tool"], held)
        })
        .map(|recipe| (*recipe.json).clone())
}

struct Roll(u64);

impl Roll {
    fn new(seed: i64, pos: Pos) -> Self {
        let mix = (seed as u64)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (pos[0] as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
            ^ (pos[2] as u64).wrapping_mul(0x94D0_49BB_1331_11EB);
        Self(mix | 1)
    }

    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// `CuttingBoardRecipe.rollResults`: each result drops with its chance.
fn results(json: &Value, roll: &mut Roll) -> Vec<ItemStack> {
    let Some(list) = json["result"].as_array() else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|entry| {
            let item = entry.get("item").or_else(|| entry.get("id"))?;
            let (id, nested_count) = match item {
                Value::String(id) => (id.as_str(), None),
                other => (other.get("id")?.as_str()?, other.get("count").and_then(Value::as_u64)),
            };
            let count = entry
                .get("count")
                .and_then(Value::as_u64)
                .or(nested_count)
                .unwrap_or(1)
                .clamp(1, 64) as u32;
            let chance = entry.get("chance").and_then(Value::as_f64).unwrap_or(1.0);
            (roll.next() < chance).then(|| ItemStack::new(id, count))
        })
        .collect()
}

// ------------------------------------------------------------ interaction

fn counter_clockwise(facing: &str) -> [i32; 2] {
    match facing {
        "north" => [-1, 0],
        "west" => [0, 1],
        "south" => [1, 0],
        _ => [0, -1],
    }
}

fn cut(game: &mut Game<'_>, pos: Pos, board: ItemStack, held: &ItemStack) -> bool {
    let Some(recipe) = find_recipe(game, &board, held) else {
        return false;
    };
    let mut roll = Roll::new(game.world.game_time, pos);
    let drops = results(&recipe, &mut roll);
    let facing = game
        .native_block_property(pos, "facing")
        .unwrap_or_else(|| "north".into());
    let [dx, dz] = counter_clockwise(&facing);
    for stack in drops {
        game.spawn_item(
            stack,
            [
                f64::from(pos[0]) + 0.5 + f64::from(dx) * 0.2,
                f64::from(pos[1]) + 0.2,
                f64::from(pos[2]) + 0.5 + f64::from(dz) * 0.2,
            ],
            [f64::from(dx) * 0.2, 0.0, f64::from(dz) * 0.2],
            10,
        );
    }
    game.damage_held(1);
    let sound = recipe["sound"].as_str().unwrap_or(KNIFE_SOUND).to_owned();
    let at = pos.map(|c| f64::from(c) + 0.5);
    game.native_play_sound_in_place(&sound, at, 4, 1.0, 1.0);
    game.native_with_mod_block_entity(pos, |entity| set_stored(&mut entity.nbt, None));
    game.native_block_entity_sync(pos);
    true
}

/// Right click on a Cutting Board. True when the click was used.
pub(super) fn right_click(game: &mut Game<'_>, pos: Pos) -> bool {
    if game.native_block(pos) != BLOCK
        || game.held_slot() != game.selected
        || !game.may_build()
        || game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft)
    {
        return false;
    }
    let held = game.held().cloned();
    let board = game
        .world
        .block_entities
        .get(&(pos[0], pos[1], pos[2]))
        .and_then(|e| match e {
            crate::containers::BlockEntity::Mod(entity) => stored(&entity.nbt),
            _ => None,
        });
    let at = pos.map(|c| f64::from(c) + 0.5);
    match (board, held) {
        // An empty board takes one item from the hand.
        (None, Some(held)) => {
            let mut one = held;
            one.count = 1;
            game.native_with_mod_block_entity(pos, |entity| set_stored(&mut entity.nbt, Some(one)));
            game.consume_held();
            game.native_play_sound_in_place("minecraft:block.wood.place", at, 4, 1.0, 1.0);
            game.native_block_entity_sync(pos);
            true
        }
        (None, None) => false,
        // A tool cuts what lies on the board.
        (Some(board), Some(held)) => cut(game, pos, board, &held),
        // An empty hand takes the item back.
        (Some(board), None) => {
            game.native_with_mod_block_entity(pos, |entity| set_stored(&mut entity.nbt, None));
            let uuid = game.native_acting_id();
            if let Some(rest) = game.native_give_stack(uuid, board) {
                game.spawn_item(rest, [at[0], at[1] + 0.3, at[2]], [0.0, 0.1, 0.0], 10);
            }
            game.native_play_sound_in_place("minecraft:entity.item.pickup", at, 4, 0.3, 1.5);
            game.native_block_entity_sync(pos);
            true
        }
    }
}

/// The board drops its item when the block breaks.
pub(super) fn destroyed(game: &mut Game<'_>, pos: Pos) {
    if game.native_block(pos) != BLOCK {
        return;
    }
    let item = game
        .native_with_mod_block_entity(pos, |entity| {
            let item = stored(&entity.nbt);
            set_stored(&mut entity.nbt, None);
            item
        })
        .flatten();
    if let Some(item) = item {
        game.spawn_item(
            item,
            [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.2, f64::from(pos[2]) + 0.5],
            [0.0, 0.1, 0.0],
            10,
        );
    }
}

// ------------------------------------------------------------------- draw

pub(super) fn draw(game: &Game<'_>, entity: &ModBlockEntity, draw: &mut BlockEntityDraw) {
    let Some(stack) = stored(&entity.nbt) else {
        return;
    };
    draw.push();
    draw.translate([0.5, 0.08, 0.5]);
    draw.rotate_about([1.0, 0.0, 0.0], 90.0, [0.0; 3]);
    draw.scale([0.5, 0.5, 0.5]);
    game.native_draw_item(draw, &stack, DisplayContext::Fixed, None, 0);
    draw.pop();
}
