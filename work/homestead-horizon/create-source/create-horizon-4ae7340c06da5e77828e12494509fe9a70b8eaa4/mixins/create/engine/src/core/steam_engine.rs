// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
//! Pure Steam Engine rules after SteamEngineBlock, SteamEngineBlockEntity and
//! PoweredShaftBlockEntity (Create 6.0.10, revision ac0c444d): attachment and
//! shaft alignment, the speed direction chosen each tick, and the generated
//! speed / stress capacity of the powered shaft two blocks in front of the
//! engine. Boiler heat and efficiency come from `crate::boiler`.
use crate::{
    boiler::STEAM_ENGINE_CAPACITY,
    kinetics::{Axis, Pos},
    placement::Direction,
};

/// Base rotation speed step: the shaft turns at `16 * modifier` RPM.
pub const SPEED_STEP: i32 = 16;
/// `getSpeedRange`: (16, 64) RPM.
pub const SPEED_RANGE: (i32, i32) = (16, 64);
/// Ticks a fresh powered shaft ignores engines (`initialTicks`).
pub const WARMUP_TICKS: i32 = 3;
/// Shaft distance in front of the engine.
pub const SHAFT_DISTANCE: i32 = 2;

fn offset(p: Pos, d: Direction, n: i32) -> Pos {
    let v = d.vector();
    [p[0] + v[0] * n, p[1] + v[1] * n, p[2] + v[2] * n]
}

fn is_positive(d: Direction) -> bool {
    matches!(d, Direction::Up | Direction::South | Direction::East)
}

/// `FaceAttachedHorizontalDirectionalBlock.FACE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    Floor,
    Wall,
    Ceiling,
}

/// `getConnectedDirection` (= `SteamEngineBlock.getFacing`): the way the engine
/// points, away from its tank. `facing` is the horizontal FACING property.
pub fn connected_direction(face: Face, facing: Direction) -> Direction {
    match face {
        Face::Ceiling => Direction::Down,
        Face::Floor => Direction::Up,
        Face::Wall => facing,
    }
}

/// `canSurvive`: the tank sits behind the engine.
pub fn can_survive(face: Face, facing: Direction, is_tank_behind: impl Fn(Direction) -> bool) -> bool {
    is_tank_behind(connected_direction(face, facing).opposite())
}

/// `getShaftPos`.
pub fn shaft_pos(engine: Pos, connected: Direction) -> Pos {
    offset(engine, connected, SHAFT_DISTANCE)
}

/// Tank block the engine draws from (one block behind).
pub fn tank_pos(engine: Pos, connected: Direction) -> Pos {
    offset(engine, connected.opposite(), 1)
}

/// `isShaftValid`: a Shaft or Powered Shaft whose axis differs from the
/// engine's facing axis.
pub fn is_shaft_valid(is_shaft: bool, shaft_axis: Axis, connected: Direction) -> bool {
    is_shaft && shaft_axis != connected.axis()
}

/// Shaft axis chosen when the engine places its own shaft (`getOffset`): the
/// first direction in the player's nearest-first order whose axis is valid,
/// falling through with the last one tried.
pub fn placed_shaft_axis(nearest_first: &[Direction], connected: Direction) -> Option<Axis> {
    let mut axis = None;
    for d in nearest_first {
        axis = Some(d.axis());
        if d.axis() != connected.axis() {
            break;
        }
    }
    axis
}

/// Inputs of `SteamEngineBlockEntity.tick` besides the boiler.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineInput {
    pub face: Face,
    /// Horizontal `FACING` property.
    pub facing: Direction,
    pub target_axis: Axis,
    /// Clamped boiler efficiency in 0..=1.
    pub efficiency: f32,
    /// RotationDirection scroll value: true = counter-clockwise.
    pub counter_clockwise: bool,
    /// `shaft.getTheoreticalSpeed()` and `shaft.hasSource()`.
    pub shaft_speed: f32,
    pub shaft_has_source: bool,
}

/// What an engine tick writes: the direction handed to the shaft and the new
/// scroll value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EngineOutput {
    pub direction: i32,
    pub counter_clockwise: bool,
}

/// `SteamEngineBlockEntity.tick`: the +-1 direction passed to `shaft.update`.
pub fn engine_tick(input: EngineInput) -> EngineOutput {
    let mut facing = connected_direction(input.face, input.facing);
    if facing.axis() == Axis::Y {
        facing = input.facing;
    }
    let vertical_target = input.target_axis == Axis::Y;
    let mut level: i32 = if input.efficiency == 0.0 || vertical_target {
        1
    } else if is_positive(facing) {
        1
    } else {
        -1
    };
    if input.target_axis == Axis::Z {
        level *= -1;
    }
    let mut ccw = input.counter_clockwise;
    if ccw {
        level *= -1;
    }
    if input.shaft_has_source
        && input.shaft_speed != 0.0
        && level != 0
        && (input.shaft_speed > 0.0) != (level > 0)
    {
        ccw = !ccw;
        level *= -1;
    }
    EngineOutput { direction: level, counter_clockwise: ccw }
}

/// `getSpeedModifier`: 1..=4 steps of 16 RPM by efficiency quarter.
pub fn speed_modifier(efficiency: f32) -> i32 {
    let extra = if efficiency >= 1.0 { 3.0 } else { 2.0_f32.min((efficiency * 4.0).floor()) };
    (1.0 + extra) as i32
}

/// `PoweredShaftBlockEntity`.
#[derive(Clone, Debug, PartialEq)]
pub struct PoweredShaft {
    /// Engine position relative to the shaft (`worldPosition - sourcePos`).
    engine: Option<[i32; 3]>,
    pub efficiency: f32,
    pub movement_direction: i32,
    initial_ticks: i32,
    capacity_base: Option<f32>,
}

impl Default for PoweredShaft {
    fn default() -> Self {
        Self {
            engine: None,
            efficiency: 0.0,
            movement_direction: 1,
            initial_ticks: WARMUP_TICKS,
            capacity_base: None,
        }
    }
}

impl PoweredShaft {
    /// Rebuild from saved parts (adapter persistence).
    pub fn restore(
        engine: Option<[i32; 3]>,
        efficiency: f32,
        movement_direction: i32,
        initial_ticks: i32,
        capacity_base: Option<f32>,
    ) -> Self {
        Self { engine, efficiency, movement_direction, initial_ticks, capacity_base }
    }

    /// Saved parts: (engine key, initial ticks left, capacity base).
    pub fn parts(&self) -> (Option<[i32; 3]>, i32, Option<f32>) {
        (self.engine, self.initial_ticks, self.capacity_base)
    }

    /// Engine position for a shaft at `shaft`, if owned.
    pub fn engine_pos(&self, shaft: Pos) -> Option<Pos> {
        self.engine.map(|k| [shaft[0] - k[0], shaft[1] - k[1], shaft[2] - k[2]])
    }

    pub fn tick(&mut self) {
        if self.initial_ticks > 0 {
            self.initial_ticks -= 1;
        }
    }

    fn key(shaft: Pos, source: Pos) -> [i32; 3] {
        [shaft[0] - source[0], shaft[1] - source[1], shaft[2] - source[2]]
    }

    pub fn is_powered_by(&self, shaft: Pos, source: Pos) -> bool {
        self.engine == Some(Self::key(shaft, source))
    }

    /// `canBePoweredBy`: warmed up and either free or already owned.
    pub fn can_be_powered_by(&self, shaft: Pos, source: Pos) -> bool {
        self.initial_ticks == 0 && (self.engine.is_none() || self.is_powered_by(shaft, source))
    }

    /// `update`: returns true when the generated rotation must be recomputed.
    pub fn update(&mut self, shaft: Pos, source: Pos, direction: i32, efficiency: f32, capacity: f32) -> bool {
        self.engine = Some(Self::key(shaft, source));
        let prev = self.efficiency;
        self.efficiency = efficiency;
        let prev_dir = self.movement_direction;
        if (efficiency - prev).abs() < 1e-5 && prev_dir == direction {
            return false;
        }
        self.capacity_base = Some(capacity);
        self.movement_direction = direction;
        true
    }

    /// `remove`: the owning engine goes away or stops.
    pub fn remove(&mut self, shaft: Pos, source: Pos) -> bool {
        if !self.is_powered_by(shaft, source) {
            return false;
        }
        self.engine = None;
        self.efficiency = 0.0;
        self.movement_direction = 0;
        self.capacity_base = None;
        true
    }

    pub fn combined_capacity(&self) -> f32 {
        self.capacity_base.map_or(0.0, |c| self.efficiency * c)
    }

    /// `getGeneratedSpeed`.
    pub fn generated_speed(&self) -> f32 {
        if self.combined_capacity() > 0.0 {
            (self.movement_direction * SPEED_STEP * speed_modifier(self.efficiency)) as f32
        } else {
            0.0
        }
    }

    /// `calculateAddedStressCapacity`: capacity at one RPM.
    pub fn added_stress_capacity(&self) -> f32 {
        self.combined_capacity() / speed_modifier(self.efficiency) as f32
    }

    /// Whether the engine's shutdown tick (`shaft.update(.., 0, 0)`) would act.
    pub fn needs_shutdown(&self) -> bool {
        self.efficiency != 0.0
    }

    /// `getRotationAngleOffset`: odd shaft coordinates along the axis are
    /// phase-shifted so the gear teeth of adjacent shafts interleave.
    pub fn rotation_angle_offset(pos: Pos, axis: Axis) -> f32 {
        if pos[axis.index()].rem_euclid(2) == 0 { 180.0 } else { 0.0 }
    }
}

/// Convenience: default capacity of a steam engine.
pub fn engine_capacity() -> f32 {
    STEAM_ENGINE_CAPACITY
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(face: Face, facing: Direction, axis: Axis, eff: f32) -> EngineInput {
        EngineInput {
            face,
            facing,
            target_axis: axis,
            efficiency: eff,
            counter_clockwise: false,
            shaft_speed: 0.0,
            shaft_has_source: false,
        }
    }

    #[test]
    fn speed_modifier_steps() {
        let cases = [
            (0.0, 1),
            (0.124, 1),
            (0.25, 2),
            (0.49, 2),
            (0.5, 3),
            (0.75, 3),
            (0.99, 3),
            (1.0, 4),
            (1.5, 4),
        ];
        for (eff, m) in cases {
            assert_eq!(speed_modifier(eff), m, "eff {eff}");
        }
    }

    #[test]
    fn shaft_speed_and_capacity() {
        let (shaft, engine) = ([0, 2, 0], [0, 0, 0]);
        let mut s = PoweredShaft::default();
        // Warm-up: three ticks before it can be claimed.
        assert!(!s.can_be_powered_by(shaft, engine));
        for _ in 0..3 {
            s.tick();
        }
        assert!(s.can_be_powered_by(shaft, engine));
        assert!(s.update(shaft, engine, 1, 1.0, STEAM_ENGINE_CAPACITY));
        assert_eq!(s.generated_speed(), 64.0);
        assert_eq!(s.added_stress_capacity(), 256.0);
        // Network stress = capacity * speed = 16384, the same as 16 * 1024.
        assert_eq!(s.added_stress_capacity() * s.generated_speed(), 16.0 * 1024.0);
        assert!(!s.update(shaft, engine, 1, 1.0, STEAM_ENGINE_CAPACITY));
        assert!(s.update(shaft, engine, -1, 0.5, STEAM_ENGINE_CAPACITY));
        assert_eq!(s.generated_speed(), -48.0);
        assert_eq!(s.added_stress_capacity(), 512.0 / 3.0);
        assert!(s.update(shaft, engine, -1, 0.1, STEAM_ENGINE_CAPACITY));
        assert_eq!(s.generated_speed(), -16.0);
    }

    #[test]
    fn stress_is_proportional_to_efficiency_independent_of_speed_step() {
        for (eff, expected_speed) in [(0.2_f32, 16.0), (0.3, 32.0), (0.6, 48.0), (1.0, 64.0)] {
            let mut s = PoweredShaft::default();
            for _ in 0..3 {
                s.tick();
            }
            s.update([0; 3], [0, -2, 0], 1, eff, STEAM_ENGINE_CAPACITY);
            assert_eq!(s.generated_speed(), expected_speed);
            let total = s.added_stress_capacity() * s.generated_speed();
            assert!((total - eff * 16.0 * 1024.0).abs() < 0.01 * 16384.0 * eff.max(0.01), "{eff}: {total}");
        }
    }

    #[test]
    fn only_the_owner_can_remove_and_a_second_engine_is_refused() {
        let shaft = [0, 2, 0];
        let mut s = PoweredShaft::default();
        for _ in 0..3 {
            s.tick();
        }
        s.update(shaft, [0, 0, 0], 1, 1.0, 1024.0);
        assert!(!s.can_be_powered_by(shaft, [0, 4, 0]));
        assert!(!s.remove(shaft, [0, 4, 0]));
        assert_eq!(s.generated_speed(), 64.0);
        assert!(s.remove(shaft, [0, 0, 0]));
        assert_eq!(s.generated_speed(), 0.0);
        assert!(s.can_be_powered_by(shaft, [0, 4, 0]));
    }

    #[test]
    fn restore_round_trips_and_locates_the_engine() {
        let mut s = PoweredShaft::default();
        for _ in 0..3 {
            s.tick();
        }
        s.update([4, 1, 4], [4, -1, 4], -1, 0.5, 1024.0);
        let (engine, warm, cap) = s.parts();
        let r = PoweredShaft::restore(engine, s.efficiency, s.movement_direction, warm, cap);
        assert_eq!(r, s);
        assert_eq!(r.engine_pos([4, 1, 4]), Some([4, -1, 4]));
        assert_eq!(PoweredShaft::default().engine_pos([0; 3]), None);
    }

    #[test]
    fn zero_efficiency_makes_no_rotation() {
        let mut s = PoweredShaft::default();
        s.update([0; 3], [0, -2, 0], 1, 0.0, 1024.0);
        assert_eq!(s.generated_speed(), 0.0);
        assert!(!s.needs_shutdown());
    }

    #[test]
    fn engine_direction_follows_facing_and_target_axis() {
        // Facing east onto an X-axis... shaft axis must differ, so Z/Y targets.
        let o = engine_tick(input(Face::Wall, Direction::East, Axis::Z, 1.0));
        assert_eq!(o.direction, -1); // positive facing, Z target negates
        let o = engine_tick(input(Face::Wall, Direction::West, Axis::Z, 1.0));
        assert_eq!(o.direction, 1);
        let o = engine_tick(input(Face::Wall, Direction::East, Axis::Y, 1.0));
        assert_eq!(o.direction, 1); // vertical shafts ignore the facing sign
        let o = engine_tick(input(Face::Wall, Direction::West, Axis::Y, 1.0));
        assert_eq!(o.direction, 1);
        let o = engine_tick(input(Face::Wall, Direction::South, Axis::X, 1.0));
        assert_eq!(o.direction, 1);
        let o = engine_tick(input(Face::Wall, Direction::North, Axis::X, 1.0));
        assert_eq!(o.direction, -1);
    }

    #[test]
    fn vertical_engines_use_the_horizontal_facing() {
        // Floor engine points up, the sign comes from FACING.
        let o = engine_tick(input(Face::Floor, Direction::North, Axis::X, 1.0));
        assert_eq!(o.direction, -1);
        let o = engine_tick(input(Face::Ceiling, Direction::South, Axis::X, 1.0));
        assert_eq!(o.direction, 1);
        // Efficiency 0 -> level 1 before the Z flip.
        let o = engine_tick(input(Face::Floor, Direction::North, Axis::X, 0.0));
        assert_eq!(o.direction, 1);
        let o = engine_tick(input(Face::Floor, Direction::North, Axis::Z, 0.0));
        assert_eq!(o.direction, -1);
    }

    #[test]
    fn scroll_value_reverses_and_an_existing_source_flips_it_back() {
        let mut i = input(Face::Wall, Direction::South, Axis::X, 1.0);
        i.counter_clockwise = true;
        let o = engine_tick(i);
        assert_eq!(o, EngineOutput { direction: -1, counter_clockwise: true });
        // A running shaft (+20 RPM, externally driven) wins: scroll flips.
        i.shaft_has_source = true;
        i.shaft_speed = 20.0;
        let o = engine_tick(i);
        assert_eq!(o, EngineOutput { direction: 1, counter_clockwise: false });
        // Same direction: untouched.
        i.shaft_speed = -20.0;
        let o = engine_tick(i);
        assert_eq!(o, EngineOutput { direction: -1, counter_clockwise: true });
        // Engines that are the only source never flip.
        i.shaft_has_source = false;
        i.shaft_speed = 20.0;
        assert_eq!(engine_tick(i).direction, -1);
    }

    #[test]
    fn placement_geometry() {
        assert_eq!(connected_direction(Face::Floor, Direction::North), Direction::Up);
        assert_eq!(connected_direction(Face::Ceiling, Direction::North), Direction::Down);
        assert_eq!(connected_direction(Face::Wall, Direction::West), Direction::West);
        let engine = [10, 64, 10];
        assert_eq!(shaft_pos(engine, Direction::East), [12, 64, 10]);
        assert_eq!(tank_pos(engine, Direction::East), [9, 64, 10]);
        assert_eq!(shaft_pos(engine, Direction::Up), [10, 66, 10]);
        // Survival: the tank must be behind.
        assert!(can_survive(Face::Floor, Direction::North, |d| d == Direction::Down));
        assert!(!can_survive(Face::Floor, Direction::North, |d| d == Direction::Up));
        assert!(can_survive(Face::Wall, Direction::East, |d| d == Direction::West));
    }

    #[test]
    fn shaft_alignment_rejects_the_facing_axis() {
        assert!(is_shaft_valid(true, Axis::Z, Direction::East));
        assert!(is_shaft_valid(true, Axis::Y, Direction::East));
        assert!(!is_shaft_valid(true, Axis::X, Direction::East));
        assert!(!is_shaft_valid(false, Axis::Z, Direction::East));
    }

    #[test]
    fn placed_shaft_takes_the_first_valid_nearest_axis() {
        use Direction::*;
        assert_eq!(placed_shaft_axis(&[East, Up, North], East), Some(Axis::Y));
        assert_eq!(placed_shaft_axis(&[North, East], East), Some(Axis::Z));
        assert_eq!(placed_shaft_axis(&[East, West], East), Some(Axis::X));
        assert_eq!(placed_shaft_axis(&[], East), None);
    }

    #[test]
    fn angle_offset_alternates() {
        assert_eq!(PoweredShaft::rotation_angle_offset([4, 0, 0], Axis::X), 180.0);
        assert_eq!(PoweredShaft::rotation_angle_offset([5, 0, 0], Axis::X), 0.0);
        assert_eq!(PoweredShaft::rotation_angle_offset([-3, 0, 0], Axis::X), 0.0);
        assert_eq!(PoweredShaft::rotation_angle_offset([0, 7, 0], Axis::Y), 0.0);
    }

    #[test]
    fn constants() {
        assert_eq!(SPEED_RANGE, (16, 64));
        assert_eq!(engine_capacity(), 1024.0);
    }
}
