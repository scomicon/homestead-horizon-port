// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
//! Pure boiler model after FluidTankBlockEntity.BoilerData and BoilerHeater /
//! BoilerHeaters (Create 6.0.10, revision ac0c444d). A boiler is an ordinary
//! tank multiblock that has at least one Steam Engine or Steam Whistle
//! attached; heat comes from the blocks directly under the controller's
//! footprint, water from fluid inserted into the tank, and size from the block
//! count. The caller supplies heater values, attachment probes and tick calls.
//!
//! Heater values are integers here. The original returns floats but every
//! built-in heater yields -1, 0, 1 or 2.
use std::collections::BTreeSet;

use crate::{burner::Heat, kinetics::Pos, placement::Direction};

/// `BoilerHeater.NO_HEAT`: the block gives no heat.
pub const NO_HEAT: i32 = -1;
/// `BoilerHeater.PASSIVE_HEAT`: small, limited heat (fire, magma, ...).
pub const PASSIVE_HEAT: i32 = 0;
/// `BoilerData`: highest boiler level.
pub const MAX_LEVEL: i32 = 18;
/// Tank blocks needed per boiler level (`boilerSize / 4`).
pub const BLOCKS_PER_LEVEL: i32 = 4;
/// `waterSupplyPerLevel`: millibuckets of water per tick for one level.
pub const WATER_SUPPLY_PER_LEVEL: i32 = 10;
/// Ticks per water sample.
pub const SAMPLE_RATE: i32 = 5;
/// Samples kept in the ring (a full pass is `SAMPLE_RATE * RING` ticks).
pub const RING: usize = 10;
/// `passiveEngineEfficiency`.
pub const PASSIVE_ENGINE_EFFICIENCY: f32 = 1.0 / 8.0;
/// Water supply of a creative tank (`waterSupplyPerLevel * 20`).
pub const CREATIVE_SUPPLY: f32 = (WATER_SUPPLY_PER_LEVEL * 20) as f32;
/// Distinct whistle pitches for the Pipe Organ advancement.
pub const PIPE_ORGAN_PITCHES: usize = 12;
/// Base stress capacity of the Steam Engine (`CStress.setCapacity(1024)`).
pub const STEAM_ENGINE_CAPACITY: f32 = 1024.0;
/// Engine speed range registered for goggles (`setGeneratorSpeed(64, true)`).
pub const GENERATOR_SPEED_MAX: i32 = 64;

/// `BoilerHeaters.blazeBurner`: heat given by a Blaze Burner of this level.
pub fn blaze_burner_heat(level: Heat) -> i32 {
    match level {
        Heat::None => NO_HEAT,
        Heat::Seething => 2,
        Heat::Fading | Heat::Kindled => 1,
        Heat::Smouldering => PASSIVE_HEAT,
    }
}

/// `BoilerHeaters.passive`: a block of the passive-heater tag that is not
/// "unheated" (an unlit campfire, for instance).
pub fn passive_heat(in_passive_tag: bool, unheated: bool) -> i32 {
    if in_passive_tag && !unheated { PASSIVE_HEAT } else { NO_HEAT }
}

/// What the adapter knows about the blocks around the boiler.
pub trait AttachmentProbe {
    /// Facing of a Steam Engine at `pos`, if there is one.
    fn engine_facing(&self, pos: Pos) -> Option<Direction>;
    /// For a Steam Whistle at `pos`: the direction it is attached in
    /// (`getAttachedDirection`) and its pitch id (`getPitchId`).
    fn whistle(&self, pos: Pos) -> Option<(Direction, i32)>;
}

/// Result of a server tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TickOutcome {
    /// Sync the boiler to clients (`notifyUpdate`).
    pub notify: bool,
    /// The max-level advancement should be awarded.
    pub maxed: bool,
}

/// `BoilerData`.
#[derive(Clone, Debug, PartialEq)]
pub struct Boiler {
    pub active_heat: i32,
    pub passive: bool,
    pub attached_engines: i32,
    pub attached_whistles: i32,
    pub water_supply: f32,
    pub needs_heat_update: bool,
    supply_over_time: [f32; RING],
    ticks_until_next_sample: i32,
    current_index: usize,
    gathered_supply: i32,
}

impl Default for Boiler {
    fn default() -> Self {
        Self {
            active_heat: 0,
            passive: false,
            attached_engines: 0,
            attached_whistles: 0,
            water_supply: 0.0,
            needs_heat_update: true,
            supply_over_time: [0.0; RING],
            ticks_until_next_sample: 0,
            current_index: 0,
            gathered_supply: 0,
        }
    }
}

/// The private sampling state, for adapter persistence.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sampling {
    pub ring: [f32; RING],
    pub ticks_until_next_sample: i32,
    pub index: usize,
    pub gathered: i32,
}

impl Boiler {
    pub fn sampling(&self) -> Sampling {
        Sampling {
            ring: self.supply_over_time,
            ticks_until_next_sample: self.ticks_until_next_sample,
            index: self.current_index,
            gathered: self.gathered_supply,
        }
    }

    pub fn set_sampling(&mut self, s: Sampling) {
        self.supply_over_time = s.ring;
        self.ticks_until_next_sample = s.ticks_until_next_sample;
        self.current_index = s.index % RING;
        self.gathered_supply = s.gathered;
    }

    /// `isActive`: only boilers with attachments tick and expose a boiler handler.
    pub fn is_active(&self) -> bool {
        self.attached_engines > 0 || self.attached_whistles > 0
    }

    /// `evaluate`: recount attachments over all tank blocks of the multiblock.
    /// Returns whether a count changed. Always requests a heat refresh.
    pub fn evaluate(&mut self, tank_blocks: &[Pos], probe: &impl AttachmentProbe) -> bool {
        let (prev_e, prev_w) = (self.attached_engines, self.attached_whistles);
        let (mut engines, mut whistles) = (0, 0);
        for &pos in tank_blocks {
            for d in Direction::ALL {
                let v = d.vector();
                let n = [pos[0] + v[0], pos[1] + v[1], pos[2] + v[2]];
                if probe.engine_facing(n) == Some(d) {
                    engines += 1;
                }
                if probe.whistle(n).is_some_and(|(att, _)| att.opposite() == d) {
                    whistles += 1;
                }
            }
        }
        self.attached_engines = engines;
        self.attached_whistles = whistles;
        self.needs_heat_update = true;
        prev_e != engines || prev_w != whistles
    }

    /// `updateTemperature`: `heaters` holds the value of every block in the
    /// `width x width` footprint one layer below the controller. Returns
    /// whether the heat state changed.
    pub fn update_temperature(&mut self, heaters: &[i32]) -> bool {
        let (prev_active, prev_passive) = (self.active_heat, self.passive);
        self.needs_heat_update = false;
        self.active_heat = 0;
        self.passive = false;
        for &h in heaters {
            if h == NO_HEAT || h < 0 {
                continue;
            }
            if h == PASSIVE_HEAT {
                self.passive = true;
            } else {
                self.active_heat += h;
            }
        }
        // Passive heat counts only when nothing gives active heat.
        self.passive &= self.active_heat == 0;
        prev_active != self.active_heat || prev_passive != self.passive
    }

    /// `IFluidHandler.fill` of the boiler handler: only water, always accepted.
    pub fn fill_water(&mut self, amount: i32, simulate: bool) -> i32 {
        if !simulate {
            self.gathered_supply += amount;
        }
        amount
    }

    // ----- derived values; `size` is `getTotalTankSize()` -----

    pub fn max_heat_for_size(size: i32) -> i32 {
        MAX_LEVEL.min(size / BLOCKS_PER_LEVEL)
    }

    pub fn max_heat_for_water(&self) -> i32 {
        MAX_LEVEL.min(self.water_supply.ceil() as i32 / WATER_SUPPLY_PER_LEVEL)
    }

    /// Heat shown for the "heat" meter: a passive source counts as one.
    pub fn display_heat(&self) -> i32 {
        if self.passive { 1 } else { self.active_heat }
    }

    /// `getActualHeat`: the boiler level.
    pub fn level(&self, size: i32) -> i32 {
        self.active_heat.min(self.max_heat_for_water()).min(Self::max_heat_for_size(size))
    }

    /// `isPassive(size)`.
    pub fn is_passive(&self, size: i32) -> bool {
        self.passive && Self::max_heat_for_size(size) > 0 && self.max_heat_for_water() > 0
    }

    /// `getEngineEfficiency`: 0..=1 share of full output each engine gets.
    pub fn engine_efficiency(&self, size: i32) -> f32 {
        if self.is_passive(size) {
            return if self.attached_engines > 0 {
                PASSIVE_ENGINE_EFFICIENCY / self.attached_engines as f32
            } else {
                0.0
            };
        }
        if self.active_heat == 0 {
            return 0.0;
        }
        let level = self.level(size);
        if self.attached_engines <= level {
            return 1.0;
        }
        level as f32 / self.attached_engines as f32
    }

    /// Whistles sound while powered, the boiler is active and hot.
    pub fn whistles_sound(&self, powered: bool) -> bool {
        powered && self.is_active() && (self.passive || self.active_heat > 0)
    }

    /// Goggle total stress capacity: `efficiency * 16 * max(level, engines)
    /// * capacity`, in stress units.
    pub fn total_stress_capacity(&self, size: i32) -> f32 {
        let level = self.level(size);
        self.engine_efficiency(size) * 16.0 * level.max(self.attached_engines) as f32 * STEAM_ENGINE_CAPACITY
    }

    /// Goggle state of the heat line.
    pub fn status(&self, size: i32) -> BoilerStatus {
        if self.is_passive(size) {
            return BoilerStatus::Passive;
        }
        match self.level(size) {
            0 => BoilerStatus::Idle,
            MAX_LEVEL => BoilerStatus::Max,
            n => BoilerStatus::Level(n),
        }
    }

    /// Goggles show the water input rate instead of stress when engines are
    /// attached, the size allows a level and heat exists, but no water arrives.
    pub fn water_limited(&self, size: i32) -> bool {
        self.attached_engines > 0
            && Self::max_heat_for_size(size) > 0
            && self.max_heat_for_water() == 0
            && self.display_heat() > 0
    }

    /// Fill ratio of the pressure gauge (`gauge.chase` target).
    pub fn gauge_target(&self, size: i32) -> f32 {
        if self.is_passive(size) {
            PASSIVE_ENGINE_EFFICIENCY
        } else if size == 0 {
            0.0
        } else {
            self.level(size) as f32 / MAX_LEVEL as f32
        }
    }

    /// One server tick of `BoilerData.tick`. `heaters` is called only when a
    /// heat refresh is pending.
    pub fn tick_server(
        &mut self,
        size: i32,
        tank_capacity: i64,
        creative: bool,
        heaters: &mut dyn FnMut() -> Vec<i32>,
    ) -> TickOutcome {
        let mut out = TickOutcome::default();
        if !self.is_active() {
            return out;
        }
        if self.needs_heat_update && self.update_temperature(&heaters()) {
            out.notify = true;
        }
        self.ticks_until_next_sample -= 1;
        if self.ticks_until_next_sample > 0 {
            return out;
        }
        if tank_capacity == 0 {
            return out;
        }
        self.ticks_until_next_sample = SAMPLE_RATE;
        let sample = self.gathered_supply as f32 / SAMPLE_RATE as f32;
        self.supply_over_time[self.current_index] = sample;
        self.water_supply = self.water_supply.max(sample);
        self.current_index = (self.current_index + 1) % RING;
        self.gathered_supply = 0;
        if self.current_index == 0 {
            self.water_supply = self.supply_over_time.iter().copied().fold(0.0, f32::max);
        }
        if creative {
            self.water_supply = CREATIVE_SUPPLY;
        }
        out.maxed = self.level(size) == MAX_LEVEL;
        out.notify = true;
        out
    }

    /// Volume of the client boiler loop (`3 / max(2, engines / 6)`).
    pub fn sound_volume(&self) -> f32 {
        3.0 / 2.max(self.attached_engines / 6) as f32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoilerStatus {
    Passive,
    Idle,
    Level(i32),
    Max,
}

/// `checkPipeOrganAdvancement`.
pub fn pipe_organ(pitch_ids: impl IntoIterator<Item = i32>) -> bool {
    pitch_ids.into_iter().collect::<BTreeSet<_>>().len() >= PIPE_ORGAN_PITCHES
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boiler(heat: i32, water: f32, engines: i32) -> Boiler {
        Boiler { active_heat: heat, water_supply: water, attached_engines: engines, ..Boiler::default() }
    }

    #[test]
    fn sampling_state_round_trips() {
        let mut a = boiler(1, 0.0, 1);
        a.fill_water(500, false);
        let mut heaters = || vec![1];
        a.tick_server(8, 64000, false, &mut heaters);
        let mut b = boiler(1, a.water_supply, 1);
        b.set_sampling(a.sampling());
        assert_eq!(b.sampling(), a.sampling());
    }

    #[test]
    fn heater_values() {
        assert_eq!(blaze_burner_heat(Heat::None), -1);
        assert_eq!(blaze_burner_heat(Heat::Smouldering), 0);
        assert_eq!(blaze_burner_heat(Heat::Fading), 1);
        assert_eq!(blaze_burner_heat(Heat::Kindled), 1);
        assert_eq!(blaze_burner_heat(Heat::Seething), 2);
        assert_eq!(passive_heat(true, false), PASSIVE_HEAT);
        assert_eq!(passive_heat(true, true), NO_HEAT);
        assert_eq!(passive_heat(false, false), NO_HEAT);
    }

    #[test]
    fn temperature_sums_active_heaters_and_discards_passive_when_active() {
        let mut b = Boiler::default();
        assert!(b.update_temperature(&[2, 2, 1, -1, 0, -1, -1, -1, -1]));
        assert_eq!((b.active_heat, b.passive), (5, false));
        assert!(!b.needs_heat_update);
        assert!(!b.update_temperature(&[2, 2, 1, -1, 0, -1, -1, -1, -1]));
        assert!(b.update_temperature(&[0, -1, -1, -1]));
        assert_eq!((b.active_heat, b.passive), (0, true));
        assert!(b.update_temperature(&[-1; 4]));
        assert_eq!((b.active_heat, b.passive), (0, false));
    }

    #[test]
    fn size_and_water_caps_the_level() {
        // 4 blocks per level, 10 mB/t of water per level, 18 at most.
        assert_eq!(Boiler::max_heat_for_size(3), 0);
        assert_eq!(Boiler::max_heat_for_size(4), 1);
        assert_eq!(Boiler::max_heat_for_size(72), 18);
        assert_eq!(Boiler::max_heat_for_size(288), 18);
        let b = boiler(18, 25.0, 1);
        assert_eq!(b.max_heat_for_water(), 2);
        assert_eq!(b.level(72), 2);
        let b = boiler(18, 24.1, 1);
        assert_eq!(b.max_heat_for_water(), 2);
        let b = boiler(18, 1000.0, 1);
        assert_eq!(b.level(72), 18);
        assert_eq!(b.level(40), 10);
        assert_eq!(boiler(3, 1000.0, 1).level(72), 3);
    }

    #[test]
    fn engine_efficiency_shares_the_level() {
        assert_eq!(boiler(4, 100.0, 2).engine_efficiency(64), 1.0);
        assert_eq!(boiler(4, 100.0, 4).engine_efficiency(64), 1.0);
        assert_eq!(boiler(4, 100.0, 8).engine_efficiency(64), 0.5);
        assert_eq!(boiler(0, 100.0, 1).engine_efficiency(64), 0.0);
        // No water -> level 0 -> efficiency 0 when engines exist.
        assert_eq!(boiler(4, 0.0, 1).engine_efficiency(64), 0.0);
    }

    #[test]
    fn passive_heat_gives_an_eighth_split_between_engines() {
        let b = Boiler { passive: true, water_supply: 10.0, attached_engines: 2, ..Boiler::default() };
        assert!(b.is_passive(4));
        assert_eq!(b.engine_efficiency(4), 1.0 / 16.0);
        assert_eq!(b.status(4), BoilerStatus::Passive);
        // Too small or no water: not passive, no efficiency.
        assert!(!b.is_passive(3));
        assert_eq!(b.engine_efficiency(3), 0.0);
        let dry = Boiler { passive: true, attached_engines: 1, ..Boiler::default() };
        assert!(!dry.is_passive(100));
    }

    #[test]
    fn stress_capacity_formula() {
        // 3 engines, full efficiency, level 3: 16 * 3 * 1024.
        let b = boiler(3, 100.0, 3);
        assert_eq!(b.total_stress_capacity(64), 16.0 * 3.0 * 1024.0);
        // Level 18 with one engine: max(level, engines) = 18.
        let b = boiler(18, 500.0, 1);
        assert_eq!(b.total_stress_capacity(72), 1.0 * 16.0 * 18.0 * 1024.0);
        // Overloaded: 6 engines at level 3 share half.
        let b = boiler(3, 100.0, 6);
        assert_eq!(b.total_stress_capacity(64), 0.5 * 16.0 * 6.0 * 1024.0);
    }

    #[test]
    fn status_text_levels() {
        assert_eq!(boiler(0, 100.0, 1).status(64), BoilerStatus::Idle);
        assert_eq!(boiler(7, 100.0, 1).status(64), BoilerStatus::Level(7));
        assert_eq!(boiler(18, 500.0, 1).status(72), BoilerStatus::Max);
    }

    #[test]
    fn water_limited_goggles() {
        let b = boiler(4, 0.0, 1);
        assert!(b.water_limited(64));
        assert!(!boiler(4, 20.0, 1).water_limited(64));
        assert!(!boiler(4, 0.0, 0).water_limited(64));
        assert!(!boiler(0, 0.0, 1).water_limited(64));
        assert!(!boiler(4, 0.0, 1).water_limited(3));
    }

    struct Probe;
    impl AttachmentProbe for Probe {
        fn engine_facing(&self, pos: Pos) -> Option<Direction> {
            match pos {
                [1, 0, 0] => Some(Direction::East),      // faces away: counts
                [-1, 0, 0] => Some(Direction::East),     // faces into tank: no
                [0, 0, 1] => Some(Direction::South),
                _ => None,
            }
        }
        fn whistle(&self, pos: Pos) -> Option<(Direction, i32)> {
            match pos {
                [0, 1, 0] => Some((Direction::Down, 5)),  // on top, attached down: counts
                [0, -1, 0] => Some((Direction::Down, 7)), // attached away: no
                _ => None,
            }
        }
    }

    #[test]
    fn attachments_are_counted_per_adjacent_face() {
        let mut b = Boiler::default();
        assert!(b.evaluate(&[[0, 0, 0]], &Probe));
        assert_eq!((b.attached_engines, b.attached_whistles), (2, 1));
        assert!(b.is_active());
        assert!(b.needs_heat_update);
        assert!(!b.evaluate(&[[0, 0, 0]], &Probe));
        assert!(b.evaluate(&[], &Probe));
        assert!(!b.is_active());
    }

    #[test]
    fn water_supply_samples_every_five_ticks_and_decays_after_fifty() {
        let mut b = boiler(4, 0.0, 1);
        b.needs_heat_update = false;
        let mut no = || -> Vec<i32> { panic!("no heat refresh expected") };
        // 100 mB inserted during the first window.
        assert_eq!(b.fill_water(100, true), 100);
        assert_eq!(b.gathered_supply, 0);
        b.fill_water(100, false);
        let first = b.tick_server(64, 16_000, false, &mut no);
        assert!(first.notify);
        assert_eq!(b.water_supply, 20.0);
        for _ in 0..4 {
            let r = b.tick_server(64, 16_000, false, &mut no);
            assert!(!r.notify);
        }
        // Next sample (index 1) is empty; the peak stays until the ring wraps.
        b.tick_server(64, 16_000, false, &mut no);
        assert_eq!(b.water_supply, 20.0);
        for _ in 0..(8 * 5) {
            b.tick_server(64, 16_000, false, &mut no);
        }
        // The window containing the 20 mB/t sample is rewritten only as the
        // ring passes it, so after 10 samples the old peak is still inside.
        assert_eq!(b.water_supply, 20.0);
        for _ in 0..5 {
            b.tick_server(64, 16_000, false, &mut no);
        }
        // Index 0 now overwritten with 0 -> next wrap yields 0.
        for _ in 0..(9 * 5) {
            b.tick_server(64, 16_000, false, &mut no);
        }
        assert_eq!(b.water_supply, 0.0);
    }

    #[test]
    fn tick_refreshes_heat_once_and_reports_max() {
        let mut b = Boiler { attached_engines: 1, ..Boiler::default() };
        let mut calls = 0;
        let mut heaters = || {
            calls += 1;
            vec![2; 9]
        };
        // Creative water: 200 mB/t -> 18 levels with a 72 block tank.
        let r = b.tick_server(72, 576_000, true, &mut heaters);
        assert_eq!(b.active_heat, 18);
        assert_eq!(b.water_supply, 200.0);
        assert!(r.maxed && r.notify);
        b.tick_server(72, 576_000, true, &mut heaters);
        assert_eq!(calls, 1);
    }

    #[test]
    fn inactive_boilers_do_not_tick_and_empty_tanks_do_not_sample() {
        let mut b = Boiler::default();
        let mut never = || -> Vec<i32> { panic!("inactive") };
        assert_eq!(b.tick_server(8, 8_000, false, &mut never), TickOutcome::default());
        let mut b = Boiler { attached_engines: 1, needs_heat_update: false, ..Boiler::default() };
        b.fill_water(50, false);
        assert_eq!(b.tick_server(8, 0, false, &mut never), TickOutcome::default());
        assert_eq!(b.water_supply, 0.0);
    }

    #[test]
    fn gauge_and_sound_volume() {
        let b = boiler(9, 500.0, 1);
        assert_eq!(b.gauge_target(72), 0.5);
        assert_eq!(boiler(9, 500.0, 1).sound_volume(), 1.5);
        assert_eq!(boiler(9, 500.0, 12).sound_volume(), 1.5);
        assert_eq!(boiler(9, 500.0, 18).sound_volume(), 1.0);
        let p = Boiler { passive: true, water_supply: 10.0, attached_engines: 1, ..Boiler::default() };
        assert_eq!(p.gauge_target(8), 0.125);
        assert_eq!(b.gauge_target(0), 0.0);
    }

    #[test]
    fn whistle_sound_conditions() {
        let hot = boiler(1, 0.0, 0);
        assert!(!hot.whistles_sound(true)); // no attachments
        let w = Boiler { attached_whistles: 1, active_heat: 1, ..Boiler::default() };
        assert!(w.whistles_sound(true));
        assert!(!w.whistles_sound(false));
        let cold = Boiler { attached_whistles: 1, ..Boiler::default() };
        assert!(!cold.whistles_sound(true));
        let passive = Boiler { attached_whistles: 1, passive: true, ..Boiler::default() };
        assert!(passive.whistles_sound(true));
    }

    #[test]
    fn pipe_organ_needs_twelve_distinct_pitch_ids() {
        assert!(!pipe_organ((0..11).collect::<Vec<_>>()));
        assert!(pipe_organ((0..12).collect::<Vec<_>>()));
        assert!(!pipe_organ([5; 20]));
        // Different sizes give distinct ids for the same pitch.
        assert!(pipe_organ((0..6).chain(100..106)));
    }
}
