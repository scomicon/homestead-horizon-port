// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Pure state machines of the Create 6.0.10 redstone blocks: Stockpile Switch
// (ThresholdSwitch*), Smart Observer, Redstone Contact, Analog Lever, the
// Brass-diode family (Pulse Repeater / Extender / Timer), Toggle Latch and
// Powered Latch. World access (neighbour signals, inventories, scheduling)
// stays with the adapter; every method here returns what must be written or
// scheduled.
use crate::redstone_link::{Direction, MAX_SIGNAL};

// ----------------------------------------------------------------------
// Stockpile Switch
// ----------------------------------------------------------------------

pub const THRESHOLD_LAZY_TICK_RATE: i32 = 10;
pub const THRESHOLD_BLOCK_TICK_DELAY: i32 = 2;
pub const DEFAULT_ON_WHEN_ABOVE: i32 = 128;
pub const DEFAULT_OFF_WHEN_BELOW: i32 = 64;
pub const DEFAULT_MAX_STACK: i32 = 64;

/// One inventory slot as seen by `updateCurrentLevel`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotSample {
    /// Stack-size limit used for capacity: `min(maxStackSize, slotLimit)`
    /// or the compat handler's `getSpaceInSlot`.
    pub space: i32,
    /// Item count when the stack passes the filter, otherwise 0.
    pub matching_count: i32,
}

/// One fluid tank: capacity in mB and the amount passing the filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TankSample {
    pub capacity: i32,
    pub matching_amount: i32,
}

/// Sum slot and tank samples into `(min, current, max)`. Slots with no space
/// are skipped; every other slot adds to the maximum.
pub fn storage_levels(slots: &[SlotSample], tanks: &[TankSample]) -> (i32, i32, i32) {
    let (mut cur, mut max) = (0i32, 0i32);
    for s in slots {
        if s.space == 0 {
            continue;
        }
        max = max.saturating_add(s.space);
        cur = cur.saturating_add(s.matching_count);
    }
    for t in tanks {
        max = max.saturating_add(t.capacity);
        cur = cur.saturating_add(t.matching_amount);
    }
    (0, cur, max)
}

/// `Mth.clamp(int, int, int)`.
fn clamp_i(v: i32, lo: i32, hi: i32) -> i32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

/// Side effects of one `updateCurrentLevel`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ThresholdUpdate {
    /// Value of the `LEVEL` block-state property (0..=5), if it was written.
    pub level_property: Option<i32>,
    /// The hysteresis state flipped: LEVEL is written with flag 3.
    pub state_changed: bool,
    /// Schedule `updatePowerAfterDelay` 2 ticks later.
    pub schedule_tick: bool,
    /// `DisplayLinkBlock.notifyGatherers` and `notifyUpdate()`.
    pub notify: bool,
    /// `sendData()` (no target found path).
    pub send_data: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ThresholdSwitch {
    pub on_when_above: i32,
    pub off_when_below: i32,
    pub inverted: bool,
    pub in_stacks: bool,
    pub current_level: i32,
    pub current_min: i32,
    pub current_max: i32,
    pub redstone_state: bool,
    pub powered_after_delay: bool,
}

impl Default for ThresholdSwitch {
    fn default() -> Self {
        Self {
            on_when_above: DEFAULT_ON_WHEN_ABOVE,
            off_when_below: DEFAULT_OFF_WHEN_BELOW,
            inverted: false,
            in_stacks: false,
            current_level: -1,
            current_min: 0,
            current_max: 0,
            redstone_state: false,
            powered_after_delay: false,
        }
    }
}

impl ThresholdSwitch {
    pub fn new() -> Self {
        Self::default()
    }

    /// `updateCurrentLevel`. `target` is `Some((min, current, max))` for an
    /// observable / inventory / tank target, `None` when nothing usable is in
    /// front of the switch.
    pub fn update_current_level(&mut self, target: Option<(i32, i32, i32)>) -> ThresholdUpdate {
        let prev_level = self.current_level;
        let prev_max = self.current_max;
        let Some((min, cur, max)) = target else {
            self.current_min = -1;
            self.current_max = -1;
            if self.current_level == -1 {
                return ThresholdUpdate::default();
            }
            self.current_level = -1;
            self.redstone_state = false;
            return ThresholdUpdate {
                level_property: Some(0),
                schedule_tick: true,
                send_data: true,
                ..Default::default()
            };
        };
        self.current_min = min;
        self.current_max = max;
        self.current_level = clamp_i(cur, min, max);
        let _ = prev_max;
        let changed = self.current_level != prev_level;

        let previously = self.redstone_state;
        if self.redstone_state && self.current_level <= self.off_when_below {
            self.redstone_state = false;
        } else if !self.redstone_state && self.current_level >= self.on_when_above {
            self.redstone_state = true;
        }
        let update = previously != self.redstone_state;

        let normed = (self.current_level - self.current_min) as f32 / (self.current_max - self.current_min) as f32;
        let level = if self.current_level > 0 { (1.0 + normed * 4.0) as i32 } else { 0 };
        ThresholdUpdate {
            level_property: Some(level),
            state_changed: update,
            schedule_tick: update,
            notify: changed || update,
            send_data: false,
        }
    }

    /// `shouldBePowered`.
    pub fn should_be_powered(&self) -> bool {
        self.inverted != self.redstone_state
    }

    /// Scheduled tick -> `updatePowerAfterDelay`.
    pub fn update_power_after_delay(&mut self) {
        self.powered_after_delay = self.should_be_powered();
    }

    /// `setInverted`: returns whether anything changed.
    pub fn set_inverted(&mut self, inverted: bool) -> bool {
        if self.inverted == inverted {
            return false;
        }
        self.inverted = inverted;
        self.update_power_after_delay();
        true
    }

    /// `getState()`: raw hysteresis state, ignores inversion.
    pub fn state(&self) -> bool {
        self.redstone_state
    }

    pub fn is_powered(&self) -> bool {
        self.powered_after_delay
    }

    pub fn level_for_display(&self) -> i32 {
        if self.current_level == -1 { 0 } else { self.current_level }
    }

    /// `ThresholdSwitchBlock.getSignal`: 15/0, nothing on the face opposite
    /// the target.
    pub fn signal(&self, side: Direction, target_direction: Direction) -> i32 {
        if side == target_direction.opposite() {
            0
        } else if self.powered_after_delay {
            MAX_SIGNAL
        } else {
            0
        }
    }
}

// ----------------------------------------------------------------------
// Smart Observer
// ----------------------------------------------------------------------

pub const OBSERVER_DEFAULT_DELAY: i32 = 6;
pub const OBSERVER_BLOCK_MATCH_DELAY: i32 = 3;
pub const OBSERVER_FUNNEL_DELAY: i32 = 4;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InventoryObservation {
    /// `invVersionTracker.stillWaiting(...)`: nothing changed since last scan.
    pub still_waiting: bool,
    /// Simulated extraction with the filter returned a non-empty stack.
    pub can_extract: bool,
}

/// What the observer sees in front of it this tick, in the order Java probes.
#[derive(Clone, Debug, Default)]
pub struct Observation {
    /// Filter non-empty, target block has an item form and passes the filter.
    pub block_matches: bool,
    /// Belt: one entry per centered item, true if it passes the filter.
    pub belt: Option<Vec<bool>>,
    /// Pipe: any inbound complete flow whose fluid passes the filter.
    pub pipe_flow_matches: Option<bool>,
    /// Chain conveyor: any looping package passes the filter.
    pub chain_package_matches: Option<bool>,
    pub inventory: Option<InventoryObservation>,
    /// Tank: simulated `extractAny` non-empty (filter handled by adapter).
    pub tank_extractable: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObserverEffect {
    /// `POWERED` went false -> true (set block state, update neighbours).
    pub powered_changed: bool,
    /// Schedule a block tick in 1 tick (it will call `block_tick`).
    pub schedule_off: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SmartObserver {
    pub turn_off_ticks: i32,
    pub powered: bool,
    pub sustain_signal: bool,
}

impl SmartObserver {
    pub fn new() -> Self {
        Self::default()
    }

    /// `activate(ticks)`.
    pub fn activate(&mut self, ticks: i32) -> bool {
        self.turn_off_ticks = ticks;
        if self.powered {
            return false;
        }
        self.powered = true;
        true
    }

    /// `SmartObserverBlock.tick`: end of the pulse.
    pub fn block_tick(&mut self) {
        self.powered = false;
    }

    /// `SmartObserverBlock.onFunnelTransfer` (filter already accepted).
    pub fn on_funnel_transfer(&mut self) -> bool {
        self.activate(OBSERVER_FUNNEL_DELAY)
    }

    pub fn tick(&mut self, obs: &Observation) -> ObserverEffect {
        let mut fx = ObserverEffect::default();
        if self.turn_off_ticks > 0 {
            self.turn_off_ticks -= 1;
            if self.turn_off_ticks == 0 {
                fx.schedule_off = true;
            }
        }
        let act = |this: &mut Self, ticks: i32, fx: &mut ObserverEffect| {
            if this.activate(ticks) {
                fx.powered_changed = true;
            }
        };
        if obs.block_matches {
            act(self, OBSERVER_BLOCK_MATCH_DELAY, &mut fx);
            return fx;
        }
        if let Some(items) = &obs.belt {
            for passes in items {
                if !passes || self.turn_off_ticks == OBSERVER_DEFAULT_DELAY {
                    continue;
                }
                act(self, OBSERVER_DEFAULT_DELAY, &mut fx);
            }
            return fx;
        }
        if let Some(matches) = obs.pipe_flow_matches {
            if matches {
                act(self, OBSERVER_DEFAULT_DELAY, &mut fx);
            }
            return fx;
        }
        if let Some(matches) = obs.chain_package_matches {
            if matches {
                act(self, OBSERVER_DEFAULT_DELAY, &mut fx);
            }
            return fx;
        }
        if let Some(inv) = obs.inventory {
            let skip = inv.still_waiting;
            if skip && self.sustain_signal {
                self.turn_off_ticks = OBSERVER_DEFAULT_DELAY;
            }
            if !skip {
                self.sustain_signal = false;
                if inv.can_extract {
                    self.sustain_signal = true;
                    act(self, OBSERVER_DEFAULT_DELAY, &mut fx);
                    return fx;
                }
            }
        }
        if obs.tank_extractable {
            act(self, OBSERVER_DEFAULT_DELAY, &mut fx);
        }
        fx
    }

    /// `SmartObserverBlock.getSignal`.
    pub fn signal(&self, side: Option<Direction>, target_direction: Direction) -> i32 {
        if self.powered && side.is_none_or(|s| s != target_direction.opposite()) {
            MAX_SIGNAL
        } else {
            0
        }
    }

    /// `canConnectRedstone` uses FACING (not the target direction).
    pub fn can_connect(side: Option<Direction>, facing: Direction) -> bool {
        side.is_none_or(|s| s != facing.opposite())
    }
}

// ----------------------------------------------------------------------
// Redstone Contact
// ----------------------------------------------------------------------

/// `hasValidContact`: the neighbour in `direction` must be a contact whose
/// FACING is `direction.opposite()`.
pub fn contact_has_valid_contact(neighbour: Option<(bool, Direction)>, direction: Direction) -> bool {
    matches!(neighbour, Some((true, facing)) if facing == direction.opposite())
}

/// `getSignal`: 15 when POWERED on every side except the back.
pub fn contact_signal(powered: bool, facing: Direction, side: Direction) -> i32 {
    if powered && side != facing.opposite() { MAX_SIGNAL } else { 0 }
}

pub fn contact_can_connect(side: Option<Direction>, facing: Direction) -> bool {
    side.is_some_and(|s| s != facing.opposite())
}

/// `tick` / `updateShape`: new POWERED value if it must change.
pub fn contact_next_powered(powered: bool, valid_contact: bool) -> Option<bool> {
    (powered != valid_contact).then_some(valid_contact)
}

// ----------------------------------------------------------------------
// Analog Lever
// ----------------------------------------------------------------------

pub const LEVER_MAX_STATE: i32 = 15;
pub const LEVER_CHANGE_DELAY: i32 = 15;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnalogLever {
    pub state: i32,
    pub last_change: i32,
}

impl AnalogLever {
    /// `changeState(back)`: `back` is the sneak flag. Returns whether the
    /// value changed (starts the 15 tick output delay).
    pub fn change_state(&mut self, back: bool) -> bool {
        let prev = self.state;
        self.state = (self.state + if back { -1 } else { 1 }).clamp(0, LEVER_MAX_STATE);
        if prev != self.state {
            self.last_change = LEVER_CHANGE_DELAY;
            true
        } else {
            false
        }
    }

    /// Server/common tick; returns true when `updateOutput` fires.
    pub fn tick(&mut self) -> bool {
        if self.last_change > 0 {
            self.last_change -= 1;
            if self.last_change == 0 {
                return true;
            }
        }
        false
    }

    pub fn signal(&self) -> i32 {
        self.state
    }

    pub fn direct_signal(&self, connected: Direction, side: Direction) -> i32 {
        if connected == side { self.state } else { 0 }
    }

    /// Lever click pitch after the state change.
    pub fn click_pitch(&self) -> f32 {
        0.25 + ((self.state + 5) as f32 / 15.0) * 0.5
    }
}

// ----------------------------------------------------------------------
// Brass diodes
// ----------------------------------------------------------------------

pub const DIODE_MIN_DELAY: i32 = 2;
pub const DIODE_MAX_DELAY: i32 = 60 * 20 * 60;
pub const DIODE_BLOCK_DELAY: i32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiodeKind {
    PulseRepeater,
    PulseExtender,
    PulseTimer,
}

impl DiodeKind {
    /// `defaultValue()`: only the timer overrides it (20).
    pub fn default_delay(self) -> i32 {
        if self == DiodeKind::PulseTimer { 20 } else { 2 }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrassDiode {
    pub kind: DiodeKind,
    pub state: i32,
    pub max_state: i32,
    /// Block state `POWERED` (delayed input).
    pub powered: bool,
    /// Block state `POWERING` (output before inversion).
    pub powering: bool,
    pub inverted: bool,
}

impl BrassDiode {
    pub fn new(kind: DiodeKind) -> Self {
        Self { kind, state: 0, max_state: kind.default_delay(), powered: false, powering: false, inverted: false }
    }

    pub fn set_max_state(&mut self, value: i32) {
        self.max_state = value.clamp(DIODE_MIN_DELAY, DIODE_MAX_DELAY);
        self.state = self.state.min(self.max_state);
    }

    pub fn is_idle(&self) -> bool {
        self.state == 0
    }

    pub fn progress(&self) -> f32 {
        let max = self.max_state.max(2);
        self.state.clamp(0, max) as f32 / max as f32
    }

    /// Output signal: `POWERING ^ INVERTED` -> 15.
    pub fn output_signal(&self) -> i32 {
        if self.powering != self.inverted { MAX_SIGNAL } else { 0 }
    }

    /// `getSignal(side)`: output only on the FACING side.
    pub fn signal(&self, facing: Direction, side: Direction) -> i32 {
        if side == facing { self.output_signal() } else { 0 }
    }

    /// Server tick. Returns true if `POWERING` changed (block update).
    pub fn tick(&mut self) -> bool {
        let before = self.powering;
        let at_max = self.state >= self.max_state;
        let at_min = self.state <= 0;
        self.update_state(self.powered, self.powering, at_max, at_min);
        before != self.powering
    }

    fn update_state(&mut self, powered: bool, powering: bool, _at_max: bool, at_min: bool) {
        match self.kind {
            DiodeKind::PulseExtender => {
                if at_min && !powered {
                    return;
                }
                if at_min || powered {
                    self.powering = true;
                    self.state = self.max_state;
                    return;
                }
                if self.state == 1 {
                    if powering {
                        self.powering = false;
                    }
                    self.state = 0;
                    return;
                }
                self.state -= 1;
            }
            DiodeKind::PulseRepeater => {
                if at_min && !powered {
                    return;
                }
                if self.state > self.max_state + 1 {
                    if !powered && !powering {
                        self.state = 0;
                    }
                    return;
                }
                self.state += 1;
                if self.state == self.max_state - 1 && !powering {
                    self.powering = !self.powering;
                }
                if self.state == self.max_state + 1 && powering {
                    self.powering = !self.powering;
                }
            }
            DiodeKind::PulseTimer => {
                if powered || self.state >= self.max_state - 1 {
                    self.state = 0;
                } else {
                    self.state += 1;
                }
                let should_power =
                    !powered && (if self.max_state == 2 { self.state == 0 } else { self.state <= 1 });
                if self.powering != should_power {
                    self.powering = should_power;
                }
            }
        }
    }

    /// Right-click toggle; returns the lever-click pitch.
    pub fn toggle_inverted(&mut self) -> f32 {
        self.inverted = !self.inverted;
        if self.inverted { 0.6 } else { 0.5 }
    }
}

/// Vanilla `DiodeBlock.tick` used by the latches: returns
/// `(new_powered, reschedule)`.
pub fn diode_tick(powered: bool, should_turn_on: bool) -> (bool, bool) {
    if powered && !should_turn_on {
        (false, false)
    } else if !powered {
        (true, !should_turn_on)
    } else {
        (powered, false)
    }
}

/// Toggle Latch: delay 1, flips POWERING on the rising edge of POWERED.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ToggleLatch {
    pub powered: bool,
    pub powering: bool,
}

pub const LATCH_DELAY: i32 = 1;

impl ToggleLatch {
    /// Returns whether another tick must be scheduled.
    pub fn tick(&mut self, should_turn_on: bool) -> bool {
        let prev = self.powered;
        let (p, again) = diode_tick(self.powered, should_turn_on);
        self.powered = p;
        if !prev && self.powered {
            self.powering = !self.powering;
        }
        again
    }

    pub fn output_signal(&self) -> i32 {
        if self.powering { MAX_SIGNAL } else { 0 }
    }

    /// Manual toggle; returns the click pitch (0.6 turning on).
    pub fn activated(&mut self) -> f32 {
        self.powering = !self.powering;
        if self.powering { 0.6 } else { 0.5 }
    }
}

/// Powered Latch: back input sets, side input resets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PoweredLatch {
    pub powered: bool,
    pub powered_side: bool,
    pub powering: bool,
}

impl PoweredLatch {
    /// `tick` with the freshly sampled inputs. Returns true if state changed.
    pub fn tick(&mut self, should_back: bool, should_side: bool) -> bool {
        let before = *self;
        if self.powered != should_back {
            self.powered = should_back;
            if should_back {
                self.powering = true;
            } else if should_side {
                self.powering = false;
            }
        }
        if self.powered_side != should_side {
            self.powered_side = should_side;
            if should_side {
                self.powering = false;
            } else if should_back {
                self.powering = true;
            }
        }
        before != *self
    }

    /// `checkTickOnNeighbor`: whether a tick must be scheduled.
    pub fn needs_tick(&self, should_back: bool, should_side: bool) -> bool {
        self.powered != should_back || self.powered_side != should_side
    }

    pub fn output_signal(&self) -> i32 {
        if self.powering { MAX_SIGNAL } else { 0 }
    }

    /// Manual toggle is refused while the two inputs disagree.
    pub fn activated(&mut self) -> Option<f32> {
        if self.powered != self.powered_side {
            return None;
        }
        self.powering = !self.powering;
        Some(if self.powering { 0.6 } else { 0.5 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_levels_skip_zero_space_and_count_only_matches() {
        let slots = [
            SlotSample { space: 64, matching_count: 10 },
            SlotSample { space: 0, matching_count: 5 },
            SlotSample { space: 16, matching_count: 0 },
        ];
        let tanks = [TankSample { capacity: 1000, matching_amount: 250 }];
        assert_eq!(storage_levels(&slots, &tanks), (0, 260, 1080));
    }

    #[test]
    fn threshold_defaults() {
        let t = ThresholdSwitch::new();
        assert_eq!((t.on_when_above, t.off_when_below, t.current_level), (128, 64, -1));
        assert!(!t.is_powered());
    }

    #[test]
    fn threshold_hysteresis() {
        let mut t = ThresholdSwitch::new();
        let u = t.update_current_level(Some((0, 100, 1000)));
        assert!(!t.state() && !u.state_changed && u.notify);
        let u = t.update_current_level(Some((0, 128, 1000)));
        assert!(t.state() && u.state_changed && u.schedule_tick && u.notify);
        // between the thresholds the state sticks
        let u = t.update_current_level(Some((0, 90, 1000)));
        assert!(t.state() && !u.state_changed);
        t.update_current_level(Some((0, 64, 1000)));
        assert!(!t.state());
        t.update_current_level(Some((0, 127, 1000)));
        assert!(!t.state());
    }

    #[test]
    fn threshold_level_property_and_clamp() {
        let mut t = ThresholdSwitch::new();
        assert_eq!(t.update_current_level(Some((0, 0, 100))).level_property, Some(0));
        assert_eq!(t.update_current_level(Some((0, 1, 100))).level_property, Some(1));
        assert_eq!(t.update_current_level(Some((0, 50, 100))).level_property, Some(3));
        assert_eq!(t.update_current_level(Some((0, 100, 100))).level_property, Some(5));
        t.update_current_level(Some((0, 500, 100)));
        assert_eq!(t.current_level, 100);
        // max == min -> NaN -> 0
        assert_eq!(t.update_current_level(Some((5, 5, 5))).level_property, Some(0));
    }

    #[test]
    fn threshold_no_target_resets_once() {
        let mut t = ThresholdSwitch::new();
        assert_eq!(t.update_current_level(None), ThresholdUpdate::default());
        t.update_current_level(Some((0, 200, 300)));
        assert!(t.state());
        let u = t.update_current_level(None);
        assert_eq!(u.level_property, Some(0));
        assert!(u.schedule_tick && u.send_data && !t.state());
        assert_eq!((t.current_min, t.current_max, t.current_level), (-1, -1, -1));
        assert_eq!(t.update_current_level(None), ThresholdUpdate::default());
        assert_eq!(t.level_for_display(), 0);
    }

    #[test]
    fn threshold_inversion_and_delay() {
        let mut t = ThresholdSwitch::new();
        t.update_current_level(Some((0, 200, 300)));
        assert!(!t.is_powered()); // not until the scheduled tick
        t.update_power_after_delay();
        assert!(t.is_powered());
        assert!(t.set_inverted(true));
        assert!(!t.is_powered());
        assert!(!t.set_inverted(true));
        assert!(t.state()); // raw state ignores inversion
        assert_eq!(t.signal(Direction::North, Direction::North), 0);
        t.update_current_level(Some((0, 10, 300)));
        t.update_power_after_delay();
        assert_eq!(t.signal(Direction::East, Direction::North), 15);
        assert_eq!(t.signal(Direction::South, Direction::North), 0);
    }

    #[test]
    fn observer_block_match_holds_three_ticks() {
        let mut o = SmartObserver::new();
        let fx = o.tick(&Observation { block_matches: true, ..Default::default() });
        assert!(fx.powered_changed && o.powered && o.turn_off_ticks == 3);
        let none = Observation::default();
        assert!(!o.tick(&none).schedule_off); // 3 -> 2
        assert!(!o.tick(&none).schedule_off); // 2 -> 1
        assert!(o.tick(&none).schedule_off); // 1 -> 0
        o.block_tick();
        assert!(!o.powered);
    }

    #[test]
    fn observer_belt_skips_while_timer_is_at_default() {
        let mut o = SmartObserver::new();
        let belt = Observation { belt: Some(vec![true]), ..Default::default() };
        o.tick(&belt);
        assert_eq!(o.turn_off_ticks, 6);
        o.tick(&belt); // decrements to 5 first, then re-activates to 6
        assert_eq!(o.turn_off_ticks, 6);
        let two = Observation { belt: Some(vec![true, true]), ..Default::default() };
        let mut o2 = SmartObserver::new();
        assert!(o2.tick(&two).powered_changed);
        let nope = Observation { belt: Some(vec![false]), ..Default::default() };
        let mut o3 = SmartObserver::new();
        assert!(!o3.tick(&nope).powered_changed);
    }

    #[test]
    fn observer_inventory_sustain() {
        let mut o = SmartObserver::new();
        let hit = Observation {
            inventory: Some(InventoryObservation { still_waiting: false, can_extract: true }),
            ..Default::default()
        };
        assert!(o.tick(&hit).powered_changed);
        assert!(o.sustain_signal);
        // inventory unchanged: waiting + sustain keeps the signal at 6
        let wait = Observation {
            inventory: Some(InventoryObservation { still_waiting: true, can_extract: true }),
            ..Default::default()
        };
        for _ in 0..10 {
            o.tick(&wait);
            assert_eq!(o.turn_off_ticks, 6);
        }
        let empty = Observation {
            inventory: Some(InventoryObservation { still_waiting: false, can_extract: false }),
            ..Default::default()
        };
        o.tick(&empty);
        assert!(!o.sustain_signal);
    }

    #[test]
    fn observer_inventory_miss_falls_through_to_tank() {
        let mut o = SmartObserver::new();
        let obs = Observation {
            inventory: Some(InventoryObservation { still_waiting: false, can_extract: false }),
            tank_extractable: true,
            ..Default::default()
        };
        assert!(o.tick(&obs).powered_changed);
    }

    #[test]
    fn observer_funnel_and_signal() {
        let mut o = SmartObserver::new();
        assert!(o.on_funnel_transfer());
        assert_eq!(o.turn_off_ticks, 4);
        assert_eq!(o.signal(None, Direction::North), 15);
        assert_eq!(o.signal(Some(Direction::South), Direction::North), 0);
        assert_eq!(o.signal(Some(Direction::East), Direction::North), 15);
        assert!(SmartObserver::can_connect(None, Direction::North));
        assert!(!SmartObserver::can_connect(Some(Direction::South), Direction::North));
        // re-activation while powered only refreshes the timer
        assert!(!o.activate(6));
        assert_eq!(o.turn_off_ticks, 6);
    }

    #[test]
    fn contact_rules() {
        assert!(contact_has_valid_contact(Some((true, Direction::South)), Direction::North));
        assert!(!contact_has_valid_contact(Some((true, Direction::North)), Direction::North));
        assert!(!contact_has_valid_contact(Some((false, Direction::South)), Direction::North));
        assert!(!contact_has_valid_contact(None, Direction::North));
        assert_eq!(contact_signal(true, Direction::Up, Direction::Down), 0);
        assert_eq!(contact_signal(true, Direction::Up, Direction::North), 15);
        assert_eq!(contact_signal(false, Direction::Up, Direction::North), 0);
        assert!(!contact_can_connect(None, Direction::Up));
        assert!(contact_can_connect(Some(Direction::East), Direction::Up));
        assert_eq!(contact_next_powered(false, true), Some(true));
        assert_eq!(contact_next_powered(true, true), None);
    }

    #[test]
    fn lever_clamps_and_delays_output() {
        let mut l = AnalogLever::default();
        assert!(!l.change_state(true)); // already 0
        assert_eq!(l.last_change, 0);
        for _ in 0..20 {
            l.change_state(false);
        }
        assert_eq!(l.state, 15);
        assert!(!l.change_state(false));
        l.last_change = 0;
        assert!(l.change_state(true));
        assert_eq!(l.state, 14);
        let mut fired = 0;
        for i in 1..=15 {
            if l.tick() {
                fired = i;
            }
        }
        assert_eq!(fired, 15);
        assert!(!l.tick());
    }

    #[test]
    fn lever_outputs_and_pitch() {
        let mut l = AnalogLever { state: 15, last_change: 0 };
        assert_eq!(l.signal(), 15);
        assert_eq!(l.direct_signal(Direction::Down, Direction::Down), 15);
        assert_eq!(l.direct_signal(Direction::Down, Direction::Up), 0);
        assert!((l.click_pitch() - 0.9167).abs() < 1e-3);
        l.state = 0;
        assert!((l.click_pitch() - 0.4167).abs() < 1e-3);
    }

    fn run_diode(d: &mut BrassDiode, powered_script: &[bool]) -> Vec<bool> {
        let mut out = Vec::new();
        for &p in powered_script {
            d.powered = p;
            d.tick();
            out.push(d.powering);
        }
        out
    }

    #[test]
    fn diode_defaults_and_limits() {
        assert_eq!(BrassDiode::new(DiodeKind::PulseTimer).max_state, 20);
        assert_eq!(BrassDiode::new(DiodeKind::PulseExtender).max_state, 2);
        let mut d = BrassDiode::new(DiodeKind::PulseRepeater);
        d.set_max_state(1);
        assert_eq!(d.max_state, 2);
        d.set_max_state(1_000_000);
        assert_eq!(d.max_state, 72_000);
        d.state = 72_000;
        d.set_max_state(10);
        assert_eq!(d.state, 10);
    }

    #[test]
    fn extender_holds_output_for_max_ticks_after_input_drops() {
        let mut d = BrassDiode::new(DiodeKind::PulseExtender);
        d.set_max_state(5);
        let mut script = vec![true];
        script.extend(std::iter::repeat(false).take(8));
        let out = run_diode(&mut d, &script);
        // on immediately, stays on while the countdown (5 -> 1) runs, then off
        assert_eq!(out, vec![true, true, true, true, true, false, false, false, false]);
        assert!(d.is_idle());
    }

    #[test]
    fn extender_retriggers() {
        let mut d = BrassDiode::new(DiodeKind::PulseExtender);
        d.set_max_state(4);
        run_diode(&mut d, &[true, false, false]);
        assert_eq!(d.state, 2); // 4 -> 3 -> 2
        run_diode(&mut d, &[true]);
        assert_eq!(d.state, 4);
    }

    #[test]
    fn repeater_delays_pulse() {
        let mut d = BrassDiode::new(DiodeKind::PulseRepeater);
        d.set_max_state(4);
        let mut script = vec![true];
        script.extend(std::iter::repeat(false).take(10));
        let out = run_diode(&mut d, &script);
        // rises when state reaches max-1 (3), falls at max+1 (5), then resets
        assert_eq!(
            out,
            vec![false, false, true, true, false, false, false, false, false, false, false]
        );
        assert_eq!(d.state, 0);
    }

    #[test]
    fn timer_cycles_and_is_held_off_by_input() {
        let mut d = BrassDiode::new(DiodeKind::PulseTimer);
        d.set_max_state(5);
        let out = run_diode(&mut d, &[false; 12]);
        // state counts 1,2,3,4 then resets to 0 (state >= max-1); on while state <= 1
        let (t, f) = (true, false);
        assert_eq!(out, vec![t, f, f, f, t, t, f, f, f, t, t, f]);
        d.powered = true;
        d.tick();
        assert_eq!((d.state, d.powering), (0, false));
    }

    #[test]
    fn timer_with_delay_two_pulses_one_tick() {
        let mut d = BrassDiode::new(DiodeKind::PulseTimer);
        d.set_max_state(2);
        let out = run_diode(&mut d, &[false; 6]);
        // max 2: state alternates 1, 0; powered only when state == 0
        assert_eq!(out, vec![false, true, false, true, false, true]);
    }

    #[test]
    fn diode_output_inversion_and_side() {
        let mut d = BrassDiode::new(DiodeKind::PulseRepeater);
        assert_eq!(d.output_signal(), 0);
        assert_eq!(d.toggle_inverted(), 0.6);
        assert_eq!(d.output_signal(), 15);
        d.powering = true;
        assert_eq!(d.output_signal(), 0);
        d.inverted = false;
        assert_eq!(d.signal(Direction::North, Direction::North), 15);
        assert_eq!(d.signal(Direction::North, Direction::East), 0);
        assert_eq!(d.toggle_inverted(), 0.6);
        assert_eq!(d.toggle_inverted(), 0.5);
    }

    #[test]
    fn diode_progress() {
        let mut d = BrassDiode::new(DiodeKind::PulseTimer);
        d.state = 10;
        assert!((d.progress() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn vanilla_diode_tick() {
        assert_eq!(diode_tick(true, false), (false, false));
        assert_eq!(diode_tick(false, true), (true, false));
        assert_eq!(diode_tick(false, false), (true, true));
        assert_eq!(diode_tick(true, true), (true, false));
    }

    #[test]
    fn toggle_latch_is_rising_edge_triggered() {
        let mut l = ToggleLatch::default();
        l.tick(true);
        assert!(l.powering && l.powered);
        l.tick(true); // held: no further flip
        assert!(l.powering);
        l.tick(false);
        assert!(!l.powered && l.powering);
        l.tick(true);
        assert!(!l.powering);
        assert_eq!(l.output_signal(), 0);
        assert_eq!(l.activated(), 0.6);
        assert_eq!(l.activated(), 0.5);
    }

    #[test]
    fn powered_latch_set_reset_priorities() {
        let mut l = PoweredLatch::default();
        assert!(l.needs_tick(true, false));
        l.tick(true, false);
        assert!(l.powering);
        l.tick(false, false);
        assert!(l.powering); // memory
        l.tick(false, true);
        assert!(!l.powering); // reset
        // both rise in the same tick: the side branch wins
        let mut l = PoweredLatch::default();
        l.tick(true, true);
        assert!(!l.powering);
        // side released while back held -> set again
        l.tick(true, false);
        assert!(l.powering);
        // back released while side held -> reset
        let mut l = PoweredLatch { powered: true, powered_side: false, powering: true };
        l.tick(false, true);
        assert!(!l.powering);
    }

    #[test]
    fn powered_latch_manual_toggle_rules() {
        let mut l = PoweredLatch { powered: true, powered_side: false, powering: false };
        assert_eq!(l.activated(), None);
        let mut l = PoweredLatch::default();
        assert_eq!(l.activated(), Some(0.6));
        assert_eq!(l.activated(), Some(0.5));
    }
}
