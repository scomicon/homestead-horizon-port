// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Pure train schedules after Create 6.0.10 content/trains/schedule: Schedule,
// ScheduleEntry, instructions (destination, rename, throttle), wait
// conditions and the ScheduleRuntime state machine. World queries (item and
// fluid contents, redstone links, players, day time) arrive through
// `ConditionEnv`; navigation start/stop through `ScheduleEnv`.

/// Ticks between two instructions' cooldown (`ScheduleRuntime.INTERVAL`).
pub const INTERVAL: i32 = 40;
/// `TimeOfDayCondition.maxTickDiff`.
pub const TIME_OF_DAY_TOLERANCE: i64 = 40;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeUnit {
    Ticks,
    Seconds,
    Minutes,
}

impl TimeUnit {
    pub fn ticks(self) -> i64 {
        match self {
            Self::Ticks => 1,
            Self::Seconds => 20,
            Self::Minutes => 1200,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operator {
    Greater,
    Less,
    Equal,
}

impl Operator {
    pub fn test(self, current: i64, target: i64) -> bool {
        match self {
            Self::Greater => current > target,
            Self::Less => current < target,
            Self::Equal => current == target,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemMeasure {
    Items,
    Stacks,
}

/// `TimeOfDayCondition.getRotation()` intervals, indexed by the stored rotation.
pub const ROTATION_TICKS: [i64; 10] = [24000, 12000, 6000, 4000, 3000, 2000, 1000, 750, 500, 250];

#[derive(Clone, Debug, PartialEq)]
pub enum Condition {
    Delay { value: i32, unit: TimeUnit },
    IdleCargo { value: i32, unit: TimeUnit },
    TimeOfDay { hour: i32, minute: i32, rotation: usize },
    FluidThreshold { filter: String, op: Operator, buckets: i32 },
    ItemThreshold { filter: String, op: Operator, amount: i32, measure: ItemMeasure },
    RedstoneLink { frequency: [String; 2], inverted: bool },
    PlayerCount { count: i32, exact: bool },
    Unloaded,
    Powered,
}

/// Per-condition progress slot (the `Time` tag of the Java condition context).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CondCtx {
    pub time: i64,
}

/// World queries for wait conditions.
pub trait ConditionEnv {
    fn day_time(&self) -> i64;
    /// Matching contents of the train, in the unit of `measure`.
    fn item_count(&self, filter: &str, measure: ItemMeasure) -> i64;
    fn fluid_buckets(&self, filter: &str) -> i64;
    fn redstone_link_powered(&self, frequency: &[String; 2]) -> bool;
    fn players_aboard(&self) -> i32;
    /// Cargo content changed since the last query (resets idle timers).
    fn cargo_changed(&mut self) -> bool;
    fn station_unloaded(&self) -> bool;
    fn station_powered(&self) -> bool;
}

impl Condition {
    pub fn id(&self) -> &'static str {
        match self {
            Self::Delay { .. } => "create:delay",
            Self::IdleCargo { .. } => "create:idle",
            Self::TimeOfDay { .. } => "create:time_of_day",
            Self::FluidThreshold { .. } => "create:fluid_threshold",
            Self::ItemThreshold { .. } => "create:item_threshold",
            Self::RedstoneLink { .. } => "create:redstone_link",
            Self::PlayerCount { .. } => "create:player_count",
            Self::Unloaded => "create:unloaded",
            Self::Powered => "create:powered",
        }
    }

    pub fn total_wait_ticks(&self) -> i64 {
        match self {
            Self::Delay { value, unit } | Self::IdleCargo { value, unit } => *value as i64 * unit.ticks(),
            _ => 0,
        }
    }

    /// `TimeOfDayCondition.getRotation()` in ticks (unknown index -> 24000).
    pub fn rotation_ticks(rotation: usize) -> i64 {
        ROTATION_TICKS.get(rotation).copied().unwrap_or(24000)
    }

    /// Target tick within the rotation: `((hour+18)%24*1000 + ceil(minute/60*1000)) % rotation`.
    pub fn time_of_day_target(hour: i32, minute: i32, rotation: usize) -> i64 {
        let h = (hour.rem_euclid(24) + 18) % 24;
        let m = (minute as f64 / 60.0 * 1000.0).ceil() as i64;
        (h as i64 * 1000 + m) % Self::rotation_ticks(rotation)
    }

    /// Clock time (hour, minute) shown for a day-time tick offset (`getWaitingStatus`).
    pub fn clock_of(departure_ticks: i64) -> (i64, i64) {
        let t = departure_ticks.rem_euclid(24000);
        ((t / 1000 + 6) % 24, (t % 1000) * 60 / 1000)
    }

    /// `tickCompletion`: true when the condition is satisfied this tick.
    pub fn tick(&self, env: &mut dyn ConditionEnv, ctx: &mut CondCtx) -> bool {
        match self {
            Self::Delay { .. } => {
                if ctx.time >= self.total_wait_ticks() {
                    return true;
                }
                ctx.time += 1;
                false
            }
            Self::IdleCargo { .. } => {
                if env.cargo_changed() {
                    ctx.time = 0;
                }
                if ctx.time >= self.total_wait_ticks() {
                    return true;
                }
                ctx.time += 1;
                false
            }
            Self::TimeOfDay { hour, minute, rotation } => {
                let rot = Self::rotation_ticks(*rotation);
                let day = env.day_time().rem_euclid(rot);
                let diff = day - Self::time_of_day_target(*hour, *minute, *rotation);
                (0..=TIME_OF_DAY_TOLERANCE).contains(&diff)
            }
            Self::FluidThreshold { filter, op, buckets } => op.test(env.fluid_buckets(filter), *buckets as i64),
            Self::ItemThreshold { filter, op, amount, measure } => op.test(env.item_count(filter, *measure), *amount as i64),
            Self::RedstoneLink { frequency, inverted } => env.redstone_link_powered(frequency) != *inverted,
            Self::PlayerCount { count, exact } => {
                let n = env.players_aboard();
                if *exact { n == *count } else { n >= *count }
            }
            Self::Unloaded => env.station_unloaded(),
            Self::Powered => env.station_powered(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Instruction {
    /// Station name filter; `*` matches any run of characters.
    Destination { filter: String },
    Rename { text: String },
    /// Throttle in percent (5..=100).
    Throttle { percent: i32 },
}

impl Instruction {
    pub fn id(&self) -> &'static str {
        match self {
            Self::Destination { .. } => "create:destination",
            Self::Rename { .. } => "create:rename",
            Self::Throttle { .. } => "create:throttle",
        }
    }

    pub fn supports_conditions(&self) -> bool {
        matches!(self, Self::Destination { .. })
    }

    pub fn throttle_fraction(percent: i32) -> f64 {
        percent.clamp(5, 100) as f64 / 100.0
    }
}

/// Station-name match with `*` wildcards (the whole name must match).
/// Case-sensitive; see docs/trains.md (unverified against Java).
pub fn matches_filter(filter: &str, name: &str) -> bool {
    let parts: Vec<&str> = filter.split('*').collect();
    if parts.len() == 1 {
        return filter == name;
    }
    let mut rest = name;
    for (i, part) in parts.iter().enumerate() {
        if i == 0 {
            match rest.strip_prefix(part) {
                Some(r) => rest = r,
                None => return false,
            }
        } else if i == parts.len() - 1 {
            return rest.ends_with(part) && rest.len() >= part.len();
        } else if part.is_empty() {
            continue;
        } else {
            match rest.find(part) {
                Some(p) => rest = &rest[p + part.len()..],
                None => return false,
            }
        }
    }
    true
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScheduleEntry {
    pub instruction: Instruction,
    /// Columns are alternatives (first finished wins); each column is a sequence.
    pub conditions: Vec<Vec<Condition>>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Schedule {
    pub entries: Vec<ScheduleEntry>,
    pub cyclic: bool,
    pub saved_progress: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeState {
    PreTransit,
    InTransit,
    PostTransit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartResult {
    NoPath,
    /// Already standing at the matching station.
    AtDestination,
    Navigating,
}

pub trait ScheduleEnv: ConditionEnv {
    /// The train's navigation currently has a destination.
    fn has_destination(&self) -> bool;
    fn derailed(&self) -> bool;
    /// Find and start a path to a station matching `filter`.
    fn start_destination(&mut self, filter: &str) -> StartResult;
    fn set_title(&mut self, text: &str);
    fn set_throttle(&mut self, fraction: f64);
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScheduleRuntime {
    pub schedule: Option<Schedule>,
    pub state: RuntimeState,
    pub current_entry: usize,
    pub is_auto_schedule: bool,
    pub paused: bool,
    pub completed: bool,
    pub cooldown: i32,
    pub ticks_in_transit: i32,
    pub progress: Vec<usize>,
    pub context: Vec<CondCtx>,
    pub display_link_update_requested: bool,
    pub title: Option<String>,
}

impl Default for ScheduleRuntime {
    fn default() -> Self {
        Self {
            schedule: None,
            state: RuntimeState::PreTransit,
            current_entry: 0,
            is_auto_schedule: false,
            paused: true,
            completed: false,
            cooldown: 0,
            ticks_in_transit: 0,
            progress: Vec::new(),
            context: Vec::new(),
            display_link_update_requested: false,
            title: None,
        }
    }
}

impl ScheduleRuntime {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn set_schedule(&mut self, schedule: Schedule, auto: bool) {
        self.reset();
        let n = schedule.entries.len();
        self.current_entry = schedule.saved_progress.min(n.saturating_sub(1));
        self.schedule = Some(schedule);
        self.paused = false;
        self.is_auto_schedule = auto;
        self.display_link_update_requested = true;
    }

    pub fn start_cooldown(&mut self) {
        self.cooldown = INTERVAL;
    }

    /// `checkEndOfScheduleReached`; true when the end was hit.
    fn check_end(&mut self) -> bool {
        let Some(s) = &self.schedule else { return false };
        if self.current_entry >= s.entries.len() {
            self.current_entry = 0;
            if !s.cyclic {
                self.paused = true;
                self.completed = true;
            }
            return true;
        }
        false
    }

    fn entry(&self) -> Option<&ScheduleEntry> {
        self.schedule.as_ref()?.entries.get(self.current_entry)
    }

    /// `destinationReached`: only valid while in transit.
    pub fn destination_reached(&mut self) {
        if self.state != RuntimeState::InTransit {
            return;
        }
        self.state = RuntimeState::PostTransit;
        self.display_link_update_requested = true;
        let columns = self.entry().map(|e| e.conditions.len()).unwrap_or(0);
        self.progress = vec![0; columns];
        self.context = vec![CondCtx::default(); columns];
    }

    /// `transitInterrupted`.
    pub fn transit_interrupted(&mut self) {
        if self.state == RuntimeState::InTransit {
            self.state = RuntimeState::PreTransit;
            self.cooldown = 0;
        }
    }

    fn advance(&mut self) {
        self.state = RuntimeState::PreTransit;
        self.current_entry += 1;
    }

    fn tick_conditions(&mut self, env: &mut dyn ScheduleEnv) {
        let Some(entry) = self.entry().cloned() else { return };
        if !entry.instruction.supports_conditions() || entry.conditions.is_empty() {
            self.advance();
            self.start_cooldown();
            return;
        }
        for (i, list) in entry.conditions.iter().enumerate() {
            if self.progress.len() <= i {
                self.progress.resize(i + 1, 0);
                self.context.resize(i + 1, CondCtx::default());
            }
            if self.progress[i] >= list.len() {
                self.advance();
                self.start_cooldown();
                return;
            }
            let cond = &list[self.progress[i]];
            if cond.tick(env, &mut self.context[i]) {
                self.context[i] = CondCtx::default();
                self.progress[i] += 1;
                if i == 0 {
                    self.display_link_update_requested = true;
                }
            }
        }
    }

    /// One server tick (`ScheduleRuntime.tick`).
    pub fn tick(&mut self, env: &mut dyn ScheduleEnv) {
        if self.schedule.is_none() || self.paused || env.derailed() {
            return;
        }
        if env.has_destination() {
            self.ticks_in_transit += 1;
            return;
        }
        if self.check_end() {
            return;
        }
        if self.cooldown > 0 {
            self.cooldown -= 1;
            if self.cooldown > 0 {
                return;
            }
        }
        match self.state {
            RuntimeState::InTransit => {}
            RuntimeState::PostTransit => self.tick_conditions(env),
            RuntimeState::PreTransit => {
                let Some(entry) = self.entry().cloned() else { return };
                match entry.instruction {
                    Instruction::Destination { filter } => match env.start_destination(&filter) {
                        StartResult::NoPath => {}
                        StartResult::AtDestination => {
                            self.state = RuntimeState::InTransit;
                            self.destination_reached();
                        }
                        StartResult::Navigating => {
                            self.state = RuntimeState::InTransit;
                            self.ticks_in_transit = 0;
                        }
                    },
                    Instruction::Rename { text } => {
                        env.set_title(&text);
                        self.title = Some(text);
                        self.advance();
                    }
                    Instruction::Throttle { percent } => {
                        env.set_throttle(Instruction::throttle_fraction(percent));
                        self.advance();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Env {
        day: i64,
        items: i64,
        fluid: i64,
        link: bool,
        players: i32,
        changed: bool,
        unloaded: bool,
        powered: bool,
        destination: bool,
        derailed: bool,
        start: Option<StartResult>,
        started: Vec<String>,
        title: Option<String>,
        throttle: Option<f64>,
    }

    impl ConditionEnv for Env {
        fn day_time(&self) -> i64 {
            self.day
        }
        fn item_count(&self, _: &str, _: ItemMeasure) -> i64 {
            self.items
        }
        fn fluid_buckets(&self, _: &str) -> i64 {
            self.fluid
        }
        fn redstone_link_powered(&self, _: &[String; 2]) -> bool {
            self.link
        }
        fn players_aboard(&self) -> i32 {
            self.players
        }
        fn cargo_changed(&mut self) -> bool {
            std::mem::take(&mut self.changed)
        }
        fn station_unloaded(&self) -> bool {
            self.unloaded
        }
        fn station_powered(&self) -> bool {
            self.powered
        }
    }

    impl ScheduleEnv for Env {
        fn has_destination(&self) -> bool {
            self.destination
        }
        fn derailed(&self) -> bool {
            self.derailed
        }
        fn start_destination(&mut self, f: &str) -> StartResult {
            self.started.push(f.to_string());
            self.start.unwrap_or(StartResult::Navigating)
        }
        fn set_title(&mut self, t: &str) {
            self.title = Some(t.into());
        }
        fn set_throttle(&mut self, f: f64) {
            self.throttle = Some(f);
        }
    }

    fn delay(secs: i32) -> Condition {
        Condition::Delay { value: secs, unit: TimeUnit::Seconds }
    }

    #[test]
    fn wildcard_matching() {
        assert!(matches_filter("Station", "Station"));
        assert!(!matches_filter("Station", "Station 2"));
        assert!(matches_filter("Station*", "Station 2"));
        assert!(matches_filter("*Depot", "Main Depot"));
        assert!(matches_filter("A*C", "ABC"));
        assert!(matches_filter("A*B*C", "AxxBxxC"));
        assert!(!matches_filter("A*B*C", "AxxCxxB"));
        assert!(matches_filter("*", ""));
        assert!(!matches_filter("A*A", "A"));
        assert!(matches_filter("**", "anything"));
    }

    #[test]
    fn time_units_and_delay() {
        assert_eq!(TimeUnit::Minutes.ticks(), 1200);
        let c = delay(2);
        assert_eq!(c.total_wait_ticks(), 40);
        let mut env = Env::default();
        let mut ctx = CondCtx::default();
        for _ in 0..40 {
            assert!(!c.tick(&mut env, &mut ctx));
        }
        assert!(c.tick(&mut env, &mut ctx));
    }

    #[test]
    fn time_of_day_targets() {
        // noon, every 24h => tick 6000
        assert_eq!(Condition::time_of_day_target(12, 0, 0), 6000);
        // 00:30 => 18500
        assert_eq!(Condition::time_of_day_target(0, 30, 0), 18500);
        // 06:00 => 0 (sunrise)
        assert_eq!(Condition::time_of_day_target(6, 0, 0), 0);
        // every 2h repeats modulo 2000
        assert_eq!(Condition::time_of_day_target(12, 0, 5), 0);
        assert_eq!(Condition::time_of_day_target(13, 15, 5), (7 * 1000 + 250 + 6000) % 2000);
        let c = Condition::TimeOfDay { hour: 12, minute: 0, rotation: 0 };
        let mut ctx = CondCtx::default();
        let mut env = Env { day: 5999, ..Default::default() };
        assert!(!c.tick(&mut env, &mut ctx));
        env.day = 6000;
        assert!(c.tick(&mut env, &mut ctx));
        env.day = 6040;
        assert!(c.tick(&mut env, &mut ctx));
        env.day = 6041;
        assert!(!c.tick(&mut env, &mut ctx));
        env.day = 24000 + 6010;
        assert!(c.tick(&mut env, &mut ctx));
        assert_eq!(Condition::clock_of(6000), (12, 0));
        assert_eq!(Condition::clock_of(18500), (0, 30));
        assert_eq!(Condition::rotation_ticks(9), 250);
        assert_eq!(Condition::rotation_ticks(99), 24000);
    }

    #[test]
    fn thresholds_and_misc_conditions() {
        let mut env = Env { items: 64, fluid: 3, players: 2, link: true, unloaded: true, ..Default::default() };
        let mut ctx = CondCtx::default();
        let item = |op, amount| Condition::ItemThreshold { filter: "x".into(), op, amount, measure: ItemMeasure::Items };
        assert!(item(Operator::Greater, 63).tick(&mut env, &mut ctx));
        assert!(!item(Operator::Greater, 64).tick(&mut env, &mut ctx));
        assert!(item(Operator::Equal, 64).tick(&mut env, &mut ctx));
        assert!(item(Operator::Less, 65).tick(&mut env, &mut ctx));
        let fl = Condition::FluidThreshold { filter: "water".into(), op: Operator::Less, buckets: 4 };
        assert!(fl.tick(&mut env, &mut ctx));
        let rl = Condition::RedstoneLink { frequency: ["a".into(), "b".into()], inverted: false };
        assert!(rl.tick(&mut env, &mut ctx));
        let rli = Condition::RedstoneLink { frequency: ["a".into(), "b".into()], inverted: true };
        assert!(!rli.tick(&mut env, &mut ctx));
        assert!(Condition::PlayerCount { count: 2, exact: true }.tick(&mut env, &mut ctx));
        assert!(!Condition::PlayerCount { count: 1, exact: true }.tick(&mut env, &mut ctx));
        assert!(Condition::PlayerCount { count: 1, exact: false }.tick(&mut env, &mut ctx));
        assert!(Condition::Unloaded.tick(&mut env, &mut ctx));
        assert!(!Condition::Powered.tick(&mut env, &mut ctx));
        assert_eq!(Condition::Powered.id(), "create:powered");
    }

    #[test]
    fn idle_cargo_resets_on_change() {
        let c = Condition::IdleCargo { value: 1, unit: TimeUnit::Seconds };
        let mut env = Env::default();
        let mut ctx = CondCtx::default();
        for _ in 0..10 {
            assert!(!c.tick(&mut env, &mut ctx));
        }
        env.changed = true;
        assert!(!c.tick(&mut env, &mut ctx));
        assert_eq!(ctx.time, 1);
        for _ in 0..19 {
            assert!(!c.tick(&mut env, &mut ctx));
        }
        assert!(c.tick(&mut env, &mut ctx));
    }

    fn sample(cyclic: bool) -> Schedule {
        Schedule {
            cyclic,
            saved_progress: 0,
            entries: vec![
                ScheduleEntry { instruction: Instruction::Destination { filter: "A*".into() }, conditions: vec![vec![delay(1)]] },
                ScheduleEntry { instruction: Instruction::Rename { text: "Express".into() }, conditions: vec![] },
                ScheduleEntry { instruction: Instruction::Throttle { percent: 50 }, conditions: vec![] },
                ScheduleEntry { instruction: Instruction::Destination { filter: "B".into() }, conditions: vec![vec![Condition::PlayerCount { count: 1, exact: false }], vec![delay(5)]] },
            ],
        }
    }

    #[test]
    fn runtime_walks_entries_and_completes() {
        let mut rt = ScheduleRuntime::default();
        let mut env = Env::default();
        rt.tick(&mut env); // paused: nothing
        assert!(env.started.is_empty());
        rt.set_schedule(sample(false), false);
        // PRE_TRANSIT -> starts navigation to A*
        rt.tick(&mut env);
        assert_eq!(env.started, vec!["A*".to_string()]);
        assert_eq!(rt.state, RuntimeState::InTransit);
        // train underway: only counts transit time
        env.destination = true;
        for _ in 0..5 {
            rt.tick(&mut env);
        }
        assert_eq!(rt.ticks_in_transit, 5);
        // arrival
        env.destination = false;
        rt.destination_reached();
        assert_eq!(rt.state, RuntimeState::PostTransit);
        assert_eq!(rt.progress, vec![0]);
        // 20 ticks of delay, +1 to register completion, +1 to advance
        let mut n = 0;
        while rt.state == RuntimeState::PostTransit && n < 100 {
            rt.tick(&mut env);
            n += 1;
        }
        assert_eq!(n, 22);
        assert_eq!(rt.current_entry, 1);
        assert_eq!(rt.cooldown, INTERVAL);
        // cooldown 40 ticks before the next instruction runs
        for _ in 0..(INTERVAL - 1) {
            rt.tick(&mut env);
        }
        assert!(env.title.is_none());
        rt.tick(&mut env);
        assert_eq!(env.title.as_deref(), Some("Express"));
        assert_eq!(rt.current_entry, 2);
        rt.tick(&mut env);
        assert_eq!(env.throttle, Some(0.5));
        assert_eq!(rt.current_entry, 3);
        // destination B with two alternative columns: players satisfy the first
        rt.tick(&mut env);
        assert_eq!(env.started.last().unwrap(), "B");
        rt.destination_reached();
        assert_eq!(rt.progress.len(), 2);
        env.players = 1;
        rt.tick(&mut env); // column 0 completes (progress 1)
        rt.tick(&mut env); // next tick: progress >= size -> advance
        assert_eq!(rt.current_entry, 4);
        // end reached: not cyclic -> paused + completed
        for _ in 0..(INTERVAL + 2) {
            rt.tick(&mut env);
        }
        assert!(rt.paused && rt.completed);
        assert_eq!(rt.current_entry, 0);
    }

    #[test]
    fn cyclic_wraps_and_interrupt_restarts() {
        let mut rt = ScheduleRuntime::default();
        let mut env = Env { start: Some(StartResult::AtDestination), ..Default::default() };
        let mut s = sample(true);
        s.entries.truncate(1);
        rt.set_schedule(s, true);
        assert!(rt.is_auto_schedule);
        rt.tick(&mut env);
        assert_eq!(rt.state, RuntimeState::PostTransit); // already there
        for _ in 0..30 {
            rt.tick(&mut env);
        }
        // the entry index ran past the end and wrapped (cyclic)
        assert_eq!(rt.current_entry, 0);
        assert!(!rt.paused && !rt.completed);
        // interrupted transit returns to PRE_TRANSIT with no cooldown
        env.start = Some(StartResult::Navigating);
        rt.state = RuntimeState::PreTransit;
        rt.cooldown = 0;
        rt.tick(&mut env);
        assert_eq!(rt.state, RuntimeState::InTransit);
        rt.transit_interrupted();
        assert_eq!(rt.state, RuntimeState::PreTransit);
        assert_eq!(rt.cooldown, 0);
        // no path: stays pre-transit and retries
        env.start = Some(StartResult::NoPath);
        let before = env.started.len();
        rt.tick(&mut env);
        rt.tick(&mut env);
        assert_eq!(env.started.len(), before + 2);
        assert_eq!(rt.state, RuntimeState::PreTransit);
        // derailed trains do nothing
        env.derailed = true;
        rt.tick(&mut env);
        assert_eq!(env.started.len(), before + 2);
    }

    #[test]
    fn sequential_conditions_in_one_column() {
        let mut rt = ScheduleRuntime::default();
        let mut env = Env::default();
        let s = Schedule {
            cyclic: false,
            saved_progress: 0,
            entries: vec![ScheduleEntry {
                instruction: Instruction::Destination { filter: "S".into() },
                conditions: vec![vec![delay(0), Condition::Powered]],
            }],
        };
        rt.set_schedule(s, false);
        rt.tick(&mut env);
        rt.destination_reached();
        rt.tick(&mut env); // delay 0 completes immediately
        assert_eq!(rt.progress, vec![1]);
        rt.tick(&mut env);
        assert_eq!(rt.progress, vec![1]); // waiting for power
        env.powered = true;
        rt.tick(&mut env);
        assert_eq!(rt.progress, vec![2]);
        rt.tick(&mut env);
        assert_eq!(rt.current_entry, 1);
        assert_eq!(Instruction::throttle_fraction(1), 0.05);
        assert_eq!(Instruction::throttle_fraction(500), 1.0);
        assert!(!Instruction::Rename { text: String::new() }.supports_conditions());
        assert_eq!(Instruction::Throttle { percent: 5 }.id(), "create:throttle");
    }
}
