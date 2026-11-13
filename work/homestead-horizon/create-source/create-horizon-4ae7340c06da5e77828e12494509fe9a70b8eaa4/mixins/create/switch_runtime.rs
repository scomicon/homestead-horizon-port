// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

// Redstone switches (module `create_port_switch`): Pulse Repeater, Pulse
// Extender, Pulse Timer, Powered Latch, Powered Toggle Latch, Analog Lever,
// Redstone Contact and the Nixie Tube. The rules live in
// `create_engine::redstone_switches` and `create_engine::nixie`; this file only
// adapts them to the game (see docs/redstone-switches.md).
//
// Ownership of state
//   * Output the neighbours read (`block_signal`) is answered from BLOCK STATE
//     only (`powering`, `inverted`, `power`, `powered`), never from the block
//     entity: the hook runs inside the level update and a ticking block entity
//     is out of the world map.
//   * Diodes, latches, the contact and the tube keep their logical state in a
//     ticking block entity (NBT below) and mirror it into the block state in
//     `flush`, after the block entities of the level have ticked (the same
//     pattern as the Redstone Link receiver).
//   * The Analog Lever has no block entity: its 0..15 value is the `power`
//     property.
//
// Directional input. The engine API only offers the strongest signal over all
// six neighbours (`native_neighbor_signal`). `input_signals` therefore reads
// redstone blocks, redstone wire and these switches per side from their block
// state and attributes any further signal (levers, torches, buttons, ...) to
// every input side whose neighbour it cannot classify (`known`). A lever beside
// a pulse repeater can thus trigger it. Documented in docs/redstone-switches.md.

use super::{Game, GameLevel, create_port_contraption as contraption};
use super::native_block_entity_draw::BlockEntityDraw;
use crate::{
    containers::{BlockEntity, ModBlockEntity},
    items::ItemStack,
};
use create_engine::{
    nixie::{self as tube, NixieTube},
    redstone_link::{Direction as Dir, MAX_SIGNAL},
    redstone_switches::{
        self as rules, AnalogLever, BrassDiode, DiodeKind, PoweredLatch, ToggleLatch,
    },
    transport::SessionCache,
};
use horizon_client::text::Text;
use horizon_core::{geometry::Direction as Face, position::BlockPos};
use horizon_world::{
    level::PlacementContext,
    nbt::{Compound, Tag},
    registry::BlockStateId,
};
use std::{cell::RefCell, collections::BTreeSet};

pub(super) const REPEATER: &str = "create:pulse_repeater";
pub(super) const EXTENDER: &str = "create:pulse_extender";
pub(super) const TIMER: &str = "create:pulse_timer";
pub(super) const POWERED_LATCH: &str = "create:powered_latch";
pub(super) const TOGGLE_LATCH: &str = "create:powered_toggle_latch";
pub(super) const LEVER: &str = "create:analog_lever";
pub(super) const CONTACT: &str = "create:redstone_contact";
pub(super) const NIXIE: &str = "create:nixie_tube";
type Pos = [i32; 3];
type Get<'a> = &'a dyn Fn(&str) -> Option<String>;

// Block entity NBT (the port's own keys; Create keeps some of these in block
// state or computes them).
const STATE: &str = "State";
const MAX_STATE: &str = "MaxState";
const POWERED: &str = "Powered";
const POWERING: &str = "Powering";
const INVERTED: &str = "Inverted";
const POWERED_SIDE: &str = "PoweredSide";
const AGAIN: &str = "Again";
/// Ticks the sampled input has differed from `Powered` (emulates the 2 tick
/// scheduled `DiodeBlock.tick` that copies the input into POWERED).
const IN_DELAY: &str = "InputDelay";
const STRENGTH: &str = "RedstoneStrength";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Diode(DiodeKind),
    ToggleLatch,
    PoweredLatch,
    Lever,
    Contact,
    Nixie,
}

fn kind(block: &str) -> Option<Kind> {
    Some(match block {
        REPEATER => Kind::Diode(DiodeKind::PulseRepeater),
        EXTENDER => Kind::Diode(DiodeKind::PulseExtender),
        TIMER => Kind::Diode(DiodeKind::PulseTimer),
        TOGGLE_LATCH => Kind::ToggleLatch,
        POWERED_LATCH => Kind::PoweredLatch,
        LEVER => Kind::Lever,
        CONTACT => Kind::Contact,
        NIXIE => Kind::Nixie,
        _ => return None,
    })
}

/// Everything except the lever has a (ticking) block entity.
fn has_entity(block: &str) -> bool {
    !matches!(kind(block), None | Some(Kind::Lever))
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

/// Writes `value`; true when the stored value changed.
fn put_int(nbt: &mut Compound, key: &str, value: i32) -> bool {
    let same = nbt.contains_key(&key.into()) && int(nbt, key) == value;
    if !same {
        nbt.insert(key.into(), Tag::Int(value));
    }
    !same
}

fn put_flag(nbt: &mut Compound, key: &str, value: bool) -> bool {
    let same = nbt.contains_key(&key.into()) && flag(nbt, key) == value;
    if !same {
        nbt.insert(key.into(), Tag::Byte(i8::from(value)));
    }
    !same
}

pub(super) fn default_nbt(block: &str) -> Compound {
    let mut nbt = Compound::new();
    match kind(block) {
        Some(Kind::Diode(k)) => {
            put_int(&mut nbt, STATE, 0);
            put_int(&mut nbt, MAX_STATE, k.default_delay());
            put_int(&mut nbt, IN_DELAY, 0);
            for key in [POWERED, POWERING, INVERTED] {
                put_flag(&mut nbt, key, false);
            }
        }
        Some(Kind::ToggleLatch) => {
            for key in [POWERED, POWERING, AGAIN] {
                put_flag(&mut nbt, key, false);
            }
        }
        Some(Kind::PoweredLatch) => {
            for key in [POWERED, POWERED_SIDE, POWERING] {
                put_flag(&mut nbt, key, false);
            }
        }
        Some(Kind::Contact) => {
            put_flag(&mut nbt, POWERED, false);
        }
        Some(Kind::Nixie) => {
            put_int(&mut nbt, STRENGTH, 0);
        }
        _ => {}
    }
    nbt
}

// ---------------------------------------------------------------- states

fn dir_of(face: Face) -> Dir {
    match face {
        Face::Down => Dir::Down,
        Face::Up => Dir::Up,
        Face::North => Dir::North,
        Face::South => Dir::South,
        Face::West => Dir::West,
        Face::East => Dir::East,
    }
}

fn named(name: &str) -> Option<Dir> {
    Some(match name {
        "down" => Dir::Down,
        "up" => Dir::Up,
        "north" => Dir::North,
        "south" => Dir::South,
        "west" => Dir::West,
        "east" => Dir::East,
        _ => return None,
    })
}

fn dir_name(dir: Dir) -> &'static str {
    match dir {
        Dir::Down => "down",
        Dir::Up => "up",
        Dir::North => "north",
        Dir::South => "south",
        Dir::West => "west",
        Dir::East => "east",
    }
}

fn truth(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn simulates(game: &Game<'_>) -> bool {
    game.serving.is_some() || game.options.dedicated.is_some()
}

fn facing_prop(game: &Game<'_>, pos: Pos) -> Option<Dir> {
    game.native_block_property(pos, "facing").as_deref().and_then(named)
}

/// The face a tube sits on and its direction, from the `mount` property
/// (`floor_south`, `ceiling_north`, `wall_east`).
fn mount_parts(mount: &str) -> Option<(&str, Dir)> {
    let (face, dir) = mount.split_once('_')?;
    Some((face, named(dir)?))
}

/// Blockstate rotation (x, y) of a mount; the generator
/// (`tools/switch_content.py`, `_MOUNT_TURNS`) uses the same table.
fn mount_turn(face: &str, dir: Dir) -> (f32, f32) {
    let floor_y = match dir {
        Dir::South => 0.0,
        Dir::West => 90.0,
        Dir::North => 180.0,
        _ => 270.0,
    };
    let wall_y = match dir {
        Dir::North => 0.0,
        Dir::East => 90.0,
        Dir::South => 180.0,
        _ => 270.0,
    };
    match face {
        "ceiling" => (180.0, floor_y),
        "wall" => (90.0, wall_y),
        _ => (0.0, floor_y),
    }
}

// ---------------------------------------------------------------- output

/// What a switch gives to a reader. `dir` is the vanilla direction handed to
/// `getSignal`: from the reader towards the switch. `get` reads block state.
fn output(block: &str, get: Get<'_>, dir: Dir, direct: bool) -> i32 {
    let Some(kind) = kind(block) else { return 0 };
    let on = |key: &str| get(key).as_deref() == Some("true");
    let facing = get("facing").as_deref().and_then(named);
    match kind {
        Kind::Diode(k) => {
            let mut diode = BrassDiode::new(k);
            diode.powering = on("powering");
            diode.inverted = on("inverted");
            facing.map_or(0, |f| diode.signal(f, dir))
        }
        Kind::ToggleLatch => {
            let latch = ToggleLatch { powered: false, powering: on("powering") };
            facing.filter(|f| *f == dir).map_or(0, |_| latch.output_signal())
        }
        Kind::PoweredLatch => {
            let latch = PoweredLatch { powered: false, powered_side: false, powering: on("powering") };
            facing.filter(|f| *f == dir).map_or(0, |_| latch.output_signal())
        }
        Kind::Lever => {
            let power = get("power").and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
            let lever = AnalogLever { state: power, last_change: 0 };
            if direct {
                facing.map_or(0, |f| lever.direct_signal(f, dir))
            } else {
                lever.signal()
            }
        }
        Kind::Contact => facing.map_or(0, |f| rules::contact_signal(on("powered"), f, dir)),
        Kind::Nixie => 0,
    }
}

// ----------------------------------------------------------------- input

fn is_air(name: &str) -> bool {
    matches!(name, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")
}

/// The signal the neighbour of `pos` in `dir` gives towards `pos`, when it can
/// be told from block state alone.
fn known(game: &Game<'_>, pos: Pos, dir: Dir) -> Option<i32> {
    let [dx, dy, dz] = dir.step();
    let at = [pos[0] + dx, pos[1] + dy, pos[2] + dz];
    let name = game.native_block(at).to_string();
    if is_air(&name) {
        return Some(0);
    }
    match name.as_str() {
        "minecraft:redstone_block" => Some(MAX_SIGNAL),
        // A diode reads wire power whatever the wire's connections.
        "minecraft:redstone_wire" => Some(
            game.native_block_property(at, "power")
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(0),
        ),
        other if kind(other).is_some() => {
            let get = |key: &str| game.native_block_property(at, key);
            Some(output(other, &get, dir, false))
        }
        _ => None,
    }
}

/// The input signal on each of `dirs` (0..15). See the module comment for the
/// approximation used for neighbours that are not classified.
fn input_signals(game: &mut Game<'_>, pos: Pos, dirs: &[Dir]) -> Vec<i32> {
    let mut values: Vec<Option<i32>> = Vec::with_capacity(dirs.len());
    for dir in dirs {
        values.push(known(game, pos, *dir));
    }
    if values.iter().any(Option::is_none) {
        let mut recognised = 0;
        for dir in Dir::ALL {
            recognised = recognised.max(known(game, pos, dir).unwrap_or(0));
        }
        let dimension = game.dim.id().to_owned();
        let all = game.native_neighbor_signal(&dimension, pos);
        let excess = if all > recognised { all } else { 0 };
        for value in &mut values {
            if value.is_none() {
                *value = Some(excess);
            }
        }
    }
    values.into_iter().map(|v| v.unwrap_or(0)).collect()
}

// ------------------------------------------------------- deferred writes

#[derive(Default)]
struct Level {
    /// Switches whose block state must follow their block entity (`flush`).
    pending: BTreeSet<Pos>,
}

thread_local! {
    static LEVELS: RefCell<SessionCache<Level>> = RefCell::new(Default::default());
}

fn with_dimension<R>(game: &Game<'_>, dimension: &str, f: impl FnOnce(&mut Level) -> R) -> R {
    let session = std::ptr::from_ref(&game.world) as usize;
    LEVELS.with_borrow_mut(|cache| f(cache.dimension(session, dimension)))
}

fn queue(game: &Game<'_>, pos: Pos) {
    with_dimension(game, game.dim.id(), |level| level.pending.insert(pos));
}

/// The block state properties that must mirror the block entity.
fn desired(kind: Kind, nbt: &Compound) -> Vec<(&'static str, bool)> {
    match kind {
        Kind::Diode(_) => vec![
            ("powered", flag(nbt, POWERED)),
            ("powering", flag(nbt, POWERING)),
            ("inverted", flag(nbt, INVERTED)),
        ],
        Kind::ToggleLatch => vec![("powered", flag(nbt, POWERED)), ("powering", flag(nbt, POWERING))],
        Kind::PoweredLatch => vec![
            ("powered", flag(nbt, POWERED)),
            ("powered_side", flag(nbt, POWERED_SIDE)),
            ("powering", flag(nbt, POWERING)),
        ],
        Kind::Contact => vec![("powered", flag(nbt, POWERED))],
        Kind::Lever | Kind::Nixie => Vec::new(),
    }
}

/// `level_tick_post`: settles queued switches, after the block entities of the
/// level have ticked. Output changes re-notify the neighbours.
pub(super) fn flush(game: &mut Game<'_>, dimension: &str) {
    if !simulates(game) || !game.native_on("create") {
        return;
    }
    let pending = with_dimension(game, dimension, |level| std::mem::take(&mut level.pending));
    for pos in pending {
        if !contraption::resident(game, pos) {
            continue;
        }
        let block = game.native_block(pos).to_string();
        let Some(kind) = kind(&block) else { continue };
        let nbt = match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
            Some(BlockEntity::Mod(entity)) if entity.id == block => entity.nbt.clone(),
            _ => continue,
        };
        let mut output_changed = false;
        for (key, value) in desired(kind, &nbt) {
            let current = game.native_block_property(pos, key).as_deref() == Some("true");
            if current != value {
                game.native_set_block_property(pos, key, truth(value));
                output_changed |= matches!(key, "powering" | "inverted") || kind == Kind::Contact;
            }
        }
        if output_changed {
            game.native_update_signal_neighbors(dimension, pos);
        }
    }
}

// ------------------------------------------------------------------ tick

fn load_diode(nbt: &Compound, kind: DiodeKind) -> BrassDiode {
    let mut diode = BrassDiode::new(kind);
    if nbt.contains_key(&MAX_STATE.into()) {
        diode.max_state = int(nbt, MAX_STATE);
    }
    diode.state = int(nbt, STATE);
    diode.powered = flag(nbt, POWERED);
    diode.powering = flag(nbt, POWERING);
    diode.inverted = flag(nbt, INVERTED);
    diode
}

fn store_diode(nbt: &mut Compound, diode: &BrassDiode, delay: i32) -> bool {
    let mut changed = put_int(nbt, STATE, diode.state);
    changed |= put_int(nbt, MAX_STATE, diode.max_state);
    changed |= put_int(nbt, IN_DELAY, delay);
    changed |= put_flag(nbt, POWERED, diode.powered);
    changed |= put_flag(nbt, POWERING, diode.powering);
    changed |= put_flag(nbt, INVERTED, diode.inverted);
    changed
}

/// Returns `(mirror, sync)`: the block state must follow / clients need the
/// block entity again.
fn tick_diode(game: &mut Game<'_>, pos: Pos, nbt: &mut Compound, kind: DiodeKind) -> (bool, bool) {
    let Some(facing) = facing_prop(game, pos) else { return (false, false) };
    // `getInputSignal`: the input is on the FACING side.
    let input = input_signals(game, pos, &[facing])[0] > 0;
    let mut diode = load_diode(nbt, kind);
    let mut delay = int(nbt, IN_DELAY);
    if input != diode.powered {
        delay += 1;
        if delay >= rules::DIODE_BLOCK_DELAY {
            diode.powered = input;
            delay = 0;
        }
    } else {
        delay = 0;
    }
    diode.tick();
    (store_diode(nbt, &diode, delay), false)
}

fn tick_toggle_latch(game: &mut Game<'_>, pos: Pos, nbt: &mut Compound) -> (bool, bool) {
    let Some(facing) = facing_prop(game, pos) else { return (false, false) };
    let should = input_signals(game, pos, &[facing])[0] > 0;
    let mut latch = ToggleLatch { powered: flag(nbt, POWERED), powering: flag(nbt, POWERING) };
    let mut again = flag(nbt, AGAIN);
    if latch.powered != should || again {
        again = latch.tick(should);
    }
    let mut changed = put_flag(nbt, POWERED, latch.powered);
    changed |= put_flag(nbt, POWERING, latch.powering);
    changed |= put_flag(nbt, AGAIN, again);
    (changed, false)
}

fn tick_powered_latch(game: &mut Game<'_>, pos: Pos, nbt: &mut Compound) -> (bool, bool) {
    let Some(facing) = facing_prop(game, pos) else { return (false, false) };
    let inputs = input_signals(game, pos, &[facing, facing.clockwise(), facing.counter_clockwise()]);
    let (back, side) = (inputs[0] > 0, inputs[1] > 0 || inputs[2] > 0);
    let mut latch = PoweredLatch {
        powered: flag(nbt, POWERED),
        powered_side: flag(nbt, POWERED_SIDE),
        powering: flag(nbt, POWERING),
    };
    if latch.needs_tick(back, side) {
        latch.tick(back, side);
    }
    let mut changed = put_flag(nbt, POWERED, latch.powered);
    changed |= put_flag(nbt, POWERED_SIDE, latch.powered_side);
    changed |= put_flag(nbt, POWERING, latch.powering);
    (changed, false)
}

/// `RedstoneContactBlock.tick`: powered while the block in front is a contact
/// facing back at this one. Contraption contacts are not ported.
fn tick_contact(game: &mut Game<'_>, pos: Pos, nbt: &mut Compound) -> (bool, bool) {
    let Some(facing) = facing_prop(game, pos) else { return (false, false) };
    let [dx, dy, dz] = facing.step();
    let at = [pos[0] + dx, pos[1] + dy, pos[2] + dz];
    let neighbour = if game.native_block(at).to_string() == CONTACT {
        game.native_block_property(at, "facing")
            .as_deref()
            .and_then(named)
            .map(|f| (true, f))
    } else {
        None
    };
    let valid = rules::contact_has_valid_contact(neighbour, facing);
    match rules::contact_next_powered(flag(nbt, POWERED), valid) {
        Some(next) => (put_flag(nbt, POWERED, next), false),
        None => (false, false),
    }
}

fn tick_nixie(game: &mut Game<'_>, pos: Pos, nbt: &mut Compound) -> (bool, bool) {
    let mount = game.native_block_property(pos, "mount").unwrap_or_default();
    let attached = match mount_parts(&mount) {
        Some(("floor", _)) => Dir::Down,
        Some(("ceiling", _)) => Dir::Up,
        Some((_, dir)) => dir.opposite(),
        None => Dir::Down,
    };
    let signals = input_signals(game, pos, &Dir::ALL);
    let mut direct = [0; 6];
    direct.copy_from_slice(&signals);
    // `getPower` also probes `getSignal(.., UP)` on the wires around the tube;
    // the port reuses the directional input for both.
    let power = tube::tube_power(direct, direct, attached);
    let mut text = NixieTube::new();
    text.update_displayed_redstone_value(power, true);
    let changed = put_int(nbt, STRENGTH, text.redstone_strength);
    (false, changed)
}

pub(super) fn tick(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity) {
    if !simulates(game) || !game.native_on("create") || !contraption::resident(game, pos) {
        return;
    }
    let Some(kind) = kind(&entity.id) else { return };
    if game.native_block(pos).to_string() != entity.id {
        return;
    }
    let (mirror, sync) = match kind {
        Kind::Diode(k) => tick_diode(game, pos, &mut entity.nbt, k),
        Kind::ToggleLatch => tick_toggle_latch(game, pos, &mut entity.nbt),
        Kind::PoweredLatch => tick_powered_latch(game, pos, &mut entity.nbt),
        Kind::Contact => tick_contact(game, pos, &mut entity.nbt),
        Kind::Nixie => tick_nixie(game, pos, &mut entity.nbt),
        Kind::Lever => (false, false),
    };
    if mirror {
        queue(game, pos);
    }
    if sync {
        game.native_block_entity_sync(pos);
    }
}

// ----------------------------------------------------------- interaction

fn click(game: &mut Game<'_>, pos: Pos, pitch: f32) {
    let at = pos.map(|v| f64::from(v) + 0.5);
    game.with_level(|_, level, _| {
        level.native_play_sound("minecraft:block.lever.click", at, 4, 0.3, pitch);
    });
}

fn holds_wrench(stack: &ItemStack) -> bool {
    stack.item.as_ref() == "create:wrench"
}

/// Right click on a switch. Empty hand or the Wrench only; any other item falls
/// through to normal placement.
/// * Pulse Repeater / Extender / Timer: toggles `inverted`.
/// * Powered Toggle Latch / Powered Latch: toggles the stored output (the
///   Powered Latch refuses while its two inputs disagree).
/// * Analog Lever: +1, sneaking -1 (0..15).
pub(super) fn right_click(game: &mut Game<'_>, pos: Pos) -> bool {
    if !simulates(game)
        || !game.native_on("create")
        || game.held_slot() != game.selected
        || !game.may_build()
    {
        return false;
    }
    let block = game.native_block(pos).to_string();
    let Some(kind) = kind(&block) else { return false };
    let held = game.stack_in(game.selected).filter(|stack| stack.count > 0).cloned();
    if held.as_ref().is_some_and(|stack| !holds_wrench(stack)) {
        return false;
    }
    let sneaking = game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft);
    match kind {
        Kind::Diode(k) => {
            let Some(pitch) = game.native_with_mod_block_entity(pos, |entity| {
                let mut diode = load_diode(&entity.nbt, k);
                let pitch = diode.toggle_inverted();
                put_flag(&mut entity.nbt, INVERTED, diode.inverted);
                pitch
            }) else {
                return false;
            };
            queue(game, pos);
            click(game, pos, pitch);
            true
        }
        Kind::ToggleLatch => {
            let Some(pitch) = game.native_with_mod_block_entity(pos, |entity| {
                let mut latch = ToggleLatch {
                    powered: flag(&entity.nbt, POWERED),
                    powering: flag(&entity.nbt, POWERING),
                };
                let pitch = latch.activated();
                put_flag(&mut entity.nbt, POWERING, latch.powering);
                pitch
            }) else {
                return false;
            };
            queue(game, pos);
            click(game, pos, pitch);
            true
        }
        Kind::PoweredLatch => {
            let Some(result) = game.native_with_mod_block_entity(pos, |entity| {
                let mut latch = PoweredLatch {
                    powered: flag(&entity.nbt, POWERED),
                    powered_side: flag(&entity.nbt, POWERED_SIDE),
                    powering: flag(&entity.nbt, POWERING),
                };
                let pitch = latch.activated();
                if pitch.is_some() {
                    put_flag(&mut entity.nbt, POWERING, latch.powering);
                }
                pitch
            }) else {
                return false;
            };
            if let Some(pitch) = result {
                queue(game, pos);
                click(game, pos, pitch);
            }
            true
        }
        Kind::Lever => {
            let power = game
                .native_block_property(pos, "power")
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(0);
            let mut lever = AnalogLever { state: power, last_change: 0 };
            if lever.change_state(sneaking) {
                game.native_set_block_property(pos, "power", &lever.state.to_string());
                let dimension = game.dim.id().to_owned();
                game.native_update_signal_neighbors(&dimension, pos);
                click(game, pos, lever.click_pitch());
            }
            true
        }
        Kind::Contact | Kind::Nixie => false,
    }
}

// ---------------------------------------------------------------- hooks

/// `block_signal` hook: `getSignal` / `getDirectSignal` of the switches.
/// `face` is the side of the switch that touches the reader; the vanilla
/// direction handed to the block is its opposite.
pub(super) fn signal(
    game: &GameLevel<'_>,
    _pos: BlockPos,
    state: BlockStateId,
    face: Face,
    direct: bool,
    signal: &mut i32,
) {
    let Some(block) = game.registry.state(state) else { return };
    if kind(&block.name).is_none() {
        return;
    }
    let get = |key: &str| block.properties.get(key).map(|v| v.to_string());
    *signal = output(&block.name, &get, dir_of(face).opposite(), direct);
}

/// `is_signal_source` hook: wire connects to the switches.
pub(super) fn signal_source(game: &GameLevel<'_>, state: BlockStateId, source: &mut bool) {
    let Some(block) = game.registry.state(state) else { return };
    if matches!(
        kind(&block.name),
        Some(Kind::Diode(_) | Kind::ToggleLatch | Kind::PoweredLatch | Kind::Lever | Kind::Contact)
    ) {
        *source = true;
    }
}

/// The neighbour a switch is attached to, as a direction from the switch.
fn support(block: &str, facing: Option<Dir>) -> Option<Dir> {
    match kind(block)? {
        Kind::Diode(_) | Kind::ToggleLatch | Kind::PoweredLatch => Some(Dir::Down),
        Kind::Lever => facing.map(Dir::opposite),
        Kind::Contact | Kind::Nixie => None,
    }
}

/// `can_survive` hook: diodes and latches stand on a block, the lever hangs on
/// the block behind it (`DiodeBlock.canSurvive`, `FaceAttachedHorizontalDirectionalBlock`).
pub(super) fn can_survive(
    game: &GameLevel<'_>,
    pos: BlockPos,
    state: BlockStateId,
    survives: &mut Option<bool>,
) {
    let Some(block) = game.registry.state(state) else { return };
    let facing = block.properties.get("facing").and_then(|name| named(name));
    let Some(support) = support(&block.name, facing) else { return };
    let [dx, dy, dz] = support.step();
    let below = game.world.block(pos.x + dx, pos.y + dy, pos.z + dz);
    *survives = Some(!game.facts.get(below).replaceable);
}

/// `update_shape` hook: losing the support removes the switch.
pub(super) fn shape(
    game: &GameLevel<'_>,
    state: BlockStateId,
    changed: Face,
    neighbour: BlockStateId,
    new: &mut Option<BlockStateId>,
) {
    let Some(block) = game.registry.state(state) else { return };
    let facing = block.properties.get("facing").and_then(|name| named(name));
    let Some(support) = support(&block.name, facing) else { return };
    if dir_of(changed) == support && game.facts.get(neighbour).replaceable {
        *new = Some(game.world.air);
    }
}

// ------------------------------------------------------------- placement

fn face_name(face: Face) -> &'static str {
    dir_name(dir_of(face))
}

/// The direction the player looks along, from the yaw (as
/// `create_port_placement::horizontal`).
fn player_facing(yaw: f32) -> Dir {
    [Dir::South, Dir::West, Dir::North, Dir::East][((f64::from(yaw) / 90.0 + 0.5).floor() as i64 & 3) as usize]
}

/// `state_for_placement` hook.
/// * diodes / latches: `facing` = opposite of the player's look (the input side
///   points at the player, as vanilla repeaters);
/// * lever / contact: `facing` = the clicked face;
/// * tube: `mount` = floor / ceiling / wall on the clicked face.
pub(super) fn placement(
    game: &GameLevel<'_>,
    state: BlockStateId,
    ctx: &PlacementContext,
    answer: &mut Option<Option<BlockStateId>>,
) {
    let Some(block) = game.native_state_block(state) else { return };
    let Some(kind) = kind(block) else { return };
    let towards_player = player_facing(ctx.yaw).opposite();
    *answer = Some(match kind {
        Kind::Diode(_) | Kind::ToggleLatch | Kind::PoweredLatch => {
            game.native_state_with_property(state, "facing", dir_name(towards_player))
        }
        Kind::Lever | Kind::Contact => game.native_state_with_property(state, "facing", face_name(ctx.face)),
        Kind::Nixie => {
            let mount = match ctx.face {
                Face::Up => format!("floor_{}", dir_name(towards_player)),
                Face::Down => format!("ceiling_{}", dir_name(towards_player)),
                side => format!("wall_{}", face_name(side)),
            };
            game.native_state_with_property(state, "mount", &mount)
        }
    });
}

// ------------------------------------------------------------------ data

/// `block_entity_update_tag` hook: clients need the displayed strength.
pub(super) fn update_tag(entity: &ModBlockEntity, tag: &mut Option<Compound>) {
    if entity.id == NIXIE && tag.is_none() {
        *tag = Some(Compound::from([(STRENGTH.into(), Tag::Int(int(&entity.nbt, STRENGTH)))]));
    }
}

/// `block_entity` hook.
pub(super) fn entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if has_entity(block) && entity.is_none() {
        *entity = Some((block.into(), default_nbt(block)));
    }
}

/// `block_entity_ticks` hook.
pub(super) fn ticks(block: &str, ticks: &mut bool) {
    if has_entity(block) {
        *ticks = true;
    }
}

// ------------------------------------------------------------------ draw

/// Glow colour of the digits (an orange Nixie tube).
const GLYPH: u32 = 0xFF8A1A;

/// `block_entity_submit` hook: the two characters of a Nixie tube on its
/// display face. The poses repeat the blockstate rotations of the `mount`.
pub(super) fn draw(
    game: &Game<'_>,
    _pos: Pos,
    block: &str,
    state: BlockStateId,
    entity: &ModBlockEntity,
    draw: &mut BlockEntityDraw,
) {
    if block != NIXIE {
        return;
    }
    let Some(mount) = game
        .pack
        .assets
        .registry
        .state(state)
        .and_then(|block| block.properties.get("mount").map(|v| v.to_string()))
    else {
        return;
    };
    let Some((face, dir)) = mount_parts(&mount) else { return };
    let (x_degrees, y_degrees) = mount_turn(face, dir);
    let mut tube = NixieTube::new();
    tube.redstone_strength = int(&entity.nbt, STRENGTH);
    let (first, second) = tube.displayed_strings();
    let text = Text::literal(format!("{first}{second}"));
    draw.push();
    // The blockstate turns the model about x, then y.
    draw.rotate_about([0.0, 1.0, 0.0], -y_degrees, [0.5; 3]);
    draw.rotate_about([1.0, 0.0, 0.0], -x_degrees, [0.5; 3]);
    if face == "wall" {
        // Keep the characters upright once the tube is tilted onto the wall.
        draw.rotate_about([0.0, 1.0, 0.0], 180.0, [0.5; 3]);
    }
    // Text plane: x right, y up, facing the top of the floor tube.
    draw.translate([0.5, 11.0 / 16.0 + 0.002, 0.5]);
    draw.rotate_about([1.0, 0.0, 0.0], -90.0, [0.0; 3]);
    draw.scale([1.0 / 32.0, -1.0 / 32.0, 1.0 / 32.0]);
    game.native_draw_text(draw, &text, [-6.0, -4.0], GLYPH, true, 0);
    draw.pop();
    draw.keep();
}
