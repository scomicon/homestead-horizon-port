// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

// Steam Engine and Powered Shaft (SteamEngineBlock / SteamEngineBlockEntity /
// PoweredShaftBlock / PoweredShaftBlockEntity). Rules: `create_engine::steam_engine`
// and `create_engine::boiler`; the boiler itself lives on the tank controller
// (`create_port_basin::tank`, `boiler_step`).
//
// Engine: block `create:steam_engine`, states `face` (floor|wall|ceiling),
// `facing`, `waterlogged`. It points away from its tank; the shaft sits two
// blocks ahead. It ticks every tick (cheap: one tank read, one shaft read) and
// pushes direction, efficiency and capacity into the Powered Shaft.
//
// Powered Shaft: block `create:powered_shaft` (axis, waterlogged), not an item.
// It is a kinetic source (`Kind::Generator`) whose rpm and per-RPM capacity are
// read by `runtime::node` from the `GeneratedSpeed` / `GeneratedCapacity` NBT
// written here. When its engine is gone it reverts to a plain `create:shaft`.

use super::{Game, GameLevel, create_port_basin::tank, native_block_entity_draw::BlockEntityDraw};
use crate::containers::{BlockEntity, ModBlockEntity};
use create_engine::{
    kinetics::{Axis, Pos},
    placement::Direction,
    steam_engine::{self as eng, EngineInput, Face, PoweredShaft},
    steam_whistle,
};
use horizon_core::{geometry::Direction as D, position::BlockPos};
use horizon_world::{
    chunk::BlockState,
    level::PlacementContext,
    nbt::{Compound, Tag},
    registry::BlockStateId,
};

pub(super) const ENGINE: &str = "create:steam_engine";
pub(super) const POWERED: &str = "create:powered_shaft";
/// Steam Whistle block of the whistle port; counted by the boiler.
pub(super) const WHISTLE: &str = "create:steam_whistle";
const SHAFT: &str = "create:shaft";

// Engine NBT.
const PLACED: &str = "Placed";
const COUNTER_CLOCKWISE: &str = "CounterClockwise";
// Powered shaft NBT. `Engine` is `shaft - engine` (PoweredShaftBlockEntity.sourcePos).
const ENGINE_KEY: &str = "Engine";
const EFFICIENCY: &str = "Efficiency";
const DIRECTION: &str = "Direction";
const WARMUP: &str = "Warmup";
const CAPACITY_BASE: &str = "CapacityBase";
const GENERATED_SPEED: &str = "GeneratedSpeed";
const GENERATED_CAPACITY: &str = "GeneratedCapacity";
const APPLIED_SPEED: &str = "AppliedSpeed";
const APPLIED_CAPACITY: &str = "AppliedCapacity";
const ORPHAN: &str = "OrphanTicks";
/// Ticks a powered shaft without an engine waits before becoming a shaft again
/// (covers the engine claiming it right after the warm-up).
const ORPHAN_GRACE: i32 = 10;
/// Piston stroke of the simple engine animation, in blocks.
const STROKE: f32 = 0.3;

fn simulates(game: &Game<'_>) -> bool {
    game.serving.is_some() || game.options.dedicated.is_some()
}

fn number(nbt: &Compound, key: &str) -> f32 {
    super::create_port::number(nbt, key)
}

fn put(nbt: &mut Compound, key: &str, tag: Tag) -> bool {
    if nbt.get(&key.into()) == Some(&tag) {
        return false;
    }
    nbt.insert(key.into(), tag);
    true
}

fn float(v: f32) -> Tag {
    Tag::Float(v.to_bits())
}

fn named(name: &str) -> Option<Direction> {
    Direction::ALL.into_iter().find(|d| d.name() == name)
}

fn direction(d: D) -> Direction {
    match d {
        D::Down => Direction::Down,
        D::Up => Direction::Up,
        D::North => Direction::North,
        D::South => Direction::South,
        D::West => Direction::West,
        D::East => Direction::East,
    }
}

fn prop<'a>(state: &'a BlockState, key: &str) -> Option<&'a str> {
    state.properties.get(key).map(String::as_str)
}

// ------------------------------------------------------------ engine state

#[derive(Clone, Copy)]
struct Spec {
    face: Face,
    facing: Direction,
    connected: Direction,
}

fn spec_of(state: &BlockState) -> Option<Spec> {
    if state.name != ENGINE {
        return None;
    }
    let face = match prop(state, "face")? {
        "floor" => Face::Floor,
        "wall" => Face::Wall,
        "ceiling" => Face::Ceiling,
        _ => return None,
    };
    let facing = named(prop(state, "facing")?)?;
    Some(Spec { face, facing, connected: eng::connected_direction(face, facing) })
}

fn spec_at(game: &Game<'_>, p: Pos) -> Option<Spec> {
    if !game.world.loaded(p[0], p[2]) {
        return None;
    }
    spec_of(game.pack.assets.registry.state(game.world.block(p[0], p[1], p[2]))?)
}

/// `AttachmentProbe::engine_facing`: the way an engine block points.
pub(super) fn engine_direction(state: &BlockState) -> Option<Direction> {
    spec_of(state).map(|s| s.connected)
}

/// `AttachmentProbe::whistle`: the direction a Steam Whistle is attached in.
pub(super) fn whistle_attachment(state: &BlockState) -> Option<Direction> {
    if state.name != WHISTLE {
        return None;
    }
    let wall = prop(state, "wall")? == "true";
    let facing = named(prop(state, "facing")?)?;
    Some(steam_whistle::attached_direction(wall, facing))
}

// ----------------------------------------------------------- powered shaft

pub(super) fn powered_nbt() -> Compound {
    Compound::from([
        (WARMUP.into(), Tag::Int(eng::WARMUP_TICKS)),
        (GENERATED_SPEED.into(), float(0.0)),
        (GENERATED_CAPACITY.into(), float(0.0)),
        (APPLIED_SPEED.into(), float(0.0)),
        (APPLIED_CAPACITY.into(), float(0.0)),
    ])
}

fn int(nbt: &Compound, key: &str, default: i32) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => *v,
        Some(Tag::Byte(v)) => i32::from(*v),
        _ => default,
    }
}

fn load_shaft(nbt: &Compound) -> PoweredShaft {
    let engine = match nbt.get(&ENGINE_KEY.into()) {
        Some(Tag::IntArray(v)) if v.len() == 3 => Some([v[0], v[1], v[2]]),
        _ => None,
    };
    let base = nbt.get(&CAPACITY_BASE.into()).map(|_| number(nbt, CAPACITY_BASE));
    PoweredShaft::restore(
        engine,
        number(nbt, EFFICIENCY),
        int(nbt, DIRECTION, 1),
        int(nbt, WARMUP, eng::WARMUP_TICKS),
        base,
    )
}

/// Persist the shaft and the derived generator values. True when anything changed.
fn store_shaft(nbt: &mut Compound, s: &PoweredShaft) -> bool {
    let (engine, warmup, base) = s.parts();
    let mut changed = match engine {
        Some(k) => put(nbt, ENGINE_KEY, Tag::IntArray(k.to_vec())),
        None => nbt.remove(&ENGINE_KEY.into()).is_some(),
    };
    changed |= match base {
        Some(c) => put(nbt, CAPACITY_BASE, float(c)),
        None => nbt.remove(&CAPACITY_BASE.into()).is_some(),
    };
    changed |= put(nbt, EFFICIENCY, float(s.efficiency));
    changed |= put(nbt, DIRECTION, Tag::Int(s.movement_direction));
    changed |= put(nbt, WARMUP, Tag::Int(warmup));
    changed |= put(nbt, GENERATED_SPEED, float(s.generated_speed()));
    changed |= put(nbt, GENERATED_CAPACITY, float(s.added_stress_capacity()));
    changed
}

/// Per-RPM capacity and signed RPM for `runtime::node` (`Kind::Generator`).
pub(super) fn generated(nbt: &Compound) -> (f32, f32) {
    (number(nbt, GENERATED_SPEED), number(nbt, GENERATED_CAPACITY))
}

fn engine_feeds(game: &Game<'_>, engine: Pos, shaft: Pos) -> bool {
    spec_at(game, engine).is_some_and(|s| eng::shaft_pos(engine, s.connected) == shaft)
}

fn set_block(game: &mut Game<'_>, p: Pos, name: &str, axis: Axis) -> bool {
    let dim = game.dim.id().to_owned();
    let Some(id) = game.pack.assets.registry.default_state(name) else {
        return false;
    };
    if !game.native_set_block(&dim, p, id, 3) {
        return false;
    }
    game.native_set_block_property(p, "axis", axis.name());
    super::create_port_kinetic_runtime::invalidate();
    true
}

/// Tick of a Powered Shaft (called from the kinetic tick before the node is
/// built). Returns true when the generated rotation changed and the source
/// must be published to its network.
pub(super) fn shaft_tick(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity) -> bool {
    if !simulates(game) {
        return false;
    }
    let mut shaft = load_shaft(&entity.nbt);
    let before = shaft.clone();
    shaft.tick();
    let source = shaft.engine_pos(pos);
    if let Some(src) = source
        && game.world.loaded(src[0], src[2])
        && !engine_feeds(game, src, pos)
    {
        shaft.remove(pos, src); // PoweredShaftBlockEntity.remove
    }
    if shaft != before {
        store_shaft(&mut entity.nbt, &shaft);
    }
    // Without an engine the shaft turns back into a plain shaft.
    if shaft.engine_pos(pos).is_none() && shaft.parts().1 == 0 {
        let orphan = int(&entity.nbt, ORPHAN, 0) + 1;
        if orphan >= ORPHAN_GRACE {
            let axis = match game.native_block_property(pos, "axis").as_deref() {
                Some("x") => Axis::X,
                Some("z") => Axis::Z,
                _ => Axis::Y,
            };
            // Publish the stop first; the replaced block entity is gone after this.
            let stopped = number(&entity.nbt, APPLIED_SPEED) != 0.0;
            set_block(game, pos, SHAFT, axis);
            return stopped;
        }
        entity.nbt.insert(ORPHAN.into(), Tag::Int(orphan));
    } else if entity.nbt.contains_key(&ORPHAN.into()) {
        entity.nbt.remove(&ORPHAN.into());
    }
    let (speed, capacity) = generated(&entity.nbt);
    if speed != number(&entity.nbt, APPLIED_SPEED) || capacity != number(&entity.nbt, APPLIED_CAPACITY) {
        entity.nbt.insert(APPLIED_SPEED.into(), float(speed));
        entity.nbt.insert(APPLIED_CAPACITY.into(), float(capacity));
        game.native_block_entity_sync(pos);
        return true;
    }
    false
}

/// `shaft.update(...)` from the engine. True when the shaft changed.
fn drive(game: &mut Game<'_>, engine: Pos, shaft: Pos, dir: i32, efficiency: f32) -> bool {
    let changed = game
        .native_with_mod_block_entity(shaft, |e| {
            if e.id != POWERED {
                return false;
            }
            let mut s = load_shaft(&e.nbt);
            if !s.can_be_powered_by(shaft, engine) {
                return false;
            }
            let before = s.clone();
            s.update(shaft, engine, dir, efficiency, eng::engine_capacity());
            if s == before {
                return false;
            }
            store_shaft(&mut e.nbt, &s);
            true
        })
        .unwrap_or(false);
    if changed {
        super::create_port_kinetic_runtime::invalidate();
        game.native_block_entity_sync(shaft);
    }
    changed
}

// ------------------------------------------------------------------ engine

/// Axis the engine gives a shaft it places itself (first valid axis; the
/// original uses the placer's nearest-first look directions, which a tick does
/// not know). Unverified choice for vertical engines: perpendicular to FACING.
fn default_axis(spec: Spec) -> Axis {
    if spec.connected.axis() == Axis::Y {
        if spec.facing.axis() == Axis::X { Axis::Z } else { Axis::X }
    } else {
        Axis::Y
    }
}

/// Make sure a Powered Shaft with a valid axis sits at `shaft`; returns its axis.
fn ensure_shaft(game: &mut Game<'_>, shaft: Pos, spec: Spec, first: bool) -> Option<Axis> {
    let (name, axis) = {
        let state = game
            .pack
            .assets
            .registry
            .state(game.world.block(shaft[0], shaft[1], shaft[2]))?;
        let axis = match prop(state, "axis") {
            Some("x") => Some(Axis::X),
            Some("y") => Some(Axis::Y),
            Some("z") => Some(Axis::Z),
            _ => None,
        };
        (state.name.clone(), axis)
    };
    match (name.as_str(), axis) {
        (POWERED, Some(a)) => eng::is_shaft_valid(true, a, spec.connected).then_some(a),
        (SHAFT, Some(a)) if eng::is_shaft_valid(true, a, spec.connected) => {
            set_block(game, shaft, POWERED, a).then_some(a)
        }
        _ if first
            && game
                .res
                .facts()
                .get(game.world.block(shaft[0], shaft[1], shaft[2]))
                .replaceable =>
        {
            let a = default_axis(spec);
            set_block(game, shaft, POWERED, a).then_some(a)
        }
        _ => None,
    }
}

/// Server tick of the engine block entity (`SteamEngineBlockEntity.tick`).
pub(super) fn tick_engine(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity) {
    if !simulates(game) || !game.native_on("create") {
        return;
    }
    let Some(spec) = spec_at(game, pos) else {
        return;
    };
    let shaft = eng::shaft_pos(pos, spec.connected);
    let tank_pos = eng::tank_pos(pos, spec.connected);
    if !game.world.loaded(shaft[0], shaft[2]) || !game.world.loaded(tank_pos[0], tank_pos[2]) {
        return;
    }
    let first = !entity.nbt.contains_key(&PLACED.into());
    if first {
        tank::request_update(game, tank_pos);
        entity.nbt.insert(PLACED.into(), Tag::Byte(1));
    }
    let Some(axis) = ensure_shaft(game, shaft, spec, first) else {
        return;
    };
    let efficiency = tank::steam(game, tank_pos).map_or(0.0, |s| s.efficiency);
    let counter_clockwise = matches!(entity.nbt.get(&COUNTER_CLOCKWISE.into()), Some(Tag::Byte(v)) if *v != 0);
    let out = eng::engine_tick(EngineInput {
        face: spec.face,
        facing: spec.facing,
        target_axis: axis,
        efficiency,
        counter_clockwise,
        // An externally driven shaft running the other way is not modelled.
        shaft_speed: 0.0,
        shaft_has_source: false,
    });
    if out.counter_clockwise != counter_clockwise {
        entity.nbt.insert(COUNTER_CLOCKWISE.into(), Tag::Byte(i8::from(out.counter_clockwise)));
    }
    drive(game, pos, shaft, out.direction, efficiency);
}

// --------------------------------------------------------------- placement

fn horizontal(yaw: f32) -> Direction {
    [Direction::South, Direction::West, Direction::North, Direction::East]
        [((f64::from(yaw) / 90.0 + 0.5).floor() as i64 & 3) as usize]
}

fn is_tank(game: &GameLevel<'_>, x: i32, y: i32, z: i32) -> bool {
    game.registry
        .state(game.world.block(x, y, z))
        .is_some_and(|s| s.name == tank::BLOCK)
}

/// `SteamEngineBlock.getStateForPlacement`: the clicked face is the face of the
/// tank the engine sits on; None when there is no tank behind it.
pub(super) fn placement(
    game: &GameLevel<'_>,
    state: BlockStateId,
    ctx: &PlacementContext,
    waterlogged: bool,
) -> Option<BlockStateId> {
    let back = direction(ctx.face).opposite().vector();
    if !is_tank(game, ctx.pos.x + back[0], ctx.pos.y + back[1], ctx.pos.z + back[2]) {
        return None;
    }
    let (face, facing) = match ctx.face {
        D::Up => ("floor", horizontal(ctx.yaw)),
        D::Down => ("ceiling", horizontal(ctx.yaw)),
        side => ("wall", direction(side)),
    };
    game.native_state_with_property(state, "face", face)
        .and_then(|s| game.native_state_with_property(s, "facing", facing.name()))
        .and_then(|s| {
            game.native_state_with_property(s, "waterlogged", if waterlogged { "true" } else { "false" })
        })
}

/// `can_survive` hook: the tank must stay behind the engine.
pub(super) fn can_survive(
    game: &GameLevel<'_>,
    pos: BlockPos,
    state: BlockStateId,
    survives: &mut Option<bool>,
) {
    let Some(block) = game.registry.state(state).filter(|b| b.name == ENGINE) else {
        return;
    };
    let Some(spec) = spec_of(block) else {
        return;
    };
    let back = spec.connected.opposite().vector();
    *survives = Some(is_tank(game, pos.x + back[0], pos.y + back[1], pos.z + back[2]));
}

pub(super) fn default_nbt() -> Compound {
    Compound::new()
}

// -------------------------------------------------------------------- draw

/// Simple engine animation: one piston slides along the engine's axis with the
/// shaft angle. Create's engine draws a piston, a linkage and a rotating
/// connector; only the piston is approximated here (see docs/steam-boiler-engine.md).
pub(super) fn draw(game: &Game<'_>, pos: Pos, _entity: &ModBlockEntity, draw: &mut BlockEntityDraw) {
    let Some(spec) = spec_at(game, pos) else {
        return;
    };
    let shaft = eng::shaft_pos(pos, spec.connected);
    let (speed, axis) = match game.world.block_entities.get(&(shaft[0], shaft[1], shaft[2])) {
        Some(BlockEntity::Mod(e)) if e.id == POWERED => (
            number(&e.nbt, "Speed"),
            match game.native_block_property(shaft, "axis").as_deref() {
                Some("x") => Axis::X,
                Some("z") => Axis::Z,
                _ => Axis::Y,
            },
        ),
        _ => (0.0, Axis::Y),
    };
    let angle = create_engine::visual::angle(
        draw.game_time as f64 + f64::from(draw.partial),
        speed,
        create_engine::visual::rotation_offset(shaft, axis, false),
    );
    let slide = STROKE * (angle.to_radians().sin() * 0.5 + 0.5);
    let v = spec.connected.vector();
    draw.push();
    draw.translate([v[0] as f32 * slide, v[1] as f32 * slide, v[2] as f32 * slide]);
    // The piston model is authored pointing +Y.
    match spec.connected {
        Direction::Down => draw.rotate_about([1.0, 0.0, 0.0], 180.0, [0.5; 3]),
        Direction::North => draw.rotate_about([1.0, 0.0, 0.0], -90.0, [0.5; 3]),
        Direction::South => draw.rotate_about([1.0, 0.0, 0.0], 90.0, [0.5; 3]),
        Direction::East => draw.rotate_about([0.0, 0.0, 1.0], -90.0, [0.5; 3]),
        Direction::West => draw.rotate_about([0.0, 0.0, 1.0], 90.0, [0.5; 3]),
        Direction::Up => {}
    }
    game.native_draw_model(draw, "create:block/steam_engine/piston", 0xFFFFFF);
    draw.pop();
}
