// Copyright (c) vectorwing / Farmer's Delight contributors (MIT)
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Farmer's Delight Stove (`StoveBlock.use`, `StoveBlockEntity.cookTick`). The block
// comes from the content mod (template `minecraft:furnace`: `facing`, `lit`); this
// module gives it its block entity (six food slots), placing food by right click,
// lighting and extinguishing, cooking while lit, the cooled-down progress while
// unlit, the drop on break and the food drawn on top. Written from the 2.x
// behaviour as remembered, not diffed against the Java source (see
// docs/farmersdelight-stove.md for what is unverified).
use super::{Game, native_block_entity_draw::BlockEntityDraw};
use crate::{containers::ModBlockEntity, items::ItemStack};
use horizon_assets::item_model::DisplayContext;
use horizon_items::item_nbt::{item_from_nbt, item_to_nbt};
use horizon_world::nbt::{Compound, Tag};

pub(super) const BLOCK: &str = "farmersdelight:stove";
const RECIPES: &str = "minecraft:campfire_cooking";
const SLOTS: usize = 6;
const ITEM_KEYS: [&str; SLOTS] = ["Item0", "Item1", "Item2", "Item3", "Item4", "Item5"];
const TIMES: &str = "CookingTimes";
const TOTALS: &str = "CookingTotalTimes";

type Pos = [i32; 3];

// ------------------------------------------------------------------- state

pub(super) fn entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if block == BLOCK && entity.is_none() {
        *entity = Some((block.into(), Compound::new()));
    }
}

/// `StoveBlockEntity`'s inventory and the per-slot cooking progress.
struct Slots {
    items: [Option<ItemStack>; SLOTS],
    times: [i32; SLOTS],
    totals: [i32; SLOTS],
}

impl Slots {
    fn load(nbt: &Compound) -> Self {
        let array = |key: &str| -> [i32; SLOTS] {
            let mut out = [0; SLOTS];
            if let Some(Tag::IntArray(values)) = nbt.get(&key.into()) {
                for (slot, value) in out.iter_mut().zip(values) {
                    *slot = *value;
                }
            }
            out
        };
        Self {
            items: std::array::from_fn(|i| nbt.get(&ITEM_KEYS[i].into()).and_then(item_from_nbt)),
            times: array(TIMES),
            totals: array(TOTALS),
        }
    }

    fn store(&self, nbt: &mut Compound) {
        for (key, item) in ITEM_KEYS.iter().zip(&self.items) {
            match item {
                Some(stack) => {
                    nbt.insert((*key).into(), item_to_nbt(stack));
                }
                None => {
                    nbt.remove(&(*key).into());
                }
            }
        }
        nbt.insert(TIMES.into(), Tag::IntArray(self.times.to_vec()));
        nbt.insert(TOTALS.into(), Tag::IntArray(self.totals.to_vec()));
    }

    /// `StoveBlockEntity.getNextEmptySlot`.
    fn free(&self) -> Option<usize> {
        self.items.iter().position(Option::is_none)
    }

    fn idle(&self) -> bool {
        self.items.iter().all(Option::is_none) && self.times.iter().all(|t| *t == 0)
    }

    fn clear(&mut self, slot: usize) {
        self.items[slot] = None;
        self.times[slot] = 0;
        self.totals[slot] = 0;
    }
}

/// Only the food goes to clients (the cooking progress stays on the server).
pub(super) fn update_tag(entity: &ModBlockEntity) -> Compound {
    ITEM_KEYS
        .iter()
        .filter_map(|key| {
            entity
                .nbt
                .get(&(*key).into())
                .map(|tag| ((*key).into(), tag.clone()))
        })
        .collect()
}

pub(super) fn ticks(block: &str, ticks: &mut bool) {
    if block == BLOCK {
        *ticks = true;
    }
}

// ----------------------------------------------------------------- helpers

fn lit(game: &Game<'_>, pos: Pos) -> bool {
    game.native_block_property(pos, "lit").as_deref() == Some("true")
}

/// `Direction.getClockWise` of `facing` as an (x, z) step.
fn clockwise(facing: &str) -> [i32; 2] {
    match facing {
        "north" => [1, 0],
        "east" => [0, 1],
        "south" => [-1, 0],
        _ => [0, -1],
    }
}

/// `StoveBlockEntity.isStoveBlockedAbove`: the block above has a full bottom face
/// (checked on single collision boxes).
fn blocked_above(game: &Game<'_>, pos: Pos) -> bool {
    const E: f64 = 1.0e-6;
    game.native_block_collision([pos[0], pos[1] + 1, pos[2]])
        .iter()
        .any(|b| b[0] <= E && b[1] <= E && b[2] <= E && b[3] >= 1.0 - E && b[5] >= 1.0 - E)
}

fn recipe_time(game: &Game<'_>, stack: &ItemStack) -> Option<i32> {
    game.recipes().cook(RECIPES, stack).map(|recipe| recipe.time as i32)
}

fn spawn_at_center(game: &mut Game<'_>, pos: Pos, stack: ItemStack) {
    game.spawn_item(
        stack,
        [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 1.0, f64::from(pos[2]) + 0.5],
        [0.0, 0.1, 0.0],
        10,
    );
}

fn center(pos: Pos) -> [f64; 3] {
    pos.map(|c| f64::from(c) + 0.5)
}

// ------------------------------------------------------------------- tick

/// `StoveBlockEntity.cookTick`, every tick: blocked above the stove drops its food;
/// lit it cooks one tick per slot; unlit it cools the progress by 2 per tick.
pub(super) fn tick(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity) {
    if entity.id != BLOCK {
        return;
    }
    let mut slots = Slots::load(&entity.nbt);
    if slots.idle() {
        return;
    }
    if blocked_above(game, pos) {
        for slot in 0..SLOTS {
            if let Some(stack) = slots.items[slot].take() {
                spawn_at_center(game, pos, stack);
            }
            slots.clear(slot);
        }
    } else if lit(game, pos) {
        let facing = game
            .native_block_property(pos, "facing")
            .unwrap_or_else(|| "north".into());
        let [dx, dz] = clockwise(&facing);
        for slot in 0..SLOTS {
            let Some(stack) = slots.items[slot].clone() else {
                continue;
            };
            if slots.totals[slot] <= 0 {
                // Saved without a total (or from an older build): ask the recipe again.
                slots.totals[slot] = recipe_time(game, &stack).unwrap_or(600);
            }
            slots.times[slot] += 1;
            if slots.times[slot] < slots.totals[slot] {
                continue;
            }
            let result = game
                .recipes()
                .cook(RECIPES, &stack)
                .map(|recipe| recipe.result.clone())
                .unwrap_or(stack);
            game.spawn_item(
                result,
                [
                    f64::from(pos[0]) + 0.5 + f64::from(dx) * 0.25,
                    f64::from(pos[1]) + 1.0,
                    f64::from(pos[2]) + 0.5 + f64::from(dz) * 0.25,
                ],
                [f64::from(dx) * 0.08, 0.25, f64::from(dz) * 0.08],
                10,
            );
            slots.clear(slot);
        }
    } else {
        for slot in 0..SLOTS {
            if slots.times[slot] > 0 {
                slots.times[slot] = (slots.times[slot] - 2).clamp(0, slots.totals[slot].max(0));
            }
        }
    }
    let changed = slots
        .items
        .iter()
        .zip(Slots::load(&entity.nbt).items.iter())
        .any(|(now, before)| now.is_some() != before.is_some());
    slots.store(&mut entity.nbt);
    if changed {
        game.native_block_entity_sync(pos);
    }
}

// ------------------------------------------------------------ interaction

fn ignite(game: &mut Game<'_>, pos: Pos, sound: &str, pitch: f32) -> bool {
    game.native_play_sound_in_place(sound, center(pos), 4, 1.0, pitch);
    game.native_set_block_property(pos, "lit", "true");
    true
}

fn extinguish(game: &mut Game<'_>, pos: Pos) {
    game.native_play_sound_in_place("minecraft:block.fire.extinguish", center(pos), 4, 1.0, 1.0);
    game.native_set_block_property(pos, "lit", "false");
}

/// Right click on a Stove. True when the click was used.
pub(super) fn right_click(game: &mut Game<'_>, pos: Pos) -> bool {
    if game.native_block(pos) != BLOCK
        || game.held_slot() != game.selected
        || !game.may_build()
        || game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft)
    {
        return false;
    }
    let Some(held) = game.held().cloned() else {
        return false;
    };
    let id: &str = &held.item;
    // `StoveBlock.use`: a lit stove goes out for a shovel or a water bucket,
    // an unlit one lights for flint and steel or a fire charge.
    if lit(game, pos) {
        if id.ends_with("_shovel") {
            extinguish(game, pos);
            game.damage_held(1);
            return true;
        }
        if id == "minecraft:water_bucket" {
            extinguish(game, pos);
            if !game.creative {
                *game.held_mut() = Some(ItemStack::new("minecraft:bucket", 1));
            }
            return true;
        }
    } else {
        // Vanilla's pitch is `random * 0.4 + 0.8`; the game time stands in for the random.
        let wobble = (game.world.game_time % 5) as f32 * 0.1;
        if id == "minecraft:flint_and_steel" {
            ignite(game, pos, "minecraft:item.flintandsteel.use", 0.8 + wobble);
            game.damage_held(1);
            return true;
        }
        if id == "minecraft:fire_charge" {
            ignite(game, pos, "minecraft:item.firecharge.use", 0.9 + wobble * 0.5);
            game.consume_held();
            return true;
        }
    }
    // Food with a campfire cooking recipe goes into the first free slot.
    let Some(time) = recipe_time(game, &held) else {
        return false;
    };
    let mut one = held;
    one.count = 1;
    let placed = game
        .native_with_mod_block_entity(pos, |entity| {
            let mut slots = Slots::load(&entity.nbt);
            let Some(slot) = slots.free() else {
                return false;
            };
            slots.items[slot] = Some(one);
            slots.times[slot] = 0;
            slots.totals[slot] = time;
            slots.store(&mut entity.nbt);
            true
        })
        .unwrap_or(false);
    if !placed {
        return false;
    }
    game.consume_held();
    game.native_block_entity_sync(pos);
    true
}

/// The stove drops its food when the block breaks.
pub(super) fn destroyed(game: &mut Game<'_>, pos: Pos) {
    if game.native_block(pos) != BLOCK {
        return;
    }
    let drops = game
        .native_with_mod_block_entity(pos, |entity| {
            let mut slots = Slots::load(&entity.nbt);
            let drops: Vec<ItemStack> = slots.items.iter_mut().filter_map(Option::take).collect();
            for slot in 0..SLOTS {
                slots.clear(slot);
            }
            slots.store(&mut entity.nbt);
            drops
        })
        .unwrap_or_default();
    for stack in drops {
        game.spawn_item(
            stack,
            [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.5, f64::from(pos[2]) + 0.5],
            [0.0, 0.1, 0.0],
            10,
        );
    }
}

// ------------------------------------------------------------------- draw

/// `StoveBlockEntityRenderer`: two rows of three items on the top face. The
/// first row is the one toward the front, slot 0 at the player's right.
pub(super) fn draw(game: &Game<'_>, pos: Pos, entity: &ModBlockEntity, draw: &mut BlockEntityDraw) {
    let facing = game
        .native_block_property(pos, "facing")
        .unwrap_or_else(|| "north".into());
    // Front direction F in (x, z); the player in front looks against it, so their
    // right hand is R = (F.z, -F.x).
    let front: [f32; 2] = match facing.as_str() {
        "north" => [0.0, -1.0],
        "east" => [1.0, 0.0],
        "south" => [0.0, 1.0],
        _ => [-1.0, 0.0],
    };
    let right = [front[1], -front[0]];
    const X: f32 = 0.3;
    const Y: f32 = 0.2;
    const OFFSETS: [[f32; 2]; SLOTS] = [[X, Y], [0.0, Y], [-X, Y], [X, -Y], [0.0, -Y], [-X, -Y]];
    for (key, [across, along]) in ITEM_KEYS.iter().zip(OFFSETS) {
        let Some(stack) = entity.nbt.get(&(*key).into()).and_then(item_from_nbt) else {
            continue;
        };
        draw.push();
        draw.translate([
            0.5 + right[0] * across + front[0] * along,
            1.02,
            0.5 + right[1] * across + front[1] * along,
        ]);
        draw.rotate_about([1.0, 0.0, 0.0], 90.0, [0.0; 3]);
        draw.scale([0.375, 0.375, 0.375]);
        game.native_draw_item(draw, &stack, DisplayContext::Fixed, None, 0);
        draw.pop();
    }
}
