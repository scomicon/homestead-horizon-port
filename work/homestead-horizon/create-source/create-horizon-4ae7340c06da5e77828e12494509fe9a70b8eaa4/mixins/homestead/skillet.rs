// Copyright (c) vectorwing / Farmer's Delight contributors (MIT)
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Farmer's Delight Skillet as a placed block (`SkilletBlock`, `SkilletBlockEntity`,
// `HeatableBlockEntity.isHeated`). The block and its item come from the content mod
// (see fd/skillet.py); this module gives the block its block entity (one stack that
// cooks with a `minecraft:campfire_cooking` recipe), the right click, the ticking
// cook, the `support` and `facing` placement state, the drop on break and the item
// drawn in the pan. Written from the 1.20 behaviour as read from the GitHub source
// of vectorwing/FarmersDelight (see docs/farmersdelight-skillet.md for what is
// unverified or missing: the hand-held cooking of the item, Fire Aspect, water).
use horizon_core::random::RandomSource;
use super::{Game, GameLevel, native_block_entity_draw::BlockEntityDraw};
use crate::{
    containers::{BlockEntity, ModBlockEntity},
    items::ItemStack,
};
use horizon_assets::item_model::DisplayContext;
use horizon_client::text::Text;
use horizon_core::geometry::Direction;
use horizon_items::item_nbt::{item_from_nbt, item_to_nbt};
use horizon_world::{
    level::PlacementContext,
    nbt::{Compound, Tag},
    registry::BlockStateId,
};

pub(super) const BLOCK: &str = "farmersdelight:skillet";
const RECIPES: &str = "minecraft:campfire_cooking";
const HEAT_SOURCES: &str = "farmersdelight:heat_sources";
const HEAT_CONDUCTORS: &str = "farmersdelight:heat_conductors";
/// `farmersdelight:tray_heat_sources` (lava, #minecraft:campfires, #minecraft:fire and the
/// Decorative Blocks braziers), spelled out because the placement hooks see no tags.
const TRAY_HEAT_SOURCES: [&str; 7] = [
    "minecraft:lava",
    "minecraft:campfire",
    "minecraft:soul_campfire",
    "minecraft:fire",
    "minecraft:soul_fire",
    "decorative_blocks:brazier",
    "decorative_blocks:soul_brazier",
];
/// `SkilletBlock.MINIMUM_COOKING_TIME`, 3 seconds.
const MINIMUM_COOKING_TIME: i32 = 60;
/// Chance per tick of the sizzle while something cooks (FD plays it from its
/// client-side animation tick; the rate is a stand-in, not read from the source).
const SIZZLE_CHANCE: f32 = 0.025;

type Pos = [i32; 3];

// ------------------------------------------------------------------- state

pub(super) fn entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if block == BLOCK && entity.is_none() {
        *entity = Some((block.into(), Compound::new()));
    }
}

pub(super) fn ticks(block: &str, ticks: &mut bool) {
    if block == BLOCK {
        *ticks = true;
    }
}

fn stored(nbt: &Compound) -> Option<ItemStack> {
    nbt.get(&"Item".into()).and_then(item_from_nbt).filter(|stack| stack.count > 0)
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

fn int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    }
}

fn set_int(nbt: &mut Compound, key: &str, value: i32) {
    nbt.insert(key.into(), Tag::Int(value));
}

pub(super) fn update_tag(entity: &ModBlockEntity) -> Compound {
    entity
        .nbt
        .get(&"Item".into())
        .map(|tag| Compound::from([("Item".into(), tag.clone())]))
        .unwrap_or_default()
}

// ----------------------------------------------------------------- recipes

/// The campfire cooking recipe of `stack`: its cooking time and its result.
fn recipe(game: &Game<'_>, stack: &ItemStack) -> Option<(u32, ItemStack)> {
    game.recipes().cook(RECIPES, stack).map(|recipe| (recipe.time, recipe.result.clone()))
}

/// `SkilletBlock.getSkilletCookingTime(original, 0)`: a fifth of the recipe's whole seconds
/// (0.2F, truncated), at least 60 ticks (`Mth.clamp(result, 60, original)`).
fn cooking_time(original: u32) -> i32 {
    let original = i32::try_from(original).unwrap_or(i32::MAX);
    let base = if original > 0 { original } else { 600 };
    let seconds = base / 20;
    let result = ((seconds as f32 * 0.2_f32) as i32) * 20;
    if result < MINIMUM_COOKING_TIME { MINIMUM_COOKING_TIME } else { result.min(original) }
}

// -------------------------------------------------------------------- heat

/// `Some(lit)` when the block at `at` is a `#farmersdelight:heat_sources` block; a source with a
/// `lit` property only heats while it is true (`HeatableBlockEntity.isHeated`).
fn source(game: &Game<'_>, at: Pos) -> Option<bool> {
    let state = game.pack.assets.registry.state(game.world.block(at[0], at[1], at[2]))?;
    if !game.native_server_block_in_tag(state, HEAT_SOURCES).unwrap_or(false) {
        return None;
    }
    Some(state.properties.get("lit").is_none_or(|value| &**value == "true"))
}

fn conductor(game: &Game<'_>, at: Pos) -> bool {
    game.pack
        .assets
        .registry
        .state(game.world.block(at[0], at[1], at[2]))
        .is_some_and(|state| game.native_server_block_in_tag(state, HEAT_CONDUCTORS).unwrap_or(false))
}

/// `HeatableBlockEntity.isHeated`: the block below is a (lit) heat source, or a heat conductor
/// above one.
fn heated(game: &Game<'_>, pos: Pos) -> bool {
    let below = [pos[0], pos[1] - 1, pos[2]];
    if let Some(lit) = source(game, below) {
        return lit;
    }
    if conductor(game, below) {
        if let Some(lit) = source(game, [pos[0], pos[1] - 2, pos[2]]) {
            return lit;
        }
    }
    false
}

// -------------------------------------------------------------------- tick

/// `Direction.getClockWise` of `facing` as a horizontal step.
fn clockwise(facing: &str) -> [i32; 2] {
    match facing {
        "north" => [1, 0],
        "east" => [0, 1],
        "south" => [-1, 0],
        _ => [0, -1],
    }
}

/// `SkilletBlockEntity.cookingTick`: heated, the cook time rises by 1 and at the total the
/// first item of the stack becomes its recipe's result and is thrown out of the pan; cold, the
/// progress falls by 2. (The waterlogged branch has no counterpart: the block is not waterloggable here.)
pub(super) fn tick(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity) {
    if entity.id != BLOCK || (game.serving.is_none() && game.options.dedicated.is_none()) {
        return;
    }
    let Some(stack) = stored(&entity.nbt) else {
        if int(&entity.nbt, "CookTime") != 0 {
            set_int(&mut entity.nbt, "CookTime", 0);
        }
        return;
    };
    let mut total = int(&entity.nbt, "CookTimeTotal");
    if total <= 0 {
        // Filled without the right click (a command): take the time from the recipe.
        let Some((time, _)) = recipe(game, &stack) else { return };
        total = cooking_time(time);
        set_int(&mut entity.nbt, "CookTimeTotal", total);
    }
    let mut time = int(&entity.nbt, "CookTime");
    if heated(game, pos) {
        time += 1;
        if game.world.random.next_float() < SIZZLE_CHANCE {
            let at = pos.map(|c| f64::from(c) + 0.5);
            game.native_play_sound_in_place("farmersdelight:block.skillet.sizzle", at, 4, 0.4, 1.0);
        }
        if time >= total {
            // With no recipe the check repeats every tick, as FD's does.
            if let Some((_, result)) = recipe(game, &stack) {
                let facing = game
                    .native_block_property(pos, "facing")
                    .unwrap_or_else(|| "north".into());
                let [dx, dz] = clockwise(&facing);
                game.spawn_item(
                    result,
                    [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.3, f64::from(pos[2]) + 0.5],
                    [f64::from(dx) * 0.08, 0.25, f64::from(dz) * 0.08],
                    10,
                );
                let mut rest = stack;
                rest.count -= 1;
                set_stored(&mut entity.nbt, (rest.count > 0).then_some(rest));
                time = 0;
                game.native_block_entity_sync(pos);
            }
        }
    } else if time > 0 {
        time = (time - 2).max(0).min(total);
    }
    set_int(&mut entity.nbt, "CookTime", time);
}

// ------------------------------------------------------------ interaction

fn message(game: &mut Game<'_>, key: &str) {
    let player = game.native_acting_id();
    game.native_action_bar(player, Text::translate(key, &[]));
}

/// `SkilletBlock.use`. True when the click was used.
/// * Empty hand: the whole stack comes out (a creative player gets nothing back).
/// * A food with a campfire cooking recipe goes in, as much as fits (`ItemStackHandler.insertItem`),
///   when the pan is empty or holds the same item; the hand keeps the rest. Anything else says
///   "This can't be cooked..." and the click passes on (placing a block beside the pan).
/// * Sneaking with an item in hand is a placement, as vanilla does not call `use` then.
pub(super) fn right_click(game: &mut Game<'_>, pos: Pos) -> bool {
    if game.native_block(pos) != BLOCK || game.held_slot() != game.selected || !game.may_build() {
        return false;
    }
    let sneaking = game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft);
    let held = game.held().filter(|stack| stack.count > 0).cloned();
    let at = pos.map(|c| f64::from(c) + 0.5);
    let Some(held) = held else {
        let Some(Some(taken)) = game.native_with_mod_block_entity(pos, |entity| {
            let taken = stored(&entity.nbt);
            set_stored(&mut entity.nbt, None);
            taken
        }) else {
            return false;
        };
        if !game.creative {
            let uuid = game.native_acting_id();
            if let Some(rest) = game.native_give_stack(uuid, taken) {
                game.spawn_item(rest, [at[0], at[1] + 0.3, at[2]], [0.0, 0.1, 0.0], 10);
            }
        }
        game.native_block_entity_sync(pos);
        return true;
    };
    if sneaking {
        return false;
    }
    let Some((time, _)) = recipe(game, &held) else {
        message(game, "farmersdelight.block.skillet.invalid_item");
        return false;
    };
    let total = cooking_time(time);
    let maximum = game.native_max_stack_size(&held.item).unwrap_or(64);
    let outcome = game
        .native_with_mod_block_entity(pos, |entity| {
            let current = stored(&entity.nbt);
            let (present, room) = match &current {
                None => (0, maximum),
                Some(stack) if stack.same_item_same_components(&held) => {
                    (stack.count, maximum.saturating_sub(stack.count))
                }
                Some(_) => return None,
            };
            let moved = held.count.min(room);
            if moved == 0 {
                return None;
            }
            let mut merged = held.clone();
            merged.count = present + moved;
            set_stored(&mut entity.nbt, Some(merged));
            set_int(&mut entity.nbt, "CookTimeTotal", total);
            set_int(&mut entity.nbt, "CookTime", 0);
            Some((moved, current.is_none()))
        })
        .flatten();
    let Some((moved, was_empty)) = outcome else {
        return false;
    };
    if !game.creative {
        let cell = game.held_mut();
        if let Some(stack) = cell.as_mut() {
            stack.count = stack.count.saturating_sub(moved);
            if stack.count == 0 {
                *cell = None;
            }
        }
    }
    game.native_play_sound_in_place("minecraft:block.lantern.place", at, 4, 0.7, 1.0);
    if was_empty && heated(game, pos) {
        game.native_play_sound_in_place("farmersdelight:block.skillet.add_food", at, 4, 0.8, 1.0);
    }
    game.native_block_entity_sync(pos);
    true
}

/// `SkilletBlock.onRemove`: the pan drops what it holds when the block breaks.
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

// --------------------------------------------------------------- placement

/// `Direction.fromYRot(yaw)`, the way the placer looks.
fn facing_name(yaw: f32) -> &'static str {
    ["south", "west", "north", "east"][((f64::from(yaw) / 90.0 + 0.5).floor() as i64 & 3) as usize]
}

fn on_tray(game: &GameLevel<'_>, below: BlockStateId) -> bool {
    game.registry
        .state(below)
        .is_some_and(|state| TRAY_HEAT_SOURCES.iter().any(|name| *name == &*state.name))
}

fn truth(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

/// `SkilletBlock.getStateForPlacement`: facing the way the placer looks, `support` when a tray
/// heat source lies below.
pub(super) fn placement(
    game: &GameLevel<'_>,
    state: BlockStateId,
    ctx: &PlacementContext,
    answer: &mut Option<Option<BlockStateId>>,
) {
    if answer.is_some() || game.native_state_block(state) != Some(BLOCK) {
        return;
    }
    let below = game.world.block(ctx.pos.x, ctx.pos.y - 1, ctx.pos.z);
    *answer = Some(
        game.native_state_with_property(state, "facing", facing_name(ctx.yaw))
            .and_then(|state| game.native_state_with_property(state, "support", truth(on_tray(game, below)))),
    );
}

/// `SkilletBlock.updateShape`: a change below sets `support` again.
pub(super) fn shape(
    game: &GameLevel<'_>,
    state: BlockStateId,
    direction: Direction,
    neighbour: BlockStateId,
    new: &mut Option<BlockStateId>,
) {
    if direction != Direction::Down {
        return;
    }
    let Some(block) = game.registry.state(state).filter(|block| &*block.name == BLOCK) else {
        return;
    };
    let support = truth(on_tray(game, neighbour));
    if block.properties.get("support").is_some_and(|value| &**value == support) {
        return;
    }
    if let Some(next) = game.native_state_with_property(state, "support", support) {
        *new = Some(next);
    }
}

// ------------------------------------------------------------------- draw

/// `Direction.toYRot` of a horizontal facing.
fn yaw_of(facing: &str) -> f32 {
    match facing {
        "west" => 90.0,
        "north" => 180.0,
        "east" => 270.0,
        _ => 0.0,
    }
}

/// `SkilletBlockEntityRenderer`: the item lies in the pan, turned with the block. The
/// translation, turn and scale are approximations, not read from the renderer.
pub(super) fn draw(game: &Game<'_>, pos: Pos, entity: &ModBlockEntity, draw: &mut BlockEntityDraw) {
    let Some(stack) = stored(&entity.nbt) else {
        return;
    };
    let facing = game.native_block_property(pos, "facing").unwrap_or_else(|| "north".into());
    draw.push();
    draw.translate([0.5, 0.1, 0.5]);
    draw.rotate_about([0.0, 1.0, 0.0], -yaw_of(&facing), [0.0; 3]);
    draw.rotate_about([1.0, 0.0, 0.0], 90.0, [0.0; 3]);
    draw.scale([0.5, 0.5, 0.5]);
    game.native_draw_item(draw, &stack, DisplayContext::Fixed, None, 0);
    draw.pop();
}

/// The block entity at `pos`, if it is the pan's (for a caller that only reads).
#[allow(dead_code, reason = "read helper for acceptance tooling")]
pub(super) fn contents(game: &Game<'_>, pos: Pos) -> Option<ItemStack> {
    match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
        Some(BlockEntity::Mod(entity)) if entity.id == BLOCK => stored(&entity.nbt),
        _ => None,
    }
}
