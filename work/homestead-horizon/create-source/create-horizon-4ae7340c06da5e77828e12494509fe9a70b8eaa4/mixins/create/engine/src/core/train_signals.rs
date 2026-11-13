// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Pure signalling after Create 6.0.10 content/trains/signal: SignalBoundary,
// SignalEdgeGroup, SignalPropagator (group splitting/merging), cross-signal
// chains and the observer rule. Block entities/redstone are adapters: they
// report powered state per signal position and read the resulting state.
use std::collections::{BTreeMap, BTreeSet};

use crate::train_graph::{Dir, NodeId, PointId, PointKind, TrackGraph, canon};

pub type GroupId = u64;
pub type BlockPos = [i32; 3];

pub const COLOR_COUNT: usize = 9;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SignalType {
    Entry,
    Cross,
}

impl SignalType {
    pub fn cycle(self) -> Self {
        match self {
            Self::Entry => Self::Cross,
            Self::Cross => Self::Entry,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SignalState {
    Red,
    Green,
    Yellow,
    Invalid,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EdgeGroup {
    pub id: GroupId,
    pub color: u8,
    pub trains: BTreeSet<u64>,
    pub reserved: Option<PointId>,
    /// intersection id -> group (crossings / diamonds)
    pub intersecting: BTreeMap<u64, GroupId>,
    pub adjacent: BTreeSet<GroupId>,
    pub fallback: bool,
}

impl EdgeGroup {
    fn new(id: GroupId) -> Self {
        Self { id, color: 0, trains: BTreeSet::new(), reserved: None, intersecting: BTreeMap::new(), adjacent: BTreeSet::new(), fallback: false }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SignalBoundary {
    pub id: PointId,
    /// Signal blocks per side with their redstone-powered flag (powered forces red).
    pub signals: [BTreeMap<BlockPos, bool>; 2],
    pub types: [SignalType; 2],
    pub groups: [Option<GroupId>; 2],
    pub states: [SignalState; 2],
    pub dirty: [bool; 2],
}

impl SignalBoundary {
    pub fn new(id: PointId) -> Self {
        Self {
            id,
            signals: [BTreeMap::new(), BTreeMap::new()],
            types: [SignalType::Entry; 2],
            groups: [None, None],
            states: [SignalState::Invalid; 2],
            dirty: [false; 2],
        }
    }

    pub fn is_forced_red(&self, side: usize) -> bool {
        self.signals[side].values().any(|p| *p)
    }

    pub fn has_signals(&self, side: usize) -> bool {
        !self.signals[side].is_empty()
    }

    /// `getStateFor(pos)`.
    pub fn state_for(&self, pos: BlockPos) -> SignalState {
        for s in 0..2 {
            if self.signals[s].contains_key(&pos) {
                return self.states[s];
            }
        }
        SignalState::Invalid
    }

    /// `getTypeFor(pos)`.
    pub fn type_for(&self, pos: BlockPos) -> Option<SignalType> {
        (0..2).find(|s| self.signals[*s].contains_key(&pos)).map(|s| self.types[s])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    Render,
    Dual,
    Skip,
}

#[derive(Clone, Debug, Default)]
pub struct SignalNetwork {
    pub groups: BTreeMap<GroupId, EdgeGroup>,
    pub boundaries: BTreeMap<PointId, SignalBoundary>,
    /// Group of each edge that holds no boundary (canonical edge key).
    pub edge_groups: BTreeMap<Dir, GroupId>,
    next_group: GroupId,
    next_intersection: u64,
}

struct Walk {
    plain: Vec<Dir>,
    hits: Vec<(PointId, usize)>,
}

impl SignalNetwork {
    pub fn new() -> Self {
        Self::default()
    }

    fn new_group(&mut self) -> GroupId {
        let id = self.next_group;
        self.next_group += 1;
        self.groups.insert(id, EdgeGroup::new(id));
        id
    }

    fn remove_group(&mut self, id: GroupId) {
        if let Some(g) = self.groups.remove(&id) {
            for a in g.adjacent {
                if let Some(o) = self.groups.get_mut(&a) {
                    o.adjacent.remove(&id);
                }
            }
            for (_, other) in g.intersecting {
                if let Some(o) = self.groups.get_mut(&other) {
                    o.intersecting.retain(|_, v| *v != id);
                }
            }
        }
    }

    // ------------------------------------------------------------- walking

    fn signal_points(&self, graph: &TrackGraph, a: NodeId, b: NodeId) -> Vec<(f64, PointId)> {
        graph
            .directed_points(a, b)
            .into_iter()
            .filter(|(_, p)| p.kind == PointKind::Signal && self.boundaries.contains_key(&p.id))
            .map(|(pos, p)| (pos, p.id))
            .collect()
    }

    fn near_side(a: NodeId, b: NodeId) -> usize {
        if a <= b { 0 } else { 1 }
    }

    /// Walk from directed edge `start` past `from_pos`, stopping at boundaries.
    fn walk(&self, graph: &TrackGraph, start: Dir, from_pos: f64, skip: Option<PointId>, filter_travel: bool) -> Walk {
        let mut out = Walk { plain: Vec::new(), hits: Vec::new() };
        let mut visited: BTreeSet<Dir> = BTreeSet::new();
        let first = self.signal_points(graph, start.0, start.1);
        if let Some((_, id)) = first.into_iter().find(|(pos, id)| *pos > from_pos + 1e-9 && Some(*id) != skip) {
            out.hits.push((id, Self::near_side(start.0, start.1)));
            return out;
        }
        let mut stack: Vec<Dir> = vec![start];
        while let Some((a, b)) = stack.pop() {
            let Some(arriving) = graph.edge(a, b) else { continue };
            for (c, e2) in graph.neighbours(b) {
                if c == a {
                    continue;
                }
                if filter_travel && !arriving.can_travel_to(e2) {
                    continue;
                }
                let pts = self.signal_points(graph, b, c);
                if let Some((_, id)) = pts.first() {
                    let hit = (*id, Self::near_side(b, c));
                    if !out.hits.contains(&hit) {
                        out.hits.push(hit);
                    }
                    continue;
                }
                let key = canon(b, c);
                if visited.insert(key) {
                    out.plain.push(key);
                    stack.push((b, c));
                }
            }
        }
        out
    }

    /// Directed start of a walk that leaves boundary `bid` through `side`.
    fn start_of(&self, graph: &TrackGraph, bid: PointId, side: usize) -> Option<(Dir, f64)> {
        let (key, p) = graph.find_point(bid)?;
        let d: Dir = if side == 1 { (key.0, key.1) } else { (key.1, key.0) };
        Some((d, graph.directed_pos(d.0, d.1, p.pos)))
    }

    // ------------------------------------------------------------ lifecycle

    /// Register a boundary placed on an edge and build groups for both sides.
    pub fn add_boundary(&mut self, graph: &TrackGraph, id: PointId) {
        self.boundaries.entry(id).or_insert_with(|| SignalBoundary::new(id));
        if let Some((key, _)) = graph.find_point(id) {
            // an edge that now holds a boundary no longer has a single group
            if let Some(old) = self.edge_groups.remove(&key) {
                self.drop_if_unused(old);
            }
        }
        for side in 0..2 {
            self.propagate_signal_group(graph, id, side);
        }
    }

    /// Add a signal block to one side (first one fixes the side's type).
    pub fn add_signal_block(&mut self, id: PointId, side: usize, pos: BlockPos, ty: SignalType) {
        if let Some(b) = self.boundaries.get_mut(&id) {
            if b.signals[side].is_empty() {
                b.types[side] = ty;
            }
            b.signals[side].insert(pos, false);
        }
    }

    pub fn remove_signal_block(&mut self, id: PointId, pos: BlockPos) {
        if let Some(b) = self.boundaries.get_mut(&id) {
            b.signals[0].remove(&pos);
            b.signals[1].remove(&pos);
        }
    }

    pub fn set_powered(&mut self, id: PointId, pos: BlockPos, powered: bool) {
        if let Some(b) = self.boundaries.get_mut(&id) {
            for s in 0..2 {
                if let Some(p) = b.signals[s].get_mut(&pos) {
                    *p = powered;
                }
            }
        }
    }

    pub fn cycle_signal_type(&mut self, id: PointId, pos: BlockPos) {
        if let Some(b) = self.boundaries.get_mut(&id) {
            for s in 0..2 {
                if b.signals[s].contains_key(&pos) {
                    b.types[s] = b.types[s].cycle();
                }
            }
        }
    }

    fn set_boundary_group(&mut self, bid: PointId, side: usize, group: GroupId) {
        let (old_opp, prev) = {
            let b = &self.boundaries[&bid];
            (b.groups[1 - side], b.groups[side])
        };
        if let (Some(prev), Some(opp)) = (prev, old_opp) {
            if let Some(g) = self.groups.get_mut(&opp) {
                g.adjacent.remove(&prev);
            }
            if let Some(g) = self.groups.get_mut(&prev) {
                g.adjacent.remove(&opp);
            }
        }
        self.boundaries.get_mut(&bid).unwrap().groups[side] = Some(group);
        if let Some(opp) = old_opp {
            if opp != group {
                if let Some(g) = self.groups.get_mut(&group) {
                    g.adjacent.insert(opp);
                }
                if let Some(g) = self.groups.get_mut(&opp) {
                    g.adjacent.insert(group);
                }
            }
        }
    }

    /// `SignalPropagator.propagateSignalGroup`: new group for `side`, absorbing the
    /// region up to the next boundaries (which get this group on their facing side).
    pub fn propagate_signal_group(&mut self, graph: &TrackGraph, bid: PointId, side: usize) {
        let Some((start, from)) = self.start_of(graph, bid, side) else { return };
        let old = self.boundaries.get(&bid).and_then(|b| b.groups[side]);
        let g = self.new_group();
        self.set_boundary_group(bid, side, g);
        if let Some(o) = old {
            self.drop_if_unused(o);
        }
        let w = self.walk(graph, start, from, Some(bid), false);
        for (hit, hside) in w.hits {
            let prev = self.boundaries[&hit].groups[hside];
            self.set_boundary_group(hit, hside, g);
            if let Some(p) = prev {
                self.drop_if_unused(p);
            }
            self.boundaries.get_mut(&hit).unwrap().dirty[hside] = false;
        }
        for e in w.plain {
            if let Some(old) = self.edge_groups.insert(e, g) {
                if old != g {
                    self.drop_if_unused(old);
                }
            }
        }
        if let Some(b) = self.boundaries.get_mut(&bid) {
            b.dirty[side] = false;
        }
        self.resolve_color(g);
    }

    /// A group is dropped when no boundary side and no edge refers to it.
    fn drop_if_unused(&mut self, id: GroupId) {
        let used = self.boundaries.values().any(|b| b.groups.contains(&Some(id))) || self.edge_groups.values().any(|g| *g == id);
        if !used {
            self.remove_group(id);
        }
    }

    /// `SignalPropagator.onSignalRemoved`: dissolve the boundary's groups; the
    /// freed region becomes one passive group and neighbours re-propagate.
    pub fn on_signal_removed(&mut self, graph: &TrackGraph, bid: PointId) {
        let Some(b) = self.boundaries.get(&bid).cloned() else { return };
        let passive = self.new_group();
        for side in 0..2 {
            if let Some((start, from)) = self.start_of(graph, bid, side) {
                let w = self.walk(graph, start, from, Some(bid), false);
                for (hit, hside) in w.hits {
                    if let Some(h) = self.boundaries.get_mut(&hit) {
                        h.dirty[hside] = true;
                    }
                }
                for e in w.plain {
                    self.edge_groups.insert(e, passive);
                }
            }
        }
        self.boundaries.remove(&bid);
        for g in b.groups.into_iter().flatten() {
            self.drop_if_unused(g);
        }
        // groups orphaned by the reassignment above
        let ids: Vec<GroupId> = self.groups.keys().copied().collect();
        for id in ids {
            self.drop_if_unused(id);
        }
        self.resolve_color(passive);
    }

    // --------------------------------------------------------------- colors

    fn cluster(&self, id: GroupId) -> BTreeSet<GroupId> {
        let mut set = BTreeSet::new();
        let mut stack = vec![id];
        while let Some(g) = stack.pop() {
            if !set.insert(g) {
                continue;
            }
            if let Some(grp) = self.groups.get(&g) {
                stack.extend(grp.intersecting.values().copied());
            }
        }
        set
    }

    /// `resolveColor`: lowest colour not used by groups adjacent to the cluster.
    pub fn resolve_color(&mut self, id: GroupId) {
        let cluster = self.cluster(id);
        let mut mask = 0u32;
        for g in &cluster {
            if let Some(grp) = self.groups.get(g) {
                for a in &grp.adjacent {
                    if !cluster.contains(a) {
                        if let Some(o) = self.groups.get(a) {
                            mask |= 1 << o.color;
                        }
                    }
                }
            }
        }
        let color = (0..COLOR_COUNT as u8).find(|c| mask & (1 << c) == 0).unwrap_or(0);
        for g in cluster {
            if let Some(grp) = self.groups.get_mut(&g) {
                grp.color = color;
            }
        }
    }

    // ----------------------------------------------------------- occupancy

    /// Group at a directed position `pos` along `(a, b)`.
    pub fn group_at(&self, graph: &TrackGraph, a: NodeId, b: NodeId, pos: f64) -> Option<GroupId> {
        let pts = self.signal_points(graph, a, b);
        if pts.is_empty() {
            return self.edge_groups.get(&canon(a, b)).copied();
        }
        let near = Self::near_side(a, b);
        let far = 1 - near;
        match pts.iter().rev().find(|(p, _)| *p <= pos + 1e-9) {
            Some((_, id)) => self.boundaries[id].groups[far],
            None => self.boundaries[&pts[0].1].groups[near],
        }
    }

    /// `isOccupiedUnless(Train)`: considers the whole intersecting cluster.
    pub fn is_occupied_unless_train(&self, group: GroupId, train: u64) -> bool {
        for g in self.cluster(group) {
            let Some(grp) = self.groups.get(&g) else { continue };
            if grp.reserved.is_some() || grp.trains.len() > 1 || (!grp.trains.is_empty() && !grp.trains.contains(&train)) {
                return true;
            }
        }
        false
    }

    /// `isOccupiedUnless(SignalBoundary)`.
    pub fn is_occupied_unless_boundary(&self, group: GroupId, boundary: PointId) -> bool {
        let Some(g) = self.groups.get(&group) else { return false };
        !g.trains.is_empty() || g.reserved.is_some_and(|r| r != boundary)
    }

    pub fn occupy(&mut self, group: GroupId, train: u64) {
        if let Some(g) = self.groups.get_mut(&group) {
            g.trains.insert(train);
        }
    }

    pub fn release(&mut self, group: GroupId, train: u64) {
        if let Some(g) = self.groups.get_mut(&group) {
            g.trains.remove(&train);
        }
    }

    /// Recompute a train's occupied groups from its bogey points (directed edge + position).
    pub fn set_train_groups(&mut self, graph: &TrackGraph, train: u64, points: &[(Dir, f64)]) {
        for g in self.groups.values_mut() {
            g.trains.remove(&train);
        }
        for ((a, b), pos) in points {
            if let Some(g) = self.group_at(graph, *a, *b, *pos) {
                self.occupy(g, train);
            }
        }
    }

    /// Reserve an unoccupied group for a boundary (a train is approaching).
    pub fn reserve(&mut self, group: GroupId, boundary: PointId) -> bool {
        match self.groups.get_mut(&group) {
            Some(g) if g.trains.is_empty() && g.reserved.is_none_or(|r| r == boundary) => {
                g.reserved = Some(boundary);
                true
            }
            _ => false,
        }
    }

    pub fn cancel_reservation(&mut self, boundary: PointId) {
        for g in self.groups.values_mut() {
            if g.reserved == Some(boundary) {
                g.reserved = None;
            }
        }
    }

    /// Link groups of crossing edges so occupying one blocks the other.
    pub fn refresh_intersections(&mut self, graph: &TrackGraph) {
        for g in self.groups.values_mut() {
            g.intersecting.clear();
        }
        for c in graph.find_crossings() {
            let mid = |d: Dir, s: &Self| {
                let l = graph.edge(d.0, d.1).map(|e| e.length()).unwrap_or(0.0);
                s.group_at(graph, d.0, d.1, l / 2.0)
            };
            if let (Some(a), Some(b)) = (mid(c.a, self), mid(c.b, self)) {
                if a != b {
                    self.next_intersection += 1;
                    let id = self.next_intersection;
                    self.groups.get_mut(&a).unwrap().intersecting.insert(id, b);
                    self.groups.get_mut(&b).unwrap().intersecting.insert(id, a);
                }
            }
        }
        let ids: Vec<GroupId> = self.groups.keys().copied().collect();
        for id in ids {
            self.resolve_color(id);
        }
    }

    // ---------------------------------------------------------------- ticking

    /// Pre-train pass: re-propagate dirty sides.
    pub fn tick_pre(&mut self, graph: &TrackGraph) {
        let work: Vec<(PointId, usize)> = self
            .boundaries
            .values()
            .flat_map(|b| (0..2).filter(|s| b.dirty[*s]).map(|s| (b.id, s)).collect::<Vec<_>>())
            .collect();
        for (id, s) in work {
            if self.boundaries.contains_key(&id) {
                self.propagate_signal_group(graph, id, s);
            }
        }
    }

    /// Post-train pass: recompute every signal state (ahead group decides).
    pub fn tick_post(&mut self, graph: &TrackGraph) {
        let ids: Vec<PointId> = self.boundaries.keys().copied().collect();
        for id in ids {
            for side in 0..2 {
                let state = self.compute_state(graph, id, side);
                self.boundaries.get_mut(&id).unwrap().states[side] = state;
            }
        }
    }

    fn compute_state(&self, graph: &TrackGraph, id: PointId, side: usize) -> SignalState {
        let b = &self.boundaries[&id];
        if !b.has_signals(side) {
            return SignalState::Invalid;
        }
        let forced = b.is_forced_red(side);
        let (mine, ahead) = (b.groups[side], b.groups[1 - side]);
        if mine.is_some() && mine == ahead {
            return SignalState::Invalid;
        }
        let Some(ahead) = ahead.filter(|g| self.groups.contains_key(g)) else { return SignalState::Invalid };
        if forced || self.is_occupied_unless_boundary(ahead, id) {
            return SignalState::Red;
        }
        self.resolve_chain(graph, id, side)
    }

    /// Boundaries (and the side facing us) reached by walking ahead of `id`/`side`.
    pub fn collect_chained(&self, graph: &TrackGraph, id: PointId, side: usize) -> Vec<(PointId, usize)> {
        let Some((start, from)) = self.start_of(graph, id, 1 - side) else { return Vec::new() };
        self.walk(graph, start, from, Some(id), true).hits
    }

    fn resolve_chain(&self, graph: &TrackGraph, id: PointId, side: usize) -> SignalState {
        let b = &self.boundaries[&id];
        if b.types[side] != SignalType::Cross {
            return SignalState::Green;
        }
        let (mut free, mut blocked) = (0, 0);
        for (oid, oside) in self.collect_chained(graph, id, side) {
            let Some(o) = self.boundaries.get(&oid) else { return SignalState::Invalid };
            if !o.has_signals(oside) {
                continue;
            }
            match o.states[oside] {
                SignalState::Green | SignalState::Invalid => free += 1,
                SignalState::Red => blocked += 1,
                SignalState::Yellow => {
                    free += 1;
                }
            }
        }
        if blocked == 0 {
            SignalState::Green
        } else if free == 0 {
            SignalState::Red
        } else {
            SignalState::Yellow
        }
    }

    /// `getOverlayFor`.
    pub fn overlay_for(&self, id: PointId, pos: BlockPos) -> Overlay {
        let Some(b) = self.boundaries.get(&id) else { return Overlay::Skip };
        for s in 0..2 {
            if b.signals[s].contains_key(&pos) {
                return if b.has_signals(1 - s) { Overlay::Dual } else { Overlay::Render };
            }
        }
        Overlay::Skip
    }

    /// Group colour of an edge, for the debug/overlay adapter.
    pub fn edge_group_color(&self, a: NodeId, b: NodeId) -> Option<u8> {
        self.edge_groups.get(&canon(a, b)).and_then(|g| self.groups.get(g)).map(|g| g.color)
    }
}

// --------------------------------------------------------------- saving form

/// Plain-data form of the signal network (trains/reservations are runtime-only).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SignalSave {
    pub groups: Vec<(GroupId, u8, Vec<GroupId>, bool)>,
    pub edge_groups: Vec<(NodeId, NodeId, GroupId)>,
    pub boundaries: Vec<SignalBoundary>,
}

impl SignalNetwork {
    pub fn save(&self) -> SignalSave {
        SignalSave {
            groups: self.groups.values().map(|g| (g.id, g.color, g.intersecting.values().copied().collect(), g.fallback)).collect(),
            edge_groups: self.edge_groups.iter().map(|((a, b), g)| (*a, *b, *g)).collect(),
            boundaries: self.boundaries.values().cloned().collect(),
        }
    }

    pub fn load(s: &SignalSave) -> Self {
        let mut n = SignalNetwork::new();
        for (id, color, inter, fallback) in &s.groups {
            let mut g = EdgeGroup::new(*id);
            g.color = *color;
            g.fallback = *fallback;
            for (i, o) in inter.iter().enumerate() {
                g.intersecting.insert(i as u64 + 1, *o);
            }
            n.groups.insert(*id, g);
            n.next_group = n.next_group.max(id + 1);
        }
        for (a, b, g) in &s.edge_groups {
            n.edge_groups.insert((*a, *b), *g);
        }
        for b in &s.boundaries {
            n.boundaries.insert(b.id, b.clone());
            for s in 0..2 {
                if let (Some(me), Some(opp)) = (b.groups[s], b.groups[1 - s]) {
                    if let Some(g) = n.groups.get_mut(&me) {
                        g.adjacent.insert(opp);
                    }
                }
            }
        }
        n.next_intersection = 1 << 20;
        n
    }
}

/// Drop signal data that refers to graph content that no longer exists.
pub fn prune_to_graph(net: &mut SignalNetwork, graph: &TrackGraph) {
    net.boundaries.retain(|id, _| graph.find_point(*id).is_some());
    net.edge_groups.retain(|(a, b), _| graph.edge(*a, *b).is_some());
}

// ------------------------------------------------------------------ observer

/// `TrackObserver`: active while a matching train overlaps it.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Observer {
    pub id: PointId,
    pub filter: Option<String>,
    pub active: bool,
}

impl Observer {
    pub fn new(id: PointId) -> Self {
        Self { id, filter: None, active: false }
    }

    /// `cargo_names`: what the train carries (names the filter is matched against).
    pub fn update(&mut self, train_overlapping: bool, cargo_names: &[&str]) -> bool {
        let matches = match &self.filter {
            None => true,
            Some(f) => cargo_names.iter().any(|c| c == f),
        };
        let next = train_overlapping && matches;
        let changed = next != self.active;
        self.active = next;
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::train_graph::EdgePoint;
    use crate::train_track::{TrackType, UP};

    fn line(n: usize) -> (TrackGraph, Vec<NodeId>) {
        let mut g = TrackGraph::new(1);
        let ids: Vec<NodeId> = (0..n).map(|i| g.add_node(0, [0.0, 0.0, i as f64 * 10.0], UP)).collect();
        for w in ids.windows(2) {
            g.connect(w[0], w[1], TrackType::standard(), None);
        }
        (g, ids)
    }

    fn place(g: &mut TrackGraph, a: NodeId, b: NodeId, id: PointId, pos: f64) {
        assert!(g.add_point(a, b, EdgePoint { id, kind: PointKind::Signal, pos }));
    }

    #[test]
    fn single_boundary_splits_line_into_two_groups() {
        let (mut g, n) = line(5);
        place(&mut g, n[1], n[2], 1, 5.0);
        let mut net = SignalNetwork::new();
        net.add_boundary(&g, 1);
        let b = &net.boundaries[&1];
        let (g0, g1) = (b.groups[0].unwrap(), b.groups[1].unwrap());
        assert_ne!(g0, g1);
        assert_eq!(net.edge_groups[&(n[0], n[1])], g0);
        assert_eq!(net.edge_groups[&(n[2], n[3])], g1);
        assert_eq!(net.edge_groups[&(n[3], n[4])], g1);
        assert!(!net.edge_groups.contains_key(&(n[1], n[2])));
        assert!(net.groups[&g0].adjacent.contains(&g1));
        assert_ne!(net.groups[&g0].color, net.groups[&g1].color);
        assert_eq!(net.groups.len(), 2);
        // group_at along both directions
        assert_eq!(net.group_at(&g, n[1], n[2], 2.0), Some(g0));
        assert_eq!(net.group_at(&g, n[1], n[2], 8.0), Some(g1));
        assert_eq!(net.group_at(&g, n[2], n[1], 2.0), Some(g1)); // 2 from n2 = pos 8
        assert_eq!(net.group_at(&g, n[2], n[1], 8.0), Some(g0));
    }

    #[test]
    fn second_boundary_splits_and_old_group_is_dropped() {
        let (mut g, n) = line(5);
        place(&mut g, n[1], n[2], 1, 5.0);
        let mut net = SignalNetwork::new();
        net.add_boundary(&g, 1);
        let old = net.boundaries[&1].groups[1].unwrap();
        place(&mut g, n[3], n[4], 2, 5.0);
        net.add_boundary(&g, 2);
        let b1 = &net.boundaries[&1];
        let b2 = &net.boundaries[&2];
        assert_ne!(b1.groups[1], Some(old));
        assert_eq!(b1.groups[1], b2.groups[0]); // shared middle block
        assert!(!net.groups.contains_key(&old));
        let mid = b1.groups[1].unwrap();
        assert_eq!(net.edge_groups[&(n[2], n[3])], mid);
        // four sides, three distinct groups: left, middle, right (right has no edges)
        assert_eq!(net.groups.len(), 3);
    }

    #[test]
    fn states_follow_occupancy() {
        let (mut g, n) = line(5);
        place(&mut g, n[1], n[2], 1, 5.0);
        place(&mut g, n[3], n[4], 2, 5.0);
        let mut net = SignalNetwork::new();
        net.add_boundary(&g, 1);
        net.add_boundary(&g, 2);
        net.add_signal_block(1, 0, [0, 0, 1], SignalType::Entry);
        net.add_signal_block(2, 0, [0, 0, 2], SignalType::Entry);
        net.tick_post(&g);
        assert_eq!(net.boundaries[&1].states[0], SignalState::Green);
        assert_eq!(net.boundaries[&1].states[1], SignalState::Invalid); // no signal on that side
        let mid = net.boundaries[&1].groups[1].unwrap();
        net.occupy(mid, 42);
        net.tick_post(&g);
        assert_eq!(net.boundaries[&1].states[0], SignalState::Red);
        assert_eq!(net.boundaries[&2].states[0], SignalState::Green);
        // train sitting between boundaries via bogey points
        net.set_train_groups(&g, 42, &[((n[2], n[3]), 3.0), ((n[2], n[3]), 6.0)]);
        assert!(net.groups[&mid].trains.contains(&42));
        assert!(!net.is_occupied_unless_train(mid, 42));
        assert!(net.is_occupied_unless_train(mid, 7));
        // forced red by redstone
        net.set_train_groups(&g, 42, &[]);
        net.set_powered(1, [0, 0, 1], true);
        net.tick_post(&g);
        assert_eq!(net.boundaries[&1].states[0], SignalState::Red);
        net.set_powered(1, [0, 0, 1], false);
        // reservation blocks other boundaries but not the reserving one
        assert!(net.reserve(mid, 1));
        assert!(!net.reserve(mid, 2));
        net.tick_post(&g);
        assert_eq!(net.boundaries[&1].states[0], SignalState::Green);
        net.cancel_reservation(1);
        assert!(net.reserve(mid, 2));
        net.tick_post(&g);
        assert_eq!(net.boundaries[&1].states[0], SignalState::Red);
    }

    #[test]
    fn same_group_both_sides_is_invalid() {
        // a ring: a single boundary on a loop has the same group on both sides
        let mut g = TrackGraph::new(1);
        let a = g.add_node(0, [0.0, 0.0, 0.0], UP);
        let b = g.add_node(0, [10.0, 0.0, 0.0], UP);
        let c = g.add_node(0, [10.0, 0.0, 10.0], UP);
        let d = g.add_node(0, [0.0, 0.0, 10.0], UP);
        for (x, y) in [(a, b), (b, c), (c, d), (d, a)] {
            g.connect(x, y, TrackType::standard(), None);
        }
        place(&mut g, a, b, 1, 5.0);
        let mut net = SignalNetwork::new();
        net.add_boundary(&g, 1);
        net.add_signal_block(1, 0, [0, 0, 0], SignalType::Entry);
        net.tick_post(&g);
        let bd = &net.boundaries[&1];
        assert_eq!(bd.groups[0], bd.groups[1]);
        assert_eq!(bd.states[0], SignalState::Invalid);
    }

    #[test]
    fn removal_merges_into_passive_group() {
        let (mut g, n) = line(5);
        place(&mut g, n[1], n[2], 1, 5.0);
        let mut net = SignalNetwork::new();
        net.add_boundary(&g, 1);
        net.on_signal_removed(&g, 1);
        g.remove_point(1);
        assert!(net.boundaries.is_empty());
        let ids: BTreeSet<_> = [(n[0], n[1]), (n[2], n[3]), (n[3], n[4])].iter().map(|e| net.edge_groups[e]).collect();
        assert_eq!(ids.len(), 1);
        assert_eq!(net.groups.len(), 1);
        // edge (n1,n2) had the boundary, still unassigned: after removal it is plain
        net.add_boundary(&g, 99); // unknown point is ignored (not in graph)
        assert!(net.boundaries[&99].groups[0].is_none());
    }

    #[test]
    fn neighbour_resplit_after_removal() {
        let (mut g, n) = line(7);
        place(&mut g, n[1], n[2], 1, 5.0);
        place(&mut g, n[3], n[4], 2, 5.0);
        place(&mut g, n[5], n[6], 3, 5.0);
        let mut net = SignalNetwork::new();
        for id in 1..=3 {
            net.add_boundary(&g, id);
        }
        net.on_signal_removed(&g, 2);
        g.remove_point(2);
        assert!(net.boundaries[&1].dirty[1] && net.boundaries[&3].dirty[0]);
        net.tick_pre(&g);
        assert!(!net.boundaries[&1].dirty[1]);
        // now boundaries 1 and 3 face each other across the merged block
        assert_eq!(net.boundaries[&1].groups[1], net.boundaries[&3].groups[0]);
        assert_eq!(net.edge_groups[&(n[2], n[3])], net.boundaries[&1].groups[1].unwrap());
        assert_eq!(net.edge_groups[&(n[4], n[5])], net.boundaries[&1].groups[1].unwrap());
    }

    fn fork() -> (TrackGraph, [NodeId; 5]) {
        let mut g = TrackGraph::new(1);
        let n0 = g.add_node(0, [0.0, 0.0, 0.0], UP);
        let n1 = g.add_node(0, [0.0, 0.0, 10.0], UP);
        let n2 = g.add_node(0, [0.0, 0.0, 20.0], UP);
        let n3 = g.add_node(0, [0.0, 0.0, 30.0], UP);
        let n4 = g.add_node(0, [-2.0, 0.0, 30.0], UP);
        for (x, y) in [(n0, n1), (n1, n2), (n2, n3), (n2, n4)] {
            g.connect(x, y, TrackType::standard(), None);
        }
        (g, [n0, n1, n2, n3, n4])
    }

    #[test]
    fn cross_signal_chain_green_yellow_red() {
        let (mut g, n) = fork();
        place(&mut g, n[1], n[2], 1, 5.0); // chain head
        place(&mut g, n[2], n[3], 2, 5.0);
        place(&mut g, n[2], n[4], 3, 5.0);
        let mut net = SignalNetwork::new();
        for id in 1..=3 {
            net.add_boundary(&g, id);
        }
        net.add_signal_block(1, 0, [0, 0, 1], SignalType::Cross);
        net.add_signal_block(2, 0, [0, 0, 2], SignalType::Entry);
        net.add_signal_block(3, 0, [0, 0, 3], SignalType::Entry);
        // both exits face the same middle group
        assert_eq!(net.boundaries[&1].groups[1], net.boundaries[&2].groups[0]);
        assert_eq!(net.boundaries[&2].groups[0], net.boundaries[&3].groups[0]);
        let chain = net.collect_chained(&g, 1, 0);
        assert_eq!(chain.len(), 2);
        assert!(chain.contains(&(2, 0)) && chain.contains(&(3, 0)));
        net.tick_post(&g);
        net.tick_post(&g);
        assert_eq!(net.boundaries[&1].states[0], SignalState::Green);
        // occupy beyond exit 2 only -> yellow (chain reads last tick's cached states)
        let beyond2 = net.boundaries[&2].groups[1].unwrap();
        net.occupy(beyond2, 5);
        net.tick_post(&g);
        net.tick_post(&g);
        assert_eq!(net.boundaries[&2].states[0], SignalState::Red);
        assert_eq!(net.boundaries[&1].states[0], SignalState::Yellow);
        // both blocked -> red
        let beyond3 = net.boundaries[&3].groups[1].unwrap();
        net.occupy(beyond3, 6);
        net.tick_post(&g);
        net.tick_post(&g);
        assert_eq!(net.boundaries[&1].states[0], SignalState::Red);
        // entry type ignores chain
        net.cycle_signal_type(1, [0, 0, 1]);
        net.tick_post(&g);
        assert_eq!(net.boundaries[&1].states[0], SignalState::Green);
        assert_eq!(net.overlay_for(1, [0, 0, 1]), Overlay::Render);
        net.add_signal_block(1, 1, [0, 0, 9], SignalType::Entry);
        assert_eq!(net.overlay_for(1, [0, 0, 1]), Overlay::Dual);
        assert_eq!(net.overlay_for(1, [5, 5, 5]), Overlay::Skip);
        assert_eq!(net.boundaries[&1].type_for([0, 0, 9]), Some(SignalType::Entry));
        assert_eq!(net.boundaries[&1].state_for([1, 1, 1]), SignalState::Invalid);
    }

    #[test]
    fn crossing_groups_block_each_other_and_share_colour() {
        let mut g = TrackGraph::new(1);
        let a = g.add_node(0, [-5.0, 0.0, 0.0], UP);
        let b = g.add_node(0, [5.0, 0.0, 0.0], UP);
        let c = g.add_node(0, [0.0, 0.0, -5.0], UP);
        let d = g.add_node(0, [0.0, 0.0, 5.0], UP);
        g.connect(a, b, TrackType::standard(), None);
        g.connect(c, d, TrackType::standard(), None);
        // far boundary on each so every edge has a group of its own
        place(&mut g, a, b, 1, 1.0);
        place(&mut g, c, d, 2, 1.0);
        let mut net = SignalNetwork::new();
        net.add_boundary(&g, 1);
        net.add_boundary(&g, 2);
        net.refresh_intersections(&g);
        let g1 = net.group_at(&g, a, b, 5.0).unwrap();
        let g2 = net.group_at(&g, c, d, 5.0).unwrap();
        assert_ne!(g1, g2);
        assert!(net.groups[&g1].intersecting.values().any(|v| *v == g2));
        net.occupy(g2, 3);
        assert!(net.is_occupied_unless_train(g1, 4));
        assert!(!net.is_occupied_unless_train(g1, 3) || net.groups[&g1].trains.is_empty());
        assert_eq!(net.groups[&g1].color, net.groups[&g2].color);
    }

    #[test]
    fn signal_save_roundtrip_and_prune() {
        let (mut g, n) = line(5);
        place(&mut g, n[1], n[2], 1, 5.0);
        let mut net = SignalNetwork::new();
        net.add_boundary(&g, 1);
        net.add_signal_block(1, 0, [1, 2, 3], SignalType::Cross);
        net.tick_post(&g);
        let s = net.save();
        let l = SignalNetwork::load(&s);
        assert_eq!(l.save(), s);
        assert_eq!(l.boundaries[&1].types[0], SignalType::Cross);
        let mut l = l;
        l.propagate_signal_group(&g, 1, 0); // new ids never collide with loaded ones
        assert!(l.groups.len() >= 2);
        g.remove_point(1);
        prune_to_graph(&mut l, &g);
        assert!(l.boundaries.is_empty());
    }

    #[test]
    fn observer_rule() {
        let mut o = Observer::new(1);
        assert!(o.update(true, &[]));
        assert!(o.active);
        assert!(!o.update(true, &["x"]));
        assert!(o.update(false, &[]));
        o.filter = Some("create:coal".into());
        assert!(!o.update(true, &["minecraft:stone"]));
        assert!(o.update(true, &["create:coal"]));
    }
}
