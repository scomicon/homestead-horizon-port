// Copyright (c) vectorwing / Farmer's Delight contributors (MIT)
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Farmer's Delight Cooking Pot as a placed block (`CookingPotBlock`, `CookingPotBlockEntity`,
// `CookingPotMenu`, `CookingPotScreen`). The block and its item come from the content mod
// (see fd/cooking_pot.py, fd/cooking_pot_menu.py); this module gives the block its block entity
// with a real container (nine slots as FD: 0-5 ingredients, 6 meal display, 7 container, 8 output,
// held in the engine's `ModBlockEntity.items`), the heated cooking tick over the
// `farmersdelight:cooking` recipes, the meal flow into the output slot, the one right click that
// stays native (serving with the matching container; every other click opens the menu), the
// `facing` and `support` placement state, and the meal drawn in the pot. The screen (layout, draw,
// tooltip) is here too. Written from the 1.20 behaviour as read from the GitHub source of
// vectorwing/FarmersDelight (see docs/farmersdelight-pot.md for what is unverified or missing).
use super::{Game, GameLevel, native_block_entity_draw::BlockEntityDraw};
use crate::{
    containers::{BlockEntity, ModBlockEntity, ModContainer},
    items::ItemStack,
};
use horizon_assets::item_model::DisplayContext;
use horizon_core::geometry::Direction;
use horizon_items::item_nbt::{item_from_nbt, item_to_nbt};
use horizon_world::{
    level::PlacementContext,
    nbt::{Compound, Tag},
    registry::BlockStateId,
};
use serde_json::Value;

pub(super) const BLOCK: &str = "farmersdelight:cooking_pot";
/// The menu type declared by `data/farmersdelight/horizon/menu/cooking_pot.json`.
const MENU: &str = "farmersdelight:cooking_pot";
const RECIPES: &str = "farmersdelight:cooking";
const HEAT_SOURCES: &str = "farmersdelight:heat_sources";
const HEAT_CONDUCTORS: &str = "farmersdelight:heat_conductors";
/// `farmersdelight:tray_heat_sources`, spelled out because the placement hooks see no tags.
const TRAY_HEAT_SOURCES: [&str; 7] = [
    "minecraft:lava",
    "minecraft:campfire",
    "minecraft:soul_campfire",
    "minecraft:fire",
    "minecraft:soul_fire",
    "decorative_blocks:brazier",
    "decorative_blocks:soul_brazier",
];
/// `CookingPotBlockEntity.INGREDIENT_REMAINDER_OVERRIDES` as remembered (unverified list).
const REMAINDER_OVERRIDES: [(&str, &str); 16] = [
    ("minecraft:powder_snow_bucket", "minecraft:bucket"),
    ("minecraft:axolotl_bucket", "minecraft:bucket"),
    ("minecraft:cod_bucket", "minecraft:bucket"),
    ("minecraft:pufferfish_bucket", "minecraft:bucket"),
    ("minecraft:salmon_bucket", "minecraft:bucket"),
    ("minecraft:tropical_fish_bucket", "minecraft:bucket"),
    ("minecraft:mushroom_stew", "minecraft:bowl"),
    ("minecraft:beetroot_soup", "minecraft:bowl"),
    ("minecraft:rabbit_stew", "minecraft:bowl"),
    ("minecraft:suspicious_stew", "minecraft:bowl"),
    ("minecraft:potion", "minecraft:glass_bottle"),
    ("minecraft:splash_potion", "minecraft:glass_bottle"),
    ("minecraft:lingering_potion", "minecraft:glass_bottle"),
    ("minecraft:experience_bottle", "minecraft:glass_bottle"),
    ("minecraft:honey_bottle", "minecraft:glass_bottle"),
    ("minecraft:dragon_breath", "minecraft:glass_bottle"),
];

/// FD's `INVENTORY_SIZE`, `MEAL_DISPLAY_SLOT`, `CONTAINER_SLOT`, `OUTPUT_SLOT`.
const SIZE: usize = 9;
const INPUTS: usize = 6;
const MEAL: usize = 6;
const CONTAINER: usize = 7;
const OUTPUT: usize = 8;
/// The pre-menu versions saved the slots in this NBT list (FD's own key); it is moved into the
/// engine's item slots once (`migrate`). `Items` itself is the engine's.
const LEGACY_INVENTORY: &str = "Inventory";
/// What the update tag carries for clients: the meal on display (an empty compound for none).
const MEAL_KEY: &str = "Meal";
/// The data slots the menu syncs: cook time, cook time total, heated (0 or 1).
const DATA: usize = 3;
const GUI_TEXTURE: &str = "farmersdelight:gui/cooking_pot";

type Pos = [i32; 3];
type Slots = Vec<Option<ItemStack>>;

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

/// `native_block_entity_container`: nine slots, the menu, and no face or hopper access (the
/// hoppers of FD's item handler capability are not ported).
pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    if block == BLOCK && container.is_none() {
        *container = Some(ModContainer {
            slots: SIZE,
            faces: std::array::from_fn(|_| Vec::new()),
            inputs: Vec::new(),
            outputs: Vec::new(),
            menu: Some(MENU.into()),
        });
    }
}

fn int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    }
}

fn set_int(nbt: &mut Compound, key: &str, value: i32) {
    if int(nbt, key) != value {
        nbt.insert(key.into(), Tag::Int(value));
    }
}

fn set_container(nbt: &mut Compound, container: Option<&str>) {
    match container {
        Some(id) => {
            nbt.insert("Container".into(), item_to_nbt(&ItemStack::new(id, 1)));
        }
        None => {
            nbt.remove(&"Container".into());
        }
    }
}

/// The entity's nine item slots, padded.
fn slots_of(entity: &ModBlockEntity) -> Slots {
    let mut slots: Slots = entity.items.clone();
    slots.resize(SIZE, None);
    slots
}

/// The one-time move of the old NBT `Inventory` list into the item slots (a slot the engine
/// already filled keeps what it has). Returns whether it changed anything.
fn migrate(entity: &mut ModBlockEntity) -> bool {
    if !entity.nbt.contains_key(&LEGACY_INVENTORY.into()) {
        return false;
    }
    let mut slots = slots_of(entity);
    if let Some(Tag::List(list)) = entity.nbt.get(&LEGACY_INVENTORY.into()) {
        for tag in list {
            let Some(Tag::Byte(slot)) = tag.get("Slot") else { continue };
            let slot = usize::try_from(*slot).unwrap_or(SIZE);
            if slot < SIZE && slots[slot].is_none() {
                slots[slot] = item_from_nbt(tag).filter(|stack| stack.count > 0);
            }
        }
    }
    entity.items = slots;
    entity.nbt.remove(&LEGACY_INVENTORY.into());
    true
}

/// The meal on display from a saved compound sent as an update tag (`Meal`).
fn meal_from_tag(nbt: &Compound) -> Option<ItemStack> {
    match nbt.get(&MEAL_KEY.into()) {
        Some(tag @ Tag::Compound(map)) if !map.is_empty() => item_from_nbt(tag).filter(|stack| stack.count > 0),
        _ => None,
    }
}

/// The meal of an entity as a client sees it: the item slot (the server's own copy), else what
/// the update tag sent.
fn shown_meal(entity: &ModBlockEntity) -> Option<ItemStack> {
    entity
        .items
        .get(MEAL)
        .cloned()
        .flatten()
        .filter(|stack| stack.count > 0)
        .or_else(|| meal_from_tag(&entity.nbt))
}

/// Clients get the meal for the draw. The tag always names `Meal` (empty when none) because a
/// client merges tags and would keep a stale one.
pub(super) fn update_tag(entity: &ModBlockEntity) -> Compound {
    let meal = entity.items.get(MEAL).cloned().flatten().filter(|stack| stack.count > 0);
    let tag = match meal {
        Some(stack) => item_to_nbt(&stack),
        None => Tag::Compound(Compound::new()),
    };
    Compound::from([(MEAL_KEY.into(), tag)])
}

/// A copy of the pot's saved compound and item slots at `pos`.
fn snapshot(game: &Game<'_>, pos: Pos) -> Option<(Compound, Slots)> {
    match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
        Some(BlockEntity::Mod(entity)) if entity.id == BLOCK => Some((entity.nbt.clone(), slots_of(entity))),
        _ => None,
    }
}

// ----------------------------------------------------------------- recipes

struct Cooking {
    time: i32,
    result: ItemStack,
    container: Option<String>,
}

fn matches_one(game: &Game<'_>, ingredient: &Value, stack: &ItemStack) -> bool {
    if let Some(list) = ingredient.as_array() {
        return list.iter().any(|alternative| matches_one(game, alternative, stack));
    }
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

/// One ingredient per stack, every stack taken (a backtracking assignment: at most six).
fn assign(game: &Game<'_>, ingredients: &[&Value], stacks: &[&ItemStack], used: &mut [bool], at: usize) -> bool {
    if at == stacks.len() {
        return true;
    }
    for (index, ingredient) in ingredients.iter().enumerate() {
        if !used[index] && matches_one(game, ingredient, stacks[at]) {
            used[index] = true;
            if assign(game, ingredients, stacks, used, at + 1) {
                return true;
            }
            used[index] = false;
        }
    }
    false
}

fn result_of(json: &Value) -> Option<ItemStack> {
    let entry = &json["result"];
    let (id, nested) = match entry.get("item").or_else(|| entry.get("id"))? {
        Value::String(id) => (id.as_str(), None),
        other => (other.get("id")?.as_str()?, other.get("count").and_then(Value::as_u64)),
    };
    let count = entry.get("count").and_then(Value::as_u64).or(nested).unwrap_or(1).clamp(1, 64) as u32;
    Some(ItemStack::new(id, count))
}

fn remainder_of(game: &Game<'_>, stack: &ItemStack) -> Option<String> {
    if let Some((_, rest)) = REMAINDER_OVERRIDES.iter().find(|(item, _)| *item == &*stack.item) {
        return Some((*rest).to_owned());
    }
    game.res.items().get(&stack.item).and_then(|info| info.remainder.clone())
}

/// The recipe whose ingredients are exactly the stocked input slots (`CookingPotRecipe.matches`
/// with `RecipeWrapper`: FD matches the non-empty slots one-to-one, so no extras and no missing).
fn find_recipe(game: &Game<'_>, slots: &[Option<ItemStack>]) -> Option<Cooking> {
    let stacks: Vec<&ItemStack> = slots.iter().take(INPUTS).flatten().collect();
    if stacks.is_empty() {
        return None;
    }
    game.native_recipes_of_type(RECIPES).iter().find_map(|recipe| {
        let json = &*recipe.json;
        let list = json["ingredients"].as_array()?;
        if list.len() != stacks.len() {
            return None;
        }
        let ingredients: Vec<&Value> = list.iter().collect();
        let mut used = vec![false; ingredients.len()];
        if !assign(game, &ingredients, &stacks, &mut used, 0) {
            return None;
        }
        let result = result_of(json)?;
        let container = json["container"]
            .get("item")
            .and_then(Value::as_str)
            .or_else(|| json["container"].as_str())
            .map(str::to_owned)
            .or_else(|| remainder_of(game, &result));
        let time = json["cookingtime"].as_i64().unwrap_or(200).clamp(1, i64::from(i32::MAX)) as i32;
        Some(Cooking { time, result, container })
    })
}

/// `CookingPotBlockEntity.canCook`: room in the meal slot for the result.
fn can_cook(game: &Game<'_>, meal: &Option<ItemStack>, result: &ItemStack) -> bool {
    match meal {
        None => true,
        Some(meal) => {
            let max = game.native_max_stack_size(&result.item).unwrap_or(64);
            meal.same_item_same_components(result) && meal.count + result.count <= max
        }
    }
}

/// `getContainer`: the container stored when the meal was made, else the meal item's remainder.
fn container_of(game: &Game<'_>, nbt: &Compound, meal: &ItemStack) -> Option<String> {
    nbt.get(&"Container".into())
        .and_then(item_from_nbt)
        .map(|stack| stack.item.to_string())
        .or_else(|| remainder_of(game, meal))
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

/// A horizontal facing as a step.
fn step(facing: &str) -> [i32; 2] {
    match facing {
        "north" => [0, -1],
        "south" => [0, 1],
        "west" => [-1, 0],
        _ => [1, 0],
    }
}

fn max_stack(game: &Game<'_>, stack: &ItemStack) -> u32 {
    game.native_max_stack_size(&stack.item).unwrap_or(64)
}

/// `moveMealToOutput` and `useStoredContainersOnMeal`: the meal in slot 6 goes to the output
/// slot, as many servings as the output has room for; a meal with a container also needs that
/// container in slot 7, one used up per serving. `container` is the meal's container (None: it
/// needs none). Returns (slots changed, the meal is gone so its stored container goes too).
fn flow_meal(game: &Game<'_>, slots: &mut Slots, container: Option<&str>) -> (bool, bool) {
    let Some(meal) = slots[MEAL].clone() else {
        return (false, false);
    };
    let max = max_stack(game, &meal);
    let have = match &slots[OUTPUT] {
        Some(out) if !out.same_item_same_components(&meal) || out.count >= max => return (false, false),
        Some(out) => out.count,
        None => 0,
    };
    let room = max - have;
    let moved = match container {
        None => meal.count.min(room),
        Some(id) => match slots[CONTAINER].as_ref().filter(|held| &*held.item == id) {
            Some(held) => meal.count.min(held.count).min(room),
            None => return (false, false),
        },
    };
    if moved == 0 {
        return (false, false);
    }
    if container.is_some() {
        if let Some(held) = &mut slots[CONTAINER] {
            held.count -= moved;
            if held.count == 0 {
                slots[CONTAINER] = None;
            }
        }
    }
    match &mut slots[OUTPUT] {
        Some(out) => out.count += moved,
        empty => {
            let mut served = meal.clone();
            served.count = moved;
            *empty = Some(served);
        }
    }
    let left = meal.count - moved;
    slots[MEAL] = (left > 0).then(|| {
        let mut rest = meal;
        rest.count = left;
        rest
    });
    (true, left == 0)
}

fn put_int(entity: &mut ModBlockEntity, key: &str, value: i32) {
    if int(&entity.nbt, key) != value {
        set_int(&mut entity.nbt, key, value);
    }
}

/// `CookingPotBlockEntity.cookingTick`: heated with a recipe that fits, the cook time rises by 1
/// and at the recipe's time one of each ingredient is used up (remainders are thrown out in
/// front of the pot) and the result joins the meal slot; otherwise the progress falls by 2.
/// Then the meal moves on to the output slot (see `flow_meal`). The cook time, its total and the
/// heat are mirrored into the menu's data slots.
pub(super) fn tick(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity) {
    if entity.id != BLOCK || (game.serving.is_none() && game.options.dedicated.is_none()) {
        return;
    }
    migrate(entity);
    let before = slots_of(entity);
    let mut slots = before.clone();
    let mut time = int(&entity.nbt, "CookTime");
    let mut total = int(&entity.nbt, "CookTimeTotal");
    let is_heated = heated(game, pos);
    let mut cooking = false;
    let mut remainders = Vec::new();
    // Some(x): the stored container becomes x this tick.
    let mut stored: Option<Option<String>> = None;
    if is_heated && slots.iter().take(INPUTS).any(Option::is_some) {
        if let Some(recipe) = find_recipe(game, &slots) {
            if can_cook(game, &slots[MEAL], &recipe.result) {
                cooking = true;
                total = recipe.time;
                time += 1;
                if time >= total {
                    time = 0;
                    for index in 0..INPUTS {
                        let Some(mut stack) = slots[index].take() else { continue };
                        if let Some(rest) = remainder_of(game, &stack) {
                            remainders.push(ItemStack::new(&rest, 1));
                        }
                        stack.count -= 1;
                        if stack.count > 0 {
                            slots[index] = Some(stack);
                        }
                    }
                    match &mut slots[MEAL] {
                        Some(meal) => meal.count += recipe.result.count,
                        empty => *empty = Some(recipe.result.clone()),
                    }
                    stored = Some(recipe.container.clone());
                }
            }
        }
    }
    if !cooking && time > 0 {
        time = (time - 2).clamp(0, total.max(0));
    }
    if let Some(meal) = slots[MEAL].clone() {
        let container = match &stored {
            Some(container) => container.clone(),
            None => container_of(game, &entity.nbt, &meal),
        };
        let (_, gone) = flow_meal(game, &mut slots, container.as_deref());
        if gone {
            stored = Some(None);
        }
    }
    let meal_changed = before[MEAL] != slots[MEAL];
    if slots != before {
        entity.items = slots;
    }
    if let Some(container) = stored {
        set_container(&mut entity.nbt, container.as_deref());
    }
    put_int(entity, "CookTime", time);
    if cooking {
        put_int(entity, "CookTimeTotal", total);
    }
    let data = [time, int(&entity.nbt, "CookTimeTotal"), i32::from(is_heated)];
    if entity.data[..] != data {
        entity.data = data.to_vec();
    }
    if !remainders.is_empty() {
        let facing = game.native_block_property(pos, "facing").unwrap_or_else(|| "north".into());
        let [dx, dz] = step(&facing);
        for rest in remainders {
            game.spawn_item(
                rest,
                [
                    f64::from(pos[0]) + 0.5 + f64::from(dx) * 0.25,
                    f64::from(pos[1]) + 0.7,
                    f64::from(pos[2]) + 0.5 + f64::from(dz) * 0.25,
                ],
                [f64::from(dx) * 0.08, 0.25, f64::from(dz) * 0.08],
                10,
            );
        }
    }
    if meal_changed {
        game.native_block_entity_sync(pos);
    }
}

// ------------------------------------------------------------ interaction

fn hand_over(game: &mut Game<'_>, stack: ItemStack, at: [f64; 3]) {
    let uuid = game.native_acting_id();
    if let Some(rest) = game.native_give_stack(uuid, stack) {
        game.spawn_item(rest, [at[0], at[1] + 0.3, at[2]], [0.0, 0.1, 0.0], 10);
    }
}

/// `useHeldItemOnMeal`: one portion of the meal leaves the pot for the player and the container
/// in hand is used up.
fn serve(game: &mut Game<'_>, pos: Pos) -> bool {
    let taken = game
        .native_with_mod_block_entity(pos, |entity| {
            migrate(entity);
            let mut slots = slots_of(entity);
            let mut meal = slots[MEAL].take()?;
            let mut one = meal.clone();
            one.count = 1;
            meal.count -= 1;
            if meal.count > 0 {
                slots[MEAL] = Some(meal);
            } else {
                set_container(&mut entity.nbt, None);
            }
            entity.items = slots;
            Some(one)
        })
        .flatten();
    let Some(portion) = taken else {
        return false;
    };
    game.consume_held();
    let at = pos.map(|c| f64::from(c) + 0.5);
    hand_over(game, portion, at);
    // The take-portion sound is a stand-in (FD's `ModSounds` event was not read).
    game.native_play_sound_in_place("minecraft:item.armor.equip_generic", at, 4, 1.0, 1.0);
    game.native_block_entity_sync(pos);
    true
}

/// The pot's right click. True only for the shortcut: a meal in the meal slot and the container
/// it needs in hand serve one portion (`CookingPotBlock.use`). Every other click (empty hand,
/// other items, no meal) is NOT consumed, so the engine opens the pot's menu. Sneaking with an
/// item in hand is a placement, as vanilla does not call `use` then.
pub(super) fn right_click(game: &mut Game<'_>, pos: Pos) -> bool {
    if game.native_block(pos) != BLOCK || game.held_slot() != game.selected || !game.may_build() {
        return false;
    }
    let Some(held) = game.held().filter(|stack| stack.count > 0).cloned() else {
        return false;
    };
    if game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft) {
        return false;
    }
    let Some((nbt, slots)) = snapshot(game, pos) else {
        return false;
    };
    let Some(meal) = &slots[MEAL] else {
        return false;
    };
    match container_of(game, &nbt, meal) {
        Some(container) => &*held.item == container.as_str() && serve(game, pos),
        None => false,
    }
}

/// `CookingPotBlock.onRemove`: the engine drops the container's item slots when the block breaks
/// (FD leaves the meal display slot out of its drops; this drops it too, as the pot item does not
/// carry the meal here). Left to do: an unmigrated pre-menu `Inventory` moves into the slots first.
pub(super) fn destroyed(game: &mut Game<'_>, pos: Pos) {
    if game.native_block(pos) != BLOCK {
        return;
    }
    game.native_with_mod_block_entity(pos, |entity| {
        migrate(entity);
    });
}

// --------------------------------------------------------------- placement

/// `Direction.fromYRot(yaw)`, the way the placer looks.
fn facing_name(yaw: f32) -> &'static str {
    ["south", "west", "north", "east"][((f64::from(yaw) / 90.0 + 0.5).floor() as i64 & 3) as usize]
}

fn opposite(facing: &str) -> &'static str {
    match facing {
        "south" => "north",
        "west" => "east",
        "north" => "south",
        _ => "west",
    }
}

fn on_tray(game: &GameLevel<'_>, below: BlockStateId) -> bool {
    game.registry
        .state(below)
        .is_some_and(|state| TRAY_HEAT_SOURCES.iter().any(|name| *name == &*state.name))
}

fn support_name(tray: bool) -> &'static str {
    if tray { "tray" } else { "none" }
}

/// `CookingPotBlock.getStateForPlacement`: facing the placer (the opposite of the way they look),
/// `handle` when placed against a ceiling, `tray` when a tray heat source lies below.
/// (Waterlogging from the clicked fluid is not done.)
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
    let support = if ctx.face == Direction::Down { "handle" } else { support_name(on_tray(game, below)) };
    *answer = Some(
        game.native_state_with_property(state, "facing", opposite(facing_name(ctx.yaw)))
            .and_then(|state| game.native_state_with_property(state, "support", support)),
    );
}

/// `CookingPotBlock.updateShape`: a change below sets `support` again unless it is `handle`.
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
    let current = block.properties.get("support").map(|value| &**value);
    if current == Some("handle") {
        return;
    }
    let support = support_name(on_tray(game, neighbour));
    if current == Some(support) {
        return;
    }
    if let Some(next) = game.native_state_with_property(state, "support", support) {
        *new = Some(next);
    }
}

// ------------------------------------------------------------------- draw

/// The meal lies in the pot (a flat item; FD draws the pot's model only, so this is an addition).
pub(super) fn draw(game: &Game<'_>, _pos: Pos, entity: &ModBlockEntity, draw: &mut BlockEntityDraw) {
    let Some(stack) = shown_meal(entity) else {
        return;
    };
    draw.push();
    draw.translate([0.5, 0.6, 0.5]);
    draw.rotate_about([1.0, 0.0, 0.0], 90.0, [0.0; 3]);
    draw.scale([0.5, 0.5, 0.5]);
    game.native_draw_item(draw, &stack, DisplayContext::Fixed, None, 0);
    draw.pop();
}

/// The meal slot of the pot at `pos`, for acceptance tooling.
#[allow(dead_code, reason = "read helper for acceptance tooling")]
pub(super) fn meal(game: &Game<'_>, pos: Pos) -> Option<ItemStack> {
    snapshot(game, pos).and_then(|(_, mut slots)| slots[MEAL].take())
}

// ------------------------------------------------------------------ screen

/// `CookingPotMenu` / `CookingPotScreen` (1.20, as read from GitHub): 176x166 over
/// `textures/gui/cooking_pot.png`, ingredients 3 wide and 2 high from (30, 17) at 18 spacing, the
/// container at (92, 55), the output at (124, 55), the player's inventory from y 84. The meal
/// display slot (6) is left out of the menu (FD's slot can be neither filled nor emptied by hand,
/// which a mod menu type cannot say); the screen draws the meal at (124, 26) itself.
pub(super) fn screen(block: &str, screen: &mut Option<crate::mod_api::NativeScreen>) {
    if block != BLOCK || screen.is_some() {
        return;
    }
    let mut slots: Vec<Option<[f32; 2]>> = (0..INPUTS)
        .map(|index| Some([30.0 + 18.0 * (index % 3) as f32, 17.0 + 18.0 * (index / 3) as f32]))
        .collect();
    slots.push(None);
    slots.push(Some([92.0, 55.0]));
    slots.push(Some([124.0, 55.0]));
    *screen = Some(crate::mod_api::NativeScreen {
        width: 176.0,
        height: 166.0,
        slots,
        inventory_y: 84.0,
        inventory_x: 8.0,
        own_labels: true,
        own_background: true,
    });
}

/// The panel, the heat icon (`data[2]`), the progress arrow (`data[0]` of `data[1]`, 24 wide), the
/// meal on display, and the labels (the title at x 28 as FD sets `titleLabelX`; the inventory
/// label as `renderLabels`: y `imageHeight - 96 + 2`).
pub(super) fn screen_draw(
    game: &Game<'_>,
    block: &str,
    data: &[i32],
    batch: &mut horizon_client::hud::HudBatch,
    origin: [f32; 2],
) {
    if block != BLOCK {
        return;
    }
    let [x, y] = origin;
    let get = |index: usize| data.get(index).copied().unwrap_or(0);
    game.native_sprite(batch, GUI_TEXTURE, [0.0, 0.0, 176.0, 166.0], [x, y, 176.0, 166.0]);
    if get(2) > 0 {
        game.native_sprite(batch, GUI_TEXTURE, [176.0, 0.0, 17.0, 15.0], [x + 47.0, y + 55.0, 17.0, 15.0]);
    }
    let (time, total) = (get(0), get(1));
    let scaled = if total != 0 && time != 0 { time * 24 / total } else { 0 };
    let width = (scaled + 1) as f32;
    game.native_sprite(batch, GUI_TEXTURE, [176.0, 15.0, width, 17.0], [x + 89.0, y + 25.0, width, 17.0]);
    if let Some(at) = game.native_screen_pos() {
        if let Some(BlockEntity::Mod(entity)) = game.world.block_entities.get(&(at[0], at[1], at[2])) {
            if let Some(meal) = shown_meal(entity) {
                game.native_item(batch, &meal.item, [x + 124.0, y + 26.0], 16.0);
                if meal.count > 1 {
                    let text = meal.count.to_string();
                    let at = [x + 124.0 + 17.0 - 6.0 * text.len() as f32, y + 26.0 + 9.0];
                    game.native_text(batch, &text, at, 0xFF_FF_FF, true);
                }
            }
        }
    }
    let title = game.native_screen_title().unwrap_or_else(|| "Cooking Pot".into());
    game.native_text(batch, &title, [x + 28.0, y + 6.0], 0x40_40_40, false);
    game.native_text(batch, "Inventory", [x + 8.0, y + 72.0], 0x40_40_40, false);
}

/// The heat icon's tooltip (FD: `cooking_pot.heated` / `cooking_pot.not_heated`; the English text
/// is unverified).
pub(super) fn screen_tooltip(block: &str, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    if block == BLOCK && (47.0..64.0).contains(&at[0]) && (55.0..70.0).contains(&at[1]) {
        let heated = data.get(2).copied().unwrap_or(0) > 0;
        lines.push(if heated { "Heated" } else { "Not heated" }.into());
    }
}
