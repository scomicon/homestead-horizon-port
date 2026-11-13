// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

//! Package ports after `PackagePortBlockEntity`, `FrogportBlockEntity` and
//! `PackagePortTarget` (Create 6.0.10): the 18-slot package inventory,
//! address filter, the frog's animation state machine and the push/pull rules
//! against adjacent inventories. The Postbox is the same port with a
//! `TrainStation` target that deposits immediately. Rendering, sounds and
//! chain-conveyor routing live in adapters; the target is reached through
//! `PackageSink`.
use crate::package::{
    ItemHandler, ItemStack, SimpleInventory, get_address, insert_stacked, is_package, match_address, match_address_stack,
};

/// Package slots of a port (`SmartInventory(18)`).
pub const PORT_SLOTS: usize = 18;
/// Animation speed: progress per tick (about 10 ticks per animation).
pub const ANIMATION_SPEED: f32 = 0.1;
/// Progress after which an outgoing package leaves / is handed over.
pub const HAND_OVER_PROGRESS: f32 = 0.5;
/// Clipboard page capacity used by `addAddressToClipboard` (entries 0..=11).
pub const CLIPBOARD_PAGE_LIMIT: usize = 12;

/// `PackagePortTarget`.
#[derive(Clone, Debug, PartialEq)]
pub enum PackageTarget {
    /// `ChainConveyorFrogportTarget`.
    ChainConveyor { relative_pos: [i32; 3], chain_pos: f32, connection: Option<[i32; 3]>, flipped: bool },
    /// `TrainStationFrogportTarget`.
    TrainStation { relative_pos: [i32; 3] },
}

impl PackageTarget {
    pub fn relative_pos(&self) -> [i32; 3] {
        match self {
            Self::ChainConveyor { relative_pos, .. } | Self::TrainStation { relative_pos } => *relative_pos,
        }
    }

    /// Stations take the package at the start of the animation.
    pub fn deposit_immediately(&self) -> bool {
        matches!(self, Self::TrainStation { .. })
    }

    /// Chain conveyors serve Frogports, stations serve Postboxes.
    pub fn can_support(&self, is_postbox: bool) -> bool {
        match self {
            Self::ChainConveyor { .. } => !is_postbox,
            Self::TrainStation { .. } => is_postbox,
        }
    }

    /// `ChainConveyorFrogportTarget.setup`: flipped when the conveyor runs backwards.
    pub fn setup(&mut self, conveyor_speed: f32) {
        if let Self::ChainConveyor { flipped, .. } = self {
            *flipped = conveyor_speed < 0.0;
        }
    }

    /// The re-registration step of `register` when the conveyor direction no
    /// longer matches `flipped`: mirror the position on the chain, negate the
    /// connection and toggle the flag. `chain_length` is the connection's
    /// length if known.
    pub fn mirror_onto_other_end(&mut self, chain_length: Option<f32>, new_relative_pos: [i32; 3]) {
        if let Self::ChainConveyor { relative_pos, chain_pos, connection, flipped } = self {
            if let Some(len) = chain_length {
                *chain_pos = len - *chain_pos;
            }
            if let Some(c) = connection {
                *c = [-c[0], -c[1], -c[2]];
            }
            *flipped = !*flipped;
            *relative_pos = new_relative_pos;
        }
    }
}

/// Where packages go (`PackagePortTarget.export`).
pub trait PackageSink {
    /// Try to hand a package to the target. `simulate` must not change state.
    fn export(&mut self, target: &PackageTarget, package: &ItemStack, simulate: bool) -> bool;
}

/// What a tick produced.
#[derive(Clone, Debug, PartialEq)]
pub enum PortEvent {
    PackageSent(ItemStack),
    PackageReceived(ItemStack),
    /// The package could not be delivered and falls to the ground.
    Dropped(ItemStack),
}

/// A Frogport or Postbox.
#[derive(Clone, Debug)]
pub struct PackagePort {
    pub inventory: SimpleInventory,
    pub accepts_packages: bool,
    pub address_filter: String,
    pub target: Option<PackageTarget>,
    pub is_postbox: bool,
    pub animated_package: Option<ItemStack>,
    pub animation_progress: f32,
    pub animating: bool,
    pub currently_depositing: bool,
    pub failed_last_export: bool,
    pub send_anticipate: bool,
    pub passive_yaw: f32,
}

impl PackagePort {
    pub fn new(is_postbox: bool) -> Self {
        let mut inventory = SimpleInventory::new(PORT_SLOTS);
        inventory.limit = 1;
        Self {
            inventory,
            accepts_packages: true,
            address_filter: String::new(),
            target: None,
            is_postbox,
            animated_package: None,
            animation_progress: 0.0,
            animating: false,
            currently_depositing: false,
            failed_last_export: false,
            send_anticipate: false,
            passive_yaw: 0.0,
        }
    }

    /// `getFilterString`: `None` when the port does not accept packages.
    pub fn filter_string(&self) -> Option<&str> {
        self.accepts_packages.then_some(self.address_filter.as_str())
    }

    /// Slot filter: only packages go into the inventory.
    pub fn insert_package(&mut self, package: ItemStack, simulate: bool) -> Option<ItemStack> {
        if !is_package(&package) {
            return Some(package);
        }
        insert_stacked(&mut self.inventory, package, simulate)
    }

    /// `isBackedUp`: all 18 slots in use.
    pub fn is_backed_up(&self) -> bool {
        self.inventory.slots.iter().all(|s| s.as_ref().is_some_and(|s| !s.is_empty()))
    }

    /// Whether routing (chain conveyor / station) may deliver this package here.
    pub fn accepts(&self, package: &ItemStack) -> bool {
        match self.filter_string() {
            Some(filter) => match_address_stack(package, filter),
            None => false,
        }
    }

    /// `sendAnticipate`: hint the visuals while a slot is free and idle.
    pub fn anticipate(&mut self) {
        if self.animating {
            return;
        }
        if self.inventory.slots.iter().any(|s| s.is_none()) {
            self.send_anticipate = true;
        }
    }

    /// `startAnimation(box, deposit)`.
    pub fn start_animation(&mut self, package: ItemStack, deposit: bool, sink: &mut dyn PackageSink) -> bool {
        if !is_package(&package) {
            return false;
        }
        if deposit {
            let Some(t) = &self.target else { return false };
            if t.deposit_immediately() && !sink.export(t, &package, false) {
                return false;
            }
        }
        self.animation_progress = 0.0;
        self.animating = true;
        self.animated_package = Some(package);
        self.currently_depositing = deposit;
        true
    }

    /// Server-side `tick`.
    pub fn tick(&mut self, sink: &mut dyn PackageSink) -> Vec<PortEvent> {
        let mut events = Vec::new();
        if !self.animating {
            return events;
        }
        self.animation_progress = (self.animation_progress + ANIMATION_SPEED).min(1.0);
        if self.currently_depositing && self.animation_progress > HAND_OVER_PROGRESS {
            if let Some(pkg) = self.animated_package.take() {
                let delivered = match &self.target {
                    None => false,
                    Some(t) if t.deposit_immediately() => true,
                    Some(t) => sink.export(t, &pkg, false),
                };
                events.push(if delivered { PortEvent::PackageSent(pkg) } else { PortEvent::Dropped(pkg) });
            }
        }
        if self.animation_progress >= 1.0 {
            self.animation_progress = 0.0;
            self.animating = false;
            if let Some(pkg) = self.animated_package.take() {
                // incoming package
                match self.insert_package(pkg.clone(), false) {
                    None => events.push(PortEvent::PackageReceived(pkg)),
                    Some(rest) => events.push(PortEvent::Dropped(rest)),
                }
            }
        }
        events
    }

    /// `lazyTick` (server, idle only). `below` is the inventory under the
    /// port (`None` when absent or another port); `below_is_packager` marks a
    /// Packager's item handler.
    pub fn lazy_tick(
        &mut self,
        below: Option<&mut dyn ItemHandler>,
        below_is_packager: bool,
        sink: &mut dyn PackageSink,
    ) -> Vec<PortEvent> {
        if self.animating {
            return Vec::new();
        }
        match below {
            Some(b) => {
                self.try_pushing(Some(&mut *b));
                self.try_pulling(Some(b), below_is_packager, sink);
            }
            None => {
                self.try_pushing(None);
                self.try_pulling(None, below_is_packager, sink);
            }
        }
        Vec::new()
    }

    /// `tryPushingToAdjacentInventories`: one package at a time into the
    /// inventory below.
    fn try_pushing(&mut self, below: Option<&mut dyn ItemHandler>) {
        self.failed_last_export = false;
        let Some(below) = below else { return };
        if self.inventory.is_empty() {
            return;
        }
        for slot in 0..self.inventory.slot_count() {
            let Some(one) = self.inventory.extract(slot, 1, true) else { continue };
            if insert_stacked(below, one.clone(), true).is_none() {
                self.inventory.extract(slot, 1, false);
                insert_stacked(below, one, false);
            } else {
                self.failed_last_export = true;
            }
        }
    }

    /// `tryPullingFromOwnAndAdjacentInventories`.
    fn try_pulling(&mut self, below: Option<&mut dyn ItemHandler>, below_is_packager: bool, sink: &mut dyn PackageSink) {
        if self.animating {
            return;
        }
        let Some(target) = self.target.clone() else { return };
        if !sink.export(&target, &default_box(), true) {
            return;
        }
        let filter = self.filter_string().map(str::to_owned);
        if let Some(p) = pull_package(&mut self.inventory, false, filter.as_deref()) {
            self.start_animation(p, true, sink);
            return;
        }
        if let Some(below) = below {
            if let Some(p) = pull_package(below, below_is_packager, filter.as_deref()) {
                self.start_animation(p, true, sink);
            }
        }
    }

    /// `getYaw`: faces the target, or the placed yaw without one.
    /// `target_center` is the exact target location, `own_center` this block's centre.
    pub fn yaw(&self, target_center: Option<[f64; 3]>, own_center: [f64; 3]) -> f32 {
        match target_center {
            None => self.passive_yaw,
            Some(t) => {
                let (dx, dz) = (t[0] - own_center[0], t[2] - own_center[2]);
                (dx.atan2(dz).to_degrees() + 180.0) as f32
            }
        }
    }
}

/// `getDefaultBox` used for the simulated export probe.
fn default_box() -> ItemStack {
    crate::package::containing("create:cardboard_package_12x12", &[]).0
}

/// `tryPullingFrom`: takes one package. With a filter set, only packages that
/// do not match it are taken from non-packager handlers (the original's
/// quirk: matching packages are meant for this port itself).
fn pull_package(h: &mut dyn ItemHandler, is_packager: bool, filter: Option<&str>) -> Option<ItemStack> {
    for slot in 0..h.slot_count() {
        let Some(candidate) = h.extract(slot, 1, true) else { continue };
        if !is_package(&candidate) {
            continue;
        }
        let ok = match filter {
            None => true,
            Some(f) => is_packager || !match_address(&get_address(&candidate), f),
        };
        if ok {
            return h.extract(slot, 1, false);
        }
    }
    None
}

/// `addAddressToClipboard`: writes `# address` to the first page with room.
/// Pages with `CLIPBOARD_PAGE_LIMIT` or more entries are skipped; a duplicate
/// (`#address` or `# address`) or blank address changes nothing.
pub fn add_address_to_clipboard(pages: &mut Vec<Vec<String>>, address: &str) -> bool {
    if address.trim().is_empty() {
        return false;
    }
    let (a, b) = (format!("#{address}"), format!("# {address}"));
    if pages.iter().flatten().any(|e| *e == a || *e == b) {
        return false;
    }
    match pages.iter_mut().find(|p| p.len() < CLIPBOARD_PAGE_LIMIT) {
        Some(p) => p.push(b),
        None => pages.push(vec![b]),
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::{add_address, containing};

    #[derive(Default)]
    struct Sink {
        accept: bool,
        exported: Vec<ItemStack>,
        probes: u32,
    }

    impl PackageSink for Sink {
        fn export(&mut self, _t: &PackageTarget, p: &ItemStack, simulate: bool) -> bool {
            if simulate {
                self.probes += 1;
                return self.accept;
            }
            if self.accept {
                self.exported.push(p.clone());
            }
            self.accept
        }
    }

    fn pkg(address: &str) -> ItemStack {
        let mut p = containing("create:cardboard_package_12x12", &[ItemStack::new("m:a", 1)]).0;
        add_address(&mut p, address);
        p
    }

    fn chain() -> PackageTarget {
        PackageTarget::ChainConveyor { relative_pos: [0, 2, 0], chain_pos: 1.5, connection: None, flipped: false }
    }

    fn station() -> PackageTarget {
        PackageTarget::TrainStation { relative_pos: [3, 0, 0] }
    }

    #[test]
    fn port_inventory_takes_only_packages_one_per_slot() {
        let mut p = PackagePort::new(false);
        assert!(p.insert_package(ItemStack::new("m:a", 1), false).is_some());
        for _ in 0..PORT_SLOTS {
            assert!(p.insert_package(pkg("x"), false).is_none());
        }
        assert!(p.is_backed_up());
        assert!(p.insert_package(pkg("x"), false).is_some());
    }

    #[test]
    fn filter_string_and_accepts() {
        let mut p = PackagePort::new(false);
        p.address_filter = "farm*".into();
        assert_eq!(p.filter_string(), Some("farm*"));
        assert!(p.accepts(&pkg("farm-1")));
        assert!(!p.accepts(&pkg("mine")));
        p.accepts_packages = false;
        assert_eq!(p.filter_string(), None);
        assert!(!p.accepts(&pkg("farm-1")));
        // an empty filter matches only unaddressed packages
        let mut q = PackagePort::new(false);
        assert!(q.accepts(&pkg("")));
        assert!(!q.accepts(&pkg("a")));
        q.address_filter = "*".into();
        assert!(q.accepts(&pkg("a")));
    }

    #[test]
    fn target_properties() {
        assert!(station().deposit_immediately() && !chain().deposit_immediately());
        assert!(chain().can_support(false) && !chain().can_support(true));
        assert!(station().can_support(true) && !station().can_support(false));
        let mut c = chain();
        c.setup(-4.0);
        assert!(matches!(c, PackageTarget::ChainConveyor { flipped: true, .. }));
        c.setup(8.0);
        assert!(matches!(c, PackageTarget::ChainConveyor { flipped: false, .. }));
        assert_eq!(station().relative_pos(), [3, 0, 0]);
    }

    #[test]
    fn mirroring_onto_the_other_end() {
        let mut c = PackageTarget::ChainConveyor { relative_pos: [0, 2, 0], chain_pos: 1.25, connection: Some([4, 0, 1]), flipped: false };
        c.mirror_onto_other_end(Some(5.0), [4, 2, 1]);
        assert_eq!(
            c,
            PackageTarget::ChainConveyor { relative_pos: [4, 2, 1], chain_pos: 3.75, connection: Some([-4, 0, -1]), flipped: true }
        );
    }

    #[test]
    fn outgoing_animation_exports_at_half_and_clears() {
        let mut p = PackagePort::new(false);
        p.target = Some(chain());
        let mut sink = Sink { accept: true, ..Default::default() };
        assert!(p.start_animation(pkg("x"), true, &mut sink));
        assert!(sink.exported.is_empty(), "chain targets export mid-animation");
        let mut sent = 0;
        for _ in 0..4 {
            assert!(p.tick(&mut sink).is_empty());
        }
        for _ in 0..12 {
            for e in p.tick(&mut sink) {
                assert!(matches!(e, PortEvent::PackageSent(_)));
                sent += 1;
            }
        }
        assert_eq!(sent, 1);
        assert_eq!(sink.exported.len(), 1);
        assert!(!p.animating && p.animated_package.is_none());
    }

    #[test]
    fn failed_export_drops_the_package() {
        let mut p = PackagePort::new(false);
        p.target = Some(chain());
        let mut sink = Sink::default();
        assert!(p.start_animation(pkg("x"), true, &mut sink));
        let evs: Vec<_> = (0..12).flat_map(|_| p.tick(&mut sink)).collect();
        assert!(matches!(evs.as_slice(), [PortEvent::Dropped(_)]));
        // no target at all also drops, and refuses to start
        let mut q = PackagePort::new(false);
        assert!(!q.start_animation(pkg("x"), true, &mut sink));
    }

    #[test]
    fn station_target_exports_immediately_and_refuses_when_full() {
        let mut p = PackagePort::new(true);
        p.target = Some(station());
        let mut sink = Sink::default();
        assert!(!p.start_animation(pkg("x"), true, &mut sink), "station refused");
        sink.accept = true;
        assert!(p.start_animation(pkg("x"), true, &mut sink));
        assert_eq!(sink.exported.len(), 1);
        let evs: Vec<_> = (0..12).flat_map(|_| p.tick(&mut sink)).collect();
        assert!(matches!(evs.as_slice(), [PortEvent::PackageSent(_)]));
        assert_eq!(sink.exported.len(), 1, "not exported twice");
    }

    #[test]
    fn incoming_package_lands_in_inventory_or_drops() {
        let mut p = PackagePort::new(false);
        let mut sink = Sink::default();
        assert!(p.start_animation(pkg("x"), false, &mut sink));
        let evs: Vec<_> = (0..12).flat_map(|_| p.tick(&mut sink)).collect();
        assert!(matches!(evs.as_slice(), [PortEvent::PackageReceived(_)]));
        assert_eq!(p.inventory.slots.iter().flatten().count(), 1);
        // full inventory drops
        for s in p.inventory.slots.iter_mut() {
            *s = Some(pkg("y"));
        }
        assert!(p.start_animation(pkg("x"), false, &mut sink));
        let evs: Vec<_> = (0..12).flat_map(|_| p.tick(&mut sink)).collect();
        assert!(matches!(evs.as_slice(), [PortEvent::Dropped(_)]));
        assert!(!p.start_animation(ItemStack::new("m:a", 1), false, &mut sink), "non-package refused");
    }

    #[test]
    fn lazy_tick_pushes_to_inventory_below_and_flags_blocked() {
        let mut p = PackagePort::new(false);
        p.inventory.slots[0] = Some(pkg("a"));
        p.inventory.slots[1] = Some(pkg("b"));
        let mut below = SimpleInventory::new(1);
        let mut sink = Sink::default();
        p.lazy_tick(Some(&mut below), false, &mut sink);
        assert_eq!(below.slots.iter().flatten().count(), 1);
        assert_eq!(p.inventory.slots.iter().flatten().count(), 1);
        assert!(p.failed_last_export, "second package could not follow");
        below.slots[0] = None;
        p.lazy_tick(Some(&mut below), false, &mut sink);
        assert!(!p.failed_last_export);
        assert!(p.inventory.is_empty());
    }

    #[test]
    fn lazy_tick_pulls_outgoing_package_when_target_accepts() {
        let mut p = PackagePort::new(false);
        p.target = Some(chain());
        let mut below = SimpleInventory::new(2).with(0, ItemStack::new("m:a", 5)).with(1, pkg("dest"));
        let mut sink = Sink { accept: true, ..Default::default() };
        p.lazy_tick(Some(&mut below), false, &mut sink);
        assert!(p.animating && p.currently_depositing);
        assert!(below.slots[1].is_none());
        assert!(sink.probes >= 1);
        // refused probe: nothing pulled
        let mut q = PackagePort::new(false);
        q.target = Some(chain());
        let mut below2 = SimpleInventory::new(1).with(0, pkg("dest"));
        let mut deny = Sink::default();
        q.lazy_tick(Some(&mut below2), false, &mut deny);
        assert!(!q.animating && below2.slots[0].is_some());
        // no target
        let mut r = PackagePort::new(false);
        let mut below3 = SimpleInventory::new(1).with(0, pkg("dest"));
        r.lazy_tick(Some(&mut below3), false, &mut sink);
        assert!(!r.animating);
    }

    #[test]
    fn pull_filter_quirk() {
        let mut p = PackagePort::new(false);
        p.target = Some(chain());
        p.address_filter = "home".into();
        let mut sink = Sink { accept: true, ..Default::default() };
        // a package matching the port's own filter stays put in a plain inventory
        let mut inv = SimpleInventory::new(1).with(0, pkg("home"));
        p.lazy_tick(Some(&mut inv), false, &mut sink);
        assert!(!p.animating);
        // a non-matching one is pulled
        let mut inv = SimpleInventory::new(1).with(0, pkg("elsewhere"));
        p.lazy_tick(Some(&mut inv), false, &mut sink);
        assert!(p.animating);
        // from a packager the filter is ignored
        let mut q = PackagePort::new(false);
        q.target = Some(chain());
        q.address_filter = "home".into();
        let mut inv = SimpleInventory::new(1).with(0, pkg("home"));
        q.lazy_tick(Some(&mut inv), true, &mut sink);
        assert!(q.animating);
        // a port that does not accept packages pulls everything
        let mut r = PackagePort::new(false);
        r.target = Some(chain());
        r.accepts_packages = false;
        let mut inv = SimpleInventory::new(1).with(0, pkg("home"));
        r.lazy_tick(Some(&mut inv), false, &mut sink);
        assert!(r.animating);
    }

    #[test]
    fn own_inventory_is_pulled_before_the_one_below() {
        let mut p = PackagePort::new(false);
        p.target = Some(chain());
        p.inventory.slots[3] = Some(pkg("mine"));
        let mut below = SimpleInventory::new(1).with(0, pkg("below"));
        let mut sink = Sink { accept: true, ..Default::default() };
        // pushing happens first and moves it down; pulling then takes from the first source
        p.lazy_tick(Some(&mut below), false, &mut sink);
        assert!(p.animating);
        assert!(p.inventory.is_empty());
        assert!(p.animated_package.as_ref().is_some());
    }

    #[test]
    fn busy_port_ignores_lazy_tick_and_anticipates_only_when_idle() {
        let mut p = PackagePort::new(false);
        p.anticipate();
        assert!(p.send_anticipate);
        p.send_anticipate = false;
        p.animating = true;
        p.anticipate();
        assert!(!p.send_anticipate);
        let mut sink = Sink::default();
        let mut below = SimpleInventory::new(1).with(0, pkg("a"));
        p.lazy_tick(Some(&mut below), false, &mut sink);
        assert!(below.slots[0].is_some());
        // a full inventory never anticipates
        let mut q = PackagePort::new(false);
        for s in q.inventory.slots.iter_mut() {
            *s = Some(pkg("a"));
        }
        q.anticipate();
        assert!(!q.send_anticipate);
    }

    #[test]
    fn yaw_faces_target() {
        let mut p = PackagePort::new(false);
        p.passive_yaw = 33.0;
        assert_eq!(p.yaw(None, [0.5; 3]), 33.0);
        // target to +z: atan2(0, 1) = 0, plus 180
        assert!((p.yaw(Some([0.5, 0.5, 3.5]), [0.5, 0.5, 0.5]) - 180.0).abs() < 1e-4);
        // target to +x: 90 + 180
        assert!((p.yaw(Some([3.5, 0.5, 0.5]), [0.5, 0.5, 0.5]) - 270.0).abs() < 1e-4);
    }

    #[test]
    fn clipboard_addresses() {
        let mut pages: Vec<Vec<String>> = vec![vec![]];
        assert!(!add_address_to_clipboard(&mut pages, " "));
        assert!(add_address_to_clipboard(&mut pages, "home"));
        assert!(!add_address_to_clipboard(&mut pages, "home"));
        pages[0].push("#shop".into());
        assert!(!add_address_to_clipboard(&mut pages, "shop"));
        for i in 0..20 {
            add_address_to_clipboard(&mut pages, &format!("a{i}"));
        }
        assert_eq!(pages[0].len(), CLIPBOARD_PAGE_LIMIT);
        assert!(pages.len() >= 2);
    }
}
