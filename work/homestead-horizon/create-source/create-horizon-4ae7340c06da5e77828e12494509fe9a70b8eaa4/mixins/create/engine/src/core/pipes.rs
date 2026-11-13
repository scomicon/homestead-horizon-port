// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
//! Full fluid pipe network after Create 6.0.10 `content/fluids`:
//! `FluidTransportBehaviour`, `PipeConnection`, `FluidNetwork`, `FluidPropagator`,
//! `PumpBlockEntity`, `OpenEndedPipe`, `FlowSource`, `FluidPipeBlock` shape rules,
//! glass / encased / valve / smart pipes. Pure and deterministic; the world is
//! reached only through [`PipeEnv`]. This module is independent of
//! `fluid_transport.rs` (the straight-line subset).
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// `CYCLES_PER_TICK` in `FluidNetwork`.
pub const CYCLES_PER_TICK: usize = 16;
/// `pauseBeforePropagation` after a network reset.
pub const PAUSE_BEFORE_PROPAGATION: i32 = 2;
/// Server config `mechanicalPumpRange` default.
pub const DEFAULT_PUMP_RANGE: i32 = 16;
/// Internal tank of `OpenEndedPipe` (one source block).
pub const OPEN_END_CAPACITY: i32 = 1000;

pub type Pos = (i32, i32, i32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Axis {
    X,
    Y,
    Z,
}

/// Minecraft `Direction` in `Iterate.directions` order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Dir {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl Dir {
    pub const ALL: [Dir; 6] = [Dir::Down, Dir::Up, Dir::North, Dir::South, Dir::West, Dir::East];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn opposite(self) -> Dir {
        Dir::ALL[self.index() ^ 1]
    }

    pub fn axis(self) -> Axis {
        match self.index() / 2 {
            0 => Axis::Y,
            1 => Axis::Z,
            _ => Axis::X,
        }
    }

    pub fn step(self) -> Pos {
        match self {
            Dir::Down => (0, -1, 0),
            Dir::Up => (0, 1, 0),
            Dir::North => (0, 0, -1),
            Dir::South => (0, 0, 1),
            Dir::West => (-1, 0, 0),
            Dir::East => (1, 0, 0),
        }
    }

    pub fn parse(name: &str) -> Option<Dir> {
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

    pub fn name(self) -> &'static str {
        ["down", "up", "north", "south", "west", "east"][self.index()]
    }
}

impl Axis {
    pub fn directions(self) -> [Dir; 2] {
        match self {
            Axis::X => [Dir::West, Dir::East],
            Axis::Y => [Dir::Down, Dir::Up],
            Axis::Z => [Dir::North, Dir::South],
        }
    }

    pub fn parse(name: &str) -> Option<Axis> {
        match name {
            "x" => Some(Axis::X),
            "y" => Some(Axis::Y),
            "z" => Some(Axis::Z),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Axis::X => "x",
            Axis::Y => "y",
            Axis::Z => "z",
        }
    }
}

pub fn offset(pos: Pos, dir: Dir) -> Pos {
    let s = dir.step();
    (pos.0 + s.0, pos.1 + s.1, pos.2 + s.2)
}

/// Opaque fluid id (a registry adapter maps it to a resource location).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Fluid(pub u32);

impl Fluid {
    pub const WATER: Fluid = Fluid(1);
    pub const LAVA: Fluid = Fluid(2);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockFace {
    pub pos: Pos,
    pub face: Dir,
}

impl BlockFace {
    pub fn new(pos: Pos, face: Dir) -> Self {
        Self { pos, face }
    }

    pub fn connected_pos(self) -> Pos {
        offset(self.pos, self.face)
    }

    /// The same link seen from the neighbouring block.
    pub fn opposite(self) -> Self {
        Self::new(self.connected_pos(), self.face.opposite())
    }

    pub fn is_equivalent(self, other: Self) -> bool {
        self == other || self.opposite() == other
    }
}

// ---------------------------------------------------------------- blocks

/// Every block that owns a `FluidTransportBehaviour`.
#[derive(Clone, Debug, PartialEq)]
pub enum PipeBlock {
    /// `FluidPipeBlock`: six open-side booleans.
    Pipe { open: [bool; 6] },
    /// `GlassFluidPipeBlock` (axis pipe with window).
    Glass { axis: Axis },
    /// `EncasedPipeBlock`: six open-side booleans.
    Encased { open: [bool; 6] },
    /// `FluidValveBlock`; `enabled` is the `ENABLED` property (open).
    Valve { axis: Axis, enabled: bool },
    /// `SmartFluidPipeBlock`; `filter` None means "accept any fluid".
    Smart { axis: Axis, filter: Option<Fluid> },
    /// `PumpBlock`.
    Pump { facing: Dir },
}

impl PipeBlock {
    pub fn pipe(open: &[Dir]) -> Self {
        let mut sides = [false; 6];
        for d in open {
            sides[d.index()] = true;
        }
        PipeBlock::Pipe { open: sides }
    }

    /// `FluidTransportBehaviour.canHaveFlowToward` of the block's behaviour.
    pub fn can_have_flow_toward(&self, d: Dir) -> bool {
        match self {
            PipeBlock::Pipe { open } | PipeBlock::Encased { open } => open[d.index()],
            PipeBlock::Glass { axis }
            | PipeBlock::Valve { axis, .. }
            | PipeBlock::Smart { axis, .. } => d.axis() == *axis,
            PipeBlock::Pump { facing } => d.axis() == facing.axis(),
        }
    }

    /// `canPullFluidFrom`: closed valves and smart-pipe filters.
    pub fn can_pull_fluid_from(&self, fluid: Fluid) -> bool {
        match self {
            PipeBlock::Valve { enabled, .. } => *enabled,
            PipeBlock::Smart { filter, .. } => filter.is_none_or(|f| f == fluid),
            _ => true,
        }
    }

    /// `FluidPropagator.getStraightPipeAxis`.
    pub fn straight_axis(&self) -> Option<Axis> {
        match self {
            PipeBlock::Pump { facing } => Some(facing.axis()),
            PipeBlock::Glass { axis }
            | PipeBlock::Valve { axis, .. }
            | PipeBlock::Smart { axis, .. } => Some(*axis),
            PipeBlock::Pipe { open } => {
                let mut found = None;
                let mut count = 0;
                for axis in [Axis::X, Axis::Y, Axis::Z] {
                    let [neg, pos] = axis.directions();
                    let (n, p) = (open[neg.index()], open[pos.index()]);
                    count += n as i32 + p as i32;
                    if n && p {
                        if found.is_some() {
                            return None;
                        }
                        found = Some(axis);
                    }
                }
                if count == 2 { found } else { None }
            }
            PipeBlock::Encased { .. } => None,
        }
    }

    pub fn is_pump(&self) -> bool {
        matches!(self, PipeBlock::Pump { .. })
    }

    fn open_sides(&self) -> Option<&[bool; 6]> {
        match self {
            PipeBlock::Pipe { open } | PipeBlock::Encased { open } => Some(open),
            _ => None,
        }
    }
}

/// `FluidPipeBlock.updateBlockState`: pure shape rule for bends and branches.
/// `connects(d)` is `canConnectTo` for side `d`; `ignore` keeps its previous value.
pub fn update_block_state(
    previous: [bool; 6],
    preferred: Dir,
    ignore: Option<Dir>,
    connects: impl Fn(Dir) -> bool,
) -> [bool; 6] {
    let prev_count = previous.iter().filter(|b| **b).count();
    let mut open = previous;
    for d in Dir::ALL {
        if Some(d) != ignore {
            open[d.index()] = connects(d);
        }
    }
    let count = open.iter().filter(|b| **b).count();
    if count > 1 {
        return open;
    }
    if count == 1 {
        let only = Dir::ALL[open.iter().position(|b| *b).unwrap()];
        open[only.opposite().index()] = true;
        return open;
    }
    if prev_count == 2 {
        return previous;
    }
    open[preferred.index()] = true;
    open[preferred.opposite().index()] = true;
    open
}

// ---------------------------------------------------------------- environment

/// Classification of a non-pipe neighbour as seen from a pipe face.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Neighbor {
    /// Block with a fluid capability on the facing side (tank, drain, ...).
    Handler,
    /// `VanillaFluidTargets.canProvideFluidWithoutCapability` (cauldron, ...):
    /// connects like a handler but behaves as an open end.
    Vanilla,
    /// `FluidPropagator.isOpenEnd` is true (air, fluid, waterlogged block).
    OpenEnd,
    /// Solid / unrelated block.
    Blocked,
}

/// World access. `pos`/`face` always name the *neighbour* block and its face
/// that looks at the pipe.
pub trait PipeEnv {
    fn is_loaded(&self, _pos: Pos) -> bool {
        true
    }
    fn neighbor(&self, pos: Pos, face: Dir) -> Neighbor;
    /// Fluids currently stored in the handler's tanks, in tank order.
    fn contents(&self, pos: Pos, face: Dir) -> Vec<Fluid>;
    fn fill(&mut self, pos: Pos, face: Dir, fluid: Fluid, amount: i32, simulate: bool) -> i32;
    fn drain(&mut self, pos: Pos, face: Dir, fluid: Fluid, amount: i32, simulate: bool) -> i32;
    /// `OpenEndedPipe.provideFluidToSpace`: place a source block (false = refused).
    fn place_fluid(&mut self, pos: Pos, fluid: Fluid, simulate: bool) -> bool;
    /// `OpenEndedPipe.removeFluidFromSpace`: remove a source block, yielding its fluid.
    fn take_source(&mut self, pos: Pos, simulate: bool) -> Option<Fluid>;
    /// `FluidReactions.handlePipeFlowCollision` hook (two fluids meet in a pipe).
    fn pipe_flow_collision(&mut self, _pos: Pos, _a: Fluid, _b: Fluid) {}
}

// ---------------------------------------------------------------- connection

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pressure {
    pub inbound: f32,
    pub outward: f32,
}

impl Pressure {
    pub fn get(self, inbound: bool) -> f32 {
        if inbound { self.inbound } else { self.outward }
    }

    fn set(&mut self, inbound: bool, value: f32) {
        if inbound {
            self.inbound = value;
        } else {
            self.outward = value;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flow {
    pub inbound: bool,
    pub fluid: Fluid,
    pub progress: f32,
    pub complete: bool,
}

/// `OpenEndedPipe` state (tank + `wasPulling`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OpenEnd {
    pub tank: Option<(Fluid, i32)>,
    pub was_pulling: bool,
}

impl OpenEnd {
    /// `OpenEndFluidHandler.fill`; `pos` is the output block (connected pos).
    pub fn fill(
        &mut self,
        env: &mut dyn PipeEnv,
        pos: Pos,
        fluid: Fluid,
        amount: i32,
        simulate: bool,
    ) -> i32 {
        if amount <= 0 || !env.is_loaded(pos) || !env.place_fluid(pos, fluid, true) {
            return 0;
        }
        if simulate {
            return amount;
        }
        if matches!(self.tank, Some((f, _)) if f != fluid) {
            self.tank = None;
        }
        let have = self.tank.map_or(0, |(_, a)| a);
        let filled = amount.min(OPEN_END_CAPACITY - have);
        let total = have + filled;
        self.tank = (total > 0).then_some((fluid, total));
        self.was_pulling = false;
        if total >= OPEN_END_CAPACITY && env.place_fluid(pos, fluid, false) {
            self.tank = None;
        }
        filled
    }

    /// `drainInner`: internal tank first, then a source block in the world.
    pub fn drain(
        &mut self,
        env: &mut dyn PipeEnv,
        pos: Pos,
        fluid: Fluid,
        amount: i32,
        simulate: bool,
    ) -> i32 {
        if !env.is_loaded(pos) || amount <= 0 {
            return 0;
        }
        let amount = amount.min(OPEN_END_CAPACITY);
        self.was_pulling = true;
        if let Some((f, have)) = self.tank {
            if f == fluid && have > 0 {
                let out = amount.min(have);
                if !simulate {
                    self.tank = (have > out).then_some((f, have - out));
                }
                return out;
            }
        }
        match env.take_source(pos, simulate) {
            Some(f) if f == fluid => {
                if !simulate && amount < OPEN_END_CAPACITY {
                    self.tank = Some((f, OPEN_END_CAPACITY - amount));
                }
                amount
            }
            _ => 0,
        }
    }

    fn provide(&self, env: &mut dyn PipeEnv, pos: Pos, pred: &dyn Fn(Fluid) -> bool) -> Option<Fluid> {
        if let Some((f, a)) = self.tank {
            if a > 0 {
                return pred(f).then_some(f);
            }
        }
        env.take_source(pos, true).filter(|f| pred(*f))
    }
}

/// `FlowSource` variants.
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    Handler,
    OpenEnd(OpenEnd),
    OtherPipe,
    Blocked,
}

impl Source {
    pub fn is_endpoint(&self) -> bool {
        matches!(self, Source::Handler | Source::OpenEnd(_))
    }
}

/// `PipeConnection`: one pipe face.
#[derive(Clone, Debug)]
pub struct PipeConnection {
    pub side: Dir,
    pub pressure: Pressure,
    pub source: Option<Source>,
    previous_source: Option<Source>,
    pub flow: Option<Flow>,
    network: Option<Box<FluidNetwork>>,
}

impl PipeConnection {
    fn new(side: Dir) -> Self {
        Self {
            side,
            pressure: Pressure::default(),
            source: None,
            previous_source: None,
            flow: None,
            network: None,
        }
    }

    /// outward - inbound (`comparePressure`).
    pub fn compare_pressure(&self) -> f32 {
        self.pressure.outward - self.pressure.inbound
    }

    pub fn has_pressure(&self) -> bool {
        self.pressure.inbound != 0.0 || self.pressure.outward != 0.0
    }

    /// `getProvidedFluid`: complete inbound flow.
    pub fn provided_fluid(&self) -> Option<Fluid> {
        self.flow.filter(|f| f.inbound && f.complete).map(|f| f.fluid)
    }

    /// `provideOutboundFlow`: complete outbound flow.
    pub fn provided_outward(&self) -> Option<Fluid> {
        self.flow.filter(|f| !f.inbound && f.complete).map(|f| f.fluid)
    }

    fn reset_network(&mut self) {
        if let Some(n) = self.network.as_mut() {
            n.reset();
        }
    }

    fn wipe_pressure(&mut self) {
        self.pressure = Pressure::default();
        if let Some(s) = self.source.take() {
            self.previous_source = Some(s);
        }
        self.reset_network();
    }

    /// `flipFlowsIfPressureReversed`.
    fn flip_flows_if_pressure_reversed(&mut self) -> bool {
        let Some(flow) = self.flow.as_mut() else { return false };
        let cmp = self.pressure.outward - self.pressure.inbound;
        let single = cmp != 0.0 && (self.pressure.inbound == 0.0 || self.pressure.outward == 0.0);
        if !single || (cmp < 0.0) == flow.inbound {
            return false;
        }
        flow.inbound = !flow.inbound;
        if !flow.complete {
            self.flow = None;
        }
        true
    }

    /// `tickFlowProgress` (server part).
    fn tick_flow_progress(&mut self) {
        if let Some(flow) = self.flow.as_mut() {
            let speed = 1.0 / 32.0 + (self.pressure.get(flow.inbound) / 128.0).clamp(0.0, 1.0) * 31.0 / 32.0;
            flow.progress = (flow.progress + speed).min(1.0);
            if flow.progress >= 1.0 {
                flow.complete = true;
            }
        }
    }

    fn try_start_flow(&mut self, inbound: bool, fluid: Option<Fluid>) -> bool {
        let Some(fluid) = fluid else { return false };
        self.flow = Some(Flow { inbound, fluid, progress: 0.0, complete: false });
        true
    }
}

// ---------------------------------------------------------------- network

/// `FluidNetwork`: the BFS owned by an inbound connection next to an endpoint.
#[derive(Clone, Debug)]
pub struct FluidNetwork {
    start: BlockFace,
    queued: Vec<BlockFace>,
    frontier: Vec<BlockFace>,
    visited: BTreeSet<Pos>,
    targets: Vec<BlockFace>,
    pub fluid: Option<Fluid>,
    pub transfer_speed: i32,
    pause: i32,
}

impl FluidNetwork {
    fn new(start: BlockFace) -> Self {
        let mut n = Self {
            start,
            queued: vec![],
            frontier: vec![],
            visited: BTreeSet::new(),
            targets: vec![],
            fluid: None,
            transfer_speed: 1,
            pause: 0,
        };
        n.reset();
        n
    }

    fn reset(&mut self) {
        self.frontier.clear();
        self.visited.clear();
        self.targets.clear();
        self.queued.clear();
        self.fluid = None;
        self.queued.push(self.start);
        self.pause = PAUSE_BEFORE_PROPAGATION;
    }

    /// Endpoints currently registered for filling.
    pub fn targets(&self) -> &[BlockFace] {
        &self.targets
    }
}

// ---------------------------------------------------------------- entity

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    WaitForPumps,
    FlipFlows,
    Idle,
}

#[derive(Clone, Debug)]
pub struct PipeEntity {
    pub block: PipeBlock,
    conns: BTreeMap<Dir, PipeConnection>,
    pub phase: Phase,
    /// Pump kinetic speed (signed); pressure uses |speed|.
    pub speed: f32,
    /// `sidesToUpdate`: [front, back].
    sides_to_update: [bool; 2],
    /// Valve pointer (`LerpedFloat`) with its chase target and step.
    pub valve_pointer: f32,
    valve_target: f32,
    valve_step: f32,
}

impl PipeEntity {
    fn new(block: PipeBlock) -> Self {
        let mut e = Self {
            block,
            conns: BTreeMap::new(),
            phase: Phase::WaitForPumps,
            speed: 0.0,
            sides_to_update: [false; 2],
            valve_pointer: 0.0,
            valve_target: 0.0,
            valve_step: 0.0,
        };
        e.create_connection_data();
        e
    }

    /// `createConnectionData`
    fn create_connection_data(&mut self) {
        for d in Dir::ALL {
            if self.block.can_have_flow_toward(d) {
                self.conns.entry(d).or_insert_with(|| PipeConnection::new(d));
            }
        }
    }

    fn has_any_pressure(&self) -> bool {
        self.conns.values().any(PipeConnection::has_pressure)
    }

    fn facing(&self) -> Option<Dir> {
        match self.block {
            PipeBlock::Pump { facing } => Some(facing),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------- simulation

/// All pipe-network block entities of one world plus their scheduled ticks.
#[derive(Clone, Debug)]
pub struct PipeSim {
    pipes: BTreeMap<Pos, PipeEntity>,
    order: Vec<Pos>,
    scheduled: Vec<(u64, Pos)>,
    pub pump_range: i32,
    pub game_time: u64,
    /// The adapter owns the valve pointer (block-entity NBT): `tick_device` then leaves `ENABLED` alone.
    pub external_valves: bool,
}

impl Default for PipeSim {
    fn default() -> Self {
        Self::new()
    }
}

impl PipeSim {
    pub fn new() -> Self {
        Self {
            pipes: BTreeMap::new(),
            order: vec![],
            scheduled: vec![],
            pump_range: DEFAULT_PUMP_RANGE,
            game_time: 0,
            external_valves: false,
        }
    }

    // ---- inspection

    pub fn block(&self, pos: Pos) -> Option<&PipeBlock> {
        self.pipes.get(&pos).map(|e| &e.block)
    }

    pub fn entity(&self, pos: Pos) -> Option<&PipeEntity> {
        self.pipes.get(&pos)
    }

    pub fn connection(&self, pos: Pos, side: Dir) -> Option<&PipeConnection> {
        self.pipes.get(&pos)?.conns.get(&side)
    }

    pub fn flow(&self, pos: Pos, side: Dir) -> Option<Flow> {
        self.connection(pos, side)?.flow
    }

    pub fn pressure(&self, pos: Pos, side: Dir) -> Option<Pressure> {
        self.connection(pos, side).map(|c| c.pressure)
    }

    pub fn network(&self, pos: Pos, side: Dir) -> Option<&FluidNetwork> {
        self.connection(pos, side)?.network.as_deref()
    }

    pub fn open_end(&self, pos: Pos, side: Dir) -> Option<&OpenEnd> {
        match self.connection(pos, side)?.source.as_ref()? {
            Source::OpenEnd(o) => Some(o),
            _ => None,
        }
    }

    pub fn positions(&self) -> impl Iterator<Item = Pos> + '_ {
        self.order.iter().copied()
    }

    fn conn_mut(&mut self, pos: Pos, side: Dir) -> Option<&mut PipeConnection> {
        self.pipes.get_mut(&pos)?.conns.get_mut(&side)
    }

    fn schedule(&mut self, pos: Pos) {
        let due = self.game_time + 1;
        if !self.scheduled.contains(&(due, pos)) {
            self.scheduled.push((due, pos));
        }
    }

    // ---- block placement (FluidPipeBlock / AxisPipeBlock / EncasedPipeBlock)

    /// `canConnectTo` for a pipe at `pos` toward `d`.
    fn can_connect_to(&self, env: &dyn PipeEnv, pos: Pos, d: Dir) -> bool {
        let n = offset(pos, d);
        if let Some(e) = self.pipes.get(&n) {
            return match e.block {
                PipeBlock::Pipe { .. } => true,
                ref b => b.can_have_flow_toward(d.opposite()),
            };
        }
        matches!(env.neighbor(n, d.opposite()), Neighbor::Handler | Neighbor::Vanilla)
    }

    /// Place a block exactly as given (no automatic connection logic) and run
    /// the neighbour shape updates / `onPlace` scheduling.
    pub fn place_block(&mut self, env: &mut dyn PipeEnv, pos: Pos, block: PipeBlock) {
        match self.pipes.get_mut(&pos) {
            Some(e) => {
                e.block = block;
                // cacheFlows/loadFlows: keep the connection data across swaps.
            }
            None => {
                self.pipes.insert(pos, PipeEntity::new(block));
                self.order.push(pos);
            }
        }
        self.after_state_change(env, pos);
    }

    /// `FluidPipeBlock.getStateForPlacement`.
    pub fn place_pipe(&mut self, env: &mut dyn PipeEnv, pos: Pos, looking: Dir) {
        let open = update_block_state([false; 6], looking, None, |d| self.can_connect_to(env, pos, d));
        self.place_block(env, pos, PipeBlock::Pipe { open });
    }

    /// Register a block that was read from a loaded chunk: no scheduled tick, no
    /// neighbour shape update (Java restores block entities without `onPlace`).
    /// Use [`PipeSim::restore_connection`] / [`PipeSim::restore_pump_speed`] for saved data.
    pub fn load_block(&mut self, pos: Pos, block: PipeBlock) {
        match self.pipes.get_mut(&pos) {
            Some(e) => e.block = block,
            None => {
                self.pipes.insert(pos, PipeEntity::new(block));
                self.order.push(pos);
            }
        }
        if let Some(e) = self.pipes.get_mut(&pos) {
            e.create_connection_data();
        }
    }

    /// Drop a block silently (its chunk was unloaded): no wipe, no neighbour update.
    pub fn forget_block(&mut self, pos: Pos) {
        if self.pipes.remove(&pos).is_some() {
            self.order.retain(|p| *p != pos);
            self.scheduled.retain(|(_, p)| *p != pos);
        }
    }

    /// `PipeConnection.deserializeNBT`: saved pressure and flow of one face.
    pub fn restore_connection(&mut self, pos: Pos, side: Dir, pressure: Pressure, flow: Option<Flow>) {
        if let Some(c) = self.conn_mut(pos, side) {
            c.pressure = pressure;
            c.flow = flow;
        }
    }

    /// `KineticBlockEntity.read`: the saved speed is taken without `onSpeedChanged`.
    pub fn restore_pump_speed(&mut self, pos: Pos, speed: f32) {
        if let Some(e) = self.pipes.get_mut(&pos) {
            if e.block.is_pump() {
                e.speed = speed;
            }
        }
    }

    /// Block removal (`onRemove` with a different block): clear and notify neighbours.
    pub fn remove_block(&mut self, env: &mut dyn PipeEnv, pos: Pos) {
        if !self.pipes.contains_key(&pos) {
            return;
        }
        // propagateChangedPipe with the air state wipes only this block.
        self.wipe_pressure(pos);
        self.pipes.remove(&pos);
        self.order.retain(|p| *p != pos);
        self.scheduled.retain(|(_, p)| *p != pos);
        self.notify_neighbors(env, pos);
    }

    /// A non-pipe block next to the network changed (placed/removed tank, ...).
    pub fn notify_neighbors(&mut self, env: &mut dyn PipeEnv, changed: Pos) {
        for d in Dir::ALL {
            let p = offset(changed, d);
            let Some(e) = self.pipes.get(&p) else { continue };
            let toward = d.opposite();
            // updateShape (FluidPipeBlock only)
            if matches!(e.block, PipeBlock::Pipe { .. }) {
                self.update_pipe_shape(env, p, toward);
            }
            // neighborChanged -> validateNeighbourChange
            let Some(e) = self.pipes.get(&p) else { continue };
            if self.pipes.contains_key(&changed) {
                continue;
            }
            let reacts = e.block.straight_axis().is_some() || matches!(e.block, PipeBlock::Encased { .. });
            if reacts && e.block.can_have_flow_toward(toward) && !e.block.is_pump() {
                self.schedule(p);
            }
        }
    }

    /// `updateShape` for the pipe at `pos` after the neighbour in `toward` changed.
    fn update_pipe_shape(&mut self, env: &mut dyn PipeEnv, pos: Pos, toward: Dir) {
        let Some(prev) = self.pipes.get(&pos).and_then(|e| e.block.open_sides().copied()) else { return };
        if !matches!(self.pipes[&pos].block, PipeBlock::Pipe { .. }) {
            return;
        }
        let new = update_block_state(prev, toward, Some(toward.opposite()), |d| self.can_connect_to(env, pos, d));
        if new != prev {
            self.pipes.get_mut(&pos).unwrap().block = PipeBlock::Pipe { open: new };
            self.after_state_change(env, pos);
        }
    }

    /// onPlace scheduling plus neighbour shape updates (bounded cascade).
    fn after_state_change(&mut self, env: &mut dyn PipeEnv, pos: Pos) {
        let mut work = VecDeque::from([pos]);
        let mut budget = 4096;
        while let Some(p) = work.pop_front() {
            budget -= 1;
            if budget == 0 {
                break;
            }
            self.schedule(p);
            for d in Dir::ALL {
                let n = offset(p, d);
                let Some(e) = self.pipes.get(&n) else { continue };
                if !matches!(e.block, PipeBlock::Pipe { .. }) {
                    continue;
                }
                let before = e.block.clone();
                self.update_pipe_shape_no_cascade(env, n, d.opposite());
                if self.pipes[&n].block != before {
                    work.push_back(n);
                }
            }
        }
    }

    fn update_pipe_shape_no_cascade(&mut self, env: &mut dyn PipeEnv, pos: Pos, toward: Dir) {
        let Some(PipeBlock::Pipe { open }) = self.pipes.get(&pos).map(|e| e.block.clone()) else { return };
        let new = update_block_state(open, toward, Some(toward.opposite()), |d| self.can_connect_to(env, pos, d));
        if new != open {
            self.pipes.get_mut(&pos).unwrap().block = PipeBlock::Pipe { open: new };
        }
    }

    /// Wrench on a plain pipe: becomes a glass pipe on `axis_hint` (the straight
    /// axis, or the axis of the nearest side the adapter picked). Returns false
    /// (PASS) when the clicked face lies on that axis.
    pub fn wrench_pipe(&mut self, env: &mut dyn PipeEnv, pos: Pos, clicked_face: Dir, nearest_axis: Axis) -> bool {
        let Some(e) = self.pipes.get(&pos) else { return false };
        if !matches!(e.block, PipeBlock::Pipe { .. }) {
            return false;
        }
        let axis = e.block.straight_axis().unwrap_or(nearest_axis);
        if clicked_face.axis() == axis {
            return false;
        }
        self.place_block(env, pos, PipeBlock::Glass { axis });
        true
    }

    /// Wrench on a glass pipe: `toRegularPipe` (both ends of the axis open).
    pub fn wrench_glass(&mut self, env: &mut dyn PipeEnv, pos: Pos) -> bool {
        let Some(PipeBlock::Glass { axis }) = self.pipes.get(&pos).map(|e| e.block.clone()) else { return false };
        self.place_block(env, pos, PipeBlock::pipe(&axis.directions()));
        true
    }

    /// Copper casing on a pipe or glass pipe (connections preserved).
    pub fn encase(&mut self, env: &mut dyn PipeEnv, pos: Pos) -> bool {
        let open = match self.pipes.get(&pos).map(|e| e.block.clone()) {
            Some(PipeBlock::Pipe { open }) => open,
            Some(PipeBlock::Glass { axis }) => {
                let mut o = [false; 6];
                for d in axis.directions() {
                    o[d.index()] = true;
                }
                o
            }
            _ => return false,
        };
        self.place_block(env, pos, PipeBlock::Encased { open });
        true
    }

    /// Wrench on an encased pipe: back to a regular pipe with the same sides.
    pub fn unencase(&mut self, env: &mut dyn PipeEnv, pos: Pos) -> bool {
        let Some(PipeBlock::Encased { open }) = self.pipes.get(&pos).map(|e| e.block.clone()) else { return false };
        self.place_block(env, pos, PipeBlock::Pipe { open });
        true
    }

    // ---- devices

    /// Kinetic speed change of a pump (`onSpeedChanged`).
    pub fn set_pump_speed(&mut self, env: &mut dyn PipeEnv, pos: Pos, speed: f32) {
        let Some(e) = self.pipes.get_mut(&pos) else { return };
        if !e.block.is_pump() {
            return;
        }
        let prev = e.speed;
        e.speed = speed;
        if prev.abs() == speed.abs() {
            return;
        }
        self.update_pressure_change(env, pos);
    }

    /// Re-orient a pump (front <-> back). Models the block-state change plus
    /// `updatePressureChange` (see docs for the assumption).
    pub fn set_pump_facing(&mut self, env: &mut dyn PipeEnv, pos: Pos, facing: Dir) {
        let Some(e) = self.pipes.get_mut(&pos) else { return };
        if !e.block.is_pump() {
            return;
        }
        let old = e.facing();
        e.block = PipeBlock::Pump { facing };
        if old != Some(facing) {
            self.update_pressure_change(env, pos);
            self.after_state_change(env, pos);
        }
    }

    /// Kinetic speed change of a valve: positive opens, otherwise closes.
    pub fn set_valve_speed(&mut self, pos: Pos, speed: f32) {
        if let Some(e) = self.pipes.get_mut(&pos) {
            if matches!(e.block, PipeBlock::Valve { .. }) {
                e.valve_target = if speed > 0.0 { 1.0 } else { 0.0 };
                e.valve_step = (speed.abs() / 16.0 / 20.0).clamp(0.0, 1.0);
            }
        }
    }

    /// Set or clear the smart pipe filter.
    pub fn set_smart_filter(&mut self, pos: Pos, filter: Option<Fluid>) {
        if let Some(e) = self.pipes.get_mut(&pos) {
            if let PipeBlock::Smart { axis, .. } = e.block {
                e.block = PipeBlock::Smart { axis, filter };
            }
        }
    }

    // ---- FluidTransportBehaviour

    fn wipe_pressure(&mut self, pos: Pos) {
        let Some(e) = self.pipes.get_mut(&pos) else { return };
        for d in Dir::ALL {
            if e.block.can_have_flow_toward(d) {
                e.conns.entry(d).or_insert_with(|| PipeConnection::new(d));
            } else {
                e.conns.remove(&d);
            }
        }
        e.phase = Phase::WaitForPumps;
        for c in e.conns.values_mut() {
            c.wipe_pressure();
        }
    }

    fn add_pressure(&mut self, pos: Pos, side: Dir, inbound: bool, amount: f32) {
        if let Some(c) = self.conn_mut(pos, side) {
            let cur = c.pressure.get(inbound);
            c.pressure.set(inbound, cur + amount);
        }
    }

    /// `PipeConnection.determineSource`.
    fn determine_source(&mut self, env: &mut dyn PipeEnv, pos: Pos, side: Dir) -> bool {
        let rel = offset(pos, side);
        if !env.is_loaded(rel) {
            return false;
        }
        let neighbor = if self.pipes.contains_key(&rel) {
            None
        } else {
            Some(env.neighbor(rel, side.opposite()))
        };
        let Some(c) = self.conn_mut(pos, side) else { return false };
        let src = match neighbor {
            None => Source::OtherPipe,
            Some(Neighbor::OpenEnd | Neighbor::Vanilla) => match c.previous_source.clone() {
                Some(Source::OpenEnd(o)) => Source::OpenEnd(o),
                _ => Source::OpenEnd(OpenEnd::default()),
            },
            Some(Neighbor::Handler) => Source::Handler,
            Some(Neighbor::Blocked) => Source::Blocked,
        };
        c.source = Some(src);
        true
    }

    fn provide_from_source(
        &mut self,
        env: &mut dyn PipeEnv,
        pos: Pos,
        side: Dir,
        pred: &dyn Fn(Fluid) -> bool,
    ) -> Option<Fluid> {
        let rel = offset(pos, side);
        match self.connection(pos, side)?.source.as_ref()? {
            Source::Handler => env.contents(rel, side.opposite()).into_iter().find(|f| pred(*f)),
            Source::OpenEnd(o) => o.provide(env, rel, pred),
            Source::OtherPipe => self
                .connection(rel, side.opposite())
                .and_then(PipeConnection::provided_outward)
                .filter(|f| pred(*f)),
            Source::Blocked => None,
        }
    }

    fn source_drain(&mut self, env: &mut dyn PipeEnv, pos: Pos, side: Dir, fluid: Fluid, amount: i32, sim: bool) -> i32 {
        let rel = offset(pos, side);
        match self.conn_mut(pos, side).and_then(|c| c.source.as_mut()) {
            Some(Source::Handler) => env.drain(rel, side.opposite(), fluid, amount, sim),
            Some(Source::OpenEnd(o)) => o.drain(env, rel, fluid, amount, sim),
            _ => 0,
        }
    }

    fn source_fill(
        &mut self,
        env: &mut dyn PipeEnv,
        target: BlockFace,
        fluid: Fluid,
        amount: i32,
        sim: bool,
        acc: &mut BTreeMap<BlockFace, i32>,
    ) -> Option<i32> {
        let rel = target.connected_pos();
        let extra = if sim { acc.get(&target).copied().unwrap_or(0) } else { 0 };
        let filled = match self.conn_mut(target.pos, target.face).and_then(|c| c.source.as_mut()) {
            Some(Source::Handler) => (env.fill(rel, target.face.opposite(), fluid, amount + extra, sim) - extra).max(0),
            Some(Source::OpenEnd(o)) => o.fill(env, rel, fluid, amount, sim),
            _ => return None,
        };
        if sim {
            *acc.entry(target).or_insert(0) += filled;
        }
        Some(filled)
    }

    /// `PipeConnection.manageFlows`; returns "send update".
    fn manage_flows(
        &mut self,
        env: &mut dyn PipeEnv,
        pos: Pos,
        side: Dir,
        internal: Option<Fluid>,
        pred: &dyn Fn(Fluid) -> bool,
    ) -> bool {
        let retained = self.conn_mut(pos, side).and_then(|c| c.network.take());
        if self.connection(pos, side).is_none() {
            return false;
        }
        if self.connection(pos, side).unwrap().source.is_none() && !self.determine_source(env, pos, side) {
            return false;
        }
        let c = self.connection(pos, side).unwrap();
        let (flow, pressure, cmp) = (c.flow, c.pressure, c.compare_pressure());
        let has_pressure = c.has_pressure();
        match flow {
            None => {
                if !has_pressure {
                    return false;
                }
                let prioritize_inbound = cmp < 0.0;
                for tf in [true, false] {
                    let inbound = prioritize_inbound == tf;
                    if pressure.get(inbound) == 0.0 {
                        continue;
                    }
                    let provided = if inbound { self.provide_from_source(env, pos, side, pred) } else { internal };
                    if self.conn_mut(pos, side).unwrap().try_start_flow(inbound, provided) {
                        return true;
                    }
                }
                false
            }
            Some(flow) => {
                let provided = if flow.inbound { self.provide_from_source(env, pos, side, pred) } else { internal };
                if !has_pressure || provided != Some(flow.fluid) {
                    self.conn_mut(pos, side).unwrap().flow = None;
                    return true;
                }
                if flow.inbound != (cmp < 0.0) {
                    let inbound = !flow.inbound;
                    let provided = if inbound { self.provide_from_source(env, pos, side, pred) } else { internal };
                    if provided.is_some() {
                        self.reset_affected_fluid_networks(pos, side);
                        self.conn_mut(pos, side).unwrap().try_start_flow(inbound, provided);
                        return true;
                    }
                }
                let endpoint = self.connection(pos, side).unwrap().source.as_ref().is_some_and(Source::is_endpoint);
                if !endpoint || !flow.inbound {
                    return false;
                }
                let mut net = retained.unwrap_or_else(|| Box::new(FluidNetwork::new(BlockFace::new(pos, side))));
                self.tick_network(env, &mut net);
                self.conn_mut(pos, side).unwrap().network = Some(net);
                false
            }
        }
    }

    /// `FluidNetwork.tick`.
    fn tick_network(&mut self, env: &mut dyn PipeEnv, net: &mut FluidNetwork) {
        if net.pause > 0 {
            net.pause -= 1;
            return;
        }
        for _ in 0..CYCLES_PER_TICK {
            let mut should_continue = false;
            let queued = std::mem::take(&mut net.queued);
            for bf in queued {
                if !env.is_loaded(bf.pos) {
                    net.queued.push(bf);
                    continue;
                }
                if let Some(c) = self.connection(bf.pos, bf.face) {
                    if bf == net.start {
                        net.transfer_speed = ((c.pressure.inbound / 2.0).max(1.0)) as i32;
                    }
                    net.frontier.push(bf);
                }
            }
            let frontier = std::mem::take(&mut net.frontier);
            let mut kept = Vec::new();
            for bf in frontier {
                if !env.is_loaded(bf.pos) {
                    kept.push(bf);
                    continue;
                }
                let Some(c) = self.connection(bf.pos, bf.face) else { continue };
                let Some(flow) = c.flow else {
                    kept.push(bf);
                    continue;
                };
                if net.fluid.is_some_and(|f| f != flow.fluid) {
                    continue;
                }
                if flow.inbound && c.compare_pressure() >= 0.0 {
                    continue;
                }
                if !flow.complete {
                    kept.push(bf);
                    continue;
                }
                net.fluid.get_or_insert(flow.fluid);
                let mut keep = false;
                for d in Dir::ALL {
                    if d == bf.face {
                        continue;
                    }
                    let Some(c2) = self.connection(bf.pos, d) else { continue };
                    let Some(f2) = c2.flow else {
                        if c2.pressure.outward > 0.0 {
                            keep = true;
                        }
                        continue;
                    };
                    if f2.inbound {
                        if c2.compare_pressure() > 0.0 {
                            keep = true;
                        }
                        continue;
                    }
                    if !f2.complete {
                        keep = true;
                        continue;
                    }
                    if c2.source.is_none() && !self.determine_source(env, bf.pos, d) {
                        keep = true;
                        continue;
                    }
                    let endpoint = self.connection(bf.pos, d).and_then(|c| c.source.as_ref()).is_some_and(Source::is_endpoint);
                    if endpoint {
                        let t = BlockFace::new(bf.pos, d);
                        if !net.targets.contains(&t) {
                            net.targets.push(t);
                        }
                    } else if net.visited.insert(offset(bf.pos, d)) {
                        net.queued.push(BlockFace::new(offset(bf.pos, d), d.opposite()));
                        should_continue = true;
                    }
                }
                if keep {
                    kept.push(bf);
                }
            }
            net.frontier = kept;
            if !should_continue {
                break;
            }
        }
        // Source handler of the start face.
        let start = net.start;
        let has_source = self
            .connection(start.pos, start.face)
            .and_then(|c| c.source.as_ref())
            .is_some_and(Source::is_endpoint);
        if !has_source || net.targets.is_empty() {
            return;
        }
        let Some(fluid) = net.fluid else { return };
        // Targets that vanished (block removed) are dropped.
        net.targets.retain(|t| self.connection(t.pos, t.face).is_some());
        let mut flow_speed = net.transfer_speed;
        for simulate in [true, false] {
            let drained = self.source_drain(env, start.pos, start.face, fluid, flow_speed, simulate);
            if drained <= 0 {
                return;
            }
            if simulate {
                flow_speed = drained;
            }
            let mut transfer = drained;
            let mut outputs = net.targets.clone();
            let mut acc = BTreeMap::new();
            while !outputs.is_empty() && transfer > 0 {
                let divided = transfer / outputs.len() as i32;
                let mut remainder = transfer % outputs.len() as i32;
                let mut remove = vec![false; outputs.len()];
                for (i, t) in outputs.iter().enumerate() {
                    let mut to = divided;
                    if remainder > 0 {
                        to += 1;
                        remainder -= 1;
                    }
                    if transfer <= 0 {
                        break;
                    }
                    match self.source_fill(env, *t, fluid, to, simulate, &mut acc) {
                        None => remove[i] = true,
                        Some(filled) => {
                            transfer -= filled;
                            if filled < to {
                                remove[i] = true;
                            }
                        }
                    }
                }
                let mut i = 0;
                outputs.retain(|_| {
                    i += 1;
                    !remove[i - 1]
                });
            }
            flow_speed -= transfer;
            if flow_speed <= 0 && simulate {
                return;
            }
        }
    }

    /// `FluidPropagator.resetAffectedFluidNetworks`.
    fn reset_affected_fluid_networks(&mut self, start: Pos, side: Dir) {
        let mut frontier = VecDeque::from([start]);
        let mut visited = BTreeSet::new();
        while let Some(pos) = frontier.pop_front() {
            if !visited.insert(pos) || !self.pipes.contains_key(&pos) {
                continue;
            }
            for d in Dir::ALL {
                if pos == start && d != side {
                    continue;
                }
                let target = offset(pos, d);
                if visited.contains(&target) {
                    continue;
                }
                let Some(c) = self.conn_mut(pos, d) else { continue };
                if !c.flow.is_some_and(|f| f.inbound) {
                    continue;
                }
                c.reset_network();
                frontier.push_back(target);
            }
        }
    }

    /// `FluidPropagator.propagateChangedPipe`.
    pub fn propagate_changed_pipe(&mut self, env: &mut dyn PipeEnv, origin: Pos) {
        let mut frontier = VecDeque::from([(0, origin)]);
        let mut visited = BTreeSet::new();
        let mut pumps: Vec<(Pos, Dir)> = vec![];
        while let Some((dist, pos)) = frontier.pop_front() {
            if !visited.insert(pos) {
                continue;
            }
            let Some(block) = self.pipes.get(&pos).map(|e| e.block.clone()) else { continue };
            self.wipe_pressure(pos);
            for d in Dir::ALL {
                if !block.can_have_flow_toward(d) {
                    continue;
                }
                let target = offset(pos, d);
                if !env.is_loaded(target) {
                    continue;
                }
                if let Some(te) = self.pipes.get(&target) {
                    if let PipeBlock::Pump { facing } = te.block {
                        if facing.axis() == d.axis() {
                            pumps.push((target, d.opposite()));
                        }
                        continue;
                    }
                }
                if visited.contains(&target) {
                    continue;
                }
                let Some(te) = self.pipes.get(&target) else { continue };
                if dist >= self.pump_range && !te.has_any_pressure() {
                    continue;
                }
                if te.block.can_have_flow_toward(d.opposite()) {
                    frontier.push_back((dist + 1, target));
                }
            }
        }
        for (pump, side) in pumps {
            self.update_pipes_on_side(pump, side);
        }
    }

    // ---- PumpBlockEntity

    fn update_pressure_change(&mut self, env: &mut dyn PipeEnv, pos: Pos) {
        let Some(facing) = self.pipes.get(&pos).and_then(PipeEntity::facing) else { return };
        for d in [facing, facing.opposite()] {
            self.propagate_changed_pipe(env, offset(pos, d));
        }
        self.wipe_pressure(pos);
        if let Some(e) = self.pipes.get_mut(&pos) {
            e.sides_to_update = [true, true];
        }
    }

    fn update_pipes_on_side(&mut self, pump: Pos, side: Dir) {
        let Some(e) = self.pipes.get_mut(&pump) else { return };
        let Some(facing) = e.facing() else { return };
        if side.axis() != facing.axis() {
            return;
        }
        let front = side == facing;
        e.sides_to_update[usize::from(!front)] = true;
        self.wipe_pressure(pump);
    }

    /// `hasReachedValidEndpoint`.
    fn has_reached_valid_endpoint(&self, env: &dyn PipeEnv, face: BlockFace, pull: bool) -> bool {
        let n = face.connected_pos();
        if let Some(e) = self.pipes.get(&n) {
            if let PipeBlock::Pump { facing } = e.block {
                if facing.axis() == face.face.axis() {
                    let other_front = face.face.opposite() == facing;
                    return !other_front != pull;
                }
            }
            return false;
        }
        matches!(
            env.neighbor(n, face.face.opposite()),
            Neighbor::Handler | Neighbor::Vanilla | Neighbor::OpenEnd
        )
    }

    /// `distributePressureTo`.
    fn distribute_pressure_to(&mut self, env: &mut dyn PipeEnv, pump: Pos, side: Dir) {
        let Some(e) = self.pipes.get(&pump) else { return };
        let speed = e.speed;
        let facing = e.facing().unwrap();
        if speed == 0.0 {
            return;
        }
        let start = BlockFace::new(pump, side);
        let pull = side != facing; // isPullingOnSide(isFront)
        if !pull {
            self.reset_affected_fluid_networks(pump, side.opposite());
        }
        if self.has_reached_valid_endpoint(env, start, pull) {
            return;
        }
        let mut graph: BTreeMap<Pos, BTreeMap<Dir, bool>> = BTreeMap::new();
        let mut targets: BTreeSet<BlockFace> = BTreeSet::new();
        let mut visited: BTreeSet<Pos> = BTreeSet::new();
        graph.entry(pump).or_default().insert(side, pull);
        graph.entry(start.connected_pos()).or_default().insert(side.opposite(), !pull);
        let mut frontier = VecDeque::from([(1, start.connected_pos())]);
        let max_distance = self.pump_range;
        while let Some((distance, pos)) = frontier.pop_front() {
            if !env.is_loaded(pos) || !visited.insert(pos) {
                continue;
            }
            let Some(block) = self.pipes.get(&pos).map(|e| e.block.clone()) else { continue };
            for face in Dir::ALL {
                if !block.can_have_flow_toward(face) {
                    continue;
                }
                let bf = BlockFace::new(pos, face);
                let connected = bf.connected_pos();
                if !env.is_loaded(connected) || bf.is_equivalent(start) {
                    continue;
                }
                if self.has_reached_valid_endpoint(env, bf, pull) {
                    graph.entry(pos).or_default().insert(face, pull);
                    targets.insert(bf);
                    continue;
                }
                let Some(ne) = self.pipes.get(&connected) else { continue };
                if ne.block.is_pump() || visited.contains(&connected) {
                    continue;
                }
                if distance + 1 >= max_distance {
                    graph.entry(pos).or_default().insert(face, pull);
                    targets.insert(bf);
                    continue;
                }
                graph.entry(pos).or_default().insert(face, pull);
                graph.entry(connected).or_default().insert(face.opposite(), !pull);
                frontier.push_back((distance + 1, connected));
            }
        }
        let mut valid: BTreeMap<i32, BTreeSet<BlockFace>> = BTreeMap::new();
        search_for_endpoint(&graph, &targets, &mut valid, BlockFace::new(pump, side.opposite()), pull, 0);
        let pressure = speed.abs();
        for set in valid.values() {
            let branches = (set.len() as i32 - 1).max(1) as f32;
            for bf in set {
                if bf.pos == pump {
                    continue;
                }
                let Some(inbound) = graph.get(&bf.pos).and_then(|m| m.get(&bf.face)).copied() else { continue };
                self.add_pressure(bf.pos, bf.face, inbound, pressure / branches);
            }
        }
    }

    // ---- ticking

    /// One game tick: scheduled block ticks first (delay 1), then block entities
    /// in registration order.
    pub fn tick(&mut self, env: &mut dyn PipeEnv) {
        self.game_time += 1;
        let now = self.game_time;
        let due: Vec<Pos> = self.scheduled.iter().filter(|(t, _)| *t <= now).map(|(_, p)| *p).collect();
        self.scheduled.retain(|(t, _)| *t > now);
        for p in due {
            if self.pipes.contains_key(&p) {
                self.propagate_changed_pipe(env, p);
            }
        }
        for p in self.order.clone() {
            if self.pipes.contains_key(&p) {
                self.tick_entity(env, p);
            }
        }
    }

    fn tick_entity(&mut self, env: &mut dyn PipeEnv, pos: Pos) {
        self.tick_behaviour(env, pos);
        let before = self.pipes.get(&pos).map(|e| e.block.clone());
        self.tick_device(env, pos);
        // AxisPipeBlock.onPlace: a changed state (valve ENABLED) schedules a propagate tick.
        if self.pipes.get(&pos).map(|e| &e.block) != before.as_ref() {
            self.schedule(pos);
        }
    }

    fn tick_device(&mut self, env: &mut dyn PipeEnv, pos: Pos) {
        let external_valves = self.external_valves;
        let Some(e) = self.pipes.get_mut(&pos) else { return };
        match e.block {
            PipeBlock::Pump { facing } => {
                // PumpFluidTransferBehaviour.tick (after the base tick).
                let speed = e.speed.abs();
                for (d, c) in e.conns.iter_mut() {
                    let pull = *d != facing;
                    c.pressure.set(pull, speed);
                    c.pressure.set(!pull, 0.0);
                }
                for (idx, front) in [true, false].into_iter().enumerate() {
                    if self.pipes[&pos].sides_to_update[idx] {
                        self.pipes.get_mut(&pos).unwrap().sides_to_update[idx] = false;
                        let side = if front { facing } else { facing.opposite() };
                        self.distribute_pressure_to(env, pos, side);
                    }
                }
            }
            PipeBlock::Valve { axis, enabled } if !external_valves => {
                let diff = e.valve_target - e.valve_pointer;
                e.valve_pointer += diff.clamp(-e.valve_step, e.valve_step);
                if enabled && e.valve_pointer == 0.0 {
                    e.block = PipeBlock::Valve { axis, enabled: false };
                } else if !enabled && e.valve_pointer == 1.0 {
                    e.block = PipeBlock::Valve { axis, enabled: true };
                }
            }
            _ => {}
        }
    }

    /// `FluidTransportBehaviour.tick` (server side).
    fn tick_behaviour(&mut self, env: &mut dyn PipeEnv, pos: Pos) {
        let phase = self.pipes[&pos].phase;
        if phase == Phase::WaitForPumps {
            self.pipes.get_mut(&pos).unwrap().phase = Phase::FlipFlows;
            return;
        }
        let sides: Vec<Dir> = self.pipes[&pos].conns.keys().copied().collect();
        for &s in &sides {
            self.conn_mut(pos, s).unwrap().flip_flows_if_pressure_reversed();
            if self.connection(pos, s).unwrap().source.is_none() {
                self.determine_source(env, pos, s);
            }
        }
        if phase == Phase::FlipFlows {
            self.pipes.get_mut(&pos).unwrap().phase = Phase::Idle;
            return;
        }
        let mut available: Option<Fluid> = None;
        let mut single: Option<Dir> = None;
        for &s in &sides {
            let Some(f) = self.connection(pos, s).unwrap().provided_fluid() else { continue };
            match available {
                None => {
                    available = Some(f);
                    single = Some(s);
                }
                Some(a) if a == f => single = None,
                Some(a) => {
                    env.pipe_flow_collision(pos, a, f);
                    return;
                }
            }
        }
        let block = self.pipes[&pos].block.clone();
        for &s in &sides {
            let internal = if single == Some(s) { None } else { available };
            let b = block.clone();
            let pred = move |f: Fluid| b.can_pull_fluid_from(f);
            self.manage_flows(env, pos, s, internal, &pred);
        }
        for &s in &sides {
            if let Some(c) = self.conn_mut(pos, s) {
                c.tick_flow_progress();
            }
        }
    }
}

/// `searchForEndpointRecursively`.
fn search_for_endpoint(
    graph: &BTreeMap<Pos, BTreeMap<Dir, bool>>,
    targets: &BTreeSet<BlockFace>,
    valid: &mut BTreeMap<i32, BTreeSet<BlockFace>>,
    current: BlockFace,
    pull: bool,
    distance: i32,
) -> bool {
    let Some(map) = graph.get(&current.pos) else { return false };
    let mut ok = false;
    for side in Dir::ALL {
        if side == current.face {
            continue;
        }
        let Some(&dir_pull) = map.get(&side) else { continue };
        let next = BlockFace::new(current.pos, side);
        if targets.contains(&next) {
            valid.entry(distance).or_default().insert(next);
            ok = true;
            continue;
        }
        if dir_pull != pull {
            continue;
        }
        if !search_for_endpoint(graph, targets, valid, next.opposite(), pull, distance + 1) {
            continue;
        }
        valid.entry(distance).or_default().insert(next);
        ok = true;
    }
    if ok {
        valid.entry(distance).or_default().insert(current);
    }
    ok
}

// ---------------------------------------------------------------- reference env

/// A fluid tank handler (single fluid).
#[derive(Clone, Debug, PartialEq)]
pub struct Tank {
    pub fluid: Option<Fluid>,
    pub amount: i32,
    pub capacity: i32,
}

impl Tank {
    pub fn new(capacity: i32) -> Self {
        Self { fluid: None, amount: 0, capacity }
    }

    pub fn filled(capacity: i32, fluid: Fluid, amount: i32) -> Self {
        Self { fluid: Some(fluid), amount, capacity }
    }

    pub fn fill(&mut self, fluid: Fluid, amount: i32, simulate: bool) -> i32 {
        if amount <= 0 || (self.amount > 0 && self.fluid != Some(fluid)) {
            return 0;
        }
        let n = amount.min(self.capacity - self.amount);
        if !simulate && n > 0 {
            self.fluid = Some(fluid);
            self.amount += n;
        }
        n
    }

    pub fn drain(&mut self, fluid: Fluid, amount: i32, simulate: bool) -> i32 {
        if self.fluid != Some(fluid) || amount <= 0 {
            return 0;
        }
        let n = amount.min(self.amount);
        if !simulate {
            self.amount -= n;
            if self.amount == 0 {
                self.fluid = None;
            }
        }
        n
    }
}

/// World cell for open ends in [`MemoryEnv`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Air,
    Solid,
    Source(Fluid),
    /// Infinite source (ocean water): removal leaves the cell unchanged.
    Infinite(Fluid),
    /// Flowing (non-source) fluid; a different fluid cannot be placed over it.
    Flowing(Fluid),
}

/// Simple in-memory world used by tests and by adapters' own tests.
#[derive(Clone, Debug, Default)]
pub struct MemoryEnv {
    pub tanks: BTreeMap<Pos, Tank>,
    pub cells: BTreeMap<Pos, Cell>,
    pub unloaded: BTreeSet<Pos>,
    pub collisions: Vec<(Pos, Fluid, Fluid)>,
}

impl MemoryEnv {
    pub fn cell(&self, pos: Pos) -> Cell {
        self.cells.get(&pos).copied().unwrap_or(Cell::Air)
    }
}

impl PipeEnv for MemoryEnv {
    fn is_loaded(&self, pos: Pos) -> bool {
        !self.unloaded.contains(&pos)
    }

    fn neighbor(&self, pos: Pos, _face: Dir) -> Neighbor {
        if self.tanks.contains_key(&pos) {
            return Neighbor::Handler;
        }
        match self.cell(pos) {
            Cell::Solid => Neighbor::Blocked,
            _ => Neighbor::OpenEnd,
        }
    }

    fn contents(&self, pos: Pos, _face: Dir) -> Vec<Fluid> {
        self.tanks.get(&pos).and_then(|t| t.fluid.filter(|_| t.amount > 0)).into_iter().collect()
    }

    fn fill(&mut self, pos: Pos, _face: Dir, fluid: Fluid, amount: i32, simulate: bool) -> i32 {
        self.tanks.get_mut(&pos).map_or(0, |t| t.fill(fluid, amount, simulate))
    }

    fn drain(&mut self, pos: Pos, _face: Dir, fluid: Fluid, amount: i32, simulate: bool) -> i32 {
        self.tanks.get_mut(&pos).map_or(0, |t| t.drain(fluid, amount, simulate))
    }

    fn place_fluid(&mut self, pos: Pos, fluid: Fluid, simulate: bool) -> bool {
        match self.cell(pos) {
            Cell::Air => {}
            Cell::Flowing(f) if f == fluid => {}
            _ => return false,
        }
        if !simulate {
            self.cells.insert(pos, Cell::Source(fluid));
        }
        true
    }

    fn take_source(&mut self, pos: Pos, simulate: bool) -> Option<Fluid> {
        match self.cell(pos) {
            Cell::Source(f) => {
                if !simulate {
                    self.cells.insert(pos, Cell::Air);
                }
                Some(f)
            }
            Cell::Infinite(f) => Some(f),
            _ => None,
        }
    }

    fn pipe_flow_collision(&mut self, pos: Pos, a: Fluid, b: Fluid) {
        self.collisions.push((pos, a, b));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: Fluid = Fluid::WATER;
    const L: Fluid = Fluid::LAVA;

    fn solid_env() -> MemoryEnv {
        MemoryEnv::default()
    }

    /// tank(0) pipes(1..) ... along +x. Returns (sim, env).
    fn line(tank_a: Tank, pipes: &[PipeBlock], tank_b: Option<Tank>) -> (PipeSim, MemoryEnv) {
        let mut env = solid_env();
        env.tanks.insert((0, 0, 0), tank_a);
        let end = pipes.len() as i32 + 1;
        match tank_b {
            Some(t) => {
                env.tanks.insert((end, 0, 0), t);
            }
            None => {
                env.cells.insert((end, 0, 0), Cell::Air);
            }
        }
        let mut sim = PipeSim::new();
        for (i, b) in pipes.iter().enumerate() {
            let pos = (i as i32 + 1, 0, 0);
            match b {
                PipeBlock::Pipe { .. } => sim.place_pipe(&mut env, pos, Dir::East),
                other => sim.place_block(&mut env, pos, other.clone()),
            }
        }
        (sim, env)
    }

    fn pipe() -> PipeBlock {
        PipeBlock::pipe(&[])
    }

    fn pump(f: Dir) -> PipeBlock {
        PipeBlock::Pump { facing: f }
    }

    fn run(sim: &mut PipeSim, env: &mut MemoryEnv, ticks: usize) {
        for _ in 0..ticks {
            sim.tick(env);
        }
    }

    fn amount(env: &MemoryEnv, pos: Pos) -> i32 {
        env.tanks[&pos].amount
    }

    #[test]
    fn first_transfer_tick_regression() {
        // Self-derived regression values for [tank, pipes.., pump, pipes.., tank]
        // (pressure 32): not a Java measurement, see docs/fluid-pipes.md.
        let mut got = vec![];
        for (n, speed) in [(1usize, 32.0f32), (3, 32.0), (3, 4.0), (3, 64.0)] {
            let mut blocks = vec![pipe(); n];
            blocks.push(pump(Dir::East));
            blocks.extend(std::iter::repeat_n(pipe(), n));
            let (mut sim, mut env) = line(Tank::filled(100_000, W, 100_000), &blocks, Some(Tank::new(100_000)));
            sim.set_pump_speed(&mut env, (n as i32 + 1, 0, 0), speed);
            let mut first = 0;
            for t in 1..=300 {
                sim.tick(&mut env);
                if first == 0 && amount(&env, (2 * n as i32 + 2, 0, 0)) > 0 {
                    first = t;
                }
            }
            got.push(first);
        }
        assert_eq!(got, vec![25, 53, 235, 25]);
    }
    #[test]
    fn dir_basics() {
        for d in Dir::ALL {
            assert_eq!(d.opposite().opposite(), d);
            assert_eq!(d.axis(), d.opposite().axis());
            assert_ne!(d, d.opposite());
            assert_eq!(Dir::parse(d.name()), Some(d));
        }
        assert_eq!(offset((1, 2, 3), Dir::North), (1, 2, 2));
        let bf = BlockFace::new((0, 0, 0), Dir::East);
        assert_eq!(bf.opposite(), BlockFace::new((1, 0, 0), Dir::West));
        assert!(bf.is_equivalent(bf.opposite()));
    }

    #[test]
    fn shape_rules_straight_bend_branch() {
        // Single neighbour -> straight through.
        let s = update_block_state([false; 6], Dir::North, None, |d| d == Dir::East);
        assert!(s[Dir::East.index()] && s[Dir::West.index()]);
        assert_eq!(s.iter().filter(|b| **b).count(), 2);
        // Two perpendicular neighbours -> bend.
        let s = update_block_state([false; 6], Dir::North, None, |d| d == Dir::East || d == Dir::Up);
        assert_eq!(s.iter().filter(|b| **b).count(), 2);
        assert!(s[Dir::East.index()] && s[Dir::Up.index()]);
        assert_eq!(PipeBlock::Pipe { open: s }.straight_axis(), None);
        // Three neighbours -> branch.
        let s = update_block_state([false; 6], Dir::North, None, |d| d != Dir::Down && d != Dir::Up && d != Dir::South);
        assert_eq!(s.iter().filter(|b| **b).count(), 3);
        // No neighbour: preferred axis, or keep a previous 2-sided shape.
        let s = update_block_state([false; 6], Dir::Up, None, |_| false);
        assert!(s[Dir::Up.index()] && s[Dir::Down.index()]);
        let prev = PipeBlock::pipe(&[Dir::East, Dir::North]);
        let PipeBlock::Pipe { open: prev } = prev else { unreachable!() };
        assert_eq!(update_block_state(prev, Dir::Up, None, |_| false), prev);
        // Ignored side keeps its previous value.
        let s = update_block_state([false; 6], Dir::East, Some(Dir::West), |d| d == Dir::East);
        assert!(s[Dir::East.index()] && s[Dir::West.index()]);
    }

    #[test]
    fn straight_axis_detection() {
        assert_eq!(PipeBlock::pipe(&[Dir::East, Dir::West]).straight_axis(), Some(Axis::X));
        assert_eq!(PipeBlock::pipe(&[Dir::East, Dir::Up]).straight_axis(), None);
        assert_eq!(PipeBlock::pipe(&[Dir::East, Dir::West, Dir::Up]).straight_axis(), None);
        assert_eq!(pump(Dir::Up).straight_axis(), Some(Axis::Y));
    }

    #[test]
    fn placement_connects_to_tanks_pipes_and_updates_neighbours() {
        let mut env = solid_env();
        env.tanks.insert((0, 0, 0), Tank::new(1000));
        let mut sim = PipeSim::new();
        sim.place_pipe(&mut env, (1, 0, 0), Dir::Up);
        // Tank on the west: open west+east although "looking" up.
        assert_eq!(sim.block((1, 0, 0)), Some(&PipeBlock::pipe(&[Dir::West, Dir::East])));
        // A pipe placed above becomes a branch of the first one's column.
        sim.place_pipe(&mut env, (1, 1, 0), Dir::Up);
        assert_eq!(sim.block((1, 1, 0)), Some(&PipeBlock::pipe(&[Dir::Down, Dir::Up])));
        // Tank west + pipe above: the first pipe turns into a bend (the straight stub is dropped).
        assert_eq!(sim.block((1, 0, 0)), Some(&PipeBlock::pipe(&[Dir::Up, Dir::West])));
        // Place a pipe east: the bend gains a third side and becomes a branch.
        sim.place_pipe(&mut env, (2, 0, 0), Dir::East);
        assert_eq!(sim.block((1, 0, 0)), Some(&PipeBlock::pipe(&[Dir::Up, Dir::West, Dir::East])));
    }

    #[test]
    fn bend_is_built_from_two_perpendicular_neighbours() {
        let mut env = solid_env();
        let mut sim = PipeSim::new();
        sim.place_pipe(&mut env, (1, 0, 0), Dir::East);
        sim.place_pipe(&mut env, (1, 1, 0), Dir::Up);
        sim.place_pipe(&mut env, (2, 0, 0), Dir::East);
        // (1,0,0) has neighbours up and east -> after both exist it is a branch
        // with the original west stub, which Create keeps (stub toward air).
        let Some(PipeBlock::Pipe { open }) = sim.block((1, 0, 0)) else { panic!() };
        assert!(open[Dir::Up.index()] && open[Dir::East.index()]);
        assert!(!open[Dir::North.index()]);
    }

    #[test]
    fn glass_and_casing_conversions_keep_connections() {
        let (mut sim, mut env) = line(Tank::new(10), &[pipe(), pipe()], Some(Tank::new(10)));
        assert!(!sim.wrench_pipe(&mut env, (1, 0, 0), Dir::East, Axis::Y)); // clicked along the axis
        assert!(sim.wrench_pipe(&mut env, (1, 0, 0), Dir::Up, Axis::Y));
        assert_eq!(sim.block((1, 0, 0)), Some(&PipeBlock::Glass { axis: Axis::X }));
        assert!(sim.wrench_glass(&mut env, (1, 0, 0)));
        assert_eq!(sim.block((1, 0, 0)), Some(&PipeBlock::pipe(&[Dir::West, Dir::East])));
        assert!(sim.encase(&mut env, (2, 0, 0)));
        assert!(matches!(sim.block((2, 0, 0)), Some(PipeBlock::Encased { .. })));
        assert!(sim.unencase(&mut env, (2, 0, 0)));
        assert_eq!(sim.block((2, 0, 0)), Some(&PipeBlock::pipe(&[Dir::West, Dir::East])));
        assert!(!sim.encase(&mut env, (9, 0, 0)));
    }

    #[test]
    fn flow_progress_uses_the_create_speed_formula() {
        let mut c = PipeConnection::new(Dir::East);
        c.pressure.outward = 128.0;
        c.flow = Some(Flow { inbound: false, fluid: W, progress: 0.0, complete: false });
        c.tick_flow_progress();
        assert_eq!(c.flow.unwrap().progress, 1.0);
        assert!(c.flow.unwrap().complete);
        let mut slow = PipeConnection::new(Dir::East);
        slow.pressure.outward = 0.0;
        slow.flow = Some(Flow { inbound: false, fluid: W, progress: 0.0, complete: false });
        for _ in 0..31 {
            slow.tick_flow_progress();
        }
        assert!(!slow.flow.unwrap().complete);
        slow.tick_flow_progress();
        assert!(slow.flow.unwrap().complete);
    }

    #[test]
    fn pump_moves_water_between_tanks() {
        let blocks = [pipe(), pipe(), pump(Dir::East), pipe(), pipe()];
        let (mut sim, mut env) = line(Tank::filled(100_000, W, 100_000), &blocks, Some(Tank::new(100_000)));
        sim.set_pump_speed(&mut env, (3, 0, 0), 32.0);
        run(&mut sim, &mut env, 5);
        assert_eq!(amount(&env, (6, 0, 0)), 0, "network still starting");
        run(&mut sim, &mut env, 100);
        let moved = amount(&env, (6, 0, 0));
        assert!(moved > 0);
        assert_eq!(moved + amount(&env, (0, 0, 0)), 100_000);
        // Steady state: transfer rate = pressure / 2 per tick.
        let before = amount(&env, (6, 0, 0));
        run(&mut sim, &mut env, 10);
        assert_eq!(amount(&env, (6, 0, 0)) - before, 160);
        assert_eq!(sim.network((1, 0, 0), Dir::West).unwrap().transfer_speed, 16);
    }

    #[test]
    fn pressure_on_straight_path_equals_pump_speed() {
        let blocks = [pipe(), pump(Dir::East), pipe(), pipe()];
        let (mut sim, mut env) = line(Tank::filled(1000, W, 1000), &blocks, Some(Tank::new(1000)));
        sim.set_pump_speed(&mut env, (2, 0, 0), 64.0);
        run(&mut sim, &mut env, 4);
        // Back side pulls (inbound), front side pushes (outward).
        assert_eq!(sim.pressure((1, 0, 0), Dir::West), Some(Pressure { inbound: 64.0, outward: 0.0 }));
        assert_eq!(sim.pressure((1, 0, 0), Dir::East), Some(Pressure { inbound: 0.0, outward: 64.0 }));
        assert_eq!(sim.pressure((3, 0, 0), Dir::West), Some(Pressure { inbound: 64.0, outward: 0.0 }));
        assert_eq!(sim.pressure((3, 0, 0), Dir::East).unwrap().outward, 64.0);
        assert_eq!(sim.pressure((4, 0, 0), Dir::West).unwrap().inbound, 64.0);
        assert_eq!(sim.pressure((4, 0, 0), Dir::East).unwrap().outward, 64.0);
        assert_eq!(sim.pressure((2, 0, 0), Dir::East).unwrap().outward, 64.0);
        assert_eq!(sim.pressure((2, 0, 0), Dir::West).unwrap().inbound, 64.0);
    }


    #[test]
    fn branches_split_pressure_between_parallel_targets() {
        // Pump (0) facing east feeds a T at (1,0,0): tank east at (2,0,0), tank south at (1,0,1).
        let mut env = solid_env();
        env.tanks.insert((2, 0, 0), Tank::new(10_000));
        env.tanks.insert((1, 0, 1), Tank::new(10_000));
        let mut sim = PipeSim::new();
        sim.place_block(&mut env, (0, 0, 0), pump(Dir::East));
        sim.place_pipe(&mut env, (1, 0, 0), Dir::East);
        assert_eq!(sim.block((1, 0, 0)), Some(&PipeBlock::pipe(&[Dir::West, Dir::South, Dir::East])));
        sim.set_pump_speed(&mut env, (0, 0, 0), 32.0);
        run(&mut sim, &mut env, 4);
        // Entry face and both exits are one distance set of size 3 -> 2 parallel branches.
        assert_eq!(sim.pressure((1, 0, 0), Dir::East).unwrap().outward, 16.0);
        assert_eq!(sim.pressure((1, 0, 0), Dir::South).unwrap().outward, 16.0);
        assert_eq!(sim.pressure((1, 0, 0), Dir::West).unwrap().inbound, 16.0);
        assert_eq!(sim.pressure((0, 0, 0), Dir::East).unwrap().outward, 32.0);
    }

    #[test]
    fn two_targets_share_the_transfer_with_remainder() {
        let mut env = solid_env();
        env.tanks.insert((-1, 0, 0), Tank::filled(100_000, W, 100_000));
        env.tanks.insert((2, 0, 0), Tank::new(100_000));
        env.tanks.insert((1, 0, 1), Tank::new(100_000));
        let mut sim = PipeSim::new();
        // source tank -> pump -> pipe junction with two tank outlets
        sim.place_block(&mut env, (0, 0, 0), pump(Dir::East));
        sim.place_pipe(&mut env, (1, 0, 0), Dir::East);
        sim.set_pump_speed(&mut env, (0, 0, 0), 5.0);
        // The pump's back side touches the source tank directly.
        run(&mut sim, &mut env, 80);
        let a = amount(&env, (2, 0, 0));
        let b = amount(&env, (1, 0, 1));
        assert!(a > 0 && b > 0, "both outlets receive fluid: {a} {b}");
        assert!((a - b).abs() <= 80, "split stays within one rounding unit per tick");
        assert_eq!(a + b + amount(&env, (-1, 0, 0)), 100_000);
    }

    #[test]
    fn pump_range_limits_pipes_per_side_to_fifteen() {
        for (n, reaches) in [(15usize, true), (16, false)] {
            let mut blocks = vec![pump(Dir::East)];
            blocks.extend(std::iter::repeat_n(pipe(), n));
            let mut env = solid_env();
            env.tanks.insert((0, 0, 0), Tank::new(10));
            let mut sim = PipeSim::new();
            sim.place_block(&mut env, (1, 0, 0), pump(Dir::East));
            env.tanks.get_mut(&(0, 0, 0)).unwrap().fluid = None;
            for i in 0..n as i32 {
                sim.place_pipe(&mut env, (2 + i, 0, 0), Dir::East);
            }
            sim.set_pump_speed(&mut env, (1, 0, 0), 32.0);
            run(&mut sim, &mut env, 4);
            let last = (1 + n as i32, 0, 0);
            let p = sim.pressure(last, Dir::West).unwrap().inbound;
            assert_eq!(p > 0.0, reaches, "n={n} pressure at last pipe: {p}");
            if !reaches {
                // Pressure stops at the 15th pipe.
                assert!(sim.pressure((16, 0, 0), Dir::West).unwrap().inbound > 0.0);
                assert_eq!(sim.pressure((17, 0, 0), Dir::West).unwrap().inbound, 0.0);
            }
        }
    }

    #[test]
    fn pump_reversal_flips_direction_and_flows() {
        let blocks = [pipe(), pump(Dir::East), pipe()];
        let (mut sim, mut env) = line(Tank::filled(5000, W, 2000), &blocks, Some(Tank::filled(5000, W, 2000)));
        sim.set_pump_speed(&mut env, (2, 0, 0), 32.0);
        run(&mut sim, &mut env, 80);
        let a1 = amount(&env, (0, 0, 0));
        let b1 = amount(&env, (4, 0, 0));
        assert!(a1 < 2000 && b1 > 2000, "east flow first: {a1} {b1}");
        sim.set_pump_facing(&mut env, (2, 0, 0), Dir::West);
        run(&mut sim, &mut env, 80);
        let a2 = amount(&env, (0, 0, 0));
        let b2 = amount(&env, (4, 0, 0));
        assert!(a2 > a1 && b2 < b1, "west flow after reversal: {a2} {b2}");
        assert_eq!(a2 + b2, 4000);
        // Pressure orientation swapped.
        assert_eq!(sim.pressure((3, 0, 0), Dir::West), Some(Pressure { inbound: 0.0, outward: 32.0 }));
    }

    #[test]
    fn complete_flows_flip_in_place_when_pressure_reverses() {
        let mut c = PipeConnection::new(Dir::East);
        c.pressure = Pressure { inbound: 0.0, outward: 16.0 };
        c.flow = Some(Flow { inbound: false, fluid: W, progress: 1.0, complete: true });
        c.pressure = Pressure { inbound: 16.0, outward: 0.0 };
        assert!(c.flip_flows_if_pressure_reversed());
        assert!(c.flow.unwrap().inbound);
        // Incomplete flows are cleared.
        c.pressure = Pressure { inbound: 0.0, outward: 16.0 };
        c.flow = Some(Flow { inbound: true, fluid: W, progress: 0.5, complete: false });
        assert!(c.flip_flows_if_pressure_reversed());
        assert!(c.flow.is_none());
        // Two-sided pressure never flips.
        c.pressure = Pressure { inbound: 4.0, outward: 16.0 };
        c.flow = Some(Flow { inbound: true, fluid: W, progress: 1.0, complete: true });
        assert!(!c.flip_flows_if_pressure_reversed());
    }

    #[test]
    fn behaviour_phases_delay_flow_creation() {
        let blocks = [pump(Dir::East), pipe()];
        let (mut sim, mut env) = line(Tank::filled(1000, W, 1000), &blocks, Some(Tank::new(1000)));
        sim.set_pump_speed(&mut env, (1, 0, 0), 32.0);
        assert_eq!(sim.entity((2, 0, 0)).unwrap().phase, Phase::WaitForPumps);
        sim.tick(&mut env); // scheduled propagate wipes (-> WaitForPumps), pump distributes
        assert!(sim.flow((1, 0, 0), Dir::West).is_none());
        run(&mut sim, &mut env, 3);
        assert_eq!(sim.entity((2, 0, 0)).unwrap().phase, Phase::Idle);
        assert!(sim.flow((1, 0, 0), Dir::West).is_some());
    }

    #[test]
    fn closed_valve_blocks_and_opening_takes_pointer_time() {
        let blocks = [
            pipe(),
            pump(Dir::East),
            PipeBlock::Valve { axis: Axis::X, enabled: false },
            pipe(),
        ];
        let (mut sim, mut env) = line(Tank::filled(100_000, W, 100_000), &blocks, Some(Tank::new(100_000)));
        sim.set_pump_speed(&mut env, (2, 0, 0), 64.0);
        run(&mut sim, &mut env, 60);
        assert_eq!(amount(&env, (5, 0, 0)), 0, "closed valve stops the flow");
        // speed 32 -> step 0.1 per tick -> open after 10 ticks.
        sim.set_valve_speed((3, 0, 0), 32.0);
        run(&mut sim, &mut env, 9);
        assert!(matches!(sim.block((3, 0, 0)), Some(PipeBlock::Valve { enabled: false, .. })));
        run(&mut sim, &mut env, 1);
        assert!(matches!(sim.block((3, 0, 0)), Some(PipeBlock::Valve { enabled: true, .. })));
        run(&mut sim, &mut env, 60);
        assert!(amount(&env, (5, 0, 0)) > 0, "open valve passes fluid");
        // Close again: flow stops.
        sim.set_valve_speed((3, 0, 0), -256.0);
        run(&mut sim, &mut env, 5);
        let after_close = amount(&env, (5, 0, 0));
        run(&mut sim, &mut env, 30);
        assert_eq!(amount(&env, (5, 0, 0)), after_close);
    }

    #[test]
    fn smart_pipe_filter_passes_only_the_chosen_fluid() {
        for (filter, expect) in [(Some(W), true), (Some(L), false), (None, true)] {
            let blocks = [
                PipeBlock::Smart { axis: Axis::X, filter },
                pump(Dir::East),
                pipe(),
            ];
            let (mut sim, mut env) = line(Tank::filled(10_000, W, 10_000), &blocks, Some(Tank::new(10_000)));
            sim.set_pump_speed(&mut env, (2, 0, 0), 32.0);
            run(&mut sim, &mut env, 80);
            assert_eq!(amount(&env, (4, 0, 0)) > 0, expect, "filter {filter:?}");
        }
    }

    #[test]
    fn fluid_mismatch_in_target_tank_blocks_transfer() {
        let blocks = [pump(Dir::East), pipe()];
        let (mut sim, mut env) = line(Tank::filled(1000, W, 1000), &blocks, Some(Tank::filled(1000, L, 10)));
        sim.set_pump_speed(&mut env, (1, 0, 0), 32.0);
        run(&mut sim, &mut env, 60);
        assert_eq!(amount(&env, (3, 0, 0)), 10);
        assert_eq!(amount(&env, (0, 0, 0)), 1000);
    }

    #[test]
    fn full_target_stops_but_keeps_source_intact() {
        let blocks = [pump(Dir::East), pipe()];
        let (mut sim, mut env) = line(Tank::filled(1000, W, 1000), &blocks, Some(Tank::new(100)));
        sim.set_pump_speed(&mut env, (1, 0, 0), 32.0);
        run(&mut sim, &mut env, 100);
        assert_eq!(amount(&env, (3, 0, 0)), 100);
        assert_eq!(amount(&env, (0, 0, 0)), 900);
    }

    #[test]
    fn open_end_spills_a_source_block_after_a_bucket() {
        let blocks = [pump(Dir::East), pipe()];
        let (mut sim, mut env) = line(Tank::filled(5000, W, 5000), &blocks, None);
        sim.set_pump_speed(&mut env, (1, 0, 0), 32.0);
        let spill = (3, 0, 0);
        // 16 mB per tick: not placed before 1000 mB have arrived.
        let mut first = None;
        for t in 1..=200 {
            sim.tick(&mut env);
            if env.cell(spill) == Cell::Source(W) && first.is_none() {
                first = Some(t);
            }
        }
        assert!(first.is_some(), "source block placed");
        assert!(first.unwrap() > 1000 / 16, "needs a full bucket: {first:?}");
        assert!(5000 - amount(&env, (0, 0, 0)) >= 1000);
        // The tank is cleared by the placement and refills afterwards.
        let oe = sim.open_end((2, 0, 0), Dir::East).expect("open end source");
        assert!(oe.tank.is_none_or(|(_, a)| a < 1000));
    }

    #[test]
    fn open_end_blocked_by_other_fluid_or_existing_source() {
        for cell in [Cell::Source(W), Cell::Flowing(L), Cell::Source(L)] {
            let blocks = [pump(Dir::East), pipe()];
            let (mut sim, mut env) = line(Tank::filled(5000, W, 5000), &blocks, None);
            env.cells.insert((3, 0, 0), cell);
            sim.set_pump_speed(&mut env, (1, 0, 0), 32.0);
            run(&mut sim, &mut env, 150);
            assert_eq!(env.cell((3, 0, 0)), cell);
            assert_eq!(amount(&env, (0, 0, 0)), 5000, "nothing leaves the tank for {cell:?}");
        }
    }

    #[test]
    fn open_end_pulls_source_blocks_into_a_tank() {
        // Open end on the pump's back side, tank in front.
        let mut env = solid_env();
        env.cells.insert((0, 0, 0), Cell::Source(W));
        env.tanks.insert((4, 0, 0), Tank::new(10_000));
        let mut sim = PipeSim::new();
        sim.place_pipe(&mut env, (1, 0, 0), Dir::East);
        sim.place_block(&mut env, (2, 0, 0), pump(Dir::East));
        sim.place_pipe(&mut env, (3, 0, 0), Dir::East);
        sim.set_pump_speed(&mut env, (2, 0, 0), 32.0);
        run(&mut sim, &mut env, 100);
        assert_eq!(env.cell((0, 0, 0)), Cell::Air, "source consumed");
        assert!(amount(&env, (4, 0, 0)) >= 1000);
    }

    #[test]
    fn infinite_water_keeps_feeding() {
        let mut env = solid_env();
        env.cells.insert((0, 0, 0), Cell::Infinite(W));
        env.tanks.insert((3, 0, 0), Tank::new(5000));
        let mut sim = PipeSim::new();
        sim.place_pipe(&mut env, (1, 0, 0), Dir::East);
        sim.place_block(&mut env, (2, 0, 0), pump(Dir::East));
        sim.set_pump_speed(&mut env, (2, 0, 0), 64.0);
        run(&mut sim, &mut env, 400);
        assert_eq!(amount(&env, (3, 0, 0)), 5000);
        assert_eq!(env.cell((0, 0, 0)), Cell::Infinite(W));
    }

    #[test]
    fn removing_a_pipe_stops_the_transfer_and_rewipes() {
        let blocks = [pipe(), pump(Dir::East), pipe(), pipe()];
        let (mut sim, mut env) = line(Tank::filled(100_000, W, 100_000), &blocks, Some(Tank::new(100_000)));
        sim.set_pump_speed(&mut env, (2, 0, 0), 32.0);
        run(&mut sim, &mut env, 60);
        assert!(amount(&env, (5, 0, 0)) > 0);
        sim.remove_block(&mut env, (4, 0, 0));
        run(&mut sim, &mut env, 5);
        let frozen = amount(&env, (5, 0, 0));
        run(&mut sim, &mut env, 40);
        // Pipe (3,0,0) now ends in air: it spills instead and the tank gets nothing more.
        assert_eq!(amount(&env, (5, 0, 0)), frozen);
        assert!(sim.pressure((3, 0, 0), Dir::East).is_some());
    }

    #[test]
    fn placing_a_pipe_after_the_fact_reconnects_the_network() {
        let blocks = [pipe(), pump(Dir::East), pipe()];
        let (mut sim, mut env) = line(Tank::filled(100_000, W, 100_000), &blocks, Some(Tank::new(100_000)));
        // gap: remove pipe 3 before starting
        sim.remove_block(&mut env, (3, 0, 0));
        sim.set_pump_speed(&mut env, (2, 0, 0), 32.0);
        run(&mut sim, &mut env, 40);
        assert_eq!(amount(&env, (4, 0, 0)), 0);
        sim.place_pipe(&mut env, (3, 0, 0), Dir::East);
        run(&mut sim, &mut env, 60);
        assert!(amount(&env, (4, 0, 0)) > 0);
    }

    #[test]
    fn unloaded_target_pauses_the_network() {
        let blocks = [pump(Dir::East), pipe(), pipe()];
        let (mut sim, mut env) = line(Tank::filled(100_000, W, 100_000), &blocks, Some(Tank::new(100_000)));
        env.unloaded.insert((4, 0, 0));
        sim.set_pump_speed(&mut env, (1, 0, 0), 32.0);
        run(&mut sim, &mut env, 80);
        assert_eq!(amount(&env, (4, 0, 0)), 0);
        env.unloaded.clear();
        // Pressure was never distributed across the unloaded edge; the adapter re-propagates on chunk load.
        sim.propagate_changed_pipe(&mut env, (2, 0, 0));
        run(&mut sim, &mut env, 80);
        assert!(amount(&env, (4, 0, 0)) > 0);
    }

    #[test]
    fn pump_without_speed_does_nothing() {
        let blocks = [pump(Dir::East), pipe()];
        let (mut sim, mut env) = line(Tank::filled(1000, W, 1000), &blocks, Some(Tank::new(1000)));
        run(&mut sim, &mut env, 50);
        assert_eq!(amount(&env, (3, 0, 0)), 0);
        sim.set_pump_speed(&mut env, (1, 0, 0), 0.0);
        run(&mut sim, &mut env, 5);
        assert_eq!(amount(&env, (3, 0, 0)), 0);
    }

    #[test]
    fn colliding_fluids_report_a_reaction_hook() {
        // Pump pulls from a T whose two tanks hold different fluids.
        let mut env = solid_env();
        env.tanks.insert((0, 0, 0), Tank::filled(1000, W, 1000));
        env.tanks.insert((1, 1, 0), Tank::filled(1000, L, 1000));
        env.tanks.insert((4, 0, 0), Tank::new(1000));
        let mut sim = PipeSim::new();
        sim.place_pipe(&mut env, (1, 0, 0), Dir::East);
        sim.place_block(&mut env, (2, 0, 0), pump(Dir::East));
        sim.place_pipe(&mut env, (3, 0, 0), Dir::East);
        sim.set_pump_speed(&mut env, (2, 0, 0), 32.0);
        run(&mut sim, &mut env, 60);
        assert!(!env.collisions.is_empty());
        assert!(env.collisions.iter().all(|(p, a, b)| *p == (1, 0, 0) && a != b));
        assert_eq!(amount(&env, (4, 0, 0)), 0);
    }

    #[test]
    fn opposing_pumps_push_against_each_other_without_transfer() {
        // Two fronts facing each other do not count as an endpoint: no pressure at all.
        let mut env = solid_env();
        let mut sim = PipeSim::new();
        sim.place_block(&mut env, (0, 0, 0), pump(Dir::East));
        sim.place_pipe(&mut env, (1, 0, 0), Dir::East);
        sim.place_block(&mut env, (2, 0, 0), pump(Dir::West));
        sim.set_pump_speed(&mut env, (0, 0, 0), 32.0);
        sim.set_pump_speed(&mut env, (2, 0, 0), 32.0);
        run(&mut sim, &mut env, 10);
        assert_eq!(sim.pressure((1, 0, 0), Dir::East), Some(Pressure::default()));
        assert_eq!(sim.pressure((1, 0, 0), Dir::West), Some(Pressure::default()));
        assert!(sim.flow((1, 0, 0), Dir::East).is_none());
    }

    #[test]
    fn external_valves_are_left_alone_by_the_simulation() {
        let blocks = [pipe(), PipeBlock::Valve { axis: Axis::X, enabled: true }, pump(Dir::East), pipe()];
        let (mut sim, mut env) = line(Tank::filled(100_000, W, 100_000), &blocks, Some(Tank::new(100_000)));
        sim.external_valves = true;
        run(&mut sim, &mut env, 20);
        assert!(matches!(sim.block((2, 0, 0)), Some(PipeBlock::Valve { enabled: true, .. })));
    }

    #[test]
    fn loaded_blocks_keep_saved_state_without_a_propagate_tick() {
        let mut env = solid_env();
        env.tanks.insert((0, 0, 0), Tank::filled(1000, W, 1000));
        let mut sim = PipeSim::new();
        sim.load_block((1, 0, 0), PipeBlock::pipe(&[Dir::West, Dir::East]));
        sim.restore_connection(
            (1, 0, 0),
            Dir::East,
            Pressure { inbound: 0.0, outward: 32.0 },
            Some(Flow { inbound: false, fluid: W, progress: 0.5, complete: false }),
        );
        sim.tick(&mut env);
        assert_eq!(sim.pressure((1, 0, 0), Dir::East), Some(Pressure { inbound: 0.0, outward: 32.0 }));
        sim.forget_block((1, 0, 0));
        assert!(sim.block((1, 0, 0)).is_none());
        sim.load_block((2, 0, 0), pump(Dir::East));
        sim.restore_pump_speed((2, 0, 0), 32.0);
        assert_eq!(sim.entity((2, 0, 0)).map(|e| e.speed), Some(32.0));
    }
}
