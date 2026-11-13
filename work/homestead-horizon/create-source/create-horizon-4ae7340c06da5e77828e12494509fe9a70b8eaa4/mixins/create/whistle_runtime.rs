// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

// Steam Whistle (WhistleBlock / WhistleExtenderBlock / WhistleBlockEntity):
// placement on a Fluid Tank, the extender stack that sets the pitch, redstone
// power and the sounding note. The rules live in `create_engine::steam_whistle`;
// this file only adapts them to the game.
//
// Blocks
//   create:steam_whistle            facing (horizontal), wall, powered, size
//   create:steam_whistle_extension  shape (single|double|double_connected), size
//
// `wall = false`: the whistle stands on a tank (attached DOWN). `wall = true`:
// it hangs on the tank in direction `facing` (the base plate is on that side).
//
// Boiler contract (Fluid Tank, mod `basin.rs`, written by the boiler code): the
// tank block entity `create:fluid_tank` of the controller carries a `Boiler`
// compound with `Level` (Int, 0 = cold) and, optionally, `Whistles` (Int, the
// attached count). A member tank block points to its controller by the relative
// IntArray `Controller` (absent on the controller itself). The whistle never
// writes the tank; it only reads these keys.
//
// Sound. A server cannot loop or stop a sound, so the sustained note is replayed
// as one-shot sounds every `replay_period` ticks while the whistle sounds (the
// last replay may therefore ring on for up to three seconds after the power is
// cut). Events: `create:whistle_high` (small), `create:whistle` (medium),
// `create:whistle_low` (large), the chiff `create:steam`. With the player's
// Create JAR these are the original files; without it `assets/create/sounds.json`
// has to alias them (see docs/steam-whistle.md).
//
// Tracing: with `CREATE_HORIZON_TRACE` set, every played sound logs one line
// `Create whistle: {json}` (action, kind, event, pos, volume, pitch, size,
// pitch_id, level); the harness counts those lines.

use super::{Game, GameLevel, create_port_contraption as contraption};
use crate::{
    containers::{BlockEntity, ModBlockEntity},
    items::ItemStack,
};
use create_engine::{
    placement::Direction,
    steam_whistle::{self as rules, Below, ExtenderShape, Increment, Size, Slot},
};
use horizon_core::{geometry::Direction as Face, position::BlockPos};
use horizon_world::{
    level::{PlacementContext, flags},
    nbt::{Compound, Tag},
    registry::BlockStateId,
};

pub(super) const BLOCK: &str = "create:steam_whistle";
pub(super) const EXTENSION: &str = "create:steam_whistle_extension";
const TANK: &str = "create:fluid_tank";
type Pos = [i32; 3];

// Block entity NBT (all keys are the port's own; WhistleBlockEntity keeps its
// pitch and size in the block state / recomputes them).
/// Cached `updatePitch` result (derived from the stack above).
const PITCH: &str = "Pitch";
/// 1 while the note is sounding.
const SOUNDING: &str = "Sounding";
/// Ticks until the sustained note is replayed.
const TIMER: &str = "SoundTimer";

/// Length of the original whistle samples at pitch 1 (3.00 s, measured on
/// `assets/create/sounds/whistle*.ogg` of the Create JAR).
const SAMPLE_TICKS: i32 = 60;
/// Replay the note this many ticks early so the notes join.
const OVERLAP_TICKS: i32 = 2;
/// Gain of the sustained note (the original's instance gain was not read).
const NOTE_VOLUME: f32 = 1.0;
/// `SoundSource.RECORDS`.
const SOURCE_RECORDS: usize = 2;
/// Pitch changes are picked up this often while the whistle is silent.
const SCAN_PERIOD: i64 = 10;
/// Blocks above the whistle that `updatePitch` can reach (13 steps of 2 + the top).
const STACK_SCAN: i32 = 14;

// ------------------------------------------------------------------- NBT

fn int(nbt: &Compound, key: &str) -> Option<i32> {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => Some(*v),
        Some(Tag::Byte(v)) => Some(i32::from(*v)),
        _ => None,
    }
}

pub(super) fn default_nbt() -> Compound {
    Compound::from([
        (PITCH.into(), Tag::Int(0)),
        (SOUNDING.into(), Tag::Byte(0)),
        (TIMER.into(), Tag::Int(0)),
    ])
}

/// What clients receive (for a future renderer / goggles line).
pub(super) fn update_tag(entity: &ModBlockEntity) -> Compound {
    [PITCH, SOUNDING]
        .into_iter()
        .filter_map(|key| entity.nbt.get(&key.into()).map(|tag| (key.into(), tag.clone())))
        .collect()
}

// ---------------------------------------------------------------- states

fn simulates(game: &Game<'_>) -> bool {
    game.serving.is_some() || game.options.dedicated.is_some()
}

fn named(name: &str) -> Option<Direction> {
    Some(match name {
        "down" => Direction::Down,
        "up" => Direction::Up,
        "north" => Direction::North,
        "south" => Direction::South,
        "west" => Direction::West,
        "east" => Direction::East,
        _ => return None,
    })
}

fn engine(face: Face) -> Direction {
    match face {
        Face::Down => Direction::Down,
        Face::Up => Direction::Up,
        Face::North => Direction::North,
        Face::South => Direction::South,
        Face::West => Direction::West,
        Face::East => Direction::East,
    }
}

fn truth(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn shifted(pos: Pos, d: Direction) -> Pos {
    let v = d.vector();
    [pos[0] + v[0], pos[1] + v[1], pos[2] + v[2]]
}

fn above(pos: Pos, height: i32) -> Pos {
    [pos[0], pos[1] + height, pos[2]]
}

fn size_event(size: Size) -> &'static str {
    match size {
        Size::Small => "create:whistle_high",
        Size::Medium => "create:whistle",
        Size::Large => "create:whistle_low",
    }
}

/// Whistle properties of the block at `pos` (`wall`, `facing`, `size`).
fn whistle_props(game: &Game<'_>, pos: Pos) -> Option<(bool, Direction, Size)> {
    if game.native_block(pos) != BLOCK {
        return None;
    }
    let wall = game.native_block_property(pos, "wall").as_deref() == Some("true");
    let facing = game
        .native_block_property(pos, "facing")
        .and_then(|name| named(&name))
        .unwrap_or(Direction::North);
    let size = game
        .native_block_property(pos, "size")
        .and_then(|name| Size::parse(&name))
        .unwrap_or(Size::Medium);
    Some((wall, facing, size))
}

/// `Some(shape)` when the block at `pos` is an extender.
fn extender_at(game: &Game<'_>, pos: Pos) -> Option<ExtenderShape> {
    if game.native_block(pos) != EXTENSION {
        return None;
    }
    Some(
        game.native_block_property(pos, "shape")
            .and_then(|name| ExtenderShape::parse(&name))
            .unwrap_or(ExtenderShape::Single),
    )
}

/// Extenders above the whistle, bottom-up (`None` entries end the stack).
fn stack_above(game: &Game<'_>, pos: Pos) -> Vec<Option<ExtenderShape>> {
    (1..=STACK_SCAN).map(|h| extender_at(game, above(pos, h))).collect()
}

/// Number of contiguous extenders above `pos`.
fn stack_height(game: &Game<'_>, pos: Pos) -> usize {
    (1..=STACK_SCAN)
        .take_while(|h| game.native_block(above(pos, *h)) == EXTENSION)
        .count()
}

// ----------------------------------------------------------------- boiler

/// `(level, whistles counted)` of the boiler of the tank block at `tank`, read
/// from the controller's `Boiler` compound. None when it is not a tank or has
/// no boiler data.
fn boiler_at(game: &Game<'_>, tank: Pos) -> Option<(i32, bool)> {
    let entity = |p: Pos| match game.world.block_entities.get(&(p[0], p[1], p[2])) {
        Some(BlockEntity::Mod(entity)) if entity.id == TANK => Some(entity),
        _ => None,
    };
    let member = entity(tank)?;
    let controller = match member.nbt.get(&"Controller".into()) {
        None => tank,
        Some(Tag::IntArray(offset)) if offset.len() == 3 => {
            [tank[0] + offset[0], tank[1] + offset[1], tank[2] + offset[2]]
        }
        Some(_) => return None,
    };
    let Some(Tag::Compound(boiler)) = entity(controller)?.nbt.get(&"Boiler".into()) else {
        return None;
    };
    let level = int(boiler, "Level").unwrap_or(0);
    // A missing `Whistles` key means the boiler code does not publish it.
    let counted = int(boiler, "Whistles").is_none_or(|count| count > 0);
    Some((level, counted))
}

// ------------------------------------------------------------------ sound

#[allow(clippy::too_many_arguments)]
fn trace(kind: &str, event: &str, pos: Pos, volume: f32, pitch: f32, size: Size, pitch_id: i32, level: i32) {
    if std::env::var_os("CREATE_HORIZON_TRACE").is_some() {
        log::info!(
            "Create whistle: {}",
            serde_json::json!({
                "action": "sound", "kind": kind, "event": event, "pos": pos,
                "volume": volume, "pitch": pitch, "size": size.name(),
                "pitch_id": pitch_id, "level": level,
            })
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn play(
    game: &mut Game<'_>,
    kind: &str,
    event: &str,
    pos: Pos,
    volume: f32,
    pitch: f32,
    whistle: &rules::Whistle,
    level: i32,
) {
    let at = pos.map(|v| f64::from(v) + 0.5);
    game.native_play_sound_in_place(event, at, SOURCE_RECORDS, volume, pitch);
    trace(kind, event, pos, volume, pitch, whistle.size, whistle.id(), level);
}

// ------------------------------------------------------------------- tick

/// Per-tick behaviour. `WhistleBlock.neighborChanged` (POWERED follows the
/// neighbour signal) and `WhistleBlockEntity.tick` (the note sounds while
/// powered, the boiler is active and hot).
pub(super) fn tick(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity) {
    if !simulates(game) || !game.native_on("create") || !contraption::resident(game, pos) {
        return;
    }
    let Some((wall, facing, size)) = whistle_props(game, pos) else { return };
    let dim = game.dim.id().to_owned();

    // POWERED mirrors the neighbour signal (flag 2: clients only).
    let power = game.native_neighbor_signal(&dim, pos) > 0;
    let powered_now = game.native_block_property(pos, "powered").as_deref() == Some("true");
    if power != powered_now {
        game.native_set_block_property_with(pos, "powered", truth(power), flags::UPDATE_CLIENTS);
    }

    let was_sounding = int(&entity.nbt, SOUNDING).unwrap_or(0) != 0;
    let mut whistle = rules::Whistle::new(size);
    whistle.pitch = int(&entity.nbt, PITCH).unwrap_or(0);
    let tank = shifted(pos, rules::attached_direction(wall, facing));
    let boiler = boiler_at(game, tank);
    let (level, counted) = boiler.unwrap_or((0, false));
    let sounding = rules::is_sounding(power, counted, level > 0, false);

    // updatePitch: cheap enough to follow the stack every few ticks, and every
    // tick while the note plays so a new extender is heard at the next replay.
    if sounding || game.world.game_time.rem_euclid(SCAN_PERIOD) == 0 {
        if whistle.update_pitch(stack_above(game, pos)) {
            entity.nbt.insert(PITCH.into(), Tag::Int(whistle.pitch));
        }
    }

    if !sounding {
        if was_sounding {
            entity.nbt.insert(SOUNDING.into(), Tag::Byte(0));
            entity.nbt.insert(TIMER.into(), Tag::Int(0));
        }
        return;
    }

    let timer = int(&entity.nbt, TIMER).unwrap_or(0);
    if was_sounding && timer > 1 {
        entity.nbt.insert(TIMER.into(), Tag::Int(timer - 1));
        return;
    }
    if !was_sounding {
        // Start of a note: the chiff (WhistleBlockEntity's initial sound).
        // The server plays it at the source; clients attenuate by distance.
        play(
            game,
            "chiff",
            "create:steam",
            pos,
            rules::chiff_volume(0.0),
            rules::start_pitch(whistle.pitch, size),
            &whistle,
            level,
        );
        entity.nbt.insert(SOUNDING.into(), Tag::Byte(1));
    }
    let rate = rules::sound_pitch(whistle.pitch);
    play(game, "note", size_event(size), pos, NOTE_VOLUME, rate, &whistle, level);
    entity.nbt.insert(
        TIMER.into(),
        Tag::Int(rules::replay_period(SAMPLE_TICKS, rate, OVERLAP_TICKS)),
    );
}

// ------------------------------------------------------------ interaction

fn holds(stack: &ItemStack, item: &str) -> bool {
    stack.item.as_ref() == item
}

/// The whistle under `pos`, which is a whistle or an extender of its stack.
fn base_of(game: &Game<'_>, pos: Pos) -> Option<Pos> {
    match game.native_block(pos).as_str() {
        BLOCK => Some(pos),
        EXTENSION => (1..=STACK_SCAN)
            .map(|h| above(pos, -h))
            .find(|p| game.native_block(*p) != EXTENSION)
            .filter(|p| game.native_block(*p) == BLOCK),
        _ => None,
    }
}

/// Right click on a whistle or one of its extenders.
/// * Steam Whistle item (not sneaking): `WhistleBlock.incrementSize`, lower the
///   pitch by one step by adding or doubling an extender; consumes the item.
/// * Wrench (not sneaking): cycles the size of the whistle and its extenders.
/// * Anything else (or sneaking): normal use.
pub(super) fn right_click(game: &mut Game<'_>, pos: Pos) -> bool {
    if !simulates(game)
        || !game.native_on("create")
        || game.held_slot() != game.selected
        || !game.may_build()
    {
        return false;
    }
    let Some(base) = base_of(game, pos) else { return false };
    let sneaking = game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft);
    let held = game.stack_in(game.selected).filter(|stack| stack.count > 0).cloned();
    match held {
        Some(stack) if !sneaking && holds(&stack, BLOCK) => increment(game, base),
        Some(stack) if !sneaking && holds(&stack, "create:wrench") => cycle_size(game, base),
        _ => false,
    }
}

fn slot_at(game: &Game<'_>, pos: Pos) -> Slot {
    if let Some(shape) = extender_at(game, pos) {
        return Slot::Extender(shape);
    }
    let state = game.world.block(pos[0], pos[1], pos[2]);
    if game.res.facts().get(state).replaceable {
        Slot::Replaceable
    } else {
        Slot::Solid
    }
}

/// Applies `stack_shapes` and `size` to the `count` extenders above `base`.
fn restack(game: &mut Game<'_>, base: Pos, count: usize, top_single: bool, size: Size) {
    for (index, shape) in rules::stack_shapes(count, top_single).into_iter().enumerate() {
        let at = above(base, index as i32 + 1);
        game.native_set_block_property(at, "shape", shape.name());
        game.native_set_block_property(at, "size", size.name());
    }
}

/// `WhistleBlock.incrementSize`.
fn increment(game: &mut Game<'_>, base: Pos) -> bool {
    let Some((_, _, size)) = whistle_props(game, base) else { return false };
    let slots: Vec<Slot> = (1..=rules::EXTENSION_SEARCH).map(|h| slot_at(game, above(base, h))).collect();
    let Some(change) = rules::increment(&slots) else { return false };
    let (height, exponent, top_single) = match change {
        Increment::MakeDouble { height, sound_exponent } => (height, sound_exponent, false),
        Increment::Place { height, sound_exponent } => (height, sound_exponent, true),
    };
    if matches!(change, Increment::Place { .. }) {
        let dim = game.dim.id().to_owned();
        let Some(state) = game.native_default_state(EXTENSION) else { return false };
        if !game.native_set_block(&dim, above(base, height), state, flags::UPDATE_ALL) {
            return false;
        }
    }
    restack(game, base, height as usize, top_single, size);
    // Pitch of the confirmation note: 2^(exponent / 12).
    let pitch = 2.0_f32.powf(exponent as f32 / 12.0);
    let at = above(base, height).map(|v| f64::from(v) + 0.5);
    game.native_play_sound_in_place("minecraft:block.note_block.chime", at, 4, 1.0, pitch);
    game.consume_held();
    true
}

/// `getRotatedBlockState` (wrench): the size cycles and the stack follows.
fn cycle_size(game: &mut Game<'_>, base: Pos) -> bool {
    let Some((_, _, size)) = whistle_props(game, base) else { return false };
    let next = size.cycled();
    game.native_set_block_property(base, "size", next.name());
    for h in 1..=stack_height(game, base) as i32 {
        game.native_set_block_property(above(base, h), "size", next.name());
    }
    super::create_port_sounds::wrench(game, base, false);
    true
}

// ---------------------------------------------------------------- hooks

/// Whether the block at the given level position is a Fluid Tank block.
fn is_tank(game: &GameLevel<'_>, pos: Pos) -> bool {
    game.registry
        .state(game.world.block(pos[0], pos[1], pos[2]))
        .is_some_and(|block| block.name == TANK)
}

/// `state_for_placement` hook: `WhistleBlock.getStateForPlacement`. `None`
/// refuses the placement (no tank on the attached side).
pub(super) fn placement(
    game: &GameLevel<'_>,
    state: BlockStateId,
    ctx: &PlacementContext,
) -> Option<BlockStateId> {
    let at = [ctx.pos.x, ctx.pos.y, ctx.pos.z];
    // `placement_state` takes the direction from the new block towards its
    // support: the opposite of the clicked face.
    let towards_support = engine(ctx.face).opposite();
    let player = super::create_port_placement::horizontal(ctx.yaw).opposite();
    let (wall, facing) =
        rules::placement_state(towards_support, player, |d| is_tank(game, shifted(at, d)))?;
    let state = game.native_state_with_property(state, "facing", facing.name())?;
    game.native_state_with_property(state, "wall", truth(wall))
}

/// `can_survive` hook: `WhistleBlock.canSurvive` and
/// `WhistleExtenderBlock.canSurvive`.
pub(super) fn can_survive(
    game: &GameLevel<'_>,
    pos: BlockPos,
    state: BlockStateId,
    survives: &mut Option<bool>,
) {
    let Some(block) = game
        .registry
        .state(state)
        .filter(|block| block.name == BLOCK || block.name == EXTENSION)
    else {
        return;
    };
    let at = [pos.x, pos.y, pos.z];
    if block.name == BLOCK {
        let wall = block.properties.get("wall").is_some_and(|v| v == "true");
        let Some(facing) = block.properties.get("facing").and_then(|name| named(name)) else {
            return;
        };
        *survives = Some(rules::can_survive(wall, facing, |d| is_tank(game, shifted(at, d))));
        return;
    }
    let below = game.registry.state(game.world.block(at[0], at[1] - 1, at[2]));
    let kind = match below {
        Some(b) if b.name == BLOCK => Below::Whistle,
        Some(b) if b.name == EXTENSION => Below::Extender(
            b.properties
                .get("shape")
                .and_then(|name| ExtenderShape::parse(name))
                .unwrap_or(ExtenderShape::Single),
        ),
        _ => Below::Other,
    };
    *survives = Some(rules::extender_can_survive(kind));
}

/// `update_shape` hook. A whistle without its tank and an extender without a
/// valid support disappear; a double extender is "connected" while another
/// extender stands on it.
pub(super) fn shape(
    game: &GameLevel<'_>,
    state: BlockStateId,
    changed: Face,
    neighbour: BlockStateId,
    new: &mut Option<BlockStateId>,
) {
    let Some(block) = game
        .registry
        .state(state)
        .filter(|block| block.name == BLOCK || block.name == EXTENSION)
    else {
        return;
    };
    let neighbour_block = game.registry.state(neighbour);
    if block.name == BLOCK {
        let wall = block.properties.get("wall").is_some_and(|v| v == "true");
        let Some(facing) = block.properties.get("facing").and_then(|name| named(name)) else {
            return;
        };
        if engine(changed) == rules::attached_direction(wall, facing)
            && !neighbour_block.is_some_and(|b| b.name == TANK)
        {
            *new = Some(game.world.air);
        }
        return;
    }
    let current = block
        .properties
        .get("shape")
        .and_then(|name| ExtenderShape::parse(name))
        .unwrap_or(ExtenderShape::Single);
    match changed {
        Face::Down => {
            let kind = match neighbour_block {
                Some(b) if b.name == BLOCK => Below::Whistle,
                Some(b) if b.name == EXTENSION => Below::Extender(
                    b.properties
                        .get("shape")
                        .and_then(|name| ExtenderShape::parse(name))
                        .unwrap_or(ExtenderShape::Single),
                ),
                _ => Below::Other,
            };
            if !rules::extender_can_survive(kind) {
                *new = Some(game.world.air);
            }
        }
        Face::Up if current != ExtenderShape::Single => {
            let stacked = neighbour_block.is_some_and(|b| b.name == EXTENSION);
            let want = if stacked { ExtenderShape::DoubleConnected } else { ExtenderShape::Double };
            if want != current {
                *new = game.native_state_with_property(state, "shape", want.name());
            }
        }
        _ => {}
    }
}
