// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Pinned Create 6.0.10 BeltFunnelBlock/FunnelBlockEntity adapter.
// Ordinary chest/barrel IO, dry horizontal belts; no external handler recursion.
use super::{Game, GameLevel, create_port_belt as belt, create_port_chute::funnel};
use crate::{containers::ModBlockEntity, items::ItemStack};
use create_engine::{
    funnel::Shape,
    kinetics::Pos,
    placement::Direction,
    transport::{Belt, Transported},
};
use horizon_world::{
    level::{PlacementContext, flags},
    nbt::Tag,
    registry::BlockStateId,
};

pub(super) const BLOCK: &str = "create:andesite_belt_funnel";
const PARENT: &str = "create:andesite_funnel";
fn flag(game: &Game<'_>, pos: Pos, key: &str) -> bool {
    game.native_block_property(pos, key).as_deref() == Some("true")
}
fn face(game: &Game<'_>, pos: Pos) -> Option<Direction> {
    let value = game.native_block_property(pos, "facing")?;
    Direction::ALL
        .into_iter()
        .find(|v| v.name() == value && v.vector()[1] == 0)
}
fn shape(game: &Game<'_>, pos: Pos) -> Option<Shape> {
    Shape::parse(&game.native_block_property(pos, "shape")?)
}
fn behind(pos: Pos, face: Direction) -> Pos {
    let d = face.vector();
    std::array::from_fn(|i| pos[i] - d[i])
}
fn ordinary(game: &Game<'_>, pos: Pos) -> bool {
    game.world.loaded(pos[0], pos[2])
        && matches!(
            game.native_block(pos).as_str(),
            "minecraft:chest"
                | "minecraft:trapped_chest"
                | "minecraft:barrel"
                | "create:item_vault"
        )
}
fn side(face: Direction) -> horizon_core::geometry::Direction {
    belt::world_direction(face)
}
/// The belt form this funnel takes when placed on a supported Belt. Horizon
/// accepts a placement answer only for the item's own block, so callers use the
/// result as a support check; `world_tick` performs the actual conversion.
pub(super) fn placement(
    game: &GameLevel<'_>,
    ctx: &PlacementContext,
    facing: Direction,
) -> Option<BlockStateId> {
    let below = game
        .registry
        .state(game.world.block(ctx.pos.x, ctx.pos.y - 1, ctx.pos.z))?;
    if below.name != "create:belt"
        || below.properties.get("slope").map(String::as_str) != Some("horizontal")
        || below.properties.get("waterlogged").map(String::as_str) != Some("false")
    {
        return None;
    }
    let belt_facing = Direction::ALL.into_iter().find(|d| {
        below
            .properties
            .get("facing")
            .is_some_and(|v| v == d.name())
    })?;
    let shape = Shape::for_belt(belt_facing, facing, !ctx.sneaking)?;
    let state = game.registry.default_state(BLOCK)?;
    let state = game.native_state_with_property(state, "facing", facing.name())?;
    game.native_state_with_property(state, "shape", shape.name())
}
pub(super) fn wrench(game: &mut Game<'_>, pos: Pos) -> bool {
    let Some(shape) = shape(game, pos) else {
        return false;
    };
    game.native_set_block_property(pos, "shape", shape.wrenched().name())
}
fn transfer_sound(game: &mut Game<'_>, pos: Pos) {
    game.native_play_sound_in_place(
        "create:funnel_flap",
        pos.map(|v| f64::from(v) + 0.5),
        4,
        1.0,
        1.0,
    );
}
pub(super) fn insert(game: &mut Game<'_>, pos: Pos, facing: Direction, stack: &ItemStack) -> u32 {
    let target = behind(pos, facing);
    if !ordinary(game, target) {
        return 0;
    }
    let remainder = game.native_insert_item(target, side(facing), stack.clone());
    let moved = stack
        .count
        .saturating_sub(remainder.as_ref().map_or(0, |s| s.count));
    if moved > 0 {
        transfer_sound(game, pos)
    }
    moved
}
pub(super) fn interact(
    game: &mut Game<'_>,
    origin: Pos,
    item: &mut Transported<ItemStack>,
    next: f32,
    belt: Belt,
) -> Option<f32> {
    let positive = belt.speed() > 0.0;
    let movement = belt.direction();
    for segment in create_engine::funnel::segments(item.position, next, belt.length, positive) {
        let d = belt.facing.vector();
        let pos = [
            origin[0] + d[0] * segment,
            origin[1] + 1,
            origin[2] + d[2] * segment,
        ];
        if game.native_block(pos) != BLOCK || flag(game, pos, "waterlogged") {
            continue;
        }
        let (Some(facing), Some(shape)) = (face(game, pos), shape(game, pos)) else {
            continue;
        };
        let Some(blocking) = create_engine::funnel::taking(facing, movement, shape) else {
            continue;
        };
        let entry = create_engine::funnel::entry(segment, shape, positive);
        if !create_engine::funnel::crosses(next, entry, positive) {
            continue;
        }
        let powered = funnel::refresh_power(game, pos);
        if !powered {
            let moved = insert(game, pos, facing, &belt::game_stack(&item.stack));
            item.stack.count -= moved;
        }
        if item.stack.count == 0 {
            return None;
        }
        if blocking {
            return Some(entry);
        }
    }
    None
}
fn push(game: &mut Game<'_>, pos: Pos, facing: Direction) -> bool {
    let source = behind(pos, facing);
    if !ordinary(game, source) {
        return false;
    }
    let Some(slots) = game.native_container_slots(game.dim.id(), source, side(facing)) else {
        return false;
    };
    let Some(slot) = slots
        .iter()
        .find(|s| s.can_take && s.stack.as_ref().is_some_and(|v| v.count > 0))
    else {
        return false;
    };
    let mut preview = slot.stack.as_ref().unwrap().clone();
    preview.count = 1;
    let target = [pos[0], pos[1] - 1, pos[2]];
    if belt::insert_from_funnel(game, target, &preview, facing, true) != 1 {
        return false;
    }
    let Some(stack) = game.native_extract_item(game.dim.id(), source, side(facing), slot.slot, 1)
    else {
        return false;
    };
    if belt::insert_from_funnel(game, target, &stack, facing, false) != stack.count {
        // Supported inventories are ordinary containers: extraction frees the
        // exact capacity needed to restore this stack, with no intervening tick.
        let remainder = game.native_insert_item(source, side(facing), stack);
        if remainder.is_some() {
            log::error!("Create Funnel: ordinary source restoration failed");
        }
        return false;
    }
    transfer_sound(game, pos);
    true
}
// `Game` has no property-setting state builder (only `GameLevel` does), so the
// state is resolved from the registry descriptor, as belt.rs does.
fn state_of(game: &Game<'_>, block: &str, properties: &[(&str, &str)]) -> Option<BlockStateId> {
    let registry = game.pack.assets.registry;
    let mut descriptor = registry.state(registry.default_state(block)?)?.clone();
    for (key, value) in properties {
        descriptor.properties.get(*key)?;
        descriptor
            .properties
            .insert((*key).to_owned(), (*value).to_owned());
    }
    registry.id(&descriptor)
}
fn parent_state(
    game: &Game<'_>,
    pos: Pos,
    facing: Direction,
    extracting: bool,
) -> Option<BlockStateId> {
    state_of(
        game,
        PARENT,
        &[
            ("facing", facing.name()),
            ("powered", if flag(game, pos, "powered") { "true" } else { "false" }),
            ("extracting", if extracting { "true" } else { "false" }),
        ],
    )
}
pub(super) fn tick(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity) {
    if entity.id != funnel::ENTITY || flag(game, pos, "waterlogged") {
        return;
    }
    let (Some(facing), Some(shape)) = (face(game, pos), shape(game, pos)) else {
        return;
    };
    let target = [pos[0], pos[1] - 1, pos[2]];
    let Some(active_belt) = belt::funnel_belt(game, target) else {
        // Preserve the original parent transition, including all shared BE NBT.
        if game.world.loaded(pos[0], pos[2])
            && game.native_block(target) != "create:belt"
            && let Some(state) = parent_state(game, pos, facing, shape == Shape::Pushing)
        {
            game.native_set_block(game.dim.id(), pos, state, flags::UPDATE_ALL);
        }
        return;
    };
    let Some(Tag::Int(cooldown)) = entity.nbt.get(&"TransferCooldown".into()) else {
        return;
    };
    let cooldown = *cooldown;
    if cooldown < 0 {
        return;
    }
    let powered = funnel::refresh_power(game, pos);
    let taking = match shape {
        Shape::Pulling => true,
        Shape::Pushing => false,
        _ => active_belt.direction() != facing,
    };
    let next = if powered {
        0
    } else if taking {
        return;
    } else if cooldown > 0 {
        cooldown - 1
    } else if push(game, pos, facing) {
        8
    } else {
        0
    };
    if next != cooldown {
        entity.nbt.insert("TransferCooldown".into(), Tag::Int(next));
    }
}
// Horizontal parent forms are retained when a supporting belt is removed.
// They use actual dropped items and ordinary containers, never a hidden buffer.
pub(super) fn world_tick(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity) {
    let Some(facing) = face(game, pos) else {
        return;
    };
    if flag(game, pos, "waterlogged") {
        return;
    }
    let target = [pos[0], pos[1] - 1, pos[2]];
    if let Some(active_belt) = belt::funnel_belt(game, target) {
        let Some(shape) =
            Shape::for_belt(active_belt.facing, facing, flag(game, pos, "extracting"))
        else {
            return;
        };
        if let Some(state) = state_of(
            game,
            BLOCK,
            &[("facing", facing.name()), ("shape", shape.name())],
        ) {
            game.native_set_block(game.dim.id(), pos, state, flags::UPDATE_ALL);
        }
        return;
    }
    if funnel::refresh_power(game, pos) {
        entity.nbt.insert("TransferCooldown".into(), Tag::Int(0));
        return;
    }
    let source = behind(pos, facing);
    if !ordinary(game, source) {
        return;
    }
    if flag(game, pos, "extracting") {
        let Some(&Tag::Int(cooldown)) = entity.nbt.get(&"TransferCooldown".into()) else {
            return;
        };
        if cooldown < 0 {
            return;
        }
        if cooldown > 0 {
            entity
                .nbt
                .insert("TransferCooldown".into(), Tag::Int(cooldown - 1));
            return;
        }
        let min = [pos[0] as f64, pos[1] as f64 - 1.0, pos[2] as f64];
        let max = pos.map(|v| v as f64 + 1.0);
        if !game
            .native_item_entities(game.dim.id(), min, max)
            .is_empty()
        {
            return;
        }
        let Some(slots) = game.native_container_slots(game.dim.id(), source, side(facing)) else {
            return;
        };
        let Some(slot) = slots
            .iter()
            .find(|s| s.can_take && s.stack.as_ref().is_some_and(|s| s.count > 0))
        else {
            return;
        };
        if let Some(stack) =
            game.native_extract_item(game.dim.id(), source, side(facing), slot.slot, 1)
        {
            let d = facing.vector();
            let spawn = [
                pos[0] as f64 + 0.5 + d[0] as f64 * 0.25,
                pos[1] as f64 + 0.05,
                pos[2] as f64 + 0.5 + d[2] as f64 * 0.25,
            ];
            game.spawn_item(stack, spawn, [0.0; 3], 10);
            entity.nbt.insert("TransferCooldown".into(), Tag::Int(8));
        }
    } else {
        let min = pos.map(f64::from);
        let max = min.map(|v| v + 1.0);
        let dim = game.dim.id().to_owned();
        for dropped in game.native_item_entities(&dim, min, max) {
            let axis = if facing.vector()[0] != 0 { 0 } else { 2 };
            let open = pos[axis] as f64 + 0.5 - facing.vector()[axis] as f64 * 0.25;
            if (dropped.position[axis] - open < 0.0) == (facing.vector()[axis] > 0) {
                continue;
            }
            let moved = insert(game, pos, facing, &dropped.stack);
            if moved > 0 {
                let rest = ItemStack {
                    count: dropped.stack.count - moved,
                    ..dropped.stack
                };
                game.native_set_item_entity(&dim, dropped.id, rest);
            }
        }
    }
}
