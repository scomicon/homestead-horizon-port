// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Pure train movement after Create 6.0.10 content/trains/entity: TravellingPoint
// (edge navigation), Train (speed, passive slowdown, stress, crash), Carriage,
// and the braking/approach rules of Navigation.tick plus its path search.
// Collision geometry, fuel inventories and conductors belong to adapters.
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use crate::train_graph::{Dir, EdgePoint, NodeId, PointId, TrackEdge, TrackGraph};
use crate::train_track::{V3, cross, dist, dot, normalize, UP};

const EPS: f64 = 1e-6;

// ------------------------------------------------------------------- config

/// `CTrains` server configuration (blocks per second, acceleration in blocks/s^2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrainConfig {
    pub top_speed: f64,
    pub turning_top_speed: f64,
    pub acceleration: f64,
    pub powered_top_speed: f64,
    pub powered_turning_top_speed: f64,
    pub powered_acceleration: f64,
    pub max_assembly_length: i32,
    pub max_bogey_count: i32,
    pub trains_cause_damage: bool,
    pub manual_speed_modifier: f64,
}

impl Default for TrainConfig {
    fn default() -> Self {
        Self {
            top_speed: 28.0,
            turning_top_speed: 14.0,
            acceleration: 3.0,
            powered_top_speed: 40.0,
            powered_turning_top_speed: 20.0,
            powered_acceleration: 3.0,
            max_assembly_length: 128,
            max_bogey_count: 20,
            trains_cause_damage: true,
            manual_speed_modifier: 0.75,
        }
    }
}

/// `Train.Penalties` (navigation path costs).
pub mod penalties {
    pub const STATION: f64 = 50.0;
    pub const STATION_WITH_TRAIN: f64 = 300.0;
    pub const MANUAL_TRAIN: f64 = 200.0;
    pub const IDLE_TRAIN: f64 = 700.0;
    pub const ARRIVING_TRAIN: f64 = 50.0;
    pub const WAITING_TRAIN: f64 = 50.0;
    pub const ANY_TRAIN: f64 = 25.0;
    pub const RED_SIGNAL: f64 = 25.0;
    pub const REDSTONE_RED_SIGNAL: f64 = 400.0;
}

// ------------------------------------------------------------------ selectors

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Steer {
    None,
    Left,
    Right,
}

impl Steer {
    fn target_dot(self) -> f64 {
        match self {
            Steer::None => 0.0,
            Steer::Left => -1.0,
            Steer::Right => 1.0,
        }
    }
}

/// Chooses between several outgoing directed edges at a junction. `from` is the
/// directed edge being left; every candidate starts at `from.1`.
pub trait TrackSelector {
    fn pick(&mut self, graph: &TrackGraph, from: Dir, candidates: &[Dir]) -> usize;
}

/// Always the lowest-id branch.
pub struct FirstBranch;
impl TrackSelector for FirstBranch {
    fn pick(&mut self, _: &TrackGraph, _: Dir, _: &[Dir]) -> usize {
        0
    }
}

/// `SteerDirection` selector: branch whose turn direction is closest to the request.
pub struct SteerSelector(pub Steer);
impl TrackSelector for SteerSelector {
    fn pick(&mut self, graph: &TrackGraph, from: Dir, candidates: &[Dir]) -> usize {
        let Some(cur) = graph.edge(from.0, from.1) else { return 0 };
        let side = cross(cur.direction(false), UP);
        let target = self.0.target_dot();
        let mut best = (f64::MAX, 0usize);
        for (i, c) in candidates.iter().enumerate() {
            if let Some(e) = graph.edge(c.0, c.1) {
                let d = dot(side, e.direction(true));
                let score = (d - target).abs();
                if score < best.0 - 1e-12 {
                    best = (score, i);
                }
            }
        }
        best.1
    }
}

/// Deterministic xorshift random branch (`random()`).
pub struct RandomSelector(pub u64);
impl TrackSelector for RandomSelector {
    fn pick(&mut self, _: &TrackGraph, _: Dir, candidates: &[Dir]) -> usize {
        let mut x = self.0.max(1);
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x % candidates.len().max(1) as u64) as usize
    }
}

/// `follow(other)`: prefer the branch leading to `target`'s edge (bounded BFS, 20 rounds).
pub struct FollowSelector {
    pub target: Dir,
}
impl TrackSelector for FollowSelector {
    fn pick(&mut self, graph: &TrackGraph, _from: Dir, candidates: &[Dir]) -> usize {
        let hit = |n: NodeId| n == self.target.0 || n == self.target.1;
        for (i, c) in candidates.iter().enumerate() {
            if hit(c.1) {
                return i;
            }
        }
        for (i, c) in candidates.iter().enumerate() {
            let mut frontier = vec![c.1];
            let mut seen: HashSet<NodeId> = HashSet::new();
            for _ in 0..20 {
                let mut next = Vec::new();
                for n in &frontier {
                    if !seen.insert(*n) {
                        continue;
                    }
                    for (m, _) in graph.neighbours(*n) {
                        if hit(m) {
                            return i;
                        }
                        next.push(m);
                    }
                }
                if next.is_empty() {
                    break;
                }
                frontier = next;
            }
        }
        0
    }
}

/// Follows a precomputed navigation path of directed edges.
pub struct PathSelector<'a>(pub &'a [Dir]);
impl TrackSelector for PathSelector<'_> {
    fn pick(&mut self, _: &TrackGraph, from: Dir, candidates: &[Dir]) -> usize {
        if let Some(i) = self.0.iter().position(|d| *d == from) {
            if let Some(next) = self.0.get(i + 1) {
                if let Some(k) = candidates.iter().position(|c| c == next) {
                    return k;
                }
            }
        }
        0
    }
}

// -------------------------------------------------------------- travelling point

/// Callback: `(distance travelled so far, point, directed edge)`; true stops travel.
pub type PointListener<'a> = &'a mut dyn FnMut(f64, &EdgePoint, Dir) -> bool;
/// Callback: distance driven on a curved edge, invoked per curved segment.
pub type TurnListener<'a> = &'a mut dyn FnMut(f64, &TrackEdge);

#[derive(Default)]
pub struct TravelHooks<'a> {
    pub on_point: Option<PointListener<'a>>,
    pub on_turn: Option<TurnListener<'a>>,
}

/// A point riding a graph edge (`TravellingPoint`). `position` is the distance
/// from `node1` along the directed edge `node1 -> node2`.
#[derive(Clone, Debug, PartialEq)]
pub struct TravellingPoint {
    pub node1: NodeId,
    pub node2: NodeId,
    pub position: f64,
    pub upside_down: bool,
    pub blocked: bool,
}

impl TravellingPoint {
    pub fn new(node1: NodeId, node2: NodeId, position: f64) -> Self {
        Self { node1, node2, position, upside_down: false, blocked: false }
    }

    pub fn dir(&self) -> Dir {
        (self.node1, self.node2)
    }

    /// `reverse()`: look the other way along the same spot.
    pub fn reverse(&mut self, graph: &TrackGraph) {
        let l = graph.edge(self.node1, self.node2).map(|e| e.length()).unwrap_or(0.0);
        std::mem::swap(&mut self.node1, &mut self.node2);
        self.position = l - self.position;
    }

    pub fn world_position(&self, graph: &TrackGraph) -> Option<V3> {
        graph.edge(self.node1, self.node2).map(|e| e.position(self.position))
    }

    pub fn travel(&mut self, graph: &TrackGraph, distance: f64, selector: &mut dyn TrackSelector, hooks: &mut TravelHooks) -> f64 {
        self.blocked = false;
        if distance.abs() < EPS {
            return 0.0;
        }
        if graph.edge(self.node1, self.node2).is_none() {
            return 0.0;
        }
        let forward = distance > 0.0;
        let mut remaining = distance.abs();
        let mut travelled = 0.0;
        let mut include_start = false;
        for _ in 0..10_000 {
            let edge = graph.edge(self.node1, self.node2).unwrap();
            let len = edge.length();
            let target = if forward { len } else { 0.0 };
            let room = (target - self.position).abs();
            let step = remaining.min(room);
            let to = if forward { self.position + step } else { self.position - step };

            // edge points inside the covered range
            if let Some(cb) = hooks.on_point.as_mut() {
                let list = graph.directed_points(self.node1, self.node2);
                let iter: Box<dyn Iterator<Item = &(f64, &EdgePoint)>> =
                    if forward { Box::new(list.iter()) } else { Box::new(list.iter().rev()) };
                for (pos, p) in iter {
                    let inside = if forward {
                        let lower = if include_start { *pos >= self.position - 1e-9 } else { *pos > self.position + 1e-9 };
                        lower && *pos <= to + 1e-9
                    } else {
                        let upper = if include_start { *pos <= self.position + 1e-9 } else { *pos < self.position - 1e-9 };
                        upper && *pos >= to - 1e-9
                    };
                    if !inside {
                        continue;
                    }
                    let d = travelled + (pos - self.position).abs();
                    if cb(d, p, (self.node1, self.node2)) {
                        let hit = *pos;
                        travelled += (hit - self.position).abs();
                        self.position = hit;
                        return if forward { travelled } else { -travelled };
                    }
                }
            }
            if let (Some(cb), true) = (hooks.on_turn.as_mut(), edge.is_turn()) {
                cb(step, edge);
            }
            self.position = to;
            travelled += step;
            remaining -= step;
            if remaining <= EPS {
                break;
            }

            // Past the node: choose the next edge.
            let from: Dir = if forward { (self.node1, self.node2) } else { (self.node2, self.node1) };
            let pivot = from.1;
            let mut cands: Vec<Dir> = Vec::new();
            let arriving = graph.edge(from.0, from.1).unwrap();
            for (c, e) in graph.neighbours(pivot) {
                if c == from.0 {
                    continue;
                }
                if !arriving.can_travel_to(e) {
                    continue;
                }
                cands.push((pivot, c));
            }
            if cands.is_empty() {
                self.blocked = true;
                break;
            }
            let pick = if cands.len() > 1 { selector.pick(graph, from, &cands).min(cands.len() - 1) } else { 0 };
            let next = cands[pick];
            let next_len = graph.edge(next.0, next.1).unwrap().length();
            if forward {
                self.node1 = next.0;
                self.node2 = next.1;
                self.position = 0.0;
            } else {
                self.node1 = next.1;
                self.node2 = next.0;
                self.position = next_len;
            }
            include_start = true;
        }
        if forward { travelled } else { -travelled }
    }
}

// ------------------------------------------------------------------- carriage

#[derive(Clone, Debug, PartialEq)]
pub struct Carriage {
    pub leading: TravellingPoint,
    pub trailing: TravellingPoint,
    /// Distance between the two bogeys.
    pub bogey_spacing: f64,
    pub stalled: bool,
}

// ---------------------------------------------------------------- navigation

#[derive(Clone, Debug, PartialEq)]
pub struct Navigation {
    pub destination: Option<u64>,
    pub distance_to_destination: f64,
    pub distance_to_signal: f64,
    pub waiting_for_signal: Option<(PointId, usize)>,
    pub ticks_waiting_for_signal: u32,
    pub destination_behind_train: bool,
    pub path: Vec<Dir>,
}

impl Default for Navigation {
    fn default() -> Self {
        Self {
            destination: None,
            distance_to_destination: 0.0,
            distance_to_signal: f64::MAX,
            waiting_for_signal: None,
            ticks_waiting_for_signal: 0,
            destination_behind_train: false,
            path: Vec::new(),
        }
    }
}

/// Scouting results the adapter computes by travelling a probe point ahead.
#[derive(Clone, Copy, Debug, Default)]
pub struct NavInput {
    pub at_station: bool,
    /// Distance to the nearest curve ahead, if any (`distanceToNextCurve`).
    pub next_curve_distance: Option<f64>,
    /// A reserved/occupied signal is pending ahead (pre-departure hold).
    pub signal_pending: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavResult {
    NoDestination,
    Cancelled,
    /// Held at the station while a signal ahead is pending.
    Hold,
    LeaveStation,
    Snapped,
    Approaching,
    Cruising,
}

impl Navigation {
    pub fn cancel(&mut self) {
        self.distance_to_destination = 0.0;
        self.path.clear();
        self.destination = None;
        self.waiting_for_signal = None;
        self.distance_to_signal = f64::MAX;
        self.ticks_waiting_for_signal = 0;
    }

    /// Speed control half of `Navigation.tick` (signal scouting is the caller's job).
    pub fn tick_speed(&mut self, train: &mut Train, input: &NavInput) -> NavResult {
        if self.destination.is_none() {
            return NavResult::NoDestination;
        }
        let acceleration = train.acceleration();
        let braking = train.speed * train.speed / (2.0 * acceleration);
        let speed_mod = if self.destination_behind_train { -1.0 } else { 1.0 };
        let pre_departure = if input.at_station { 4.5 } else { 0.0 };

        if self.waiting_for_signal.is_none() {
            self.distance_to_signal = f64::MAX;
            self.ticks_waiting_for_signal = 0;
        } else if self.distance_to_signal <= 1.0 / 16.0 {
            self.ticks_waiting_for_signal += 1;
        }

        let mut target_distance =
            if self.waiting_for_signal.is_some() { self.distance_to_signal } else { self.distance_to_destination };
        target_distance += 0.25;

        let mut leave = false;
        if target_distance > 1.0 / 32.0 && input.at_station {
            if input.signal_pending && self.distance_to_signal < pre_departure {
                return NavResult::Hold;
            }
            leave = true;
        }
        train.currently_backwards = self.destination_behind_train;

        if target_distance < -10.0 {
            self.cancel();
            return NavResult::Cancelled;
        }
        if target_distance - train.speed.abs() < 1.0 / 32.0 {
            train.speed = target_distance.max(1.0 / 32.0) * speed_mod;
            return NavResult::Snapped;
        }

        let mut top = train.max_speed();
        if target_distance < 10.0 {
            let max_approach = top * (target_distance / 10.0);
            if train.speed.abs() > max_approach {
                train.speed += (max_approach - train.speed.abs()) * 0.5 * speed_mod;
                return NavResult::Approaching;
            }
        }
        top *= train.throttle;
        let turn_top = top.min(train.max_turn_speed());
        let mut target = if target_distance > braking { top * speed_mod } else { 0.0 };
        if let Some(curve) = input.next_curve_distance {
            let slowing = braking - turn_top * turn_top / (2.0 * acceleration);
            let target_turn = if curve > slowing { top * speed_mod } else { turn_top * speed_mod };
            if target_turn.abs() < target.abs() {
                target = target_turn;
            }
        }
        train.target_speed = target;
        train.approach_target_speed(1.0);
        if leave { NavResult::LeaveStation } else { NavResult::Cruising }
    }
}

/// Which navigation refreshes `updateNavigationTarget` asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Refresh {
    pub full: bool,
    pub partial: bool,
    pub signal: bool,
}

// ------------------------------------------------------------------- train

#[derive(Clone, Debug, PartialEq)]
pub struct Train {
    pub id: u64,
    pub config: TrainConfig,
    pub carriages: Vec<Carriage>,
    /// Distance between consecutive carriages (bogey to bogey), length `carriages - 1`.
    pub carriage_spacing: Vec<f64>,
    pub speed: f64,
    pub target_speed: f64,
    pub throttle: f64,
    pub fuel_ticks: i32,
    pub derailed: bool,
    pub on_graph: bool,
    pub manual_tick: bool,
    pub currently_backwards: bool,
    pub speed_before_stall: Option<f64>,
    pub navigation: Navigation,
    pub stress: Vec<f64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TickReport {
    pub travelled: f64,
    pub blocked: bool,
    pub derailed_now: bool,
    pub max_stress: f64,
    pub refresh: Refresh,
}

impl Train {
    pub fn new(id: u64, config: TrainConfig, carriages: Vec<Carriage>, carriage_spacing: Vec<f64>) -> Self {
        let n = carriage_spacing.len();
        Self {
            id,
            config,
            carriages,
            carriage_spacing,
            speed: 0.0,
            target_speed: 0.0,
            throttle: 1.0,
            fuel_ticks: 0,
            derailed: false,
            on_graph: true,
            manual_tick: false,
            currently_backwards: false,
            speed_before_stall: None,
            navigation: Navigation::default(),
            stress: vec![0.0; n],
        }
    }

    /// blocks per tick
    pub fn max_speed(&self) -> f64 {
        (if self.fuel_ticks > 0 { self.config.powered_top_speed } else { self.config.top_speed }) / 20.0
    }
    pub fn max_turn_speed(&self) -> f64 {
        (if self.fuel_ticks > 0 { self.config.powered_turning_top_speed } else { self.config.turning_top_speed }) / 20.0
    }
    /// blocks per tick^2
    pub fn acceleration(&self) -> f64 {
        (if self.fuel_ticks > 0 { self.config.powered_acceleration } else { self.config.acceleration }) / 400.0
    }

    /// `tickPassiveSlowdown`.
    pub fn tick_passive_slowdown(&mut self) {
        if !self.manual_tick && self.navigation.destination.is_none() && self.speed != 0.0 {
            let a = self.acceleration();
            self.speed = if self.speed > 0.0 { (self.speed - a).max(0.0) } else { (self.speed + a).min(0.0) };
        }
        self.manual_tick = false;
    }

    /// `tickDerailedSlowdown`.
    pub fn tick_derailed_slowdown(&mut self) {
        self.speed /= 3.0;
        if self.speed.abs() < EPS {
            self.speed = 0.0;
        }
    }

    /// `approachTargetSpeed`; returns true when a manual driver asks to leave the station.
    pub fn approach_target_speed(&mut self, acceleration_mod: f64) -> bool {
        if (self.target_speed - self.speed).abs() < EPS {
            return false;
        }
        let leave = self.manual_tick;
        let a = self.acceleration() * acceleration_mod;
        if self.speed < self.target_speed {
            self.speed = (self.speed + a).min(self.target_speed);
        } else if self.speed > self.target_speed {
            self.speed = (self.speed - a).max(self.target_speed);
        }
        leave
    }

    /// `crash()`.
    pub fn crash(&mut self) {
        self.navigation.cancel();
        if self.derailed {
            return;
        }
        self.speed = -self.speed.clamp(-0.5, 0.5);
        self.derailed = true;
        self.on_graph = false;
    }

    /// Explosion power for a train-train collision (`collideWithOtherTrains`), if any.
    pub fn collision_power(speed_a: f64, speed_b: f64) -> Option<f64> {
        let combined = speed_a.abs() + speed_b.abs();
        if combined > 0.2 { Some((3.0 * combined).min(5.0)) } else { None }
    }

    /// Arrival at the destination station (front listener).
    pub fn arrive(&mut self) {
        self.speed = 0.0;
        self.navigation.distance_to_destination = 0.0;
    }

    /// `updateNavigationTarget` distance bookkeeping and refresh triggers.
    pub fn update_navigation_target(&mut self, distance: f64) -> Refresh {
        let nav = &mut self.navigation;
        if nav.destination.is_none() {
            return Refresh::default();
        }
        let sub = if nav.destination_behind_train { -distance } else { distance };
        let before = nav.distance_to_destination;
        let sig_before = nav.distance_to_signal;
        nav.distance_to_destination -= sub;
        let waiting = nav.waiting_for_signal.is_some();
        if waiting {
            nav.distance_to_signal -= sub;
        }
        let after = nav.distance_to_destination;
        Refresh {
            full: before > 100.0 && before % 100.0 > 20.0 && after % 100.0 <= 20.0,
            partial: before < 100.0 && before % 50.0 > 5.0 && after % 50.0 <= 5.0,
            signal: waiting && sig_before % 50.0 > 5.0 && nav.distance_to_signal % 50.0 < 5.0,
        }
    }

    /// `getNavigationPenalty` used by other trains' path searches.
    pub fn navigation_penalty(&self, has_active_schedule: bool, ticks_waiting_for_signal: u32) -> f64 {
        let n = &self.navigation;
        if self.manual_tick {
            return penalties::MANUAL_TRAIN;
        }
        if !has_active_schedule {
            return penalties::IDLE_TRAIN;
        }
        if n.waiting_for_signal.is_some() && ticks_waiting_for_signal > 0 {
            return penalties::WAITING_TRAIN + (ticks_waiting_for_signal / 20).min(1000) as f64;
        }
        if (n.destination.is_some() && n.distance_to_destination < 50.0) || n.distance_to_signal < 20.0 {
            return penalties::ARRIVING_TRAIN;
        }
        penalties::ANY_TRAIN
    }

    /// Distances/stress between carriages (`Train.tick`, stress loop).
    pub fn compute_stress(&mut self, graph: &TrackGraph) -> f64 {
        let mut max = 0.0f64;
        for i in 1..self.carriages.len() {
            let prev = &self.carriages[i - 1];
            let cur = &self.carriages[i];
            let (Some(a), Some(b)) = (prev.trailing.world_position(graph), cur.leading.world_position(graph)) else {
                continue;
            };
            let mut d = dist(a, b);
            if prev.trailing.upside_down != cur.leading.upside_down {
                d = (d * d - 4.0).max(0.0).sqrt();
            }
            let s = self.carriage_spacing[i - 1] - d;
            self.stress[i - 1] = s;
            max = max.max(s.abs());
        }
        for c in &self.carriages {
            if let (Some(a), Some(b)) = (c.leading.world_position(graph), c.trailing.world_position(graph)) {
                max = max.max((c.bogey_spacing - dist(a, b)).abs());
            }
        }
        max
    }

    /// One server tick of `Train.tick` (movement part). The navigation speed
    /// control must already have run (`Navigation::tick_speed`).
    pub fn tick(&mut self, graph: &TrackGraph, selector: &mut dyn TrackSelector, mut front: Option<PointListener>) -> TickReport {
        let mut report = TickReport::default();
        if !self.on_graph {
            self.tick_passive_slowdown();
            if self.derailed {
                self.tick_derailed_slowdown();
            }
            return report;
        }
        self.tick_passive_slowdown();
        if self.derailed {
            self.tick_derailed_slowdown();
        }
        let mut distance = self.speed;
        let max_stress_pre = self.compute_stress(graph);
        let mut max_stress = max_stress_pre;

        let any_stalled = self.carriages.iter().any(|c| c.stalled);
        if any_stalled {
            if self.speed_before_stall.is_none() {
                self.speed_before_stall = Some(self.speed);
            }
            distance = 0.0;
            self.speed = 0.0;
        } else if let Some(s) = self.speed_before_stall.take() {
            self.speed = s.clamp(-1.0, 1.0);
            distance = self.speed;
        }

        let approaching = self.navigation.distance_to_destination < 5.0 && self.navigation.destination.is_some();
        let leading_mod = if approaching { 0.75 } else { 0.5 };
        let trailing_mod = if approaching { 0.0 } else { 0.125 };
        let from_back = self.speed < 0.0;
        let n = self.carriages.len();
        let mut blocked = false;
        let mut front_travelled = 0.0;

        // Per-carriage stress share.
        let totals: Vec<f64> = (0..n)
            .map(|i| {
                let lead = if i == 0 { 0.0 } else { self.stress[i - 1] * -(if from_back { trailing_mod } else { leading_mod }) };
                let trail = if i == self.stress.len() { 0.0 } else { self.stress[i] * (if from_back { leading_mod } else { trailing_mod }) };
                if self.derailed { 0.0 } else { lead + trail }
            })
            .collect();

        // Flat point order, travelled from the front of motion towards the back.
        let order: Vec<(usize, bool)> = if from_back {
            (0..n).rev().flat_map(|i| [(i, false), (i, true)]).collect() // trailing then leading
        } else {
            (0..n).flat_map(|i| [(i, true), (i, false)]).collect()
        };
        let mut prev_dir: Option<Dir> = None;
        for (k, (ci, is_leading)) in order.iter().enumerate() {
            let d = distance + totals[*ci];
            let is_front = k == 0;
            let mut sel_follow;
            let sel: &mut dyn TrackSelector = match (is_front, prev_dir) {
                (false, Some(t)) => {
                    sel_follow = FollowSelector { target: t };
                    &mut sel_follow
                }
                _ => &mut *selector,
            };
            let point = if *is_leading { &mut self.carriages[*ci].leading } else { &mut self.carriages[*ci].trailing };
            let travelled = if is_front {
                let mut hooks = TravelHooks { on_point: front.as_deref_mut().map(|f| f as PointListener), on_turn: None };
                point.travel(graph, d, sel, &mut hooks)
            } else {
                let mut hooks = TravelHooks::default();
                point.travel(graph, d, sel, &mut hooks)
            };
            if is_front {
                front_travelled = travelled;
                // A stop at an edge point or a dead end limits the whole train.
                distance = travelled - totals[*ci];
            }
            blocked |= point.blocked;
            prev_dir = Some(point.dir());
        }
        report.travelled = front_travelled;
        let after = self.compute_stress(graph);
        max_stress = max_stress.max(after);

        if blocked {
            self.speed = 0.0;
            self.navigation.cancel();
        } else if max_stress > 4.0 {
            self.speed = 0.0;
            self.navigation.cancel();
            self.derailed = true;
            report.derailed_now = true;
        }
        report.blocked = blocked;
        report.max_stress = max_stress;
        report.refresh = self.update_navigation_target(front_travelled);
        report
    }
}

// ---------------------------------------------------------------- path search

#[derive(Clone, Debug, PartialEq)]
pub struct PathResult {
    /// Directed edges from the start edge to the goal edge (inclusive).
    pub edges: Vec<Dir>,
    /// Distance from the origin to the start of the goal edge.
    pub distance: f64,
    pub cost: f64,
}

struct Entry {
    cost: f64,
    seq: u64,
    edge: Dir,
    dist_at_end: f64,
}
impl PartialEq for Entry {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for Entry {}
impl PartialOrd for Entry {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Entry {
    fn cmp(&self, o: &Self) -> Ordering {
        o.cost.total_cmp(&self.cost).then(o.seq.cmp(&self.seq))
    }
}

/// Weight sequence for occupied signals along a path (`clamp(ticks*2, 25, 200)`, halving).
pub struct SignalWeight(f64);
impl SignalWeight {
    pub fn new(ticks_waiting: u32) -> Self {
        Self((ticks_waiting as f64 * 2.0).clamp(penalties::RED_SIGNAL, 200.0))
    }
    pub fn next(&mut self) -> f64 {
        let w = self.0;
        self.0 /= 2.0;
        w
    }
}

/// Uniform-cost search over directed edges (`Navigation.search`, no heuristic).
/// `start_remaining` is the distance from the point to the end of `start`.
/// `penalty(edge)` is the extra cost for entering an edge; `max_cost < 0` disables
/// cost pruning. Returns the cheapest path to an edge accepted by `goal`.
pub fn find_path(
    graph: &TrackGraph,
    start: Dir,
    start_remaining: f64,
    goal: &mut dyn FnMut(Dir) -> bool,
    penalty: &mut dyn FnMut(Dir) -> f64,
    max_distance: f64,
    max_cost: f64,
) -> Option<PathResult> {
    graph.edge(start.0, start.1)?;
    let mut heap = BinaryHeap::new();
    let mut seq = 0u64;
    let mut parent: HashMap<Dir, Option<Dir>> = HashMap::new();
    let mut best: HashMap<Dir, (f64, f64)> = HashMap::new(); // dist_at_end, cost_before
    heap.push(Entry { cost: start_remaining, seq, edge: start, dist_at_end: start_remaining });
    parent.insert(start, None);
    best.insert(start, (start_remaining, 0.0));
    let mut done: HashSet<Dir> = HashSet::new();
    while let Some(Entry { cost, edge, dist_at_end, .. }) = heap.pop() {
        if !done.insert(edge) {
            continue;
        }
        let e = graph.edge(edge.0, edge.1).unwrap();
        let d_start = dist_at_end - e.length();
        if goal(edge) {
            let mut edges = vec![edge];
            let mut cur = edge;
            while let Some(Some(p)) = parent.get(&cur) {
                edges.push(*p);
                cur = *p;
            }
            edges.reverse();
            return Some(PathResult { edges, distance: d_start.max(0.0), cost });
        }
        for (c, e2) in graph.neighbours(edge.1) {
            if c == edge.0 || !e.can_travel_to(e2) {
                continue;
            }
            let next: Dir = (edge.1, c);
            if done.contains(&next) {
                continue;
            }
            let nd = dist_at_end + e2.length();
            if nd - e2.length() > max_distance {
                continue;
            }
            let pen = penalty(next);
            let ncost = cost + e2.length() + pen;
            if max_cost >= 0.0 && ncost - e2.length() > max_cost {
                continue;
            }
            let better = best.get(&next).map_or(true, |(_, bc)| ncost < *bc);
            if better {
                best.insert(next, (nd, ncost));
                parent.insert(next, Some(edge));
                seq += 1;
                heap.push(Entry { cost: ncost, seq, edge: next, dist_at_end: nd });
            }
        }
    }
    None
}

/// Unit helper for adapters: normalised direction between two points.
pub fn heading(a: V3, b: V3) -> V3 {
    normalize([b[0] - a[0], b[1] - a[1], b[2] - a[2]])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::train_graph::PointKind;
    use crate::train_track::TrackType;

    fn chain(n: usize, step: f64) -> (TrackGraph, Vec<NodeId>) {
        let mut g = TrackGraph::new(1);
        let ids: Vec<NodeId> = (0..n).map(|i| g.add_node(0, [0.0, 0.0, i as f64 * step], UP)).collect();
        for w in ids.windows(2) {
            g.connect(w[0], w[1], TrackType::standard(), None);
        }
        (g, ids)
    }

    #[test]
    fn config_units() {
        let t = Train::new(1, TrainConfig::default(), vec![], vec![]);
        assert!((t.max_speed() - 1.4).abs() < 1e-12);
        assert!((t.max_turn_speed() - 0.7).abs() < 1e-12);
        assert!((t.acceleration() - 0.0075).abs() < 1e-12);
        let mut p = t.clone();
        p.fuel_ticks = 10;
        assert!((p.max_speed() - 2.0).abs() < 1e-12);
    }

    #[test]
    fn travel_forward_across_nodes_and_back() {
        let (g, ids) = chain(4, 10.0);
        let mut p = TravellingPoint::new(ids[0], ids[1], 5.0);
        let d = p.travel(&g, 17.0, &mut FirstBranch, &mut TravelHooks::default());
        assert!((d - 17.0).abs() < 1e-9);
        assert_eq!((p.node1, p.node2), (ids[2], ids[3]));
        assert!((p.position - 2.0).abs() < 1e-9);
        let d = p.travel(&g, -14.0, &mut FirstBranch, &mut TravelHooks::default());
        assert!((d + 14.0).abs() < 1e-9);
        assert_eq!((p.node1, p.node2), (ids[0], ids[1]));
        assert!((p.position - 8.0).abs() < 1e-9);
        assert_eq!(p.travel(&g, 0.0, &mut FirstBranch, &mut TravelHooks::default()), 0.0);
    }

    #[test]
    fn travel_blocked_at_dead_end() {
        let (g, ids) = chain(2, 10.0);
        let mut p = TravellingPoint::new(ids[0], ids[1], 5.0);
        let d = p.travel(&g, 20.0, &mut FirstBranch, &mut TravelHooks::default());
        assert!((d - 5.0).abs() < 1e-9);
        assert!(p.blocked);
        assert!((p.position - 10.0).abs() < 1e-9);
        let d = p.travel(&g, -30.0, &mut FirstBranch, &mut TravelHooks::default());
        assert!((d + 10.0).abs() < 1e-9);
        assert!(p.blocked && p.position == 0.0);
    }

    fn junction() -> (TrackGraph, [NodeId; 4]) {
        // a(0,0,0) -> b(0,0,10) -> c(0,0,20) straight, and a gentle branch b -> d to the right (-x)
        let mut g = TrackGraph::new(1);
        let a = g.add_node(0, [0.0, 0.0, 0.0], UP);
        let b = g.add_node(0, [0.0, 0.0, 10.0], UP);
        let c = g.add_node(0, [0.0, 0.0, 20.0], UP);
        let d = g.add_node(0, [-4.0, 0.0, 20.0], UP);
        for (x, y) in [(a, b), (b, c), (b, d)] {
            g.connect(x, y, TrackType::standard(), None);
        }
        (g, [a, b, c, d])
    }

    #[test]
    fn junction_filters_by_alignment_and_steer() {
        let (g, [a, b, c, d]) = junction();
        // straight vs branch (-x,+z direction ~ 21.8 degrees: dot ~0.93 > 7/8): both candidates
        let mut p = TravellingPoint::new(a, b, 8.0);
        p.travel(&g, 4.0, &mut SteerSelector(Steer::None), &mut TravelHooks::default());
        assert_eq!((p.node1, p.node2), (b, c));
        // facing +z, right-hand side is -x: Right picks the branch
        let mut p = TravellingPoint::new(a, b, 8.0);
        p.travel(&g, 4.0, &mut SteerSelector(Steer::Right), &mut TravelHooks::default());
        assert_eq!((p.node1, p.node2), (b, d));
        let mut p = TravellingPoint::new(a, b, 8.0);
        p.travel(&g, 4.0, &mut SteerSelector(Steer::Left), &mut TravelHooks::default());
        assert_eq!((p.node1, p.node2), (b, c));
    }

    #[test]
    fn follow_selector_prefers_target_branch() {
        let (g, [a, b, _c, d]) = junction();
        let mut p = TravellingPoint::new(a, b, 8.0);
        p.travel(&g, 4.0, &mut FollowSelector { target: (b, d) }, &mut TravelHooks::default());
        assert_eq!((p.node1, p.node2), (b, d));
        let (cands_sel, _) = (PathSelector(&[(a, b), (b, d)]), 0);
        let mut p = TravellingPoint::new(a, b, 8.0);
        let mut s = cands_sel;
        p.travel(&g, 4.0, &mut s, &mut TravelHooks::default());
        assert_eq!((p.node1, p.node2), (b, d));
    }

    #[test]
    fn point_listener_stops_travel() {
        let (mut g, ids) = chain(3, 10.0);
        g.add_point(ids[1], ids[2], EdgePoint { id: 9, kind: PointKind::Station, pos: 4.0 });
        let mut hits = Vec::new();
        let mut p = TravellingPoint::new(ids[0], ids[1], 2.0);
        let mut cb = |d: f64, pt: &EdgePoint, e: Dir| {
            hits.push((d, pt.id, e));
            true
        };
        let d = p.travel(&g, 30.0, &mut FirstBranch, &mut TravelHooks { on_point: Some(&mut cb), on_turn: None });
        assert!((d - 12.0).abs() < 1e-9); // 8 + 4
        assert_eq!((p.node1, p.node2), (ids[1], ids[2]));
        assert!((p.position - 4.0).abs() < 1e-9);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].1, 9);
        // backwards over the same point from the other end
        let mut q = TravellingPoint::new(ids[1], ids[2], 8.0);
        let mut seen = 0;
        let mut cb2 = |_: f64, _: &EdgePoint, _: Dir| {
            seen += 1;
            false
        };
        q.travel(&g, -6.0, &mut FirstBranch, &mut TravelHooks { on_point: Some(&mut cb2), on_turn: None });
        assert_eq!(seen, 1);
    }

    #[test]
    fn point_at_edge_start_is_not_missed_after_hop() {
        let (mut g, ids) = chain(3, 10.0);
        g.add_point(ids[1], ids[2], EdgePoint { id: 4, kind: PointKind::Signal, pos: 0.0 });
        let mut n = 0;
        let mut cb = |_: f64, _: &EdgePoint, _: Dir| {
            n += 1;
            false
        };
        let mut p = TravellingPoint::new(ids[0], ids[1], 9.0);
        p.travel(&g, 3.0, &mut FirstBranch, &mut TravelHooks { on_point: Some(&mut cb), on_turn: None });
        assert_eq!(n, 1);
    }

    #[test]
    fn turn_listener_counts_curve_distance() {
        let mut g = TrackGraph::new(1);
        let a = g.add_node(0, [0.0; 3], UP);
        let b = g.add_node(0, [8.0, 0.0, 8.0], UP);
        let bz = crate::train_track::BezierConnection::new([0.0; 3], [8.0, 0.0, 8.0], [0.0, 0.0, 1.0], [-1.0, 0.0, 0.0], UP, UP, false);
        let len = bz.length();
        g.connect(a, b, TrackType::standard(), Some(bz));
        let mut turned = 0.0;
        let mut cb = |d: f64, _: &TrackEdge| turned += d;
        let mut p = TravellingPoint::new(a, b, 0.0);
        p.travel(&g, 5.0, &mut FirstBranch, &mut TravelHooks { on_point: None, on_turn: Some(&mut cb) });
        assert!((turned - 5.0).abs() < 1e-9);
        assert!((p.position - 5.0).abs() < 1e-9 && len > 5.0);
        let w = p.world_position(&g).unwrap();
        assert!(w[2] > 3.0);
    }

    #[test]
    fn passive_slowdown_and_derail() {
        let mut t = Train::new(1, TrainConfig::default(), vec![], vec![]);
        t.speed = 0.02;
        t.tick_passive_slowdown();
        assert!((t.speed - 0.0125).abs() < 1e-12);
        t.tick_passive_slowdown();
        t.tick_passive_slowdown();
        assert_eq!(t.speed, 0.0);
        t.speed = -0.01;
        t.tick_passive_slowdown();
        assert!((t.speed + 0.0025).abs() < 1e-12);
        t.manual_tick = true;
        t.tick_passive_slowdown(); // manual tick suppresses once
        assert!((t.speed + 0.0025).abs() < 1e-12);
        t.speed = 0.9;
        t.crash();
        assert!(t.derailed && !t.on_graph);
        assert!((t.speed + 0.5).abs() < 1e-12);
        t.speed = 0.9;
        t.tick_derailed_slowdown();
        assert!((t.speed - 0.3).abs() < 1e-12);
        assert_eq!(Train::collision_power(0.05, 0.05), None);
        assert_eq!(Train::collision_power(1.0, 1.0), Some(5.0));
        assert!((Train::collision_power(0.1, 0.2).unwrap() - 0.9).abs() < 1e-12);
    }

    #[test]
    fn approach_target_speed_steps() {
        let mut t = Train::new(1, TrainConfig::default(), vec![], vec![]);
        t.target_speed = 1.0;
        t.approach_target_speed(1.0);
        assert!((t.speed - 0.0075).abs() < 1e-12);
        t.speed = 0.999;
        t.approach_target_speed(1.0);
        assert_eq!(t.speed, 1.0);
        t.target_speed = 0.0;
        t.approach_target_speed(2.0);
        assert!((t.speed - 0.985).abs() < 1e-12);
    }

    #[test]
    fn navigation_speed_rules() {
        let mut t = Train::new(1, TrainConfig::default(), vec![], vec![]);
        let mut nav = Navigation { destination: Some(1), distance_to_destination: 200.0, ..Default::default() };
        let inp = NavInput::default();
        // far away: accelerate towards 1.4
        assert_eq!(nav.tick_speed(&mut t, &inp), NavResult::Cruising);
        assert!((t.target_speed - 1.4).abs() < 1e-12);
        assert!((t.speed - 0.0075).abs() < 1e-12);
        // within 10: approach cap 1.4 * (5.25/10) = 0.735
        t.speed = 1.0;
        nav.distance_to_destination = 5.0;
        assert_eq!(nav.tick_speed(&mut t, &inp), NavResult::Approaching);
        assert!((t.speed - (1.0 + (0.735 - 1.0) * 0.5)).abs() < 1e-12);
        // snap
        t.speed = 0.5;
        nav.distance_to_destination = 0.0;
        assert_eq!(nav.tick_speed(&mut t, &inp), NavResult::Snapped);
        assert!((t.speed - 0.25).abs() < 1e-12);
        // braking: v=1.0 -> 66.67 blocks; target 40 < braking -> target speed 0
        let mut t = Train::new(1, TrainConfig::default(), vec![], vec![]);
        t.speed = 1.0;
        let mut nav = Navigation { destination: Some(1), distance_to_destination: 40.0, ..Default::default() };
        nav.tick_speed(&mut t, &inp);
        assert_eq!(t.target_speed, 0.0);
        assert!((t.speed - (1.0 - 0.0075)).abs() < 1e-12);
        // curve ahead: slowing distance = braking - 0.49/(2*0.0075) ~ 34.0 for v=1.0
        let mut t = Train::new(1, TrainConfig::default(), vec![], vec![]);
        t.speed = 1.0;
        let mut nav = Navigation { destination: Some(1), distance_to_destination: 500.0, ..Default::default() };
        nav.tick_speed(&mut t, &NavInput { next_curve_distance: Some(10.0), ..Default::default() });
        assert!((t.target_speed - 0.7).abs() < 1e-12);
        nav.tick_speed(&mut t, &NavInput { next_curve_distance: Some(500.0), ..Default::default() });
        assert!((t.target_speed - 1.4).abs() < 1e-12);
        // throttle
        let mut t = Train::new(1, TrainConfig::default(), vec![], vec![]);
        t.throttle = 0.5;
        nav.tick_speed(&mut t, &NavInput::default());
        assert!((t.target_speed - 0.7).abs() < 1e-12);
        // reverse destination and cancel
        let mut t = Train::new(1, TrainConfig::default(), vec![], vec![]);
        let mut nav = Navigation { destination: Some(1), distance_to_destination: -50.0, destination_behind_train: true, ..Default::default() };
        assert_eq!(nav.tick_speed(&mut t, &NavInput::default()), NavResult::Cancelled);
        assert!(nav.destination.is_none());
        let mut nav = Navigation::default();
        assert_eq!(nav.tick_speed(&mut t, &NavInput::default()), NavResult::NoDestination);
    }

    #[test]
    fn station_hold_and_leave() {
        let mut t = Train::new(1, TrainConfig::default(), vec![], vec![]);
        let mut nav = Navigation { destination: Some(1), distance_to_destination: 100.0, distance_to_signal: 2.0, waiting_for_signal: Some((1, 0)), ..Default::default() };
        let inp = NavInput { at_station: true, signal_pending: true, ..Default::default() };
        assert_eq!(nav.tick_speed(&mut t, &inp), NavResult::Hold);
        let mut nav = Navigation { destination: Some(1), distance_to_destination: 100.0, ..Default::default() };
        assert_eq!(nav.tick_speed(&mut t, &NavInput { at_station: true, ..Default::default() }), NavResult::LeaveStation);
    }

    fn one_carriage_train(ids: &[NodeId], pos: f64) -> Train {
        let car = Carriage { leading: TravellingPoint::new(ids[0], ids[1], pos), trailing: TravellingPoint::new(ids[0], ids[1], pos - 3.0), bogey_spacing: 3.0, stalled: false };
        // trailing starts 3 behind; keep >= 0 on first edge
        Train::new(1, TrainConfig::default(), vec![car], vec![])
    }

    #[test]
    fn full_run_to_station_stops_exactly_at_point() {
        let (mut g, ids) = chain(7, 50.0); // 300 blocks
        g.add_point(ids[5], ids[6], EdgePoint { id: 1, kind: PointKind::Station, pos: 20.0 });
        let mut t = one_carriage_train(&ids, 10.0);
        t.navigation.destination = Some(1);
        // distance from leading point to station: (50-10) + 4*50 + 20
        t.navigation.distance_to_destination = 40.0 + 200.0 + 20.0;
        let mut arrived = false;
        let mut ticks = 0;
        let mut max_speed = 0.0f64;
        while ticks < 5000 && !arrived {
            ticks += 1;
            let mut nav = std::mem::take(&mut t.navigation);
            nav.tick_speed(&mut t, &NavInput::default());
            t.navigation = nav;
            max_speed = max_speed.max(t.speed);
            let mut hit = false;
            let mut cb = |_: f64, p: &EdgePoint, _: Dir| {
                if p.kind == PointKind::Station {
                    hit = true;
                    return true;
                }
                false
            };
            let rep = t.tick(&g, &mut FirstBranch, Some(&mut cb));
            assert!(!rep.blocked && !rep.derailed_now, "tick {ticks}: {rep:?}");
            if hit {
                t.arrive();
                arrived = true;
            }
        }
        assert!(arrived, "no arrival");
        assert_eq!(t.speed, 0.0);
        assert_eq!(t.carriages[0].leading.dir(), (ids[5], ids[6]));
        assert!((t.carriages[0].leading.position - 20.0).abs() < 1e-6);
        assert!(max_speed > 1.3 && max_speed <= 1.4 + 1e-9, "{max_speed}");
        // 260 blocks at <= 1.4/tick, accelerating and braking at 0.0075/tick^2
        assert!(ticks > 185 && ticks < 800, "{ticks}");
        // trailing bogey stayed 3 blocks behind (stress ~0 on a straight line)
        assert!(t.stress.is_empty());
        let a = t.carriages[0].leading.world_position(&g).unwrap();
        let b = t.carriages[0].trailing.world_position(&g).unwrap();
        assert!((dist(a, b) - 3.0).abs() < 1e-6);
    }

    #[test]
    fn two_carriages_follow_and_stress_resolves() {
        let (g, ids) = chain(5, 50.0);
        let c0 = Carriage { leading: TravellingPoint::new(ids[0], ids[1], 40.0), trailing: TravellingPoint::new(ids[0], ids[1], 37.0), bogey_spacing: 3.0, stalled: false };
        let c1 = Carriage { leading: TravellingPoint::new(ids[0], ids[1], 30.0), trailing: TravellingPoint::new(ids[0], ids[1], 27.0), bogey_spacing: 3.0, stalled: false };
        let mut t = Train::new(1, TrainConfig::default(), vec![c0, c1], vec![7.0]);
        t.speed = 1.0;
        t.navigation.destination = Some(1); // suppress passive slowdown
        t.navigation.distance_to_destination = 1000.0;
        for _ in 0..40 {
            let rep = t.tick(&g, &mut FirstBranch, None);
            assert!(!rep.blocked);
        }
        // spacing between carriages (trailing of 0 -> leading of 1) is 7 from the start (37 - 30)
        let a = t.carriages[0].trailing.world_position(&g).unwrap();
        let b = t.carriages[1].leading.world_position(&g).unwrap();
        assert!((dist(a, b) - 7.0).abs() < 1e-6);
        assert!(t.carriages[0].leading.position > 70.0 || t.carriages[0].leading.node1 != ids[0]);
        // an over-stretched train derails
        let mut s = t.clone();
        s.carriages[1].leading = TravellingPoint::new(s.carriages[0].leading.node1, s.carriages[0].leading.node2, 0.0);
        s.carriages[1].trailing = s.carriages[1].leading.clone();
        s.speed = 0.0;
        let rep = s.tick(&g, &mut FirstBranch, None);
        assert!(rep.derailed_now && s.derailed);
    }

    #[test]
    fn reverse_motion_uses_back_point() {
        let (g, ids) = chain(4, 50.0);
        let mut t = one_carriage_train(&ids, 40.0);
        t.speed = -1.0;
        t.navigation.destination = Some(1);
        t.navigation.destination_behind_train = true;
        t.navigation.distance_to_destination = 500.0;
        for _ in 0..10 {
            t.tick(&g, &mut FirstBranch, None);
        }
        assert!((t.carriages[0].leading.position - 30.0).abs() < 1e-6);
        assert!((t.navigation.distance_to_destination - 490.0).abs() < 1e-6);
    }

    #[test]
    fn dead_end_blocks_and_stops() {
        let (g, ids) = chain(2, 50.0);
        let mut t = one_carriage_train(&ids, 45.0);
        t.speed = 1.0;
        t.navigation.destination = Some(1);
        t.navigation.distance_to_destination = 100.0;
        let rep = t.tick(&g, &mut FirstBranch, None);
        assert!(!rep.blocked);
        let rep = t.tick(&g, &mut FirstBranch, None);
        assert!(!rep.blocked || t.speed == 0.0);
        for _ in 0..10 {
            t.tick(&g, &mut FirstBranch, None);
        }
        assert_eq!(t.speed, 0.0);
        assert!(t.navigation.destination.is_none());
    }

    #[test]
    fn stall_saves_and_restores_speed() {
        let (g, ids) = chain(4, 50.0);
        let mut t = one_carriage_train(&ids, 20.0);
        t.speed = 0.8;
        t.navigation.destination = Some(1);
        t.navigation.distance_to_destination = 100.0;
        t.carriages[0].stalled = true;
        t.tick(&g, &mut FirstBranch, None);
        assert_eq!(t.speed, 0.0);
        assert_eq!(t.speed_before_stall, Some(0.8));
        assert!((t.carriages[0].leading.position - 20.0).abs() < 1e-12);
        t.carriages[0].stalled = false;
        t.tick(&g, &mut FirstBranch, None);
        assert!((t.speed - 0.8).abs() < 1e-9);
        assert!(t.speed_before_stall.is_none());
    }

    #[test]
    fn refresh_triggers_and_penalties() {
        let mut t = Train::new(1, TrainConfig::default(), vec![], vec![]);
        t.navigation.destination = Some(1);
        t.navigation.distance_to_destination = 121.0;
        let r = t.update_navigation_target(2.0); // 121 -> 119: crosses a 100 multiple + 20
        assert!(r.full);
        t.navigation.distance_to_destination = 60.0;
        let r = t.update_navigation_target(10.0); // 60 -> 50 -> %50 = 0 <= 5
        assert!(r.partial);
        t.navigation.distance_to_destination = 500.0;
        assert_eq!(t.update_navigation_target(1.0), Refresh::default());
        t.navigation.destination_behind_train = true;
        t.navigation.distance_to_destination = 10.0;
        t.update_navigation_target(1.0);
        assert!((t.navigation.distance_to_destination - 11.0).abs() < 1e-12);
        let mut m = Train::new(2, TrainConfig::default(), vec![], vec![]);
        assert_eq!(m.navigation_penalty(false, 0), penalties::IDLE_TRAIN);
        assert_eq!(m.navigation_penalty(true, 0), penalties::ANY_TRAIN);
        m.manual_tick = true;
        assert_eq!(m.navigation_penalty(true, 0), penalties::MANUAL_TRAIN);
        m.manual_tick = false;
        m.navigation.waiting_for_signal = Some((1, 0));
        assert_eq!(m.navigation_penalty(true, 400), 70.0);
    }

    #[test]
    fn path_search_prefers_cheaper_route_and_penalties() {
        // two routes a->b->d (short) and a->c->d (long)
        let mut g = TrackGraph::new(1);
        let s = g.add_node(0, [0.0, 0.0, 0.0], UP);
        let a = g.add_node(0, [0.0, 0.0, 10.0], UP);
        let b = g.add_node(0, [-2.0, 0.0, 20.0], UP);
        let c = g.add_node(0, [2.0, 0.0, 20.0], UP);
        let d = g.add_node(0, [0.0, 0.0, 30.0], UP);
        let e = g.add_node(0, [-4.0, 0.0, 40.0], UP);
        for (x, y) in [(s, a), (a, b), (a, c), (b, d), (c, d), (d, e)] {
            g.connect(x, y, TrackType::standard(), None);
        }
        let mut goal = |de: Dir| de == (d, e);
        let mut none = |_: Dir| 0.0;
        let p = find_path(&g, (s, a), 10.0, &mut goal, &mut none, f64::MAX, -1.0).unwrap();
        assert_eq!(p.edges.first(), Some(&(s, a)));
        assert_eq!(p.edges.last(), Some(&(d, e)));
        assert_eq!(p.edges.len(), 4);
        // distance to start of goal edge: 10 + two ~10.2 edges
        assert!(p.distance > 30.0 && p.distance < 31.0, "{}", p.distance);
        // penalise the route through b -> path goes through c
        let mut pen = |de: Dir| if de == (a, b) { 100.0 } else { 0.0 };
        let mut goal2 = |de: Dir| de == (d, e);
        let p2 = find_path(&g, (s, a), 10.0, &mut goal2, &mut pen, f64::MAX, -1.0).unwrap();
        assert!(p2.edges.contains(&(a, c)));
        // cost pruning
        let mut goal3 = |de: Dir| de == (d, e);
        let mut pen3 = |_: Dir| 1000.0;
        assert!(find_path(&g, (s, a), 10.0, &mut goal3, &mut pen3, f64::MAX, 50.0).is_none());
        // unreachable beyond max distance
        let mut goal4 = |de: Dir| de == (d, e);
        assert!(find_path(&g, (s, a), 10.0, &mut goal4, &mut none, 20.0, -1.0).is_none());
        // path selector follows the found route
        let mut t = TravellingPoint::new(s, a, 5.0);
        t.travel(&g, 12.0, &mut PathSelector(&p2.edges), &mut TravelHooks::default());
        assert_eq!(t.node2, c);
        let mut w = SignalWeight::new(0);
        assert_eq!((w.next(), w.next()), (25.0, 12.5));
        assert_eq!(SignalWeight::new(500).next(), 200.0);
    }

    #[test]
    fn one_way_blocked_by_u_turn_only() {
        // a loop of one edge: can't reverse
        let (g, ids) = chain(2, 10.0);
        let mut p = TravellingPoint::new(ids[0], ids[1], 9.0);
        p.travel(&g, 5.0, &mut RandomSelector(7), &mut TravelHooks::default());
        assert!(p.blocked);
        assert_eq!(heading([0.0; 3], [0.0, 0.0, 2.0]), [0.0, 0.0, 1.0]);
    }
}
