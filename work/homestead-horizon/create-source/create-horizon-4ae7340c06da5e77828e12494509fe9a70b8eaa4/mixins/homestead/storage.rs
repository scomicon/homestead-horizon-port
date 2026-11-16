// Copyright (c) vectorwing / Farmer's Delight contributors (MIT)
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Farmer's Delight baskets and cabinets (`BasketBlock`, `BasketBlockEntity`, `CabinetBlock`,
// `CabinetBlockEntity`). The blocks and their items come from the content mod (fd/stage23.py:
// barrel templates, so their use opens the block's menu); this module gives them
//
// * a block entity with a real container of 27 slots, opened by the vanilla 9x3 chest menu,
//   reachable from every face by hoppers and Create (the original's item handler capability),
// * the placement state: a basket faces the clicked face, a cabinet faces the placer
//   (`CabinetBlock.getStateForPlacement`: the horizontal direction, opposite),
// * the basket's pickup: dropped items in the cell in front of its opening are drawn into it,
//   one pass every `PERIOD` ticks, as a hopper pulls from above.
//
// Written from the behaviour as remembered from vectorwing/FarmersDelight 1.20; the Java was not
// read for this slice (see docs/fd-stage23.md for what is unverified or missing). The engine
// drops the container's items when the block is broken, so no destroy hook is needed.
use super::{Game, GameLevel};
use crate::{
    containers::{ModBlockEntity, ModContainer},
    items::ItemStack,
};
use horizon_core::geometry::Direction;
use horizon_world::{
    level::PlacementContext,
    nbt::{Compound, Tag},
    registry::BlockStateId,
};

pub(super) const BASKET: &str = "farmersdelight:basket";
/// The vanilla 9x3 chest menu: 27 slots.
const MENU: &str = "minecraft:generic_9x3";
const SLOTS: usize = 27;
/// Ticks between two pickup passes of a basket.
const PERIOD: i32 = 8;
const COOLDOWN: &str = "Cooldown";

type Pos = [i32; 3];

fn is_cabinet(block: &str) -> bool {
    block
        .strip_prefix("farmersdelight:")
        .is_some_and(|name| name.ends_with("_cabinet"))
}

fn is_storage(block: &str) -> bool {
    block == BASKET || is_cabinet(block)
}

// ------------------------------------------------------------------ block entity

pub(super) fn entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if is_storage(block) && entity.is_none() {
        *entity = Some((block.into(), Compound::new()));
    }
}

/// Only the basket ticks (the pickup); cabinets are passive.
pub(super) fn ticks(block: &str, ticks: &mut bool) {
    if block == BASKET {
        *ticks = true;
    }
}

/// `native_block_entity_container`: 27 slots, every face reaches all of them (hoppers, Create),
/// the 9x3 menu.
pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    if is_storage(block) && container.is_none() {
        *container = Some(ModContainer {
            slots: SLOTS,
            faces: std::array::from_fn(|_| (0..SLOTS).collect()),
            inputs: (0..SLOTS).collect(),
            outputs: (0..SLOTS).collect(),
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

// ------------------------------------------------------------------- pickup

fn vector(name: &str) -> [i32; 3] {
    match name {
        "down" => [0, -1, 0],
        "north" => [0, 0, -1],
        "south" => [0, 0, 1],
        "west" => [-1, 0, 0],
        "east" => [1, 0, 0],
        _ => [0, 1, 0],
    }
}

fn max_stack(game: &Game<'_>, stack: &ItemStack) -> u32 {
    game.native_max_stack_size(&stack.item).unwrap_or(64)
}

/// Moves as much of `stack` as fits into `slots`: first onto equal stacks, then into empty slots
/// (the item handler's insert). `stack.count` is what is left.
fn insert(max: u32, slots: &mut [Option<ItemStack>], stack: &mut ItemStack) {
    for slot in slots.iter_mut().flatten() {
        if stack.count == 0 {
            return;
        }
        if slot.same_item_same_components(stack) && slot.count < max {
            let moved = (max - slot.count).min(stack.count);
            slot.count += moved;
            stack.count -= moved;
        }
    }
    for slot in slots.iter_mut() {
        if stack.count == 0 {
            return;
        }
        if slot.is_none() {
            let moved = max.min(stack.count);
            let mut put = stack.clone();
            put.count = moved;
            *slot = Some(put);
            stack.count -= moved;
        }
    }
}

/// `block_entity_tick` of a basket: every `PERIOD` ticks, dropped items in the cell in front of
/// its opening move into it. A block entity under tick is not in the world's map, so the slots
/// are changed through `entity` itself.
pub(super) fn tick(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity) {
    if entity.id != BASKET || (game.serving.is_none() && game.options.dedicated.is_none()) {
        return;
    }
    let wait = int(&entity.nbt, COOLDOWN);
    if wait > 0 {
        entity.nbt.insert(COOLDOWN.into(), Tag::Int(wait - 1));
        return;
    }
    entity.nbt.insert(COOLDOWN.into(), Tag::Int(PERIOD));
    let facing = game
        .native_block_property(pos, "facing")
        .unwrap_or_else(|| "up".into());
    let [dx, dy, dz] = vector(&facing);
    let min = [pos[0] + dx, pos[1] + dy, pos[2] + dz].map(f64::from);
    let max = min.map(|c| c + 1.0);
    let dim = game.dim.id().to_owned();
    let dropped = game.native_item_entities(&dim, min, max);
    if dropped.is_empty() {
        return;
    }
    let mut slots: Vec<Option<ItemStack>> = entity.items.clone();
    slots.resize(SLOTS, None);
    let mut changed = false;
    for item in dropped {
        let mut left = item.stack.clone();
        let before = left.count;
        insert(max_stack(game, &left), &mut slots, &mut left);
        if left.count != before {
            changed = true;
            game.native_set_item_entity(&dim, item.id, left);
        }
    }
    if changed {
        entity.items = slots;
    }
}

// --------------------------------------------------------------- placement

/// The blockstate's name of a direction (`Direction` has no name of its own here).
fn face_name(face: Direction) -> &'static str {
    match face {
        Direction::Down => "down",
        Direction::Up => "up",
        Direction::North => "north",
        Direction::South => "south",
        Direction::West => "west",
        Direction::East => "east",
    }
}

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

/// `BasketBlock` faces the clicked face (its opening is there); `CabinetBlock.getStateForPlacement`
/// is the horizontal direction opposite to the placer's look, never up or down.
pub(super) fn placement(
    game: &GameLevel<'_>,
    state: BlockStateId,
    ctx: &PlacementContext,
    answer: &mut Option<Option<BlockStateId>>,
) {
    if answer.is_some() {
        return;
    }
    let Some(block) = game.native_state_block(state) else {
        return;
    };
    let facing = if block == BASKET {
        face_name(ctx.face)
    } else if is_cabinet(&block) {
        opposite(facing_name(ctx.yaw))
    } else {
        return;
    };
    *answer = Some(game.native_state_with_property(state, "facing", facing));
}
