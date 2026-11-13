// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Pure track graph after Create 6.0.10 content/trains/graph: TrackGraph,
// TrackNode, TrackEdge, EdgeData (edge points), crossings, merge/split and a
// plain-data saving form. Signal state lives in train_signals.rs.
use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::train_track::{BezierConnection, TrackType, V3, add, dist, dot, normalize, scale, sub};

pub type NodeId = u32;
pub type GraphId = u32;
pub type PointId = u64;
/// Directed edge: travelling from `.0` to `.1`.
pub type Dir = (NodeId, NodeId);

/// Node identity in the world: dimension plus position rounded to 1/16 block
/// (Java `TrackNodeLocation` rounds to the track block grid; the finer key is a
/// Rust simplification that never merges distinct nodes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeKey {
    pub dim: u32,
    pub q: [i64; 3],
}

impl NodeKey {
    pub fn new(dim: u32, p: V3) -> Self {
        Self { dim, q: [(p[0] * 16.0).round() as i64, (p[1] * 16.0).round() as i64, (p[2] * 16.0).round() as i64] }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrackNode {
    pub id: NodeId,
    pub dim: u32,
    pub pos: V3,
    pub normal: V3,
}

impl TrackNode {
    pub fn key(&self) -> NodeKey {
        NodeKey::new(self.dim, self.pos)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrackEdge {
    pub node1: NodeId,
    pub node2: NodeId,
    pub turn: Option<BezierConnection>,
    pub material: TrackType,
    pub interdimensional: bool,
    length: f64,
    p1: V3,
    p2: V3,
}

impl TrackEdge {
    pub fn is_turn(&self) -> bool {
        self.turn.is_some()
    }

    /// `getLength`: 0 for interdimensional edges, bezier length for curves.
    pub fn length(&self) -> f64 {
        self.length
    }

    /// World position at distance `s` from `node1` (clamped).
    pub fn position(&self, s: f64) -> V3 {
        let s = s.clamp(0.0, self.length);
        match &self.turn {
            Some(b) => b.position_at_length(s),
            None => {
                let f = if self.length < 1e-12 { 0.0 } else { s / self.length };
                add(scale(self.p1, 1.0 - f), scale(self.p2, f))
            }
        }
    }

    fn sample(&self, f: f64) -> V3 {
        self.position(f * self.length)
    }

    /// `getDirection(fromFirst)`: direction leaving `node1` (true) or arriving at `node2`.
    pub fn direction(&self, from_first: bool) -> V3 {
        if self.length < 1e-12 {
            return normalize(sub(self.p2, self.p1));
        }
        let (a, b) = if from_first { (0.0, 0.25) } else { (0.75, 1.0) };
        normalize(sub(self.sample(b), self.sample(a)))
    }

    /// Unit direction at distance `s` (half-block central difference).
    pub fn direction_at(&self, s: f64) -> V3 {
        let step = 0.5_f64.min(self.length / 2.0).max(1e-6);
        let a = self.position((s - step).max(0.0));
        let b = self.position((s + step).min(self.length));
        let d = normalize(sub(b, a));
        if d == [0.0; 3] { normalize(sub(self.p2, self.p1)) } else { d }
    }

    /// `canTravelTo`: arrival direction of `self` vs departure direction of `other` must align.
    pub fn can_travel_to(&self, other: &TrackEdge) -> bool {
        if self.interdimensional || other.interdimensional {
            return true;
        }
        dot(self.direction(false), other.direction(true)) > 7.0 / 8.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PointKind {
    Signal,
    Station,
    Observer,
}

/// An edge point (`TrackEdgePoint`): `pos` is measured from the canonical
/// edge's first node (the smaller node id).
#[derive(Clone, Debug, PartialEq)]
pub struct EdgePoint {
    pub id: PointId,
    pub kind: PointKind,
    pub pos: f64,
}

/// Canonical (sorted) form of an undirected edge.
pub fn canon(a: NodeId, b: NodeId) -> Dir {
    if a <= b { (a, b) } else { (b, a) }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Crossing {
    pub a: Dir,
    pub b: Dir,
}

/// Saving form (plain data; the adapter encodes it to NBT or similar).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct GraphSave {
    pub id: GraphId,
    pub next_node: NodeId,
    pub nodes: Vec<(NodeId, u32, V3, V3)>,
    /// `(a, b, material, bezier)` for canonical `a < b` edges only; the reverse edge is derived.
    pub edges: Vec<(NodeId, NodeId, String, Option<BezierSave>)>,
    pub points: Vec<(NodeId, NodeId, PointId, PointKind, f64)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BezierSave {
    pub axis1: V3,
    pub axis2: V3,
    pub normal1: V3,
    pub normal2: V3,
    pub girder: bool,
}

#[derive(Clone, Debug)]
pub struct TrackGraph {
    pub id: GraphId,
    nodes: BTreeMap<NodeId, TrackNode>,
    by_key: HashMap<NodeKey, NodeId>,
    conns: BTreeMap<NodeId, BTreeMap<NodeId, TrackEdge>>,
    points: BTreeMap<Dir, Vec<EdgePoint>>,
    next_node: NodeId,
}

impl TrackGraph {
    pub fn new(id: GraphId) -> Self {
        Self { id, nodes: BTreeMap::new(), by_key: HashMap::new(), conns: BTreeMap::new(), points: BTreeMap::new(), next_node: 0 }
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn nodes(&self) -> impl Iterator<Item = &TrackNode> {
        self.nodes.values()
    }

    pub fn node(&self, id: NodeId) -> Option<&TrackNode> {
        self.nodes.get(&id)
    }

    pub fn find_node(&self, dim: u32, pos: V3) -> Option<NodeId> {
        self.by_key.get(&NodeKey::new(dim, pos)).copied()
    }

    /// Add a node or return the existing one at the same location.
    pub fn add_node(&mut self, dim: u32, pos: V3, normal: V3) -> NodeId {
        let key = NodeKey::new(dim, pos);
        if let Some(&id) = self.by_key.get(&key) {
            return id;
        }
        let id = self.next_node;
        self.next_node += 1;
        self.nodes.insert(id, TrackNode { id, dim, pos, normal });
        self.by_key.insert(key, id);
        self.conns.entry(id).or_default();
        id
    }

    pub fn edge(&self, a: NodeId, b: NodeId) -> Option<&TrackEdge> {
        self.conns.get(&a)?.get(&b)
    }

    /// Outgoing neighbours of a node, ordered by id.
    pub fn neighbours(&self, a: NodeId) -> impl Iterator<Item = (NodeId, &TrackEdge)> {
        self.conns.get(&a).into_iter().flat_map(|m| m.iter().map(|(k, v)| (*k, v)))
    }

    pub fn edge_count(&self) -> usize {
        self.conns.values().map(|m| m.len()).sum::<usize>() / 2
    }

    /// Connect two existing nodes (both directions). `bezier` must run from `a` to `b`.
    /// Returns false when a node is missing, a == b, or the edge already exists.
    pub fn connect(&mut self, a: NodeId, b: NodeId, material: TrackType, bezier: Option<BezierConnection>) -> bool {
        if a == b || !self.nodes.contains_key(&a) || !self.nodes.contains_key(&b) || self.edge(a, b).is_some() {
            return false;
        }
        let (na, nb) = (self.nodes[&a].clone(), self.nodes[&b].clone());
        let interdim = na.dim != nb.dim;
        let make = |n1: &TrackNode, n2: &TrackNode, turn: Option<BezierConnection>| {
            let length = if interdim {
                0.0
            } else if let Some(t) = &turn {
                t.length()
            } else {
                dist(n1.pos, n2.pos)
            };
            TrackEdge { node1: n1.id, node2: n2.id, turn, material: material.clone(), interdimensional: interdim, length, p1: n1.pos, p2: n2.pos }
        };
        let rev = bezier.as_ref().map(|b| b.reversed());
        let fwd_edge = make(&na, &nb, bezier);
        let rev_edge = make(&nb, &na, rev);
        self.conns.entry(a).or_default().insert(b, fwd_edge);
        self.conns.entry(b).or_default().insert(a, rev_edge);
        true
    }

    /// Remove an edge and its points; returns the removed points.
    pub fn disconnect(&mut self, a: NodeId, b: NodeId) -> Vec<EdgePoint> {
        if let Some(m) = self.conns.get_mut(&a) {
            m.remove(&b);
        }
        if let Some(m) = self.conns.get_mut(&b) {
            m.remove(&a);
        }
        self.points.remove(&canon(a, b)).unwrap_or_default()
    }

    /// Remove a node with all its edges. Returns removed edge points.
    pub fn remove_node(&mut self, id: NodeId) -> Vec<EdgePoint> {
        let mut removed = Vec::new();
        let others: Vec<NodeId> = self.conns.get(&id).map(|m| m.keys().copied().collect()).unwrap_or_default();
        for o in others {
            removed.extend(self.disconnect(id, o));
        }
        self.conns.remove(&id);
        if let Some(n) = self.nodes.remove(&id) {
            self.by_key.remove(&n.key());
        }
        removed
    }

    // ------------------------------------------------------------ edge points

    /// Place a point on an edge at `pos` measured from the **canonical first node**.
    pub fn add_point(&mut self, a: NodeId, b: NodeId, point: EdgePoint) -> bool {
        let Some(e) = self.edge(a, b) else { return false };
        let key = canon(a, b);
        let l = e.length();
        if point.pos < 0.0 || point.pos > l + 1e-9 {
            return false;
        }
        let list = self.points.entry(key).or_default();
        if list.iter().any(|p| p.id == point.id) {
            return false;
        }
        list.push(point);
        list.sort_by(|x, y| x.pos.total_cmp(&y.pos).then(x.id.cmp(&y.id)));
        true
    }

    pub fn remove_point(&mut self, id: PointId) -> Option<(Dir, EdgePoint)> {
        for (k, list) in self.points.iter_mut() {
            if let Some(i) = list.iter().position(|p| p.id == id) {
                return Some((*k, list.remove(i)));
            }
        }
        None
    }

    pub fn points_on(&self, a: NodeId, b: NodeId) -> &[EdgePoint] {
        self.points.get(&canon(a, b)).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn find_point(&self, id: PointId) -> Option<(Dir, &EdgePoint)> {
        self.points.iter().find_map(|(k, l)| l.iter().find(|p| p.id == id).map(|p| (*k, p)))
    }

    /// Position of a point measured along the directed edge `a -> b`.
    pub fn directed_pos(&self, a: NodeId, b: NodeId, canonical_pos: f64) -> f64 {
        if a <= b { canonical_pos } else { self.edge(a, b).map(|e| e.length()).unwrap_or(0.0) - canonical_pos }
    }

    /// Points on `a -> b` as `(directed position, point)`, ascending in travel direction.
    pub fn directed_points(&self, a: NodeId, b: NodeId) -> Vec<(f64, &EdgePoint)> {
        let mut v: Vec<(f64, &EdgePoint)> = self.points_on(a, b).iter().map(|p| (self.directed_pos(a, b, p.pos), p)).collect();
        v.sort_by(|x, y| x.0.total_cmp(&y.0));
        v
    }

    // ------------------------------------------------------ connectivity

    /// Connected components (undirected), each as a sorted set.
    pub fn components(&self) -> Vec<BTreeSet<NodeId>> {
        let mut seen: BTreeSet<NodeId> = BTreeSet::new();
        let mut out = Vec::new();
        for &start in self.nodes.keys() {
            if seen.contains(&start) {
                continue;
            }
            let mut comp = BTreeSet::new();
            let mut stack = vec![start];
            while let Some(n) = stack.pop() {
                if !comp.insert(n) {
                    continue;
                }
                for (m, _) in self.neighbours(n) {
                    if !comp.contains(&m) {
                        stack.push(m);
                    }
                }
            }
            seen.extend(comp.iter().copied());
            out.push(comp);
        }
        out
    }

    /// Split off every connected component except the one containing the lowest
    /// node id (`TrackGraph.removeNode` + `Create.RAILWAYS.sync` split). Node ids
    /// are preserved in the new graphs; `alloc_id` provides their graph ids.
    pub fn split_disconnected(&mut self, mut alloc_id: impl FnMut() -> GraphId) -> Vec<TrackGraph> {
        let comps = self.components();
        let mut out = Vec::new();
        for comp in comps.into_iter().skip(1) {
            let mut g = TrackGraph::new(alloc_id());
            g.next_node = self.next_node;
            for &n in &comp {
                if let Some(node) = self.nodes.remove(&n) {
                    self.by_key.remove(&node.key());
                    g.by_key.insert(node.key(), n);
                    g.nodes.insert(n, node);
                }
                if let Some(c) = self.conns.remove(&n) {
                    g.conns.insert(n, c);
                }
            }
            let keys: Vec<Dir> = self.points.keys().copied().filter(|(a, _)| comp.contains(a)).collect();
            for k in keys {
                if let Some(p) = self.points.remove(&k) {
                    g.points.insert(k, p);
                }
            }
            out.push(g);
        }
        out
    }

    /// Merge `other` into `self`. Nodes at the same location are unified; the
    /// returned map gives every `other` node id's new id. Edge points are kept
    /// (their canonical positions are re-oriented when the id order flips).
    pub fn merge(&mut self, other: TrackGraph) -> HashMap<NodeId, NodeId> {
        let mut map = HashMap::new();
        for node in other.nodes.values() {
            let id = self.add_node(node.dim, node.pos, node.normal);
            map.insert(node.id, id);
        }
        for (a, m) in &other.conns {
            for (b, e) in m {
                if a < b {
                    let (na, nb) = (map[a], map[b]);
                    self.connect(na, nb, e.material.clone(), e.turn.clone());
                }
            }
        }
        for ((a, b), list) in &other.points {
            let (na, nb) = (map[a], map[b]);
            let flip = canon(na, nb) != (na, nb);
            let Some(len) = self.edge(na, nb).map(|e| e.length()) else { continue };
            for p in list {
                let mut p = p.clone();
                if flip {
                    p.pos = len - p.pos;
                }
                self.add_point(na, nb, p);
            }
        }
        map
    }

    // ------------------------------------------------------------ crossings

    /// Pairs of edges that cross each other in the horizontal plane (diamond
    /// crossings and bezier overlaps). Polylines are sampled every 0.5 blocks and
    /// compared when the heights differ by less than one block.
    pub fn find_crossings(&self) -> Vec<Crossing> {
        let mut edges: Vec<(Dir, Vec<V3>)> = Vec::new();
        for (a, m) in &self.conns {
            for (b, e) in m {
                if a < b && !e.interdimensional {
                    let n = ((e.length() / 0.5).ceil() as usize).max(1);
                    let pts: Vec<V3> = (0..=n).map(|i| e.position(e.length() * i as f64 / n as f64)).collect();
                    edges.push(((*a, *b), pts));
                }
            }
        }
        let mut out = Vec::new();
        for i in 0..edges.len() {
            for j in (i + 1)..edges.len() {
                let (ka, pa) = &edges[i];
                let (kb, pb) = &edges[j];
                if ka.0 == kb.0 || ka.0 == kb.1 || ka.1 == kb.0 || ka.1 == kb.1 {
                    continue; // sharing a node is a junction, not a crossing
                }
                if self.nodes[&ka.0].dim != self.nodes[&kb.0].dim {
                    continue;
                }
                if polylines_cross(pa, pb) {
                    out.push(Crossing { a: *ka, b: *kb });
                }
            }
        }
        out
    }

    // ------------------------------------------------------------- saving

    pub fn save(&self) -> GraphSave {
        let mut s = GraphSave { id: self.id, next_node: self.next_node, ..Default::default() };
        for n in self.nodes.values() {
            s.nodes.push((n.id, n.dim, n.pos, n.normal));
        }
        for (a, m) in &self.conns {
            for (b, e) in m {
                if a < b {
                    let bz = e.turn.as_ref().map(|t| BezierSave { axis1: t.axis1, axis2: t.axis2, normal1: t.normal1, normal2: t.normal2, girder: t.girder });
                    s.edges.push((*a, *b, e.material.0.clone(), bz));
                }
            }
        }
        for ((a, b), list) in &self.points {
            for p in list {
                s.points.push((*a, *b, p.id, p.kind, p.pos));
            }
        }
        s
    }

    pub fn load(s: &GraphSave) -> Self {
        let mut g = TrackGraph::new(s.id);
        for (id, dim, pos, normal) in &s.nodes {
            let n = TrackNode { id: *id, dim: *dim, pos: *pos, normal: *normal };
            g.by_key.insert(n.key(), *id);
            g.nodes.insert(*id, n);
            g.conns.entry(*id).or_default();
        }
        g.next_node = s.next_node;
        for (a, b, mat, bz) in &s.edges {
            let (pa, pb) = match (g.nodes.get(a), g.nodes.get(b)) {
                (Some(x), Some(y)) => (x.pos, y.pos),
                _ => continue,
            };
            let turn = bz.as_ref().map(|z| BezierConnection::new(pa, pb, z.axis1, z.axis2, z.normal1, z.normal2, z.girder));
            g.connect(*a, *b, TrackType(mat.clone()), turn);
        }
        for (a, b, id, kind, pos) in &s.points {
            g.points.entry((*a, *b)).or_default().push(EdgePoint { id: *id, kind: *kind, pos: *pos });
        }
        g
    }
}

fn seg_intersect(p: V3, p2: V3, q: V3, q2: V3) -> bool {
    let r = [p2[0] - p[0], p2[2] - p[2]];
    let s = [q2[0] - q[0], q2[2] - q[2]];
    let den = r[0] * s[1] - r[1] * s[0];
    if den.abs() < 1e-12 {
        return false;
    }
    let d = [q[0] - p[0], q[2] - p[2]];
    let t = (d[0] * s[1] - d[1] * s[0]) / den;
    let u = (d[0] * r[1] - d[1] * r[0]) / den;
    if !(0.0..=1.0).contains(&t) || !(0.0..=1.0).contains(&u) {
        return false;
    }
    let y1 = p[1] + (p2[1] - p[1]) * t;
    let y2 = q[1] + (q2[1] - q[1]) * u;
    (y1 - y2).abs() < 1.0
}

fn polylines_cross(a: &[V3], b: &[V3]) -> bool {
    for w in a.windows(2) {
        for v in b.windows(2) {
            if seg_intersect(w[0], w[1], v[0], v[1]) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::train_track::UP;

    fn std() -> TrackType {
        TrackType::standard()
    }

    fn line(g: &mut TrackGraph, n: usize) -> Vec<NodeId> {
        let ids: Vec<NodeId> = (0..n).map(|i| g.add_node(0, [0.5, 0.0, i as f64 * 10.0], UP)).collect();
        for w in ids.windows(2) {
            assert!(g.connect(w[0], w[1], std(), None));
        }
        ids
    }

    #[test]
    fn nodes_dedupe_and_edges_symmetric() {
        let mut g = TrackGraph::new(1);
        let a = g.add_node(0, [0.0, 0.0, 0.0], UP);
        assert_eq!(g.add_node(0, [0.0, 0.0, 0.0], UP), a);
        assert_ne!(g.add_node(1, [0.0, 0.0, 0.0], UP), a);
        let b = g.add_node(0, [3.0, 0.0, 4.0], UP);
        assert!(g.connect(a, b, std(), None));
        assert!(!g.connect(b, a, std(), None));
        assert!((g.edge(a, b).unwrap().length() - 5.0).abs() < 1e-12);
        assert!((g.edge(b, a).unwrap().length() - 5.0).abs() < 1e-12);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.find_node(0, [3.0, 0.0, 4.0]), Some(b));
    }

    #[test]
    fn interdimensional_edge_has_zero_length_and_travels_anywhere() {
        let mut g = TrackGraph::new(1);
        let a = g.add_node(0, [0.0; 3], UP);
        let b = g.add_node(1, [0.0, 0.0, 0.0], UP);
        let c = g.add_node(0, [0.0, 0.0, 10.0], UP);
        g.connect(a, b, std(), None);
        g.connect(a, c, std(), None);
        assert_eq!(g.edge(a, b).unwrap().length(), 0.0);
        assert!(g.edge(c, a).unwrap().can_travel_to(g.edge(a, b).unwrap()));
    }

    #[test]
    fn curve_edges_use_bezier_length_and_reverse() {
        let mut g = TrackGraph::new(1);
        let a = g.add_node(0, [0.0, 0.0, 0.0], UP);
        let b = g.add_node(0, [8.0, 0.0, 8.0], UP);
        let bz = BezierConnection::new([0.0; 3], [8.0, 0.0, 8.0], [0.0, 0.0, 1.0], [-1.0, 0.0, 0.0], UP, UP, false);
        let len = bz.length();
        g.connect(a, b, std(), Some(bz));
        assert!((g.edge(a, b).unwrap().length() - len).abs() < 1e-9);
        assert!((g.edge(b, a).unwrap().length() - len).abs() < 1e-6);
        let mid = g.edge(a, b).unwrap().position(len / 2.0);
        let mid_r = g.edge(b, a).unwrap().position(len / 2.0);
        assert!(dist(mid, mid_r) < 0.05);
        assert!(g.edge(a, b).unwrap().is_turn());
    }

    #[test]
    fn can_travel_to_rejects_sharp_turns_and_accepts_straight() {
        let mut g = TrackGraph::new(1);
        let a = g.add_node(0, [0.0, 0.0, 0.0], UP);
        let b = g.add_node(0, [0.0, 0.0, 10.0], UP);
        let c = g.add_node(0, [0.0, 0.0, 20.0], UP);
        let d = g.add_node(0, [10.0, 0.0, 10.0], UP);
        g.connect(a, b, std(), None);
        g.connect(b, c, std(), None);
        g.connect(b, d, std(), None);
        let ab = g.edge(a, b).unwrap();
        assert!(ab.can_travel_to(g.edge(b, c).unwrap()));
        assert!(!ab.can_travel_to(g.edge(b, d).unwrap())); // 90 degrees: dot 0 < 7/8
        assert!(!ab.can_travel_to(g.edge(b, a).unwrap())); // U-turn
    }

    #[test]
    fn components_split_and_merge_roundtrip() {
        let mut g = TrackGraph::new(1);
        let ids = line(&mut g, 5);
        assert_eq!(g.components().len(), 1);
        let mut gid = 10;
        g.remove_node(ids[2]);
        let split = g.split_disconnected(|| {
            gid += 1;
            gid
        });
        assert_eq!(split.len(), 1);
        assert_eq!(g.node_count(), 2);
        assert_eq!(split[0].node_count(), 2);
        assert_eq!(split[0].id, 11);
        assert!(split[0].edge(ids[3], ids[4]).is_some());
        // rebuild the missing node and merge back
        let mut g2 = g.clone();
        let mid = g2.add_node(0, [0.5, 0.0, 20.0], UP);
        let map = g2.merge(split.into_iter().next().unwrap());
        assert_eq!(map.len(), 2);
        assert!(g2.connect(ids[1], mid, std(), None));
        assert!(g2.connect(mid, map[&ids[3]], std(), None));
        assert_eq!(g2.node_count(), 5);
        assert_eq!(g2.components().len(), 1);
    }

    #[test]
    fn merge_unifies_shared_locations_and_flips_points() {
        let mut a = TrackGraph::new(1);
        let n0 = a.add_node(0, [0.0, 0.0, 0.0], UP);
        let n1 = a.add_node(0, [0.0, 0.0, 10.0], UP);
        a.connect(n0, n1, std(), None);
        let mut b = TrackGraph::new(2);
        // inserted in reverse so ids are swapped relative to a
        let m1 = b.add_node(0, [0.0, 0.0, 10.0], UP);
        let m0 = b.add_node(0, [0.0, 0.0, 0.0], UP);
        b.connect(m1, m0, std(), None);
        assert!(b.add_point(m1, m0, EdgePoint { id: 7, kind: PointKind::Station, pos: 2.0 }));
        let map = a.merge(b);
        assert_eq!(a.node_count(), 2);
        assert_eq!(map[&m0], n0);
        assert_eq!(map[&m1], n1);
        // canonical order in b was (m1=0, m0=1) with pos from m1 (z=10): 2.0 -> from z=10
        // in a canonical is (n0, n1); pos from n0 = 10 - 2 = 8
        let p = a.points_on(n0, n1);
        assert_eq!(p.len(), 1);
        assert!((p[0].pos - 8.0).abs() < 1e-9);
        assert!((a.directed_pos(n1, n0, p[0].pos) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn points_sorted_bounded_and_removed_with_edges() {
        let mut g = TrackGraph::new(1);
        let ids = line(&mut g, 2);
        let (a, b) = (ids[0], ids[1]);
        assert!(g.add_point(a, b, EdgePoint { id: 2, kind: PointKind::Signal, pos: 7.0 }));
        assert!(g.add_point(b, a, EdgePoint { id: 1, kind: PointKind::Signal, pos: 3.0 }));
        assert!(!g.add_point(a, b, EdgePoint { id: 3, kind: PointKind::Signal, pos: 11.0 }));
        assert!(!g.add_point(a, b, EdgePoint { id: 2, kind: PointKind::Signal, pos: 5.0 }));
        let d = g.directed_points(b, a);
        assert_eq!(d[0].1.id, 2); // pos 10-7 = 3 from b first
        assert!((d[0].0 - 3.0).abs() < 1e-9);
        assert_eq!(g.find_point(1).unwrap().0, (a, b));
        let removed = g.disconnect(a, b);
        assert_eq!(removed.len(), 2);
        assert!(g.find_point(1).is_none());
    }

    #[test]
    fn diamond_crossing_detected() {
        let mut g = TrackGraph::new(1);
        let a = g.add_node(0, [-5.0, 0.0, 0.0], UP);
        let b = g.add_node(0, [5.0, 0.0, 0.0], UP);
        let c = g.add_node(0, [0.0, 0.0, -5.0], UP);
        let d = g.add_node(0, [0.0, 0.0, 5.0], UP);
        g.connect(a, b, std(), None);
        g.connect(c, d, std(), None);
        assert_eq!(g.find_crossings().len(), 1);
        // 3 blocks above: bridge, not a crossing
        let mut h = TrackGraph::new(2);
        let a = h.add_node(0, [-5.0, 0.0, 0.0], UP);
        let b = h.add_node(0, [5.0, 0.0, 0.0], UP);
        let c = h.add_node(0, [0.0, 3.0, -5.0], UP);
        let d = h.add_node(0, [0.0, 3.0, 5.0], UP);
        h.connect(a, b, std(), None);
        h.connect(c, d, std(), None);
        assert!(h.find_crossings().is_empty());
    }

    #[test]
    fn save_load_roundtrip() {
        let mut g = TrackGraph::new(9);
        let a = g.add_node(0, [0.0, 0.0, 0.0], UP);
        let b = g.add_node(0, [8.0, 0.0, 8.0], UP);
        let c = g.add_node(0, [16.0, 0.0, 8.0], UP);
        let bz = BezierConnection::new([0.0; 3], [8.0, 0.0, 8.0], [0.0, 0.0, 1.0], [-1.0, 0.0, 0.0], UP, UP, true);
        g.connect(a, b, std(), Some(bz));
        g.connect(b, c, std(), None);
        g.add_point(b, c, EdgePoint { id: 5, kind: PointKind::Station, pos: 4.0 });
        let s = g.save();
        let h = TrackGraph::load(&s);
        assert_eq!(h.save(), s);
        assert_eq!(h.node_count(), 3);
        assert!((h.edge(a, b).unwrap().length() - g.edge(a, b).unwrap().length()).abs() < 1e-12);
        assert_eq!(h.points_on(b, c).len(), 1);
        assert_eq!(h.find_node(0, [16.0, 0.0, 8.0]), Some(c));
        // adding to a loaded graph never reuses ids
        let mut h = h;
        assert_eq!(h.add_node(0, [99.0, 0.0, 0.0], UP), 3);
    }
}
