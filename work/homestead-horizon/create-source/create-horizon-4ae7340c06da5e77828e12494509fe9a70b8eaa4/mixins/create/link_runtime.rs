// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

// Redstone Link (RedstoneLinkBlock / RedstoneLinkBlockEntity / LinkBehaviour):
// block entity state, the per-level frequency network, transmitter polling,
// receiver output and frequency / mode interaction. The rules live in
// `create_engine::redstone_link`; this file only adapts them to the game.
//
// Block states: `facing` (the face the plate looks out of; it is attached to the
// opposite neighbour), `powered`, `receiver` (false = transmitter, the default).
// Receiver output is graded (0..15): `signal` / `signal_source` answer the
// `block_signal` / `is_signal_source` hooks from the block entity.
//
// Ownership of the network: a thread-local `LinkNetwork` per (world, dimension),
// exactly like the other session caches of this mod. It is not saved. Every
// link registers lazily on its first tick after the chunk loads (the block
// entity NBT is the source of truth) and a throttled sweep removes entries whose
// chunk was unloaded or whose block is gone (`LinkBehaviour.unload/remove`).
// Receiver side effects (property `powered`, neighbour update) are deferred to
// `flush`, called after the block entities have ticked, because a ticking block
// entity is not in the world while it runs and must not be read by the signal hook.

use super::{Game, GameLevel, create_port_contraption as contraption};
use crate::{containers::{BlockEntity, ModBlockEntity}, items::ItemStack};
use create_engine::{
    redstone_link::{
        self as link, Direction, Frequency, FrequencyPair, LinkId, LinkMode, LinkNetwork,
    },
    transport::SessionCache,
};
use horizon_core::{geometry::Direction as Face, position::BlockPos};
use horizon_items::item_nbt::{item_from_nbt, item_to_nbt};
use horizon_world::{
    level::flags,
    nbt::{Compound, Tag},
    registry::BlockStateId,
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};

pub(super) const BLOCK: &str = "create:redstone_link";
type Pos = [i32; 3];

// Block entity NBT. Names follow RedstoneLinkBlockEntity / LinkBehaviour where
// they are known (`Transmit`, `Receive`, `FrequencyFirst`, `FrequencyLast`).
const FIRST: &str = "FrequencyFirst";
const LAST: &str = "FrequencyLast";
const TRANSMIT: &str = "Transmit";
const RECEIVE: &str = "Receive";
/// Which frequency slot the next item click fills (port simplification of the two
/// ghost slots): 0 = first, 1 = last.
const NEXT_SLOT: &str = "create_horizon:LinkNextSlot";
/// Ticks between two liveness sweeps of a level's network.
const SWEEP_PERIOD: i64 = 10;

struct Dimension {
    net: LinkNetwork,
    ids: BTreeMap<Pos, LinkId>,
    /// Receivers whose property / neighbours must be settled by `flush`.
    pending: BTreeSet<Pos>,
    swept: i64,
}

impl Default for Dimension {
    fn default() -> Self {
        Self {
            net: LinkNetwork::new(link::DEFAULT_LINK_RANGE),
            ids: BTreeMap::new(),
            pending: BTreeSet::new(),
            swept: i64::MIN,
        }
    }
}

thread_local! {
    static NETWORKS: RefCell<SessionCache<Dimension>> = RefCell::new(Default::default());
}

fn simulates(game: &Game<'_>) -> bool {
    game.serving.is_some() || game.options.dedicated.is_some()
}

fn with_dimension<R>(game: &Game<'_>, dimension: &str, f: impl FnOnce(&mut Dimension) -> R) -> R {
    let session = std::ptr::from_ref(&game.world) as usize;
    NETWORKS.with_borrow_mut(|cache| f(cache.dimension(session, dimension)))
}

fn with_net<R>(game: &Game<'_>, f: impl FnOnce(&mut Dimension) -> R) -> R {
    with_dimension(game, game.dim.id(), f)
}

/// Positions of the receivers written by the network, in write order.
fn positions(dimension: &Dimension, writes: Vec<(LinkId, i32)>) -> Vec<(Pos, i32)> {
    writes
        .into_iter()
        .filter_map(|(id, strength)| dimension.net.get(id).map(|entry| (entry.pos, strength)))
        .collect()
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

pub(super) fn default_nbt() -> Compound {
    Compound::from([
        (TRANSMIT.into(), Tag::Int(0)),
        (RECEIVE.into(), Tag::Int(0)),
    ])
}

/// What clients receive: the frequency and the signal (for a future renderer).
pub(super) fn update_tag(entity: &ModBlockEntity) -> Compound {
    [FIRST, LAST, TRANSMIT, RECEIVE]
        .into_iter()
        .filter_map(|key| entity.nbt.get(&key.into()).map(|tag| (key.into(), tag.clone())))
        .collect()
}

/// `Frequency.of(stack)`: item identity plus the dye colour component.
fn frequency(stack: &ItemStack) -> Frequency {
    Frequency::of(&stack.item, stack.data().and_then(|data| data.dyed_color))
}

fn slot(nbt: &Compound, key: &str) -> Frequency {
    nbt.get(&key.into())
        .and_then(item_from_nbt)
        .map_or_else(Frequency::empty, |stack| frequency(&stack))
}

fn pair(nbt: &Compound) -> FrequencyPair {
    (slot(nbt, FIRST), slot(nbt, LAST))
}

/// Records a received strength; true when the stored value changed.
fn store_received(nbt: &mut Compound, strength: i32) -> bool {
    let known = nbt.contains_key(&RECEIVE.into());
    let changed = !known || int(nbt, RECEIVE) != strength;
    if changed {
        nbt.insert(RECEIVE.into(), Tag::Int(strength));
    }
    changed
}

// ---------------------------------------------------------------- states

fn direction(face: Face) -> Direction {
    match face {
        Face::Down => Direction::Down,
        Face::Up => Direction::Up,
        Face::North => Direction::North,
        Face::South => Direction::South,
        Face::West => Direction::West,
        Face::East => Direction::East,
    }
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

fn is_receiver(game: &Game<'_>, pos: Pos) -> bool {
    game.native_block_property(pos, "receiver").as_deref() == Some("true")
}

fn truth(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

// --------------------------------------------------------------- network

/// Registers the link at `pos` or brings its entry up to date with `nbt` and the
/// mode. Returns the receiver writes and whether the entry was newly created.
fn update_link(game: &Game<'_>, pos: Pos, nbt: &Compound, receiver: bool) -> (Vec<(Pos, i32)>, bool) {
    let key = pair(nbt);
    with_net(game, |d| {
        let mode = if receiver {
            LinkMode::Receive { received: int(nbt, RECEIVE) }
        } else {
            LinkMode::Transmit { strength: int(nbt, TRANSMIT) }
        };
        let existing = d.ids.get(&pos).copied();
        let same_kind = existing.and_then(|id| d.net.get(id)).is_some_and(|entry| {
            matches!(
                (entry.mode, receiver),
                (LinkMode::Receive { .. }, true) | (LinkMode::Transmit { .. }, false)
            )
        });
        let mut created = false;
        let writes = match existing {
            Some(id) if same_kind => {
                let mut out = Vec::new();
                if d.net.get(id).is_some_and(|entry| entry.key != key) {
                    let writes = d.net.set_frequency(id, key);
                    out.extend(positions(d, writes));
                }
                // A block replaced in place keeps its registry entry: bring the
                // transmitted strength back in line with the block entity.
                if !receiver
                    && d.net.get(id).is_some_and(|entry| {
                        matches!(entry.mode, LinkMode::Transmit { strength } if strength != int(nbt, TRANSMIT))
                    })
                {
                    let writes = d.net.transmit(id, int(nbt, TRANSMIT));
                    out.extend(positions(d, writes));
                }
                out
            }
            old => {
                // New block, or the mode was toggled: leave and rejoin.
                let mut out = Vec::new();
                if let Some(id) = old {
                    d.ids.remove(&pos);
                    let writes = d.net.remove_from_network(id);
                    out.extend(positions(d, writes));
                }
                let id = d.net.create(pos, key, mode);
                d.ids.insert(pos, id);
                let writes = d.net.add_to_network(id);
                out.extend(positions(d, writes));
                created = true;
                out
            }
        };
        (writes, created)
    })
}

/// `RedstoneLinkBlockEntity.transmit`.
fn transmit(game: &Game<'_>, pos: Pos, strength: i32) -> Vec<(Pos, i32)> {
    with_net(game, |d| {
        let Some(id) = d.ids.get(&pos).copied() else { return Vec::new() };
        let writes = d.net.transmit(id, strength);
        positions(d, writes)
    })
}

/// Entries whose chunk is gone or whose block is no longer a link leave the
/// network; their former receivers are re-evaluated. Throttled per level.
fn sweep(game: &Game<'_>, own: Pos) -> Vec<(Pos, i32)> {
    let now = game.world.game_time;
    let due = with_net(game, |d| {
        let due = now < d.swept || now - d.swept >= SWEEP_PERIOD;
        if due {
            d.swept = now;
        }
        due
    });
    if !due {
        return Vec::new();
    }
    let entries: Vec<(Pos, LinkId)> =
        with_net(game, |d| d.ids.iter().map(|(pos, id)| (*pos, *id)).collect());
    let gone: Vec<(Pos, LinkId)> = entries
        .into_iter()
        .filter(|(pos, _)| {
            *pos != own && (!contraption::resident(game, *pos) || game.native_block(*pos) != BLOCK)
        })
        .collect();
    if gone.is_empty() {
        return Vec::new();
    }
    with_net(game, |d| {
        let mut out = Vec::new();
        for (pos, id) in gone {
            d.ids.remove(&pos);
            let writes = d.net.remove_from_network(id);
            out.extend(positions(d, writes));
        }
        out
    })
}

fn queue(game: &Game<'_>, pos: Pos) {
    with_net(game, |d| d.pending.insert(pos));
}

/// Stores the received strengths in the receivers' block entities and queues the
/// ones that changed for `flush`. `own` is the entity being ticked (it is out of
/// the world, so it is written directly).
fn apply(game: &mut Game<'_>, mut own: Option<(Pos, &mut Compound)>, writes: Vec<(Pos, i32)>) {
    for (target, strength) in writes {
        let is_own = own.as_ref().is_some_and(|(pos, _)| *pos == target);
        let changed = if is_own {
            match own.as_mut() {
                Some((_, nbt)) => store_received(nbt, strength),
                None => false,
            }
        } else {
            game.native_with_mod_block_entity(target, |entity| {
                entity.id == BLOCK && store_received(&mut entity.nbt, strength)
            })
            .unwrap_or(false)
        };
        if changed {
            queue(game, target);
        }
    }
}

/// Settles queued receivers: `powered` follows `Receive > 0` and the blocks around
/// re-read the signal. Call after the block entities of the level have ticked.
pub(super) fn flush(game: &mut Game<'_>, dimension: &str) {
    if !simulates(game) || !game.native_on("create") {
        return;
    }
    let pending = with_dimension(game, dimension, |d| std::mem::take(&mut d.pending));
    for pos in pending {
        if !contraption::resident(game, pos) || game.native_block(pos) != BLOCK || !is_receiver(game, pos) {
            continue;
        }
        let received = match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
            Some(BlockEntity::Mod(entity)) if entity.id == BLOCK => int(&entity.nbt, RECEIVE),
            _ => continue,
        };
        let powered = game.native_block_property(pos, "powered").as_deref() == Some("true");
        let (next, _) = link::receiver_tick(received, powered, false);
        if next != powered {
            game.native_set_block_property(pos, "powered", truth(next));
        }
        game.native_update_signal_neighbors(dimension, pos);
    }
}

// ------------------------------------------------------------------ tick

pub(super) fn tick(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity) {
    if !simulates(game) || !game.native_on("create") || !contraption::resident(game, pos) {
        return;
    }
    let receiver = is_receiver(game, pos);
    let mut writes = sweep(game, pos);
    let (registered, created) = update_link(game, pos, &entity.nbt, receiver);
    writes.extend(registered);
    if receiver {
        let received = int(&entity.nbt, RECEIVE);
        let powered = game.native_block_property(pos, "powered").as_deref() == Some("true");
        if created || link::receiver_tick(received, powered, false).1 {
            queue(game, pos);
        }
    } else {
        let dim = game.dim.id().to_owned();
        let power = game.native_neighbor_signal(&dim, pos).clamp(0, link::MAX_SIGNAL);
        if created || power != int(&entity.nbt, TRANSMIT) {
            entity.nbt.insert(TRANSMIT.into(), Tag::Int(power));
            writes.extend(transmit(game, pos, power));
        }
        // updateTransmittedSignal mirrors the input into POWERED (flag 2).
        let lit = power > 0;
        if game.native_block_property(pos, "powered").as_deref() != Some(truth(lit)) {
            game.native_set_block_property_with(pos, "powered", truth(lit), flags::UPDATE_CLIENTS);
        }
    }
    apply(game, Some((pos, &mut entity.nbt)), writes);
}

// ----------------------------------------------------------- interaction

fn holds_wrench(stack: &ItemStack) -> bool {
    stack.item.as_ref() == "create:wrench"
}

/// Right click on a link.
/// * Item in hand (not sneaking): sets the first, then the last frequency.
///   Create targets the two slots on the plate with the crosshair; this port
///   alternates because the hook has no hit position.
/// * Empty hand, or the Wrench: toggles transmitter / receiver
///   (`RedstoneLinkBlock.toggleMode`).
/// * Sneaking with an item falls through to normal block placement.
pub(super) fn right_click(game: &mut Game<'_>, pos: Pos) -> bool {
    if !simulates(game)
        || !game.native_on("create")
        || game.held_slot() != game.selected
        || !game.may_build()
        || game.native_block(pos) != BLOCK
    {
        return false;
    }
    let sneaking = game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft);
    let held = game.stack_in(game.selected).filter(|stack| stack.count > 0).cloned();
    match held {
        Some(stack) if holds_wrench(&stack) => !sneaking && toggle(game, pos),
        Some(stack) => !sneaking && set_frequency(game, pos, &stack),
        // Sneaking with an empty hand clears both frequency slots (port addition:
        // Create clears a ghost slot in its own slot UI, which this port lacks).
        None if sneaking => reset_frequency(game, pos),
        None => toggle(game, pos),
    }
}

/// Clears `FrequencyFirst` / `FrequencyLast` and re-keys the link in the network.
fn reset_frequency(game: &mut Game<'_>, pos: Pos) -> bool {
    let Some(Some(nbt)) = game.native_with_mod_block_entity(pos, |entity| {
        if entity.id != BLOCK {
            return None;
        }
        entity.nbt.remove(&FIRST.into());
        entity.nbt.remove(&LAST.into());
        entity.nbt.insert(NEXT_SLOT.into(), Tag::Byte(0));
        Some(entity.nbt.clone())
    }) else {
        return false;
    };
    reconcile(game, pos, &nbt);
    true
}

fn set_frequency(game: &mut Game<'_>, pos: Pos, stack: &ItemStack) -> bool {
    let mut item = stack.clone();
    item.count = 1;
    let Some(Some(nbt)) = game.native_with_mod_block_entity(pos, |entity| {
        if entity.id != BLOCK {
            return None;
        }
        let last = flag(&entity.nbt, NEXT_SLOT);
        entity
            .nbt
            .insert(if last { LAST } else { FIRST }.into(), item_to_nbt(&item));
        entity.nbt.insert(NEXT_SLOT.into(), Tag::Byte(i8::from(!last)));
        Some(entity.nbt.clone())
    }) else {
        return false;
    };
    reconcile(game, pos, &nbt);
    true
}

/// `RedstoneLinkBlock.toggleMode`.
fn toggle(game: &mut Game<'_>, pos: Pos) -> bool {
    let dim = game.dim.id().to_owned();
    let was_receiver = is_receiver(game, pos);
    let neighbor = game.native_neighbor_signal(&dim, pos);
    let current = game
        .native_with_mod_block_entity(pos, |entity| int(&entity.nbt, TRANSMIT))
        .unwrap_or(0);
    let result = link::toggle_mode(was_receiver, neighbor > 0, current);
    game.native_with_mod_block_entity(pos, |entity| {
        entity.nbt.insert(TRANSMIT.into(), Tag::Int(result.transmit));
        entity.nbt.insert(RECEIVE.into(), Tag::Int(0));
    });
    game.native_set_block_property(pos, "receiver", truth(result.receiver));
    game.native_set_block_property(pos, "powered", truth(result.powered));
    if let Some(Some(nbt)) = game.native_with_mod_block_entity(pos, |entity| {
        (entity.id == BLOCK).then(|| entity.nbt.clone())
    }) {
        reconcile(game, pos, &nbt);
    }
    true
}

/// Brings the network up to date after the block entity changed outside a tick.
fn reconcile(game: &mut Game<'_>, pos: Pos, nbt: &Compound) {
    let receiver = is_receiver(game, pos);
    let (writes, _) = update_link(game, pos, nbt, receiver);
    apply(game, None, writes);
    if receiver {
        queue(game, pos);
    }
    game.native_block_entity_sync(pos);
}

// ---------------------------------------------------------------- hooks

/// `block_signal` hook: `RedstoneLinkBlock.getSignal` / `getDirectSignal`.
/// `face` is the side of the link that touches the reader; the vanilla
/// direction handed to the block is its opposite.
pub(super) fn signal(
    game: &GameLevel<'_>,
    pos: BlockPos,
    state: BlockStateId,
    face: Face,
    direct: bool,
    signal: &mut i32,
) {
    let Some(block) = game.registry.state(state).filter(|block| block.name == BLOCK) else {
        return;
    };
    let receiver = block.properties.get("receiver").is_some_and(|v| v == "true");
    if !receiver {
        *signal = 0;
        return;
    }
    let received = match game.world.block_entities.get(&(pos.x, pos.y, pos.z)) {
        Some(BlockEntity::Mod(entity)) => int(&entity.nbt, RECEIVE),
        _ => 0,
    };
    *signal = if direct {
        let Some(facing) = block.properties.get("facing").and_then(|name| named(name)) else {
            return;
        };
        link::link_block_direct_signal(true, received, facing, direction(face).opposite())
    } else {
        link::link_block_signal(true, received)
    };
}

/// `is_signal_source` hook: `POWERED && RECEIVER`.
pub(super) fn signal_source(game: &GameLevel<'_>, state: BlockStateId, source: &mut bool) {
    let Some(block) = game.registry.state(state).filter(|block| block.name == BLOCK) else {
        return;
    };
    let is = |name: &str| block.properties.get(name).is_some_and(|v| v == "true");
    *source = link::link_block_is_signal_source(is("powered"), is("receiver"));
}

/// `can_survive` hook: `RedstoneLinkBlock.canSurvive`, the attached neighbour must
/// not be replaceable.
pub(super) fn can_survive(
    game: &GameLevel<'_>,
    pos: BlockPos,
    state: BlockStateId,
    survives: &mut Option<bool>,
) {
    let Some(block) = game.registry.state(state).filter(|block| block.name == BLOCK) else {
        return;
    };
    let Some(facing) = block.properties.get("facing").and_then(|name| named(name)) else {
        return;
    };
    let [dx, dy, dz] = facing.opposite().step();
    let support = game.world.block(pos.x + dx, pos.y + dy, pos.z + dz);
    *survives = Some(!game.facts.get(support).replaceable);
}

/// `update_shape` hook: losing the attached neighbour removes the link
/// (`neighborChanged` / `updateOrDestroy`).
pub(super) fn shape(
    game: &GameLevel<'_>,
    state: BlockStateId,
    changed: Face,
    neighbour: BlockStateId,
    new: &mut Option<BlockStateId>,
) {
    let Some(block) = game.registry.state(state).filter(|block| block.name == BLOCK) else {
        return;
    };
    let Some(facing) = block.properties.get("facing").and_then(|name| named(name)) else {
        return;
    };
    if direction(changed) == facing.opposite() && game.facts.get(neighbour).replaceable {
        *new = Some(game.world.air);
    }
}

/// Linked Controller access to the level's network (controller_runtime.rs). `f` gets the network and
/// answers whether receiver values may have changed; if so every receiver is re-read.
pub(super) fn controller_net<R>(
    game: &mut Game<'_>,
    dimension: &str,
    f: impl FnOnce(&mut LinkNetwork) -> (R, bool),
) -> R {
    let (result, writes) = with_dimension(game, dimension, |d| {
        let (result, scan) = f(&mut d.net);
        let writes: Vec<(Pos, i32)> = if scan {
            d.ids
                .iter()
                .filter_map(|(pos, id)| {
                    d.net
                        .get(*id)
                        .filter(|entry| matches!(entry.mode, LinkMode::Receive { .. }))
                        .map(|entry| (*pos, entry.received()))
                })
                .collect()
        } else {
            Vec::new()
        };
        (result, writes)
    });
    apply(game, None, writes);
    result
}
