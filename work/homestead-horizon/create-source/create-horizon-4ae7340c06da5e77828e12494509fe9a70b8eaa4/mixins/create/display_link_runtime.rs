// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

// Display Link (DisplayLinkBlock / DisplayLinkBlockEntity): a block attached to
// a source block that periodically writes a text taken from the source to a
// target, here a Display Board. The state machine, the text rules and the board
// target rules live in `create_engine::display_link`; sources are read in
// `display_source.rs`, the board side is `display_board_runtime.rs`.
//
// Block states: `facing` (the plate looks out of that face and is attached to the
// opposite neighbour, the source), `powered`.
//
// There is no configuration GUI. Substitutes (documented in
// docs/redstone-displays.md):
//   * Target: right click a Display Board with the Display Link item (not
//     sneaking) to select it; the next Display Link placed within 1200 ticks and
//     64 blocks takes it as its target (`TargetOffset`).
//   * Empty hand: next source (`Source`, `auto` first). Sneaking, empty hand: next
//     target line. Wrench: next display mode of the source; sneaking Wrench: next
//     update interval. Any named item (name tag): its name becomes the `Label`.
//
// Block entity NBT: TargetOffset (IntArray), TargetLine, Source (String), Mode,
// Label (String), Interval, RefreshTicks, Fresh, LastText (String), Status (String).

use super::{
    Game, GameLevel, create_port_contraption as contraption, create_port_display_board as boards,
    create_port_display_source as sources,
};
use crate::{containers::ModBlockEntity, items::ItemStack};
use create_engine::{
    display_board::{FlapLayout, FlapSection, MONOSPACE},
    display_link::{self as dl, BarTarget, LinkAction},
};
use horizon_client::text::Text;
use horizon_core::geometry::Direction as Face;
use horizon_world::{
    level::{PlacementContext, flags},
    nbt::{Compound, Tag},
    registry::BlockStateId,
};
use sources::{Out, Section};
use std::cell::RefCell;

pub(super) const BLOCK: &str = "create:display_link";
type Pos = [i32; 3];

const TARGET: &str = "TargetOffset";
const LINE: &str = "TargetLine";
const SOURCE: &str = "Source";
const MODE: &str = "Mode";
const LABEL: &str = "Label";
const INTERVAL: &str = "Interval";
const REFRESH: &str = "RefreshTicks";
const FRESH: &str = "Fresh";
const LAST: &str = "LastText";
const STATUS: &str = "Status";

/// Selectable update intervals in ticks (`getPassiveRefreshTicks` default is 100).
const INTERVALS: [i32; 4] = [20, 100, 200, 600];
/// A selected target is forgotten after this many ticks.
const SELECTION_TTL: i64 = 1200;
/// Blocks that power the link through the attached face and therefore must not
/// count as "powered" (Java ignores the attached face; this port only sees the
/// strongest neighbour signal).
const EMITTERS: [&str; 5] = [
    "minecraft:redstone_block",
    "minecraft:lever",
    "minecraft:redstone_torch",
    "minecraft:daylight_detector",
    "minecraft:target",
];

thread_local! {
    /// The board the player selected with the Display Link item:
    /// `(dimension, position, game time)`.
    static SELECTED: RefCell<Option<(String, Pos, i64)>> = const { RefCell::new(None) };
}

fn simulates(game: &Game<'_>) -> bool {
    game.serving.is_some() || game.options.dedicated.is_some()
}

// ------------------------------------------------------------------- NBT

fn int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => *v,
        Some(Tag::Byte(v)) => i32::from(*v),
        _ => 0,
    }
}

fn flag(nbt: &Compound, key: &str) -> bool {
    matches!(nbt.get(&key.into()), Some(Tag::Byte(v)) if *v != 0)
}

fn string(nbt: &Compound, key: &str) -> String {
    match nbt.get(&key.into()) {
        Some(Tag::String(v)) => v.to_string().ok().unwrap_or_default(),
        _ => String::new(),
    }
}

fn offset(nbt: &Compound) -> Pos {
    match nbt.get(&TARGET.into()) {
        Some(Tag::IntArray(v)) if v.len() == 3 => [v[0], v[1], v[2]],
        _ => [0; 3],
    }
}

fn set_string(nbt: &mut Compound, key: &str, value: &str) -> bool {
    if string(nbt, key) == value && nbt.contains_key(&key.into()) {
        return false;
    }
    nbt.insert(key.into(), Tag::String(value.into()));
    true
}

pub(super) fn default_nbt() -> Compound {
    Compound::from([
        (TARGET.into(), Tag::IntArray(vec![0, 0, 0])),
        (LINE.into(), Tag::Int(0)),
        (SOURCE.into(), Tag::String("auto".into())),
        (MODE.into(), Tag::Int(0)),
        (LABEL.into(), Tag::String("".into())),
        (INTERVAL.into(), Tag::Int(dl::DEFAULT_PASSIVE_REFRESH_TICKS)),
        (REFRESH.into(), Tag::Int(0)),
        (FRESH.into(), Tag::Byte(1)),
        (LAST.into(), Tag::String("".into())),
        (STATUS.into(), Tag::String("".into())),
    ])
}

/// What clients receive (for tools and a future goggles line).
pub(super) fn update_tag(entity: &ModBlockEntity) -> Compound {
    [TARGET, LINE, SOURCE, MODE, LABEL, INTERVAL, LAST, STATUS]
        .into_iter()
        .filter_map(|key| entity.nbt.get(&key.into()).map(|tag| (key.into(), tag.clone())))
        .collect()
}

// ---------------------------------------------------------------- states

fn truth(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn step(name: &str) -> Option<[i32; 3]> {
    Some(match name {
        "down" => [0, -1, 0],
        "up" => [0, 1, 0],
        "north" => [0, 0, -1],
        "south" => [0, 0, 1],
        "west" => [-1, 0, 0],
        "east" => [1, 0, 0],
        _ => return None,
    })
}

fn face_name(face: Face) -> &'static str {
    match face {
        Face::Down => "down",
        Face::Up => "up",
        Face::North => "north",
        Face::South => "south",
        Face::West => "west",
        Face::East => "east",
    }
}

fn add(a: Pos, b: [i32; 3], sign: i32) -> Pos {
    [a[0] + b[0] * sign, a[1] + b[1] * sign, a[2] + b[2] * sign]
}

// ------------------------------------------------------------------ tick

/// Takes the pending board selection for a freshly placed link
/// (`DisplayLinkBlockItem.placeInWorld`).
fn bind_selection(game: &Game<'_>, pos: Pos, nbt: &mut Compound) -> bool {
    let now = game.world.game_time;
    let picked = SELECTED.with_borrow_mut(Option::take);
    let Some((dimension, target, when)) = picked else { return false };
    let delta = [target[0] - pos[0], target[1] - pos[1], target[2] - pos[2]];
    let range = dl::DISPLAY_LINK_RANGE;
    let close = delta.iter().map(|v| i64::from(*v).pow(2)).sum::<i64>() < i64::from(range) * i64::from(range);
    if dimension != game.dim.id() || now - when > SELECTION_TTL || target == pos || !close {
        return false;
    }
    nbt.insert(TARGET.into(), Tag::IntArray(delta.to_vec()));
    true
}

pub(super) fn tick(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity) {
    if !simulates(game) || !game.native_on("create") || !contraption::resident(game, pos) {
        return;
    }
    let Some(facing) = game.native_block_property(pos, "facing") else { return };
    let Some(attached) = step(&facing) else { return };
    let source = add(pos, attached, -1);
    let mut dirty = false;

    let mut forced = false;
    if flag(&entity.nbt, FRESH) {
        entity.nbt.insert(FRESH.into(), Tag::Byte(0));
        bind_selection(game, pos, &mut entity.nbt);
        forced = true;
        dirty = true;
    }

    // POWERED follows the neighbour signal (`DisplayLinkBlock.neighborChanged`).
    let dimension = game.dim.id().to_owned();
    let emitter = EMITTERS.contains(&game.native_block(source).as_str());
    let signal = game.native_neighbor_signal(&dimension, pos) > 0 && !emitter;
    let powered_now = game.native_block_property(pos, "powered").as_deref() == Some("true");
    let mut link = dl::DisplayLink {
        target_offset: offset(&entity.nbt),
        target_line: int(&entity.nbt, LINE),
        refresh_ticks: if forced { i32::MAX / 2 } else { int(&entity.nbt, REFRESH) },
        powered: powered_now,
        has_active_source: true,
    };
    let mut action = link.set_powered(signal);
    if signal != powered_now {
        game.native_set_block_property_with(pos, "powered", truth(signal), flags::UPDATE_CLIENTS);
    }
    if action == LinkAction::Nothing {
        let interval = int(&entity.nbt, INTERVAL).max(1);
        action = link.tick(false, interval, true);
    }
    if int(&entity.nbt, REFRESH) != link.refresh_ticks {
        entity.nbt.insert(REFRESH.into(), Tag::Int(link.refresh_ticks));
    }
    if action != LinkAction::Nothing {
        dirty |= update(game, pos, source, &link, &mut entity.nbt);
    }
    if dirty {
        game.native_block_entity_sync(pos);
    }
}

/// `updateGatheredData`: reads the source and writes the target board. Returns
/// whether the link's own data changed.
fn update(game: &mut Game<'_>, pos: Pos, source: Pos, link: &dl::DisplayLink, nbt: &mut Compound) -> bool {
    let status = |nbt: &mut Compound, text: &str| set_string(nbt, STATUS, text);
    if !link.has_valid_target() {
        return status(nbt, "invalid target");
    }
    let target = add(pos, link.target_offset, 1);
    let range = i64::from(dl::DISPLAY_LINK_RANGE);
    if link.target_offset.iter().map(|v| i64::from(*v).pow(2)).sum::<i64>() >= range * range {
        return status(nbt, "out of range");
    }
    let id = sources::resolve(game, source, &string(nbt, SOURCE));
    let mode = int(nbt, MODE);
    let Some(out) = sources::read(game, id, source, mode) else {
        return status(nbt, "no data at source");
    };
    let Some((_, board)) = boards::stats(game, target) else {
        return status(nbt, "no board at target");
    };
    let label = string(nbt, LABEL);
    let columns = dl::board_stats(Some(&board)).max_columns;
    let (value, section) = match out {
        Out::Value(text, section) => (text, section),
        Out::Bar(progress) => (
            dl::progress_bar(progress, columns, &label, BarTarget::Flap).unwrap_or_default(),
            Section::Progress,
        ),
    };
    let cells = vec![if label.is_empty() { vec![value.clone()] } else { vec![format!("{label} "), value.clone()] }];
    let layout_label = label.clone();
    let mut load = move |layout: &mut FlapLayout, _line: usize, max: i32| match section {
        Section::Alphabet => dl::load_single_line_layout(layout, "Default", &layout_label, max, |size| {
            FlapSection::new(size as f32 * MONOSPACE, "alphabet", false, false)
        }),
        Section::Numeric => dl::load_single_line_layout(layout, "Number", &layout_label, max, dl::numeric_section),
        Section::Instant => dl::load_single_line_layout(layout, "Instant", &layout_label, max, dl::instant_section),
        Section::Progress => dl::load_single_line_layout(layout, "Progress", &layout_label, max, dl::progress_section),
    };
    let wrote = boards::accept(game, target, link.target_line, &cells, true, pos, &mut load);
    let mut changed = set_string(nbt, STATUS, if wrote { "ok" } else { "board not ready" });
    if wrote {
        changed |= set_string(nbt, LAST, &value);
    }
    changed
}

// ----------------------------------------------------------- interaction

fn holds(stack: &ItemStack, item: &str) -> bool {
    stack.item.as_ref() == item
}

fn say(game: &mut Game<'_>, text: String) {
    let player = game.native_acting_id();
    game.native_action_bar(player, Text::literal(text));
}

/// Right click on a Display Board or a Display Link.
pub(super) fn right_click(game: &mut Game<'_>, pos: Pos) -> bool {
    if !simulates(game)
        || !game.native_on("create")
        || game.held_slot() != game.selected
        || !game.may_build()
    {
        return false;
    }
    let block = game.native_block(pos);
    if block != BLOCK && block != boards::BOARD {
        return false;
    }
    let sneaking = game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft);
    let held = game.stack_in(game.selected).filter(|stack| stack.count > 0).cloned();
    if block == boards::BOARD {
        // Selecting a target: the Display Link item clicked on a board.
        return match held {
            Some(stack) if !sneaking && holds(&stack, BLOCK) => {
                let now = game.world.game_time;
                let dimension = game.dim.id().to_owned();
                SELECTED.with_borrow_mut(|slot| *slot = Some((dimension, pos, now)));
                say(game, format!("Display board selected at {} {} {}", pos[0], pos[1], pos[2]));
                true
            }
            _ => false,
        };
    }
    match held {
        Some(stack) if holds(&stack, BLOCK) => false,
        Some(stack) if holds(&stack, "create:wrench") => configure(game, pos, if sneaking { Edit::Interval } else { Edit::Mode }),
        Some(stack) if !sneaking && stack.custom_name().is_some() => {
            configure(game, pos, Edit::Label(stack.custom_name().unwrap_or_default()))
        }
        Some(_) => false,
        None => configure(game, pos, if sneaking { Edit::Line } else { Edit::Source }),
    }
}

enum Edit {
    Source,
    Line,
    Mode,
    Interval,
    Label(String),
}

fn configure(game: &mut Game<'_>, pos: Pos, edit: Edit) -> bool {
    let message = game.native_with_mod_block_entity(pos, |entity| {
        if entity.id != BLOCK {
            return None;
        }
        let nbt = &mut entity.nbt;
        let source = string(nbt, SOURCE);
        let message = match edit {
            Edit::Source => {
                let list = &sources::SOURCES;
                let next = list.iter().position(|id| *id == source).map_or(0, |i| (i + 1) % list.len());
                nbt.insert(SOURCE.into(), Tag::String(list[next].into()));
                nbt.insert(MODE.into(), Tag::Int(0));
                format!("Source: {}", list[next])
            }
            Edit::Line => {
                let line = (int(nbt, LINE) + 1).rem_euclid(16);
                nbt.insert(LINE.into(), Tag::Int(line));
                format!("Target line: {line}")
            }
            Edit::Mode => {
                let mode = (int(nbt, MODE) + 1).rem_euclid(sources::modes(&source).max(1));
                nbt.insert(MODE.into(), Tag::Int(mode));
                format!("Mode: {}", sources::mode_name(&source, mode))
            }
            Edit::Interval => {
                let now = int(nbt, INTERVAL);
                let next = INTERVALS.iter().position(|v| *v == now).map_or(0, |i| (i + 1) % INTERVALS.len());
                nbt.insert(INTERVAL.into(), Tag::Int(INTERVALS[next]));
                format!("Update every {} ticks", INTERVALS[next])
            }
            Edit::Label(label) => {
                nbt.insert(LABEL.into(), Tag::String(label.as_str().into()));
                format!("Label: {label}")
            }
        };
        // Apply at the next tick (`ConfigurationPacket` updates the gathered data).
        nbt.insert(REFRESH.into(), Tag::Int(i32::MAX / 2));
        Some(message)
    });
    let Some(Some(message)) = message else { return false };
    game.native_block_entity_sync(pos);
    say(game, message);
    true
}

// ---------------------------------------------------------------- hooks

/// `state_for_placement`: the link faces the clicked face (board: the player).
pub(super) fn placement(game: &GameLevel<'_>, state: BlockStateId, ctx: &PlacementContext, answer: &mut Option<Option<BlockStateId>>) {
    if answer.is_some() {
        return;
    }
    match game.native_state_block(state).unwrap_or("") {
        BLOCK => *answer = Some(game.native_state_with_property(state, "facing", face_name(ctx.face))),
        boards::BOARD => *answer = Some(boards::placement(game, state, ctx)),
        _ => {}
    }
}

/// `can_survive`: the attached neighbour must not be replaceable.
pub(super) fn can_survive(
    game: &GameLevel<'_>,
    pos: horizon_core::position::BlockPos,
    state: BlockStateId,
    survives: &mut Option<bool>,
) {
    let Some(block) = game.registry.state(state).filter(|block| block.name == BLOCK) else { return };
    let Some(delta) = block.properties.get("facing").and_then(|name| step(name)) else { return };
    let support = game.world.block(pos.x - delta[0], pos.y - delta[1], pos.z - delta[2]);
    *survives = Some(!game.facts.get(support).replaceable);
}

/// `update_shape`: losing the attached neighbour removes the link; boards update
/// their `up` / `down` states.
pub(super) fn shape(
    game: &GameLevel<'_>,
    state: BlockStateId,
    changed: Face,
    neighbour: BlockStateId,
    new: &mut Option<BlockStateId>,
) {
    boards::shape(game, state, changed, neighbour, new);
    let Some(block) = game.registry.state(state).filter(|block| block.name == BLOCK) else { return };
    let Some(delta) = block.properties.get("facing").and_then(|name| step(name)) else { return };
    let behind = [-delta[0], -delta[1], -delta[2]];
    if step(face_name(changed)) == Some(behind) && game.facts.get(neighbour).replaceable {
        *new = Some(game.world.air);
    }
}
