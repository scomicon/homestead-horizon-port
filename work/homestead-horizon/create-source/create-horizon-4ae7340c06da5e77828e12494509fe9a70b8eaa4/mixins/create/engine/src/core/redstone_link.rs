// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Pure Redstone Link network after RedstoneLinkNetworkHandler, LinkBehaviour,
// RedstoneLinkBlock / RedstoneLinkBlockEntity and the Linked Controller
// server/client handlers (Create 6.0.10, redstone/link/**).
//
// One `LinkNetwork` stands for the per-level map of the Java handler. The
// adapter owns one per level (`onLoadWorld` = `LinkNetwork::new`,
// `onUnloadWorld` = drop) and calls the `*_signal` helpers from block code.
use std::collections::BTreeMap;

/// Server default of `logistics.linkRange` in 6.0.10 (CLogistics, min 1).
/// The adapter should pass the configured value to `LinkNetwork::new`.
pub const DEFAULT_LINK_RANGE: i32 = 256;
/// Lifetime in ticks of a held Linked Controller input (`TIMEOUT`).
pub const CONTROLLER_TIMEOUT: i32 = 30;
/// Client keep-alive packet cadence in ticks (`PACKET_RATE`).
pub const CONTROLLER_PACKET_RATE: i32 = 5;
/// Frequency slots of a Linked Controller: 6 buttons x 2 items.
pub const CONTROLLER_SLOTS: usize = 12;
pub const CONTROLLER_BUTTONS: usize = 6;
pub const MAX_SIGNAL: i32 = 15;

/// Block face, vanilla `Direction` order (`DOWN, UP, NORTH, SOUTH, WEST, EAST`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Direction {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl Direction {
    pub const ALL: [Direction; 6] = [
        Direction::Down,
        Direction::Up,
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ];

    pub fn opposite(self) -> Direction {
        match self {
            Direction::Down => Direction::Up,
            Direction::Up => Direction::Down,
            Direction::North => Direction::South,
            Direction::South => Direction::North,
            Direction::West => Direction::East,
            Direction::East => Direction::West,
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn step(self) -> [i32; 3] {
        match self {
            Direction::Down => [0, -1, 0],
            Direction::Up => [0, 1, 0],
            Direction::North => [0, 0, -1],
            Direction::South => [0, 0, 1],
            Direction::West => [-1, 0, 0],
            Direction::East => [1, 0, 0],
        }
    }

    pub fn is_horizontal(self) -> bool {
        !matches!(self, Direction::Down | Direction::Up)
    }

    /// Clockwise quarter turn about the Y axis (horizontal faces only).
    pub fn clockwise(self) -> Direction {
        match self {
            Direction::North => Direction::East,
            Direction::East => Direction::South,
            Direction::South => Direction::West,
            Direction::West => Direction::North,
            other => other,
        }
    }

    pub fn counter_clockwise(self) -> Direction {
        self.clockwise().clockwise().clockwise()
    }
}

/// `RedstoneLinkNetworkHandler.Frequency`: item identity plus dye colour.
/// Equality ignores every other component, as in Java.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Frequency {
    item: String,
    color: i32,
}

impl Frequency {
    pub fn empty() -> Self {
        Self { item: String::new(), color: -1 }
    }

    /// `Frequency.of(stack)`. `item` is the registry id, `dyed_color` the
    /// `DYED_COLOR` component RGB if present.
    pub fn of(item: &str, dyed_color: Option<i32>) -> Self {
        if item.is_empty() || item == "minecraft:air" {
            return Self::empty();
        }
        Self { item: item.to_string(), color: dyed_color.unwrap_or(-1) }
    }

    pub fn is_empty(&self) -> bool {
        self.item.is_empty()
    }

    pub fn item(&self) -> &str {
        &self.item
    }
}

pub type FrequencyPair = (Frequency, Frequency);

/// What a network member does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkMode {
    /// Redstone Link in transmitter mode; `strength` is `getSignal()`.
    Transmit { strength: i32 },
    /// Redstone Link in receiver mode; `received` is the block entity value.
    Receive { received: i32 },
    /// Held Linked Controller input (`ManualFrequencyEntry`).
    Manual { timer: i32 },
}

#[derive(Clone, Debug)]
pub struct Linkable {
    pub pos: [i32; 3],
    pub key: FrequencyPair,
    pub mode: LinkMode,
    /// `isAlive()`: false when the chunk or block entity is gone.
    pub alive: bool,
    /// `LinkBehaviour.newPosition`; gates `setReceivedStrength`.
    pub new_position: bool,
}

impl Linkable {
    pub fn transmitted_strength(&self) -> i32 {
        match self.mode {
            LinkMode::Transmit { strength } => strength,
            LinkMode::Manual { timer } if timer > 0 => MAX_SIGNAL,
            _ => 0,
        }
    }

    pub fn is_listening(&self) -> bool {
        matches!(self.mode, LinkMode::Receive { .. })
    }

    pub fn received(&self) -> i32 {
        match self.mode {
            LinkMode::Receive { received } => received,
            _ => 0,
        }
    }

    fn is_alive(&self) -> bool {
        match self.mode {
            LinkMode::Manual { timer } => timer > 0 && self.alive,
            _ => self.alive,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LinkId(pub u64);

/// `withinRange`: strict squared-distance comparison (`Vec3i.closerThan`).
pub fn within_range(a: [i32; 3], b: [i32; 3], range: i32) -> bool {
    let d = |i: usize| (a[i] as i64 - b[i] as i64).pow(2);
    d(0) + d(1) + d(2) < (range as i64) * (range as i64)
}

/// Per-level network map.
#[derive(Clone, Debug)]
pub struct LinkNetwork {
    range: i32,
    next_id: u64,
    entries: BTreeMap<LinkId, Linkable>,
    groups: BTreeMap<FrequencyPair, Vec<LinkId>>,
    power_version: u64,
}

impl LinkNetwork {
    pub fn new(range: i32) -> Self {
        Self { range: range.max(1), next_id: 1, entries: BTreeMap::new(), groups: BTreeMap::new(), power_version: 0 }
    }

    /// `globalPowerVersion`, bumped by every `update_network_of`.
    pub fn power_version(&self) -> u64 {
        self.power_version
    }

    pub fn get(&self, id: LinkId) -> Option<&Linkable> {
        self.entries.get(&id)
    }

    pub fn get_mut(&mut self, id: LinkId) -> Option<&mut Linkable> {
        self.entries.get_mut(&id)
    }

    pub fn group_len(&self, key: &FrequencyPair) -> usize {
        self.groups.get(key).map_or(0, Vec::len)
    }

    pub fn network_count(&self) -> usize {
        self.groups.len()
    }

    /// Create a member without joining the network (block entity constructed).
    pub fn create(&mut self, pos: [i32; 3], key: FrequencyPair, mode: LinkMode) -> LinkId {
        let id = LinkId(self.next_id);
        self.next_id += 1;
        self.entries.insert(id, Linkable { pos, key, mode, alive: true, new_position: true });
        id
    }

    /// `addToNetwork`: join the group of the key, then update from this actor.
    pub fn add_to_network(&mut self, id: LinkId) -> Vec<(LinkId, i32)> {
        let Some(entry) = self.entries.get(&id) else { return Vec::new() };
        let group = self.groups.entry(entry.key.clone()).or_default();
        if !group.contains(&id) {
            group.push(id);
        }
        self.update_network_of(id)
    }

    /// `removeFromNetwork`: leave the group; an empty group is dropped silently,
    /// otherwise the network is re-evaluated from the removed actor's position.
    /// The entry itself is deleted afterwards.
    pub fn remove_from_network(&mut self, id: LinkId) -> Vec<(LinkId, i32)> {
        let Some(key) = self.entries.get(&id).map(|e| e.key.clone()) else { return Vec::new() };
        let mut writes = Vec::new();
        if let Some(group) = self.groups.get_mut(&key) {
            group.retain(|m| *m != id);
            if group.is_empty() {
                self.groups.remove(&key);
            } else {
                writes = self.update_network_of(id);
            }
        }
        self.entries.remove(&id);
        writes
    }

    /// `LinkBehaviour.setFrequency`: leave, change key, rejoin.
    pub fn set_frequency(&mut self, id: LinkId, key: FrequencyPair) -> Vec<(LinkId, i32)> {
        let Some(entry) = self.entries.get(&id) else { return Vec::new() };
        if entry.key == key {
            return Vec::new();
        }
        // Leave without deleting the entry.
        let old = entry.key.clone();
        let mut writes = Vec::new();
        if let Some(group) = self.groups.get_mut(&old) {
            group.retain(|m| *m != id);
            if group.is_empty() {
                self.groups.remove(&old);
            } else {
                writes = self.update_network_of(id);
            }
        }
        if let Some(e) = self.entries.get_mut(&id) {
            e.key = key;
        }
        writes.extend(self.add_to_network(id));
        writes
    }

    /// `LinkedControllerServerHandler`-independent `notifySignalChange`.
    pub fn notify_signal_change(&mut self, id: LinkId) -> Vec<(LinkId, i32)> {
        self.update_network_of(id)
    }

    /// Set a transmitter's strength (`RedstoneLinkBlockEntity.transmit`) and
    /// notify the network.
    pub fn transmit(&mut self, id: LinkId, strength: i32) -> Vec<(LinkId, i32)> {
        if let Some(e) = self.entries.get_mut(&id) {
            if let LinkMode::Transmit { strength: s } = &mut e.mode {
                *s = strength;
            }
        }
        self.update_network_of(id)
    }

    /// `LinkBehaviour.setReceivedStrength`: only honoured while `new_position`.
    fn set_received(&mut self, id: LinkId, power: i32) -> Option<(LinkId, i32)> {
        let e = self.entries.get_mut(&id)?;
        if !e.new_position {
            return None;
        }
        if let LinkMode::Receive { received } = &mut e.mode {
            *received = power;
            return Some((id, power));
        }
        None
    }

    /// `updateNetworkOf`. Returns every receiver written (Java writes
    /// unconditionally, with no change detection). Dead members are pruned
    /// from the group while scanning. The max rule is reproduced exactly:
    /// the `< 15` guard is checked before the `max`.
    pub fn update_network_of(&mut self, actor: LinkId) -> Vec<(LinkId, i32)> {
        self.power_version += 1;
        let Some(actor_entry) = self.entries.get(&actor) else { return Vec::new() };
        let key = actor_entry.key.clone();
        let origin = actor_entry.pos;
        let range = self.range;

        let mut members = self.groups.get(&key).cloned().unwrap_or_default();
        let mut power = 0;
        members.retain(|id| {
            let Some(other) = self.entries.get(id) else { return false };
            if !other.is_alive() {
                return false;
            }
            if *id == actor || within_range(origin, other.pos, range) {
                if power < MAX_SIGNAL {
                    power = other.transmitted_strength().max(power);
                }
            }
            true
        });
        if let Some(g) = self.groups.get_mut(&key) {
            *g = members.clone();
        }

        let mut writes = Vec::new();
        if self.entries.get(&actor).is_some_and(Linkable::is_listening) {
            self.entries.get_mut(&actor).unwrap().new_position = true;
            writes.extend(self.set_received(actor, power));
        }
        for id in members {
            if id == actor {
                continue;
            }
            let (listening, pos) = match self.entries.get(&id) {
                Some(e) => (e.is_listening(), e.pos),
                None => continue,
            };
            if listening && within_range(origin, pos, range) {
                writes.extend(self.set_received(id, power));
            }
        }
        writes
    }
}

/// `hasAnyLoadedPower`: any member of the key, in any level, transmitting > 0.
/// Range and liveness are deliberately not checked, as in Java.
pub fn has_any_loaded_power(levels: &[&LinkNetwork], key: &FrequencyPair) -> bool {
    levels.iter().any(|net| {
        net.groups
            .get(key)
            .is_some_and(|g| g.iter().any(|id| net.entries.get(id).is_some_and(|e| e.transmitted_strength() > 0)))
    })
}

// ----------------------------------------------------------------------
// Redstone Link block (RedstoneLinkBlock / RedstoneLinkBlockEntity)
// ----------------------------------------------------------------------

/// Factory-panel contribution to a transmitter (`panelSupport`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelInput {
    None,
    /// `shouldBePoweredTristate()` == null: panel not loaded; abort update.
    NotLoaded,
    Powered(bool),
}

/// `RedstoneLinkBlock.getPower` + `updateTransmittedSignal` combine rule.
/// `direct[d]` = `level.getSignal(pos.relative(d), d)`;
/// `up[d]` = `level.getSignal(pos.relative(d), UP)`.
/// Returns `None` when the update must be aborted (panel not loaded).
pub fn transmitter_power(direct: [i32; 6], up: [i32; 6], facing: Direction, panel: PanelInput) -> Option<i32> {
    let attached = facing.opposite();
    let mut power = 0;
    for d in Direction::ALL {
        power = power.max(direct[d.index()]);
    }
    for d in Direction::ALL {
        if d != attached {
            power = power.max(up[d.index()]);
        }
    }
    let from_panel = match panel {
        PanelInput::None => 0,
        PanelInput::NotLoaded => return None,
        PanelInput::Powered(true) => 15,
        PanelInput::Powered(false) => 0,
    };
    Some(power.max(from_panel))
}

/// `RedstoneLinkBlock.getSignal`: receivers emit the received strength on every
/// side; transmitters never emit.
pub fn link_block_signal(receiver: bool, received: i32) -> i32 {
    if receiver { received } else { 0 }
}

/// `getDirectSignal`: only the front face (`side == FACING`).
pub fn link_block_direct_signal(receiver: bool, received: i32, facing: Direction, side: Direction) -> i32 {
    if side == facing { link_block_signal(receiver, received) } else { 0 }
}

/// `isSignalSource`: `POWERED && RECEIVER`.
pub fn link_block_is_signal_source(powered: bool, receiver: bool) -> bool {
    powered && receiver
}

/// Receiver block-entity tick: sync `POWERED` with `received > 0`.
/// Returns `(new_powered, update_self_and_attached)`.
pub fn receiver_tick(received: i32, powered: bool, received_changed: bool) -> (bool, bool) {
    let mut changed = received_changed;
    let mut new_powered = powered;
    if (received > 0) != powered {
        new_powered = !powered;
        changed = true;
    }
    (new_powered, changed)
}

/// Result of `RedstoneLinkBlock.toggleMode` (wrench / sneak-use).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToggleResult {
    pub receiver: bool,
    pub powered: bool,
    /// Value passed to `transmit`: 0 when it was a receiver, else the power.
    pub transmit: i32,
}

pub fn toggle_mode(was_receiver: bool, has_neighbor_signal: bool, current_power: i32) -> ToggleResult {
    ToggleResult {
        receiver: !was_receiver,
        powered: has_neighbor_signal,
        transmit: if was_receiver { 0 } else { current_power },
    }
}

// ----------------------------------------------------------------------
// Linked Controller (server handler, client packet cadence)
// ----------------------------------------------------------------------

/// `LinkedControllerItem.toFrequency(stack, button)`: pair (2n, 2n+1).
pub fn controller_frequency(items: &[Frequency; CONTROLLER_SLOTS], button: usize) -> Option<FrequencyPair> {
    if button >= CONTROLLER_BUTTONS {
        return None;
    }
    Some((items[button * 2].clone(), items[button * 2 + 1].clone()))
}

/// `LinkedControllerServerHandler`: per-player held inputs backed by
/// `Manual` members of the level's `LinkNetwork`.
#[derive(Clone, Debug, Default)]
pub struct ControllerServer {
    held: BTreeMap<u128, Vec<(FrequencyPair, LinkId)>>,
}

impl ControllerServer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn held_count(&self, player: u128) -> usize {
        self.held.get(&player).map_or(0, Vec::len)
    }

    /// `receivePressed`. Returns `true` when the `LINKED_CONTROLLER`
    /// advancement is awarded (a new press hit a network with a listening
    /// receiver).
    pub fn receive_pressed(
        &mut self,
        net: &mut LinkNetwork,
        pos: [i32; 3],
        player: u128,
        pairs: &[FrequencyPair],
        pressed: bool,
    ) -> bool {
        let mut award = false;
        for pair in pairs {
            let existing = self
                .held
                .get(&player)
                .and_then(|v| v.iter().find(|(k, _)| k == pair))
                .map(|(_, id)| *id);
            if let Some(id) = existing {
                if let Some(e) = net.get_mut(id) {
                    if !pressed {
                        e.mode = LinkMode::Manual { timer: 0 };
                    } else {
                        e.mode = LinkMode::Manual { timer: CONTROLLER_TIMEOUT };
                        e.pos = pos;
                    }
                }
                continue;
            }
            if !pressed {
                continue;
            }
            let id = net.create(pos, pair.clone(), LinkMode::Manual { timer: CONTROLLER_TIMEOUT });
            net.add_to_network(id);
            self.held.entry(player).or_default().push((pair.clone(), id));
            let listening = net
                .groups
                .get(pair)
                .is_some_and(|g| g.iter().any(|m| net.get(*m).is_some_and(Linkable::is_listening)));
            award |= listening;
        }
        award
    }

    /// `tick(world)`: decrement, drop dead entries from the network (which
    /// re-evaluates receivers) and forget empty players. Returns all receiver
    /// writes caused by removals.
    pub fn tick(&mut self, net: &mut LinkNetwork) -> Vec<(LinkId, i32)> {
        let mut writes = Vec::new();
        self.held.retain(|_, list| {
            list.retain(|(_, id)| {
                let alive = match net.get_mut(*id) {
                    Some(e) => {
                        if let LinkMode::Manual { timer } = &mut e.mode {
                            *timer -= 1;
                            *timer > 0
                        } else {
                            false
                        }
                    }
                    None => false,
                };
                if !alive {
                    writes.extend(net.remove_from_network(*id));
                }
                alive
            });
            !list.is_empty()
        });
        writes
    }
}

/// Packet emitted by the client handler.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputPacket {
    pub buttons: Vec<usize>,
    pub pressed: bool,
}

/// `LinkedControllerClientHandler` ACTIVE-mode input diffing (pure part).
#[derive(Clone, Debug, Default)]
pub struct ControllerClient {
    current: Vec<usize>,
    packet_cooldown: i32,
}

impl ControllerClient {
    pub fn new() -> Self {
        Self::default()
    }

    /// One tick with the set of buttons currently pressed. Packet order and
    /// the cooldown rule match Java: released, newly pressed, keep-alive
    /// (skipped while the cooldown is above zero).
    pub fn tick(&mut self, mut pressed: Vec<usize>) -> Vec<InputPacket> {
        pressed.sort_unstable();
        pressed.dedup();
        if self.packet_cooldown > 0 {
            self.packet_cooldown -= 1;
        }
        let new: Vec<usize> = pressed.iter().copied().filter(|b| !self.current.contains(b)).collect();
        let released: Vec<usize> = self.current.iter().copied().filter(|b| !pressed.contains(b)).collect();
        let mut out = Vec::new();
        if !released.is_empty() {
            out.push(InputPacket { buttons: released, pressed: false });
        }
        if !new.is_empty() {
            out.push(InputPacket { buttons: new, pressed: true });
            self.packet_cooldown = CONTROLLER_PACKET_RATE;
        }
        if self.packet_cooldown == 0 && !pressed.is_empty() {
            out.push(InputPacket { buttons: pressed.clone(), pressed: true });
            self.packet_cooldown = CONTROLLER_PACKET_RATE;
        }
        self.current = pressed;
        out
    }

    pub fn reset(&mut self) {
        self.current.clear();
        self.packet_cooldown = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(name: &str) -> Frequency {
        Frequency::of(name, None)
    }
    fn key(a: &str, b: &str) -> FrequencyPair {
        (f(a), f(b))
    }

    #[test]
    fn frequency_equality_ignores_everything_but_item_and_colour() {
        assert_eq!(Frequency::of("minecraft:air", Some(3)), Frequency::empty());
        assert_ne!(Frequency::of("a:b", Some(1)), Frequency::of("a:b", Some(2)));
        assert_eq!(Frequency::of("a:b", None), Frequency::of("a:b", None));
        assert!(Frequency::of("", None).is_empty());
    }

    #[test]
    fn range_is_strict_squared_distance() {
        assert!(within_range([0, 0, 0], [255, 0, 0], 256));
        assert!(!within_range([0, 0, 0], [256, 0, 0], 256));
    }

    #[test]
    fn range_diagonal_uses_euclid() {
        assert!(within_range([0, 0, 0], [160, 160, 0], 256));
        assert!(!within_range([0, 0, 0], [200, 200, 0], 256));
    }

    #[test]
    fn transmitter_drives_receiver_and_max_wins() {
        let mut n = LinkNetwork::new(256);
        let k = key("minecraft:stone", "minecraft:dirt");
        let rx = n.create([0, 0, 0], k.clone(), LinkMode::Receive { received: 0 });
        n.add_to_network(rx);
        let t1 = n.create([1, 0, 0], k.clone(), LinkMode::Transmit { strength: 0 });
        n.add_to_network(t1);
        let t2 = n.create([2, 0, 0], k.clone(), LinkMode::Transmit { strength: 0 });
        n.add_to_network(t2);
        n.transmit(t1, 7);
        assert_eq!(n.get(rx).unwrap().received(), 7);
        n.transmit(t2, 11);
        assert_eq!(n.get(rx).unwrap().received(), 11);
        n.transmit(t2, 0);
        assert_eq!(n.get(rx).unwrap().received(), 7);
        n.transmit(t1, 0);
        assert_eq!(n.get(rx).unwrap().received(), 0);
    }

    #[test]
    fn frequencies_are_ordered_pairs() {
        let mut n = LinkNetwork::new(256);
        let rx = n.create([0, 0, 0], key("a:a", "b:b"), LinkMode::Receive { received: 0 });
        n.add_to_network(rx);
        let t = n.create([1, 0, 0], key("b:b", "a:a"), LinkMode::Transmit { strength: 15 });
        n.add_to_network(t);
        assert_eq!(n.get(rx).unwrap().received(), 0);
        assert_eq!(n.network_count(), 2);
    }

    #[test]
    fn out_of_range_members_do_not_interact() {
        let mut n = LinkNetwork::new(10);
        let k = key("a:a", "b:b");
        let rx = n.create([0, 0, 0], k.clone(), LinkMode::Receive { received: 0 });
        n.add_to_network(rx);
        let far = n.create([50, 0, 0], k.clone(), LinkMode::Transmit { strength: 9 });
        let w = n.add_to_network(far);
        assert!(w.is_empty());
        assert_eq!(n.get(rx).unwrap().received(), 0);
        let near = n.create([9, 0, 0], k.clone(), LinkMode::Transmit { strength: 4 });
        n.add_to_network(near);
        assert_eq!(n.get(rx).unwrap().received(), 4);
    }

    #[test]
    fn receiver_in_range_of_actor_but_not_of_far_transmitter_uses_actor_origin() {
        // Range is evaluated from the actor, not from each receiver.
        let mut n = LinkNetwork::new(10);
        let k = key("a:a", "b:b");
        let tx = n.create([0, 0, 0], k.clone(), LinkMode::Transmit { strength: 8 });
        n.add_to_network(tx);
        let rx_near = n.create([5, 0, 0], k.clone(), LinkMode::Receive { received: 0 });
        n.add_to_network(rx_near);
        let rx_far = n.create([20, 0, 0], k.clone(), LinkMode::Receive { received: 0 });
        n.add_to_network(rx_far);
        assert_eq!(n.get(rx_near).unwrap().received(), 8);
        assert_eq!(n.get(rx_far).unwrap().received(), 0);
    }

    #[test]
    fn dead_members_are_pruned() {
        let mut n = LinkNetwork::new(256);
        let k = key("a:a", "b:b");
        let tx = n.create([0, 0, 0], k.clone(), LinkMode::Transmit { strength: 15 });
        n.add_to_network(tx);
        let rx = n.create([1, 0, 0], k.clone(), LinkMode::Receive { received: 0 });
        n.add_to_network(rx);
        assert_eq!(n.get(rx).unwrap().received(), 15);
        n.get_mut(tx).unwrap().alive = false;
        n.update_network_of(rx);
        assert_eq!(n.get(rx).unwrap().received(), 0);
        assert_eq!(n.group_len(&k), 1);
    }

    #[test]
    fn removing_transmitter_clears_receivers_and_empty_group_is_dropped() {
        let mut n = LinkNetwork::new(256);
        let k = key("a:a", "b:b");
        let tx = n.create([0, 0, 0], k.clone(), LinkMode::Transmit { strength: 12 });
        n.add_to_network(tx);
        let rx = n.create([1, 0, 0], k.clone(), LinkMode::Receive { received: 0 });
        n.add_to_network(rx);
        assert_eq!(n.get(rx).unwrap().received(), 12);
        n.get_mut(tx).unwrap().mode = LinkMode::Transmit { strength: 12 };
        n.remove_from_network(tx);
        assert_eq!(n.get(rx).unwrap().received(), 0);
        n.remove_from_network(rx);
        assert_eq!(n.network_count(), 0);
    }

    #[test]
    fn newly_added_receiver_gets_current_power() {
        let mut n = LinkNetwork::new(256);
        let k = key("a:a", "b:b");
        let tx = n.create([0, 0, 0], k.clone(), LinkMode::Transmit { strength: 6 });
        n.add_to_network(tx);
        let rx = n.create([3, 0, 0], k.clone(), LinkMode::Receive { received: 0 });
        let w = n.add_to_network(rx);
        assert_eq!(w, vec![(rx, 6)]);
    }

    #[test]
    fn set_received_is_gated_by_new_position() {
        let mut n = LinkNetwork::new(256);
        let k = key("a:a", "b:b");
        let tx = n.create([0, 0, 0], k.clone(), LinkMode::Transmit { strength: 9 });
        n.add_to_network(tx);
        let rx = n.create([1, 0, 0], k.clone(), LinkMode::Receive { received: 3 });
        n.groups.get_mut(&k).unwrap().push(rx);
        n.get_mut(rx).unwrap().new_position = false;
        n.update_network_of(tx);
        assert_eq!(n.get(rx).unwrap().received(), 3);
        // When the receiver itself is the actor, newPosition is raised first.
        n.update_network_of(rx);
        assert_eq!(n.get(rx).unwrap().received(), 9);
    }

    #[test]
    fn max_guard_is_checked_before_max() {
        // A first member above 15 is stored unclamped; later ones are skipped.
        let mut n = LinkNetwork::new(256);
        let k = key("a:a", "b:b");
        let a = n.create([0, 0, 0], k.clone(), LinkMode::Transmit { strength: 20 });
        n.add_to_network(a);
        let rx = n.create([1, 0, 0], k.clone(), LinkMode::Receive { received: 0 });
        n.add_to_network(rx);
        assert_eq!(n.get(rx).unwrap().received(), 20);
    }

    #[test]
    fn power_version_increments_on_every_update() {
        let mut n = LinkNetwork::new(256);
        let k = key("a:a", "b:b");
        let a = n.create([0, 0, 0], k, LinkMode::Transmit { strength: 0 });
        n.add_to_network(a);
        let v = n.power_version();
        n.transmit(a, 0);
        assert_eq!(n.power_version(), v + 1);
    }

    #[test]
    fn set_frequency_moves_between_networks() {
        let mut n = LinkNetwork::new(256);
        let k1 = key("a:a", "b:b");
        let k2 = key("c:c", "d:d");
        let tx = n.create([0, 0, 0], k1.clone(), LinkMode::Transmit { strength: 15 });
        n.add_to_network(tx);
        let rx = n.create([1, 0, 0], k1.clone(), LinkMode::Receive { received: 0 });
        n.add_to_network(rx);
        assert_eq!(n.get(rx).unwrap().received(), 15);
        n.set_frequency(rx, k2.clone());
        // Rejoining the empty network re-evaluates the actor: it now hears 0.
        assert_eq!(n.get(rx).unwrap().received(), 0);
        assert_eq!(n.group_len(&k1), 1);
        assert_eq!(n.group_len(&k2), 1);
    }

    #[test]
    fn has_any_loaded_power_ignores_range_and_liveness() {
        let mut a = LinkNetwork::new(1);
        let b = LinkNetwork::new(1);
        let k = key("a:a", "b:b");
        let t = a.create([0, 0, 0], k.clone(), LinkMode::Transmit { strength: 2 });
        a.add_to_network(t);
        a.get_mut(t).unwrap().alive = false;
        assert!(has_any_loaded_power(&[&b, &a], &k));
        assert!(!has_any_loaded_power(&[&b], &k));
    }

    #[test]
    fn transmitter_power_skips_attached_face_for_up_probe() {
        let mut direct = [0; 6];
        let mut up = [0; 6];
        up[Direction::South.index()] = 9; // attached face when facing North
        assert_eq!(transmitter_power(direct, up, Direction::North, PanelInput::None), Some(0));
        up[Direction::North.index()] = 5;
        assert_eq!(transmitter_power(direct, up, Direction::North, PanelInput::None), Some(5));
        direct[Direction::South.index()] = 7;
        assert_eq!(transmitter_power(direct, up, Direction::North, PanelInput::None), Some(7));
    }

    #[test]
    fn panel_input_combination() {
        let z = [0; 6];
        assert_eq!(transmitter_power(z, z, Direction::Up, PanelInput::Powered(true)), Some(15));
        assert_eq!(transmitter_power(z, z, Direction::Up, PanelInput::Powered(false)), Some(0));
        assert_eq!(transmitter_power(z, z, Direction::Up, PanelInput::NotLoaded), None);
    }

    #[test]
    fn block_output_semantics() {
        assert_eq!(link_block_signal(false, 9), 0);
        assert_eq!(link_block_signal(true, 9), 9);
        assert_eq!(link_block_direct_signal(true, 9, Direction::North, Direction::North), 9);
        assert_eq!(link_block_direct_signal(true, 9, Direction::North, Direction::East), 0);
        assert!(link_block_is_signal_source(true, true));
        assert!(!link_block_is_signal_source(false, true));
        assert!(!link_block_is_signal_source(true, false));
    }

    #[test]
    fn receiver_tick_syncs_powered_property() {
        assert_eq!(receiver_tick(5, false, false), (true, true));
        assert_eq!(receiver_tick(0, true, false), (false, true));
        assert_eq!(receiver_tick(5, true, false), (true, false));
        assert_eq!(receiver_tick(5, true, true), (true, true));
    }

    #[test]
    fn toggle_mode_rules() {
        assert_eq!(toggle_mode(true, true, 9), ToggleResult { receiver: false, powered: true, transmit: 0 });
        assert_eq!(toggle_mode(false, false, 9), ToggleResult { receiver: true, powered: false, transmit: 9 });
    }

    #[test]
    fn controller_slots_are_adjacent_pairs() {
        let mut items: [Frequency; CONTROLLER_SLOTS] = std::array::from_fn(|_| Frequency::empty());
        items[4] = f("a:a");
        items[5] = f("b:b");
        assert_eq!(controller_frequency(&items, 2), Some((f("a:a"), f("b:b"))));
        assert_eq!(controller_frequency(&items, 6), None);
    }

    #[test]
    fn controller_press_hold_release_cycle() {
        let mut n = LinkNetwork::new(256);
        let k = key("a:a", "b:b");
        let rx = n.create([5, 0, 0], k.clone(), LinkMode::Receive { received: 0 });
        n.add_to_network(rx);
        let mut c = ControllerServer::new();
        let award = c.receive_pressed(&mut n, [0, 0, 0], 1, &[k.clone()], true);
        assert!(award);
        assert_eq!(n.get(rx).unwrap().received(), 15);
        // held refreshes without a second award
        assert!(!c.receive_pressed(&mut n, [1, 0, 0], 1, &[k.clone()], true));
        for _ in 0..29 {
            c.tick(&mut n);
        }
        assert_eq!(n.get(rx).unwrap().received(), 15);
        // refresh then release
        c.receive_pressed(&mut n, [1, 0, 0], 1, &[k.clone()], true);
        c.receive_pressed(&mut n, [1, 0, 0], 1, &[k.clone()], false);
        assert_eq!(c.held_count(1), 1);
        c.tick(&mut n);
        assert_eq!(c.held_count(1), 0);
        assert_eq!(n.get(rx).unwrap().received(), 0);
    }

    #[test]
    fn controller_entry_expires_after_timeout_without_keepalive() {
        let mut n = LinkNetwork::new(256);
        let k = key("a:a", "b:b");
        let rx = n.create([5, 0, 0], k.clone(), LinkMode::Receive { received: 0 });
        n.add_to_network(rx);
        let mut c = ControllerServer::new();
        c.receive_pressed(&mut n, [0, 0, 0], 7, &[k.clone()], true);
        for _ in 0..29 {
            c.tick(&mut n);
        }
        assert_eq!(n.get(rx).unwrap().received(), 15);
        c.tick(&mut n);
        assert_eq!(n.get(rx).unwrap().received(), 0);
    }

    #[test]
    fn controller_release_of_unknown_is_ignored_and_no_award_without_receiver() {
        let mut n = LinkNetwork::new(256);
        let k = key("a:a", "b:b");
        let mut c = ControllerServer::new();
        assert!(!c.receive_pressed(&mut n, [0, 0, 0], 1, &[k.clone()], false));
        assert!(!c.receive_pressed(&mut n, [0, 0, 0], 1, &[k.clone()], true));
        assert_eq!(c.held_count(1), 1);
    }

    #[test]
    fn two_players_hold_same_frequency_independently() {
        let mut n = LinkNetwork::new(256);
        let k = key("a:a", "b:b");
        let rx = n.create([5, 0, 0], k.clone(), LinkMode::Receive { received: 0 });
        n.add_to_network(rx);
        let mut c = ControllerServer::new();
        c.receive_pressed(&mut n, [0, 0, 0], 1, &[k.clone()], true);
        c.receive_pressed(&mut n, [0, 0, 0], 2, &[k.clone()], true);
        c.receive_pressed(&mut n, [0, 0, 0], 1, &[k.clone()], false);
        c.tick(&mut n);
        assert_eq!(n.get(rx).unwrap().received(), 15);
    }

    #[test]
    fn client_packet_cadence() {
        let mut c = ControllerClient::new();
        // tick 1: press button 0 -> one "new" packet, cooldown 5
        let p = c.tick(vec![0]);
        assert_eq!(p, vec![InputPacket { buttons: vec![0], pressed: true }]);
        // ticks 2..5: cooldown 4..1, no packets
        for _ in 0..4 {
            assert!(c.tick(vec![0]).is_empty());
        }
        // tick 6: cooldown reaches 0 -> keep-alive
        let p = c.tick(vec![0]);
        assert_eq!(p, vec![InputPacket { buttons: vec![0], pressed: true }]);
        // release
        let p = c.tick(vec![]);
        assert_eq!(p, vec![InputPacket { buttons: vec![0], pressed: false }]);
    }

    #[test]
    fn client_new_press_while_held_sends_new_only() {
        let mut c = ControllerClient::new();
        c.tick(vec![1]);
        let p = c.tick(vec![1, 3]);
        assert_eq!(p, vec![InputPacket { buttons: vec![3], pressed: true }]);
    }
}
