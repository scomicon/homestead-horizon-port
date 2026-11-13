// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

// General fluid pipe network (Create 6.0.10 `content/fluids`): the game adapter
// of `create_engine::pipes`. It replaces the straight-line `create_port_basin::pump`
// tick: a long-lived `PipeSim` per (world, dimension) is driven from
//   * `maintenance`  (level_tick_pre): drain block events, reconcile world -> sim,
//                    `PipeSim::tick`, write sim-owned pipe shapes back to the world;
//   * `tick`/`tick_valve` (block entity ticks): register loaded blocks, restore the
//                    saved NBT, feed pump speed, save pressure / flow for clients;
//   * `changed`      (block_changed observer, no Game borrow): queue a position.
// See docs/fluid-pipe-network.md. Written against the staged engine by reading
// signatures; not compiled in game.

use super::{
    Game, GameLevel, create_port_basin as basin, create_port_basin_recipes::item_drain,
    native_block_entity_draw::BlockEntityDraw,
};
use crate::containers::ModBlockEntity;
use create_engine::{
    pipes::{self, Axis, Dir, Fluid, Flow, Neighbor, PipeBlock, PipeEnv, PipeSim, Pressure},
    placement::Direction as EDir,
    transport::SessionCache,
};
use horizon_client::mod_api::{NativeFluidAction, NativeFluidDrain, NativeFluidStack};
use horizon_core::geometry::Direction as Face;
use horizon_world::{
    chunk::BlockState,
    level::{PlacementContext, flags},
    nbt::{Compound, Tag},
    registry::BlockStateId,
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};

pub(in crate::game) const PIPE: &str = "create:fluid_pipe";
pub(in crate::game) const PUMP: &str = "create:mechanical_pump";
pub(in crate::game) const VALVE: &str = "create:fluid_valve";
pub(in crate::game) const GLASS: &str = "create:glass_fluid_pipe";
pub(in crate::game) const ENCASED: &str = "create:encased_fluid_pipe";
pub(in crate::game) const SMART: &str = "create:smart_fluid_pipe";

/// Block entity key holding the smart pipe filter as a fluid id (no filter GUI yet).
const FILTER: &str = "create_horizon:FilterFluid";
/// Ticks between two liveness sweeps of a level's network.
const SWEEP_PERIOD: i64 = 100;

type Pos = [i32; 3];

// ------------------------------------------------------------------ state

/// Per (world, dimension) simulation. Not saved: rebuilt from blocks and block
/// entity NBT as the entities tick after a chunk load.
#[derive(Default)]
struct Net {
    sim: PipeSim,
    /// Registered through the load path (not placed): saved NBT still to restore.
    loaded: BTreeSet<Pos>,
    swept: i64,
}

/// Data shared with the observers that have no `Game` borrow.
#[derive(Default)]
struct Shared {
    /// Positions of every block in the simulation (to ignore unrelated changes).
    known: BTreeSet<Pos>,
    /// (position, block replaced) queued by `changed`.
    events: Vec<(Pos, bool)>,
    /// Positions this adapter is writing itself: their echo is not a neighbour change
    /// (`FluidPropagator.validateNeighbourChange` ignores liquid blocks).
    ignore: BTreeSet<Pos>,
}

impl Shared {
    fn near(&self, pos: Pos) -> bool {
        self.known.contains(&pos)
            || Dir::ALL.iter().any(|d| {
                let s = d.step();
                self.known.contains(&[pos[0] + s.0, pos[1] + s.1, pos[2] + s.2])
            })
    }
}

thread_local! {
    static NETS: RefCell<SessionCache<Net>> = RefCell::new(SessionCache::default());
    static SHARED: RefCell<BTreeMap<String, Shared>> = const { RefCell::new(BTreeMap::new()) };
}

/// Called from `create_port_basin::reset` next to `pump::reset()`.
pub(super) fn reset() {
    NETS.with(|nets| nets.borrow_mut().clear());
    SHARED.with(|shared| shared.borrow_mut().clear());
}

fn simulates(game: &Game<'_>) -> bool {
    game.serving.is_some() || game.options.dedicated.is_some()
}

fn shared<R>(dimension: &str, f: impl FnOnce(&mut Shared) -> R) -> R {
    SHARED.with(|s| f(s.borrow_mut().entry(dimension.to_owned()).or_default()))
}

fn tp(pos: Pos) -> pipes::Pos {
    (pos[0], pos[1], pos[2])
}

fn ap(pos: pipes::Pos) -> Pos {
    [pos.0, pos.1, pos.2]
}

fn dir_of(d: EDir) -> Dir {
    Dir::parse(d.name()).unwrap_or(Dir::North)
}

fn face_of(d: Dir) -> Face {
    match d {
        Dir::Down => Face::Down,
        Dir::Up => Face::Up,
        Dir::North => Face::North,
        Dir::South => Face::South,
        Dir::West => Face::West,
        Dir::East => Face::East,
    }
}

fn dir_from_vector(v: [i32; 3]) -> Option<Dir> {
    Dir::ALL.into_iter().find(|d| {
        let s = d.step();
        [s.0, s.1, s.2] == v
    })
}

fn truth(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

// -------------------------------------------------------------- world model

fn is_true(state: &BlockState, key: &str) -> bool {
    state.properties.get(key).is_some_and(|value| value == "true")
}

fn open_sides(state: &BlockState) -> [bool; 6] {
    std::array::from_fn(|i| is_true(state, Dir::ALL[i].name()))
}

pub(in crate::game) fn is_family(block: &str) -> bool {
    matches!(block, PIPE | PUMP | VALVE | GLASS | ENCASED | SMART)
}

/// Pipes without a kinetic input (their block entity ticks with rpm 0).
pub(in crate::game) fn is_passive(block: &str) -> bool {
    matches!(block, PIPE | GLASS | ENCASED | SMART)
}

/// `PipeBlock` of a block state; None for anything that is not a pipe-network block.
fn parse(state: &BlockState) -> Option<PipeBlock> {
    Some(match state.name.as_str() {
        PIPE => PipeBlock::Pipe { open: open_sides(state) },
        ENCASED => PipeBlock::Encased { open: open_sides(state) },
        GLASS => PipeBlock::Glass { axis: Axis::parse(state.properties.get("axis")?)? },
        PUMP => PipeBlock::Pump { facing: Dir::parse(state.properties.get("facing")?)? },
        VALVE => {
            // Same derivation as `pump::valve_axis(state, false)`.
            let facing = state.properties.get("facing")?;
            let front = EDir::ALL.into_iter().find(|d| d.name() == facing.as_str())?;
            let first = is_true(state, "axis_along_first");
            let axis = create_engine::placement::gauge_axis(front, !first);
            PipeBlock::Valve { axis: Axis::parse(axis.name())?, enabled: is_true(state, "enabled") }
        }
        SMART => {
            // SmartFluidPipeBlock: wall = vertical pipe, floor / ceiling = perpendicular to
            // `facing` (unverified: recalled as `facing.getClockWise().getAxis()`).
            let facing = Dir::parse(state.properties.get("facing")?)?;
            let axis = if state.properties.get("face").is_some_and(|f| f == "wall") {
                Axis::Y
            } else {
                match facing.axis() {
                    Axis::X => Axis::Z,
                    Axis::Z => Axis::X,
                    Axis::Y => Axis::Y,
                }
            };
            PipeBlock::Smart { axis, filter: None }
        }
        _ => return None,
    })
}

fn state<'a>(game: &'a Game<'_>, pos: Pos) -> Option<&'a BlockState> {
    let [x, y, z] = pos;
    if !game.world.loaded(x, z) {
        return None;
    }
    game.pack.assets.registry.state(game.world.block(x, y, z))
}

/// `FluidPropagator.hasFluidCapability` by block and face (`face` = the neighbour's
/// face that looks at the pipe). Mirrors `pump::storage_connection`, plus the Basin.
fn handler(block: &str, face: Dir) -> bool {
    match block {
        basin::tank::BLOCK | basin::ID => true,
        item_drain::BLOCK => face != Dir::Up,
        super::create_port_spout::BLOCK => face != Dir::Down,
        _ => false,
    }
}

fn fluid_of(id: &str) -> Option<Fluid> {
    match id {
        "minecraft:water" => Some(Fluid::WATER),
        "minecraft:lava" => Some(Fluid::LAVA),
        _ => None,
    }
}

fn fluid_id(fluid: Fluid) -> &'static str {
    match fluid {
        Fluid::LAVA => "minecraft:lava",
        _ => "minecraft:water",
    }
}

/// The fluid state of a block: (fluid, is_source). Waterlogged blocks hold a water source.
fn fluid_state(state: &BlockState) -> Option<(Fluid, bool)> {
    let source = state.properties.get("level").is_none_or(|level| level == "0");
    match state.name.as_str() {
        "minecraft:water" => Some((Fluid::WATER, source)),
        "minecraft:lava" => Some((Fluid::LAVA, source)),
        _ if is_true(state, "waterlogged") => Some((Fluid::WATER, true)),
        _ => None,
    }
}

fn replaceable(game: &Game<'_>, pos: Pos) -> bool {
    game.res.facts().get(game.world.block(pos[0], pos[1], pos[2])).replaceable
}

/// Writes a complete block state: the default state of `name` with `props`.
fn swap_block(game: &mut Game<'_>, pos: Pos, name: &str, props: &[(&str, String)]) -> bool {
    let registry = game.pack.assets.registry;
    let Some(base) = registry.default_state(name).and_then(|id| registry.state(id)) else {
        return false;
    };
    let mut next = base.clone();
    for (key, value) in props {
        if next.properties.get(*key).is_none() {
            return false;
        }
        next.properties.insert((*key).into(), value.as_str().into());
    }
    let Ok(id) = registry.resolve(&next) else {
        return false;
    };
    let dimension = game.dim.id().to_owned();
    game.native_set_block(&dimension, pos, id, flags::UPDATE_ALL)
}

/// Sets properties of the block in place without neighbour or shape updates: the
/// simulation already cascades shape updates itself.
fn set_properties(game: &mut Game<'_>, pos: Pos, edits: &[(&'static str, &'static str)]) -> bool {
    let registry = game.pack.assets.registry;
    let Some(current) = registry.state(game.world.block(pos[0], pos[1], pos[2])) else {
        return false;
    };
    let mut next = current.clone();
    for (key, value) in edits {
        if next.properties.get(*key).is_none() {
            return false;
        }
        next.properties.insert((*key).into(), (*value).into());
    }
    let Ok(id) = registry.resolve(&next) else {
        return false;
    };
    let dimension = game.dim.id().to_owned();
    game.native_set_block(&dimension, pos, id, flags::UPDATE_CLIENTS)
}

// ------------------------------------------------------------- environment

/// `PipeEnv` over the Horizon world. Fluid handlers are reached through the
/// shared sided helpers of `create_port_basin` (`fluid_tanks`, `fluid_fill`,
/// `fluid_drain`) that also answer the `native_custom_fluid_*` hooks, so a
/// Fluid Tank (multiblock aware), Basin, Item Drain and Spout behave like for
/// hoppers and other mods.
struct Env<'a, 'g> {
    game: &'a mut Game<'g>,
    dimension: String,
}

impl Env<'_, '_> {
    fn quiet(&mut self, pos: Pos) {
        let dimension = self.dimension.clone();
        shared(&dimension, |s| s.ignore.insert(pos));
    }
}

impl PipeEnv for Env<'_, '_> {
    fn is_loaded(&self, pos: pipes::Pos) -> bool {
        self.game.world.loaded(pos.0, pos.2)
    }

    fn neighbor(&self, pos: pipes::Pos, face: Dir) -> Neighbor {
        let p = ap(pos);
        let Some(state) = state(self.game, p) else {
            return Neighbor::Blocked;
        };
        if handler(&state.name, face) {
            return Neighbor::Handler;
        }
        // FluidPropagator.isOpenEnd: replaceable space or a fluid; any other block is a wall.
        if replaceable(self.game, p) || fluid_state(state).is_some() {
            return Neighbor::OpenEnd;
        }
        Neighbor::Blocked
    }

    fn contents(&self, pos: pipes::Pos, face: Dir) -> Vec<Fluid> {
        let mut views = None;
        basin::fluid_tanks(self.game, ap(pos), Some(face_of(face)), &mut views);
        views
            .unwrap_or_default()
            .into_iter()
            .filter(|tank| tank.can_drain)
            .filter_map(|tank| tank.content)
            .filter(|stack| stack.amount > 0 && stack.components.is_empty())
            .filter_map(|stack| fluid_of(&stack.fluid))
            .collect()
    }

    fn fill(&mut self, pos: pipes::Pos, face: Dir, fluid: Fluid, amount: i32, simulate: bool) -> i32 {
        let stack = NativeFluidStack {
            fluid: fluid_id(fluid).to_owned(),
            amount,
            components: Compound::new(),
        };
        let action = if simulate { NativeFluidAction::Simulate } else { NativeFluidAction::Execute };
        let mut accepted = None;
        basin::fluid_fill(self.game, ap(pos), Some(face_of(face)), &stack, action, &mut accepted);
        accepted.unwrap_or(0).max(0)
    }

    fn drain(&mut self, pos: pipes::Pos, face: Dir, fluid: Fluid, amount: i32, simulate: bool) -> i32 {
        let wanted = NativeFluidStack {
            fluid: fluid_id(fluid).to_owned(),
            amount,
            components: Compound::new(),
        };
        let action = if simulate { NativeFluidAction::Simulate } else { NativeFluidAction::Execute };
        let mut drained = None;
        basin::fluid_drain(
            self.game,
            ap(pos),
            Some(face_of(face)),
            &NativeFluidDrain::Exact(wanted),
            action,
            &mut drained,
        );
        drained.map_or(0, |stack| stack.amount.max(0))
    }

    /// `OpenEndedPipe.provideFluidToSpace` (config `pipesPlaceFluidSourceBlocks` = true).
    fn place_fluid(&mut self, pos: pipes::Pos, fluid: Fluid, simulate: bool) -> bool {
        let p = ap(pos);
        let Some(current) = state(self.game, p) else {
            return false;
        };
        if let Some((existing, source)) = fluid_state(current) {
            // FluidReactions.handlePipeSpillCollision is not ported: a different fluid blocks.
            if existing != fluid || source {
                return false;
            }
        }
        let waterloggable = current.properties.get("waterlogged").is_some();
        if waterloggable && fluid == Fluid::WATER {
            if !simulate {
                self.quiet(p);
                self.game.native_set_block_property(p, "waterlogged", "true");
            }
            return true;
        }
        if !replaceable(self.game, p) {
            return false;
        }
        if !simulate {
            // Ultra-warm dimension: water evaporates instead of being placed.
            if fluid == Fluid::WATER && self.game.dim.id() == "minecraft:the_nether" {
                return true;
            }
            let Some(block) = self.game.native_default_state(fluid_id(fluid)) else {
                return false;
            };
            self.quiet(p);
            let dimension = self.dimension.clone();
            self.game.native_set_block(&dimension, p, block, flags::UPDATE_ALL);
        }
        true
    }

    /// `OpenEndedPipe.removeFluidFromSpace`.
    fn take_source(&mut self, pos: pipes::Pos, simulate: bool) -> Option<Fluid> {
        let p = ap(pos);
        let current = state(self.game, p)?;
        let waterloggable = current.properties.get("waterlogged").is_some();
        if !waterloggable && !replaceable(self.game, p) {
            return None;
        }
        let (fluid, source) = fluid_state(current)?;
        let water_block = current.name == "minecraft:water";
        if !source {
            return None;
        }
        if !simulate {
            self.quiet(p);
            if waterloggable && !water_block {
                self.game.native_set_block_property(p, "waterlogged", "false");
            } else {
                let air = self.game.native_default_state("minecraft:air")?;
                let dimension = self.dimension.clone();
                self.game.native_set_block(&dimension, p, air, flags::UPDATE_ALL);
            }
        }
        Some(fluid)
    }
}

/// Takes the level's `Net` out of the cache while `f` runs (the world calls made
/// by the environment can re-enter observers).
fn with_sim<R>(game: &mut Game<'_>, f: impl FnOnce(&mut Env<'_, '_>, &mut Net) -> R) -> R {
    let session = std::ptr::from_ref(&game.world) as usize;
    let dimension = game.dim.id().to_owned();
    let mut net = NETS.with(|nets| std::mem::take(nets.borrow_mut().dimension(session, &dimension)));
    net.sim.external_valves = true;
    let result = {
        let mut env = Env { game: &mut *game, dimension: dimension.clone() };
        f(&mut env, &mut net)
    };
    NETS.with(|nets| *nets.borrow_mut().dimension(session, &dimension) = net);
    result
}

fn known(dimension: &str, pos: Pos, present: bool) {
    shared(dimension, |s| {
        if present {
            s.known.insert(pos);
        } else {
            s.known.remove(&pos);
        }
    });
}

// ---------------------------------------------------------------- observers

/// `block_changed` observer (no `Game` borrow): queue positions the network cares about.
/// Call with `level.native_state_block(old)` / `(new)`.
pub(super) fn changed(dimension: &str, pos: Pos, old: Option<&str>, new: Option<&str>) {
    let family = |name: Option<&str>| name.is_some_and(is_family);
    let fluid = |name: Option<&str>| matches!(name, Some("minecraft:water" | "minecraft:lava"));
    let replaced = family(new) && old != new;
    shared(dimension, |s| {
        if s.ignore.remove(&pos) {
            return;
        }
        // LiquidBlock neighbours never re-propagate a pipe.
        if !family(old) && !family(new) && fluid(new) {
            return;
        }
        if family(old) || family(new) || s.near(pos) {
            s.events.push((pos, replaced));
        }
    });
}

// -------------------------------------------------------------- reconcile

fn reconcile(env: &mut Env<'_, '_>, net: &mut Net, pos: Pos, placed: bool) {
    let p = tp(pos);
    let Some(current) = state(env.game, pos) else {
        return; // chunk not loaded: nothing to say
    };
    match parse(current) {
        None => {
            if net.sim.block(p).is_some() {
                net.sim.remove_block(env, p);
                net.loaded.remove(&pos);
                known(&env.dimension, pos, false);
            }
            net.sim.notify_neighbors(env, p);
        }
        Some(block) => {
            known(&env.dimension, pos, true);
            match net.sim.block(p).cloned() {
                None if placed => net.sim.place_block(env, p, block),
                None => {
                    net.sim.load_block(p, block);
                    net.loaded.insert(pos);
                }
                Some(old) if old == block => {}
                Some(PipeBlock::Pump { facing: a }) if matches!(block, PipeBlock::Pump { facing: b } if a != b) => {
                    if let PipeBlock::Pump { facing } = block {
                        net.sim.set_pump_facing(env, p, facing);
                    }
                }
                Some(_) => net.sim.place_block(env, p, block),
            }
        }
    }
}

/// Sim-owned shapes (plain / encased pipe sides after `update_block_state`) go back to the world.
fn write_back(env: &mut Env<'_, '_>, net: &Net) {
    let positions: Vec<pipes::Pos> = net.sim.positions().collect();
    for p in positions {
        let pos = ap(p);
        let (Some(world), Some(simulated)) = (state(env.game, pos), net.sim.block(p)) else {
            continue;
        };
        let (PipeBlock::Pipe { open } | PipeBlock::Encased { open }) = simulated else {
            continue;
        };
        if !matches!(world.name.as_str(), PIPE | ENCASED) {
            continue;
        }
        let edits: Vec<(&'static str, &'static str)> = Dir::ALL
            .into_iter()
            .filter(|d| is_true(world, d.name()) != open[d.index()])
            .map(|d| (d.name(), truth(open[d.index()])))
            .collect();
        if !edits.is_empty() {
            let dimension = env.dimension.clone();
            shared(&dimension, |s| s.ignore.insert(pos));
            set_properties(env.game, pos, &edits);
        }
    }
}

/// Drops blocks of unloaded chunks silently and blocks that vanished without an event.
fn sweep(env: &mut Env<'_, '_>, net: &mut Net) {
    let positions: Vec<pipes::Pos> = net.sim.positions().collect();
    for p in positions {
        let pos = ap(p);
        match state(env.game, pos) {
            None => {
                net.sim.forget_block(p);
                net.loaded.remove(&pos);
                known(&env.dimension, pos, false);
            }
            Some(current) if parse(current).is_none() => reconcile(env, net, pos, false),
            Some(_) => {}
        }
    }
}

/// Once per level tick, before the block entities (`motor_level.rs`).
pub(super) fn maintenance(game: &mut Game<'_>) {
    if !simulates(game) || !game.native_on("create") {
        return;
    }
    let dimension = game.dim.id().to_owned();
    let (events, active) = shared(&dimension, |s| {
        s.ignore.clear();
        (std::mem::take(&mut s.events), !s.known.is_empty())
    });
    if events.is_empty() && !active {
        return;
    }
    let now = game.world.game_time;
    let mut merged: Vec<(Pos, bool)> = Vec::new();
    for (pos, placed) in events {
        match merged.iter_mut().find(|(p, _)| *p == pos) {
            Some(entry) => entry.1 |= placed,
            None => merged.push((pos, placed)),
        }
    }
    with_sim(game, |env, net| {
        for (pos, placed) in merged {
            reconcile(env, net, pos, placed);
        }
        if now.rem_euclid(SWEEP_PERIOD) == 0 || now < net.swept {
            net.swept = now;
            sweep(env, net);
        }
        net.sim.tick(env);
        write_back(env, net);
    });
    shared(&dimension, |s| s.ignore.clear());
}

// ------------------------------------------------------------------- NBT

fn tag_number(n: f32) -> Tag {
    Tag::Float(n.to_bits())
}

fn parse_flow(data: &Compound) -> Option<Flow> {
    let Some(Tag::Compound(fluid)) = data.get(&"Fluid".into()) else {
        return None;
    };
    let Some(Tag::String(id)) = fluid.get(&"id".into()) else {
        return None;
    };
    let fluid = fluid_of(&id.to_string_lossy())?;
    let inbound = matches!(data.get(&"In".into()), Some(Tag::Byte(b)) if *b != 0);
    let progress = match data.get(&"Progress".into()) {
        Some(Tag::Compound(p)) => match p.get(&"Value".into()) {
            Some(Tag::Float(value)) => f32::from_bits(*value),
            _ => 1.0,
        },
        _ => 1.0,
    };
    Some(Flow { inbound, fluid, progress, complete: progress >= 1.0 })
}

/// `PipeConnection.deserializeNBT` for every face (same layout the straight-line
/// pump wrote: `<side>: { Pressure: [in, out], Flow: { Fluid, In, Progress } }`).
fn restore(sim: &mut PipeSim, pos: Pos, nbt: &Compound) {
    for d in Dir::ALL {
        let Some(Tag::Compound(connection)) = nbt.get(&d.name().into()) else {
            continue;
        };
        let mut pressure = Pressure::default();
        if let Some(Tag::List(values)) = connection.get(&"Pressure".into())
            && let [Tag::Float(inbound), Tag::Float(outward)] = values.as_slice()
        {
            pressure = Pressure { inbound: f32::from_bits(*inbound), outward: f32::from_bits(*outward) };
        }
        let flow = match connection.get(&"Flow".into()) {
            Some(Tag::Compound(data)) => parse_flow(data),
            _ => None,
        };
        if pressure != Pressure::default() || flow.is_some() {
            sim.restore_connection(tp(pos), d, pressure, flow);
        }
    }
}

/// Saves pressure and flow of every face for clients and the next load; true when changed.
fn store(sim: &PipeSim, pos: Pos, nbt: &mut Compound) -> bool {
    let mut changed = false;
    for d in Dir::ALL {
        let Some(c) = sim.connection(tp(pos), d) else {
            continue;
        };
        let mut connection = Compound::new();
        let pressure = c.pressure;
        if pressure.inbound != 0.0 || pressure.outward != 0.0 {
            connection.insert(
                "Pressure".into(),
                Tag::List([pressure.inbound, pressure.outward].map(tag_number).into()),
            );
        }
        if let Some(flow) = c.flow {
            let mut data = Compound::from([
                (
                    "Fluid".into(),
                    Tag::Compound(Compound::from([
                        ("id".into(), Tag::String(fluid_id(flow.fluid).into())),
                        ("amount".into(), Tag::Int(1)),
                    ])),
                ),
                ("In".into(), Tag::Byte(i8::from(flow.inbound))),
            ]);
            if flow.progress != 1.0 {
                data.insert(
                    "Progress".into(),
                    Tag::Compound(Compound::from([
                        ("Value".into(), tag_number(flow.progress)),
                        ("Target".into(), tag_number(0.0)),
                        ("Speed".into(), tag_number(0.0)),
                    ])),
                );
            }
            connection.insert("Flow".into(), Tag::Compound(data));
        }
        let key = d.name().into();
        let value = Tag::Compound(connection);
        if nbt.get(&key) == Some(&value) {
            continue;
        }
        nbt.insert(key, value);
        changed = true;
    }
    changed
}

fn filter_of(nbt: &Compound) -> Option<Fluid> {
    match nbt.get(&FILTER.into()) {
        Some(Tag::String(id)) => fluid_of(&id.to_string_lossy()),
        _ => None,
    }
}

fn trace_enabled() -> bool {
    static TRACE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *TRACE.get_or_init(|| std::env::var_os("CREATE_TRACE_FLUID_NET").is_some())
}

// ----------------------------------------------------------- block entity

/// Block entity tick of a pipe, pump or valve (`rpm` is the running speed; 0 for
/// passive pipes). Replaces `pump::tick`. True when the NBT changed (sync).
pub(in crate::game) fn tick(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity, rpm: f32) -> bool {
    if !simulates(game) || !game.native_on("create") || !is_family(entity.id.as_str()) {
        return false;
    }
    let Some(block) = state(game, pos).and_then(parse) else {
        return false;
    };
    let p = tp(pos);
    let changed = with_sim(game, |env, net| {
        if net.sim.block(p).is_none() {
            net.sim.load_block(p, block.clone());
            net.loaded.insert(pos);
            known(&env.dimension, pos, true);
        }
        if net.loaded.remove(&pos) {
            restore(&mut net.sim, pos, &entity.nbt);
            if block.is_pump() {
                net.sim.restore_pump_speed(p, rpm);
            }
        }
        if block.is_pump() {
            net.sim.set_pump_speed(env, p, rpm);
        }
        if matches!(block, PipeBlock::Smart { .. }) {
            net.sim.set_smart_filter(p, filter_of(&entity.nbt));
        }
        store(&net.sim, pos, &mut entity.nbt)
    });
    if trace_enabled() {
        let faces: Vec<_> = Dir::ALL
            .into_iter()
            .filter_map(|d| {
                NETS.with(|nets| {
                    let session = std::ptr::from_ref(&game.world) as usize;
                    let mut nets = nets.borrow_mut();
                    let net = nets.dimension(session, game.dim.id());
                    net.sim.connection(p, d).map(|c| {
                        serde_json::json!({
                            "side": d.name(), "pressure": [c.pressure.inbound, c.pressure.outward],
                            "flow": c.flow.map(|f| (fluid_id(f.fluid), f.inbound, f.progress)),
                        })
                    })
                })
            })
            .collect();
        log::info!(
            "Create fluid net: {}",
            serde_json::json!({"time": game.world.game_time, "pos": pos, "block": entity.id.as_str(), "rpm": rpm, "faces": faces})
        );
    }
    changed
}

#[derive(Clone, Copy, Default, PartialEq)]
struct Pointer {
    value: f32,
    target: f32,
    speed: f32,
}

impl Pointer {
    fn read(nbt: &Compound) -> Option<Self> {
        let Some(tag) = nbt.get(&"Pointer".into()) else {
            return Some(Self::default());
        };
        let Tag::Compound(fields) = tag else {
            return None;
        };
        let number = |key: &str| match fields.get(&key.into()) {
            Some(Tag::Float(bits)) => Some(f32::from_bits(*bits)),
            _ => None,
        };
        let (value, target, speed) = (number("Value")?, number("Target")?, number("Speed")?);
        ((0.0..=1.0).contains(&value) && matches!(target, 0.0 | 1.0) && (0.0..=1.0).contains(&speed))
            .then_some(Self { value, target, speed })
    }

    fn tick(&mut self, rpm: f32) {
        self.target = if rpm > 0.0 { 1.0 } else { 0.0 };
        self.speed = (rpm.abs() / 16.0 / 20.0).clamp(0.0, 1.0);
        if self.value < self.target {
            self.value = (self.value + self.speed).min(self.target);
        } else if self.value > self.target {
            self.value = (self.value - self.speed).max(self.target);
        }
    }

    fn save(self, nbt: &mut Compound) {
        if nbt.get(&"Pointer".into()).is_none() {
            nbt.insert("Pointer".into(), Tag::Compound(Compound::new()));
        }
        if let Some(Tag::Compound(fields)) = nbt.get_mut(&"Pointer".into()) {
            fields.insert("Value".into(), tag_number(self.value));
            fields.insert("Target".into(), tag_number(self.target));
            fields.insert("Speed".into(), tag_number(self.speed));
        }
    }
}

pub(in crate::game) fn valve_nbt() -> Compound {
    let mut nbt = Compound::new();
    Pointer::default().save(&mut nbt);
    nbt
}

/// `FluidValveBlockEntity.tick`: the pointer chases the speed; `enabled` flips at
/// 0 / 1. The block-state write reaches the simulation through `changed`.
pub(in crate::game) fn tick_valve(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity, rpm: f32) -> bool {
    let mut changed = tick(game, pos, entity, rpm);
    let Some(mut pointer) = Pointer::read(&entity.nbt) else {
        return changed;
    };
    let previous = pointer;
    pointer.tick(rpm);
    if pointer != previous || entity.nbt.get(&"Pointer".into()).is_none() {
        pointer.save(&mut entity.nbt);
        changed = true;
    }
    let value = match pointer.value {
        0.0 => Some("false"),
        1.0 => Some("true"),
        _ => None,
    };
    if let Some(value) = value {
        changed |= game.native_set_block_property(pos, "enabled", value);
    }
    changed
}

// -------------------------------------------------------------- placement

/// `FluidPipeBlock.getStateForPlacement` (and a basic `SmartFluidPipeBlock` one).
/// Route `PIPE` / `SMART` here from `placement_runtime.rs`; pump and valve keep
/// `pump::placement`. `None` refuses the placement (as the old hook did in lava).
pub(in crate::game) fn placement(
    game: &GameLevel<'_>,
    state: BlockStateId,
    ctx: &PlacementContext,
    block: &str,
) -> Option<BlockStateId> {
    let pos = [ctx.pos.x, ctx.pos.y, ctx.pos.z];
    let target = game.registry.state(game.world.block(pos[0], pos[1], pos[2]))?;
    let water = target.name == "minecraft:water";
    if target.name == "minecraft:lava" {
        return None;
    }
    let look = dir_of(crate::game::create_port::nearest(ctx.yaw, ctx.pitch));
    if block == SMART {
        let vertical = matches!(look, Dir::Up | Dir::Down);
        let facing = dir_of(crate::game::create_port_placement::horizontal(ctx.yaw));
        let face = if vertical { if look == Dir::Up { "ceiling" } else { "floor" } } else { "wall" };
        let state = game.native_state_with_property(state, "facing", facing.name())?;
        let state = game.native_state_with_property(state, "face", face)?;
        return game.native_state_with_property(state, "waterlogged", truth(water));
    }
    let connects = |d: Dir| -> bool {
        let at = [pos[0] + d.step().0, pos[1] + d.step().1, pos[2] + d.step().2];
        let Some(neighbor) = game.registry.state(game.world.block(at[0], at[1], at[2])) else {
            return false;
        };
        if handler(&neighbor.name, d.opposite()) {
            return true;
        }
        if neighbor.name == PIPE {
            return true; // FluidPipeBlock.isPipe (brackets are not ported)
        }
        parse(neighbor).is_some_and(|b| b.can_have_flow_toward(d.opposite()))
    };
    let open = pipes::update_block_state([false; 6], look, None, connects);
    let mut state = state;
    for d in Dir::ALL {
        state = game.native_state_with_property(state, d.name(), truth(open[d.index()]))?;
    }
    game.native_state_with_property(state, "waterlogged", truth(water))
}

// ---------------------------------------------------------------- wrench

/// Wrench on a pipe (window / casing toggles). Casing items are not handled yet.
pub(in crate::game) fn reacts(game: &Game<'_>, pos: Pos) -> bool {
    game.native_on("create")
        && matches!(game.native_block(pos).as_str(), PIPE | GLASS | ENCASED)
        && game.may_build()
        && !game.native_sneaking()
        && game
            .held()
            .is_some_and(|held| crate::game::create_port_chassis::wrench(game, held))
}

/// `face` is the clicked face's normal vector as handed to `right_click_block`.
pub(in crate::game) fn right_click(game: &mut Game<'_>, pos: Pos, face: [i32; 3]) -> bool {
    if !reacts(game, pos) {
        return false;
    }
    let Some(current) = state(game, pos).cloned() else {
        return true;
    };
    let Some(block) = parse(&current) else {
        return true;
    };
    let waterlogged = truth(is_true(&current, "waterlogged"));
    let swapped = match block {
        // FluidPipeBlock: a straight pipe gains a window when a side face is clicked.
        PipeBlock::Pipe { .. } => {
            let clicked = dir_from_vector(face);
            match (block.straight_axis(), clicked) {
                (Some(axis), Some(clicked)) if clicked.axis() != axis => swap_block(
                    game,
                    pos,
                    GLASS,
                    &[("axis", axis.name().into()), ("alt", "false".into()), ("waterlogged", waterlogged.into())],
                ),
                _ => false,
            }
        }
        PipeBlock::Glass { axis } => {
            let mut props: Vec<(&str, String)> = Dir::ALL
                .into_iter()
                .map(|d| (d.name(), truth(d.axis() == axis).to_owned()))
                .collect();
            props.push(("waterlogged", waterlogged.to_owned()));
            swap_block(game, pos, PIPE, &props)
        }
        PipeBlock::Encased { open } => {
            let mut props: Vec<(&str, String)> =
                Dir::ALL.into_iter().map(|d| (d.name(), truth(open[d.index()]).to_owned())).collect();
            props.push(("waterlogged", "false".to_owned()));
            swap_block(game, pos, PIPE, &props)
        }
        _ => false,
    };
    if swapped {
        crate::game::create_port_sounds::cog_wrench(game, pos);
    }
    true
}

/// Casing on a pipe or glass pipe -> Encased Fluid Pipe with the same open sides.
/// Not routed by default: the casing item's consumption path was not found.
#[allow(dead_code)]
pub(in crate::game) fn encase(game: &mut Game<'_>, pos: Pos) -> bool {
    let Some(block) = state(game, pos).and_then(parse) else {
        return false;
    };
    let open = match block {
        PipeBlock::Pipe { open } => open,
        PipeBlock::Glass { axis } => std::array::from_fn(|i| Dir::ALL[i].axis() == axis),
        _ => return false,
    };
    let props: Vec<(&str, String)> =
        Dir::ALL.into_iter().map(|d| (d.name(), truth(open[d.index()]).to_owned())).collect();
    swap_block(game, pos, ENCASED, &props)
}

// ------------------------------------------------------------------- draw

/// `PipeAttachmentModel` quads for the open sides of a plain pipe (generalises the
/// straight-line drawing of `pump::draw`). Route `PIPE` here; keep `pump::draw` for
/// `PUMP` and `VALVE`. Flowing fluid is not drawn yet.
pub(in crate::game) fn draw(game: &Game<'_>, pos: Pos, _entity: &ModBlockEntity, draw: &mut BlockEntityDraw) {
    if !crate::game::create_port::original_assets() {
        return;
    }
    let Some(block) = state(game, pos).and_then(parse) else {
        return;
    };
    if !matches!(block, PipeBlock::Pipe { .. }) {
        return;
    }
    use create_assets::PipeAttachment::{Connection, Drain, Rim, RimConnector};
    for d in Dir::ALL.into_iter().filter(|d| block.can_have_flow_toward(*d)) {
        let at = [pos[0] + d.step().0, pos[1] + d.step().1, pos[2] + d.step().2];
        let neighbor = state(game, at);
        let models: &[create_assets::PipeAttachment] = match neighbor {
            Some(n) if handler(&n.name, d.opposite()) => &[RimConnector, Drain],
            Some(n) => match parse(n).filter(|b| b.can_have_flow_toward(d.opposite())) {
                Some(PipeBlock::Pump { .. }) => &[RimConnector],
                Some(_) => &[Connection],
                None => &[RimConnector, Rim],
            },
            None => &[RimConnector, Rim],
        };
        for part in models {
            game.native_draw_model(draw, part.model(d.index()), 0xFFFFFF);
        }
    }
}
