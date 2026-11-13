// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

//! Factory Gauge logic after `FactoryPanelBehaviour` (Create 6.0.10): stock
//! monitoring with promises, crafting requests to the network, restocking
//! and the connection rules. The network is reached through `GaugeEnv`, so
//! the adapter decides how summaries, packagers and promise queues are backed.
use crate::package::{BigStack, ItemStack, OrderWithCrafts, PackageOrder};
use crate::packager::RequestType;
use crate::stock_network::{FreqId, InventorySummary, PackagerRequests, RequestPromise, RequestPromiseQueue};

/// Default of the `factoryGaugeTimer` config option (ticks between requests).
/// Unverified against the shipped config; callers pass the configured value.
pub const DEFAULT_TIMER_INTERVAL: i32 = 100;
/// Most source panels one gauge can have (`targetedBy`).
pub const MAX_SOURCES: usize = 9;
/// Value box limits: maximum 100, step 10.
pub const MAX_TARGET_AMOUNT: u32 = 100;
pub const TARGET_STEP: u32 = 10;
/// Lazy tick rate of the behaviour.
pub const LAZY_TICK_RATE: u32 = 40;
/// Restocker order cap in stacks (`maxStackSize * 9`).
pub const RESTOCK_MAX_STACKS: u32 = 9;

/// Position + slot of a gauge on its block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanelKey {
    pub pos: [i32; 3],
    pub slot: u8,
}

/// `FactoryPanelConnection`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Connection {
    pub from: PanelKey,
    pub amount: u32,
    pub arrow_bend_mode: u8,
}

/// What a source panel looks like to its target.
#[derive(Clone, Debug)]
pub struct SourcePanel {
    pub filter: Option<ItemStack>,
    pub network: FreqId,
    pub active: bool,
}

/// Observable results the adapter turns into packets/sounds/redstone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GaugeEvent {
    /// `FactoryPanelEffectPacket`: connection path flashes green/red.
    Effect { from: Option<PanelKey>, success: bool },
    /// Two confirm sounds when the panel becomes satisfied (`demand > 0`).
    ConfirmSounds,
    /// `satisfied` changed: notify redstone outputs.
    RedstoneOutputsChanged,
    /// Client/NBT sync.
    SendData,
    Advancement(&'static str),
}

/// The world as seen by a gauge.
pub trait GaugeEnv {
    /// `LogisticsManager.getSummaryOfNetwork(freq, accurate)`.
    fn summary(&mut self, freq: FreqId, accurate: bool) -> InventorySummary;
    fn invalidate_summary(&mut self, freq: FreqId);
    /// `Create.LOGISTICS.getUnloadedLinkCount`.
    fn unloaded_link_count(&self, freq: FreqId) -> usize;
    fn source(&self, key: &PanelKey) -> Option<SourcePanel>;
    /// `LogisticsManager.findPackagersForRequest`.
    fn find_packagers(&mut self, freq: FreqId, order: &OrderWithCrafts, address: &str) -> PackagerRequests;
    fn is_too_busy(&self, packager: u32, ty: RequestType) -> bool;
    /// `performPackageRequests`.
    fn perform(&mut self, requests: PackagerRequests);
    /// Network-wide panel promise queue (`LogisticsNetwork.panelPromises`).
    fn network_promises(&mut self, freq: FreqId) -> Option<&mut RequestPromiseQueue>;
    /// Restocker only: the attached packager's `getAvailableItems()`; `None`
    /// without a packager or inventory.
    fn restocked_packager(&mut self) -> Option<InventorySummary>;
    /// `LogisticsManager.getStockOf(freq, stack, ignored = restocked inventory)`.
    fn stock_of(&mut self, freq: FreqId, stack: &ItemStack) -> u32;
    /// `LogisticsManager.broadcastPackageRequest` with `RESTOCK`.
    fn broadcast_restock(&mut self, freq: FreqId, order: &OrderWithCrafts, address: &str) -> bool;
}

/// Gauge behaviour state.
#[derive(Clone, Debug)]
pub struct FactoryPanel {
    pub filter: Option<ItemStack>,
    pub count: u32,
    pub up_to: bool,
    /// -1: never; 0: 30 s; N: N minutes.
    pub promise_clearing_interval: i32,
    pub timer: i32,
    pub satisfied: bool,
    pub promised_satisfied: bool,
    pub waiting_for_network: bool,
    pub redstone_powered: bool,
    pub active: bool,
    pub restocker: bool,
    pub virtual_be: bool,
    pub network: FreqId,
    pub recipe_address: String,
    pub recipe_output: u32,
    pub active_crafting_arrangement: Vec<ItemStack>,
    pub targeted_by: Vec<Connection>,
    pub targeted_by_links: Vec<[i32; 3]>,
    pub targeting: Vec<PanelKey>,
    pub restocker_promises: RequestPromiseQueue,
    pub timer_interval: i32,
    // last reported values for change detection
    last_level: u32,
    last_unloaded: usize,
    last_promised: u32,
    pub events: Vec<GaugeEvent>,
}

impl FactoryPanel {
    pub fn new(network: FreqId) -> Self {
        Self {
            filter: None,
            count: 0,
            up_to: true,
            promise_clearing_interval: -1,
            timer: 0,
            satisfied: false,
            promised_satisfied: false,
            waiting_for_network: false,
            redstone_powered: false,
            active: true,
            restocker: false,
            virtual_be: false,
            network,
            recipe_address: String::new(),
            recipe_output: 1,
            active_crafting_arrangement: Vec::new(),
            targeted_by: Vec::new(),
            targeted_by_links: Vec::new(),
            targeting: Vec::new(),
            restocker_promises: RequestPromiseQueue::default(),
            timer_interval: DEFAULT_TIMER_INTERVAL,
            last_level: 0,
            last_unloaded: 0,
            last_promised: 0,
            events: Vec::new(),
        }
    }

    /// `setValueSettings`: row 0 counts items, other rows count stacks.
    pub fn set_value_settings(&mut self, row: u32, value: i32) {
        self.count = value.max(0) as u32;
        self.up_to = row == 0;
    }

    fn filter_empty(&self) -> bool {
        self.filter.as_ref().is_none_or(|f| f.is_empty())
    }

    /// `getPromiseExpiryTimeInTicks`.
    pub fn promise_expiry_ticks(&self) -> Option<u32> {
        match self.promise_clearing_interval {
            -1 => None,
            0 => Some(20 * 30),
            n if n > 0 => Some(n as u32 * 20 * 60),
            _ => None,
        }
    }

    /// Target stock: items, or stacks times the item's stack size.
    pub fn demand(&self) -> u32 {
        let per = if self.up_to { 1 } else { self.filter.as_ref().map_or(1, |f| f.max_stack) };
        self.count * per
    }

    /// `isMissingAddress`.
    pub fn is_missing_address(&self) -> bool {
        (!self.targeted_by.is_empty() || self.restocker) && self.count != 0 && self.recipe_address.is_empty()
    }

    pub fn reset_timer(&mut self) {
        self.timer = self.timer_interval;
    }

    pub fn reset_timer_slightly(&mut self) {
        self.timer = self.timer_interval / 2;
    }

    fn level_in_storage(&mut self, env: &mut dyn GaugeEnv) -> u32 {
        if self.virtual_be {
            return 1;
        }
        let Some(f) = self.filter.clone().filter(|f| !f.is_empty()) else { return 0 };
        if self.restocker {
            env.restocked_packager().map_or(0, |s| s.count_of(&f))
        } else {
            env.summary(self.network, false).count_of(&f)
        }
    }

    fn promised(&mut self, env: &mut dyn GaugeEnv, force_clear: bool) -> u32 {
        let Some(f) = self.filter.clone().filter(|f| !f.is_empty()) else { return 0 };
        let expiry = self.promise_expiry_ticks();
        let net = self.network;
        if self.restocker {
            if force_clear {
                self.restocker_promises.force_clear(&f);
                self.reset_timer_slightly();
            }
            self.restocker_promises.total_promised_and_remove_expired(&f, expiry)
        } else {
            let Some(q) = env.network_promises(net) else { return 0 };
            if force_clear {
                q.force_clear(&f);
                self.timer = self.timer_interval / 2;
            }
            q.total_promised_and_remove_expired(&f, expiry)
        }
    }

    /// `tick` on the server: monitor, then requests.
    pub fn tick(&mut self, env: &mut dyn GaugeEnv) {
        self.tick_storage_monitor(env);
        self.tick_requests(env);
    }

    /// `tickStorageMonitor`.
    pub fn tick_storage_monitor(&mut self, env: &mut dyn GaugeEnv) {
        let unloaded = if self.restocker {
            usize::from(env.restocked_packager().is_none())
        } else {
            env.unloaded_link_count(self.network)
        };
        if !self.restocker && self.last_unloaded != 0 && unloaded == 0 {
            env.invalidate_summary(self.network);
        }
        let in_storage = self.level_in_storage(env);
        let promised = self.promised(env, false);
        let demand = self.demand();
        let empty = self.filter_empty();
        let should_satisfy = empty || in_storage >= demand;
        let should_promise_satisfy = empty || in_storage + promised >= demand;
        let should_wait = unloaded > 0;

        if self.last_level == in_storage
            && self.last_unloaded == unloaded
            && self.last_promised == promised
            && self.satisfied == should_satisfy
            && self.promised_satisfied == should_promise_satisfy
            && self.waiting_for_network == should_wait
        {
            return;
        }
        if should_satisfy && !self.satisfied && demand > 0 {
            self.events.push(GaugeEvent::ConfirmSounds);
        }
        let satisfied_changed = self.satisfied != should_satisfy;
        self.last_level = in_storage;
        self.last_unloaded = unloaded;
        self.last_promised = promised;
        self.satisfied = should_satisfy;
        self.promised_satisfied = should_promise_satisfy;
        self.waiting_for_network = should_wait;
        self.events.push(GaugeEvent::SendData);
        if satisfied_changed {
            self.events.push(GaugeEvent::RedstoneOutputsChanged);
        }
    }

    /// `tickRequests`.
    pub fn tick_requests(&mut self, env: &mut dyn GaugeEnv) {
        if self.targeted_by.is_empty() && !self.restocker {
            return;
        }
        if self.restocker {
            self.restocker_promises.tick();
        }
        if self.satisfied || self.promised_satisfied || self.waiting_for_network || self.redstone_powered {
            return;
        }
        if self.timer > 0 {
            self.timer = self.timer.min(self.timer_interval) - 1;
            return;
        }
        self.reset_timer();
        if self.recipe_address.trim().is_empty() {
            return;
        }
        if self.restocker {
            self.try_restock(env);
            return;
        }

        // group the source connections by network, then by item
        struct Group {
            item: ItemStack,
            total: u32,
            conns: Vec<PanelKey>,
        }
        let mut by_network: Vec<(FreqId, Vec<Group>)> = Vec::new();
        for c in &self.targeted_by {
            let Some(src) = env.source(&c.from) else { return };
            let item = src.filter.clone().unwrap_or_else(|| ItemStack::new("minecraft:air", 0));
            let nets = match by_network.iter().position(|(n, _)| *n == src.network) {
                Some(i) => i,
                None => {
                    by_network.push((src.network, Vec::new()));
                    by_network.len() - 1
                }
            };
            let groups = &mut by_network[nets].1;
            match groups.iter_mut().find(|g| g.item.same_item(&item)) {
                Some(g) => {
                    g.total += c.amount;
                    g.conns.push(c.from);
                }
                None => groups.push(Group { item, total: c.amount, conns: vec![c.from] }),
            }
        }

        let mut orders: Vec<(FreqId, Vec<BigStack>)> = Vec::new();
        let mut failed = false;
        let mut effects = Vec::new();
        for (net, groups) in &by_network {
            let summary = env.summary(*net, true);
            let mut items = Vec::new();
            for g in groups {
                let stock = summary.count_of(&g.item);
                if g.total == 0 || g.item.is_empty() || stock < g.total {
                    for k in &g.conns {
                        effects.push(GaugeEvent::Effect { from: Some(*k), success: false });
                    }
                    failed = true;
                    continue;
                }
                items.push(BigStack::new(g.item.copy_with_count(1), g.total));
                for k in &g.conns {
                    effects.push(GaugeEvent::Effect { from: Some(*k), success: true });
                }
            }
            orders.push((*net, items));
        }
        self.events.extend(effects);
        if failed {
            return;
        }

        let crafts = if self.active_crafting_arrangement.is_empty() {
            OrderWithCrafts::empty().ordered_crafts
        } else {
            let pattern = self.active_crafting_arrangement.iter().map(|s| BigStack::new(s.copy_with_count(1), 1)).collect();
            OrderWithCrafts::single_recipe(pattern).ordered_crafts
        };
        let mut all: Vec<PackagerRequests> = Vec::new();
        for (net, items) in orders {
            let order = OrderWithCrafts { ordered_stacks: PackageOrder::new(items), ordered_crafts: crafts.clone() };
            let found = env.find_packagers(net, &order, &self.recipe_address);
            if found.iter().any(|(p, _)| env.is_too_busy(*p, RequestType::Restock)) {
                return;
            }
            all.push(found);
        }
        for requests in all {
            env.perform(requests);
        }
        if let Some(f) = self.filter.clone().filter(|f| !f.is_empty()) {
            let net = self.network;
            let promise = RequestPromise::new(BigStack::new(f.copy_with_count(1), self.recipe_output));
            if let Some(q) = env.network_promises(net) {
                q.add(promise);
            }
        }
        self.events.push(GaugeEvent::Advancement("factory_gauge"));
    }

    /// `tryRestock`.
    fn try_restock(&mut self, env: &mut dyn GaugeEnv) {
        let Some(f) = self.filter.clone().filter(|f| !f.is_empty()) else { return };
        let Some(in_packager) = env.restocked_packager() else { return };
        let in_storage = in_packager.count_of(&f);
        let available = env.stock_of(self.network, &f);
        if available == 0 {
            self.events.push(GaugeEvent::Effect { from: None, success: false });
            return;
        }
        let promised = self.restocker_promises.total_promised_and_remove_expired(&f, self.promise_expiry_ticks());
        let demand = self.demand() as i64;
        let cap = f.max_stack as i64 * RESTOCK_MAX_STACKS as i64;
        let to_order = (demand - promised as i64 - in_storage as i64).clamp(0, cap) as u32;
        let ordered = to_order.min(available);
        self.events.push(GaugeEvent::Effect { from: None, success: true });
        if ordered == 0 {
            return;
        }
        let order = OrderWithCrafts::simple(vec![BigStack::new(f.copy_with_count(1), ordered)]);
        if env.broadcast_restock(self.network, &order, &self.recipe_address) {
            self.restocker_promises.add(RequestPromise::new(BigStack::new(f.copy_with_count(1), ordered)));
        }
    }

    /// `checkForRedstoneInput` (lazy tick). Each entry is the linked support's
    /// `shouldPanelBePowered()`, or `None` if its chunk is unloaded / it is gone.
    pub fn check_for_redstone_input(&mut self, supports: &[Option<bool>]) {
        if !self.active {
            return;
        }
        let mut should_power = false;
        for s in supports {
            match s {
                None => return,
                Some(p) => should_power |= *p,
            }
        }
        if should_power == self.redstone_powered {
            return;
        }
        self.redstone_powered = should_power;
        self.events.push(GaugeEvent::SendData);
        self.timer = 1;
    }

    /// `addConnection` from a source: `Err` is a rejection.
    pub fn add_connection(&mut self, from: PanelKey, source: Option<&SourcePanel>, is_link: bool) -> Result<(), &'static str> {
        if is_link {
            if !self.targeted_by_links.contains(&from.pos) {
                self.targeted_by_links.push(from.pos);
            }
            return Ok(());
        }
        if self.restocker {
            return Err("restocker accepts no panel input");
        }
        if self.targeted_by.len() >= MAX_SOURCES {
            return Err("too many sources");
        }
        match source {
            Some(s) if s.active => {}
            _ => return Err("source must be an active gauge"),
        }
        if self.targeted_by.iter().all(|c| c.from != from) {
            self.targeted_by.push(Connection { from, amount: 1, arrow_bend_mode: 0 });
        }
        Ok(())
    }

    pub fn remove_connection(&mut self, from: &PanelKey) {
        self.targeted_by.retain(|c| c.from != *from);
    }

    /// Wrench on a gauge with connections: cycle the arrow bend mode (mod 4).
    pub fn cycle_arrow_bend(&mut self) {
        for c in &mut self.targeted_by {
            c.arrow_bend_mode = (c.arrow_bend_mode + 1) % 4;
        }
    }

    /// Maximum distance a moved gauge may keep to its links (`moveTo`).
    pub const MOVE_RANGE: i32 = 24;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::XorShift;
    use crate::stock_network::{StockLink, find_packagers_for_request, group_by_packager, stock_of, summary_of, summary_of_network};
    use std::collections::BTreeMap;

    fn it(id: &str) -> ItemStack {
        ItemStack::new(id, 1)
    }

    /// A tiny world: one network of stock links, a promise queue, sources.
    struct World {
        links: Vec<StockLink>,
        promises: RequestPromiseQueue,
        sources: BTreeMap<PanelKey, SourcePanel>,
        performed: Vec<PackagerRequests>,
        restock_pkg: Option<InventorySummary>,
        busy: bool,
        unloaded: usize,
        restock_log: Vec<OrderWithCrafts>,
        invalidations: u32,
        rng: XorShift,
    }

    impl World {
        fn new(items: &[(&str, u32)]) -> Self {
            Self {
                links: vec![StockLink::new(1, 7, 10, 100, summary_of(items))],
                promises: RequestPromiseQueue::default(),
                sources: BTreeMap::new(),
                performed: Vec::new(),
                restock_pkg: None,
                busy: false,
                unloaded: 0,
                restock_log: Vec::new(),
                invalidations: 0,
                rng: XorShift(5),
            }
        }
    }

    impl GaugeEnv for World {
        fn summary(&mut self, freq: FreqId, _accurate: bool) -> InventorySummary {
            summary_of_network(&self.links, freq)
        }
        fn invalidate_summary(&mut self, _f: FreqId) {
            self.invalidations += 1;
        }
        fn unloaded_link_count(&self, _f: FreqId) -> usize {
            self.unloaded
        }
        fn source(&self, key: &PanelKey) -> Option<SourcePanel> {
            self.sources.get(key).cloned()
        }
        fn find_packagers(&mut self, freq: FreqId, order: &OrderWithCrafts, address: &str) -> PackagerRequests {
            group_by_packager(find_packagers_for_request(&self.links, freq, order, None, address, &mut self.rng))
        }
        fn is_too_busy(&self, _p: u32, _t: RequestType) -> bool {
            self.busy
        }
        fn perform(&mut self, r: PackagerRequests) {
            self.performed.push(r);
        }
        fn network_promises(&mut self, _f: FreqId) -> Option<&mut RequestPromiseQueue> {
            Some(&mut self.promises)
        }
        fn restocked_packager(&mut self) -> Option<InventorySummary> {
            self.restock_pkg.clone()
        }
        fn stock_of(&mut self, freq: FreqId, s: &ItemStack) -> u32 {
            stock_of(&self.links, freq, s, None)
        }
        fn broadcast_restock(&mut self, _f: FreqId, o: &OrderWithCrafts, _a: &str) -> bool {
            self.restock_log.push(o.clone());
            !self.busy
        }
    }

    fn key(n: i32) -> PanelKey {
        PanelKey { pos: [n, 0, 0], slot: 0 }
    }

    /// Gauge crafting `out` from one source gauge holding `src_item`.
    fn crafter(src_item: &str, need: u32, world: &mut World) -> FactoryPanel {
        let mut g = FactoryPanel::new(7);
        g.filter = Some(it("out"));
        g.count = 4;
        g.recipe_address = "assembly".into();
        g.targeted_by.push(Connection { from: key(1), amount: need, arrow_bend_mode: 0 });
        world.sources.insert(key(1), SourcePanel { filter: Some(it(src_item)), network: 7, active: true });
        g
    }

    #[test]
    fn expiry_and_demand_helpers() {
        let mut g = FactoryPanel::new(1);
        assert_eq!(g.promise_expiry_ticks(), None);
        g.promise_clearing_interval = 0;
        assert_eq!(g.promise_expiry_ticks(), Some(600));
        g.promise_clearing_interval = 3;
        assert_eq!(g.promise_expiry_ticks(), Some(3600));
        g.filter = Some(ItemStack::new("a", 1).with_max(16));
        g.set_value_settings(0, 5);
        assert_eq!(g.demand(), 5);
        g.set_value_settings(1, 5);
        assert_eq!(g.demand(), 80);
        g.set_value_settings(1, -4);
        assert_eq!(g.count, 0);
    }

    #[test]
    fn missing_address_flag() {
        let mut g = FactoryPanel::new(1);
        g.count = 2;
        assert!(!g.is_missing_address());
        g.restocker = true;
        assert!(g.is_missing_address());
        g.recipe_address = "x".into();
        assert!(!g.is_missing_address());
    }

    #[test]
    fn monitor_reports_satisfaction_and_sounds_once() {
        let mut w = World::new(&[("out", 3)]);
        let mut g = FactoryPanel::new(7);
        g.filter = Some(it("out"));
        g.count = 5;
        g.tick_storage_monitor(&mut w);
        assert!(!g.satisfied && !g.promised_satisfied);
        assert!(g.events.contains(&GaugeEvent::SendData));
        g.events.clear();
        // unchanged: no events
        g.tick_storage_monitor(&mut w);
        assert!(g.events.is_empty());
        // stock reaches demand
        w.links[0].available = summary_of(&[("out", 5)]);
        g.tick_storage_monitor(&mut w);
        assert!(g.satisfied);
        assert_eq!(g.events.iter().filter(|e| **e == GaugeEvent::ConfirmSounds).count(), 1);
        assert!(g.events.contains(&GaugeEvent::RedstoneOutputsChanged));
    }

    #[test]
    fn promises_count_towards_promised_satisfied_only() {
        let mut w = World::new(&[("out", 1)]);
        w.promises.add(RequestPromise::new(BigStack::new(it("out"), 4)));
        let mut g = FactoryPanel::new(7);
        g.filter = Some(it("out"));
        g.count = 5;
        g.tick_storage_monitor(&mut w);
        assert!(!g.satisfied && g.promised_satisfied);
    }

    #[test]
    fn empty_filter_and_virtual_panels_are_satisfied_or_one() {
        let mut w = World::new(&[]);
        let mut g = FactoryPanel::new(7);
        g.count = 3;
        g.tick_storage_monitor(&mut w);
        assert!(g.satisfied && g.promised_satisfied);
        let mut v = FactoryPanel::new(7);
        v.filter = Some(it("a"));
        v.virtual_be = true;
        v.count = 1;
        v.tick_storage_monitor(&mut w);
        assert!(v.satisfied);
    }

    #[test]
    fn unloaded_links_make_it_wait_and_refresh_cache_on_return() {
        let mut w = World::new(&[]);
        w.unloaded = 2;
        let mut g = FactoryPanel::new(7);
        g.filter = Some(it("a"));
        g.count = 1;
        g.tick_storage_monitor(&mut w);
        assert!(g.waiting_for_network);
        w.unloaded = 0;
        g.tick_storage_monitor(&mut w);
        assert!(!g.waiting_for_network);
        assert_eq!(w.invalidations, 1);
    }

    #[test]
    fn crafting_request_sends_order_with_recipe_and_adds_promise() {
        let mut w = World::new(&[("plank", 10)]);
        let mut g = crafter("plank", 4, &mut w);
        g.recipe_output = 2;
        g.active_crafting_arrangement = vec![it("plank"), it("plank")];
        g.tick(&mut w);
        assert_eq!(w.performed.len(), 1);
        let (p, reqs) = &w.performed[0][0];
        assert_eq!(*p, 10);
        assert_eq!(reqs[0].count, 4);
        assert_eq!(reqs[0].address, "assembly");
        let ctx = reqs[0].context.as_ref().unwrap();
        assert_eq!(ctx.ordered_crafts.len(), 1);
        assert_eq!(ctx.ordered_crafts[0].count, 1);
        // promise of recipe_output items registered on the network queue
        assert_eq!(w.promises.total_promised_and_remove_expired(&it("out"), None), 2);
        assert!(g.events.contains(&GaugeEvent::Effect { from: Some(key(1)), success: true }));
        assert!(g.events.contains(&GaugeEvent::Advancement("factory_gauge")));
        assert_eq!(g.timer, DEFAULT_TIMER_INTERVAL);
    }

    #[test]
    fn crafting_waits_for_timer_between_attempts() {
        let mut w = World::new(&[("plank", 100)]);
        let mut g = crafter("plank", 1, &mut w);
        g.tick_requests(&mut w);
        assert_eq!(w.performed.len(), 1);
        // satisfied flags stay false in this isolated call; the timer blocks
        for _ in 0..DEFAULT_TIMER_INTERVAL {
            g.tick_requests(&mut w);
        }
        assert_eq!(w.performed.len(), 1);
        g.tick_requests(&mut w);
        assert_eq!(w.performed.len(), 2);
    }

    #[test]
    fn missing_ingredient_fails_the_whole_request() {
        let mut w = World::new(&[("plank", 3)]);
        let mut g = crafter("plank", 4, &mut w);
        g.tick_requests(&mut w);
        assert!(w.performed.is_empty());
        assert!(g.events.contains(&GaugeEvent::Effect { from: Some(key(1)), success: false }));
        assert_eq!(w.promises.total_promised_and_remove_expired(&it("out"), None), 0);
    }

    #[test]
    fn blank_address_missing_source_and_busy_packager_block_requests() {
        let mut w = World::new(&[("plank", 9)]);
        let mut g = crafter("plank", 1, &mut w);
        g.recipe_address = "  ".into();
        g.tick_requests(&mut w);
        assert!(w.performed.is_empty());
        let mut g2 = crafter("plank", 1, &mut w);
        g2.targeted_by.push(Connection { from: key(99), amount: 1, arrow_bend_mode: 0 });
        g2.tick_requests(&mut w);
        assert!(w.performed.is_empty(), "unknown source panel aborts");
        let mut g3 = crafter("plank", 1, &mut w);
        w.busy = true;
        g3.tick_requests(&mut w);
        assert!(w.performed.is_empty());
        assert_eq!(w.promises.total_promised_and_remove_expired(&it("out"), None), 0);
    }

    #[test]
    fn duplicate_item_sources_are_summed() {
        let mut w = World::new(&[("plank", 5)]);
        let mut g = crafter("plank", 2, &mut w);
        w.sources.insert(key(2), SourcePanel { filter: Some(it("plank")), network: 7, active: true });
        g.targeted_by.push(Connection { from: key(2), amount: 3, arrow_bend_mode: 0 });
        g.tick_requests(&mut w);
        assert_eq!(w.performed[0][0].1[0].count, 5);
        let mut g2 = crafter("plank", 2, &mut w);
        g2.targeted_by.push(Connection { from: key(2), amount: 4, arrow_bend_mode: 0 });
        g2.tick_requests(&mut w);
        assert_eq!(w.performed.len(), 1, "2+4 > 5 stock fails");
    }

    #[test]
    fn satisfied_or_redstone_blocks_requests() {
        let mut w = World::new(&[("plank", 9)]);
        let mut g = crafter("plank", 1, &mut w);
        g.promised_satisfied = true;
        g.tick_requests(&mut w);
        assert!(w.performed.is_empty());
        g.promised_satisfied = false;
        g.redstone_powered = true;
        g.tick_requests(&mut w);
        assert!(w.performed.is_empty());
        g.redstone_powered = false;
        g.waiting_for_network = true;
        g.tick_requests(&mut w);
        assert!(w.performed.is_empty());
    }

    #[test]
    fn restocker_orders_missing_amount_up_to_network_stock() {
        let mut w = World::new(&[("iron", 20)]);
        w.restock_pkg = Some(summary_of(&[("iron", 3)]));
        let mut g = FactoryPanel::new(7);
        g.restocker = true;
        g.filter = Some(it("iron"));
        g.count = 10;
        g.recipe_address = "rs".into();
        g.tick(&mut w);
        assert_eq!(w.restock_log.len(), 1);
        assert_eq!(w.restock_log[0].stacks()[0].count, 7);
        assert_eq!(g.restocker_promises.total_promised_and_remove_expired(&it("iron"), None), 7);
        // a promise is outstanding: promised-satisfied until it expires
        g.tick(&mut w);
        assert!(g.promised_satisfied);
        assert_eq!(w.restock_log.len(), 1);
    }

    #[test]
    fn restocker_is_capped_by_network_stock_and_nine_stacks() {
        let mut w = World::new(&[("iron", 5)]);
        w.restock_pkg = Some(summary_of(&[]));
        let mut g = FactoryPanel::new(7);
        g.restocker = true;
        g.filter = Some(it("iron"));
        g.count = 30;
        g.up_to = false; // 30 stacks = 1920 items
        g.recipe_address = "rs".into();
        g.tick_requests(&mut w);
        assert_eq!(w.restock_log[0].stacks()[0].count, 5);
        let mut w2 = World::new(&[("iron", 5000)]);
        w2.restock_pkg = Some(summary_of(&[]));
        let mut g2 = FactoryPanel::new(7);
        g2.restocker = true;
        g2.filter = Some(it("iron"));
        g2.count = 30;
        g2.up_to = false;
        g2.recipe_address = "rs".into();
        g2.tick_requests(&mut w2);
        assert_eq!(w2.restock_log[0].stacks()[0].count, 576);
    }

    #[test]
    fn restocker_without_stock_or_packager_does_nothing() {
        let mut w = World::new(&[]);
        w.restock_pkg = Some(summary_of(&[]));
        let mut g = FactoryPanel::new(7);
        g.restocker = true;
        g.filter = Some(it("iron"));
        g.count = 10;
        g.recipe_address = "rs".into();
        g.tick_requests(&mut w);
        assert!(w.restock_log.is_empty());
        assert!(g.events.contains(&GaugeEvent::Effect { from: None, success: false }));
        w.restock_pkg = None;
        g.timer = 0;
        g.tick_storage_monitor(&mut w);
        assert!(g.waiting_for_network, "no restocked packager counts as an unloaded link");
    }

    #[test]
    fn restock_promises_expire() {
        let mut w = World::new(&[("iron", 50)]);
        w.restock_pkg = Some(summary_of(&[]));
        let mut g = FactoryPanel::new(7);
        g.restocker = true;
        g.promise_clearing_interval = 0; // 600 ticks
        g.filter = Some(it("iron"));
        g.count = 8;
        g.recipe_address = "rs".into();
        g.tick(&mut w);
        assert_eq!(w.restock_log.len(), 1);
        for _ in 0..600 {
            g.restocker_promises.tick();
        }
        assert_eq!(g.restocker_promises.total_promised_and_remove_expired(&it("iron"), g.promise_expiry_ticks()), 0);
    }

    #[test]
    fn redstone_input_rules() {
        let mut g = FactoryPanel::new(1);
        g.check_for_redstone_input(&[Some(false), Some(true)]);
        assert!(g.redstone_powered && g.timer == 1);
        g.timer = 50;
        g.check_for_redstone_input(&[Some(true)]);
        assert_eq!(g.timer, 50, "unchanged");
        g.check_for_redstone_input(&[Some(false), None]);
        assert!(g.redstone_powered, "unloaded support aborts");
        g.check_for_redstone_input(&[Some(false)]);
        assert!(!g.redstone_powered);
        g.active = false;
        g.check_for_redstone_input(&[Some(true)]);
        assert!(!g.redstone_powered);
    }

    #[test]
    fn connection_rules() {
        let src = SourcePanel { filter: Some(it("a")), network: 1, active: true };
        let mut g = FactoryPanel::new(1);
        assert!(g.add_connection(key(1), Some(&src), false).is_ok());
        assert_eq!(g.targeted_by[0].amount, 1);
        assert!(g.add_connection(key(1), Some(&src), false).is_ok());
        assert_eq!(g.targeted_by.len(), 1, "no duplicate connection");
        assert!(g.add_connection(key(2), None, false).is_err());
        let inactive = SourcePanel { active: false, ..src.clone() };
        assert!(g.add_connection(key(2), Some(&inactive), false).is_err());
        assert!(g.add_connection(key(3), None, true).is_ok(), "links need no panel");
        for n in 10..20 {
            let _ = g.add_connection(key(n), Some(&src), false);
        }
        assert_eq!(g.targeted_by.len(), MAX_SOURCES);
        assert!(g.add_connection(key(50), Some(&src), false).is_err());
        assert!(g.add_connection(key(50), None, true).is_ok(), "link inputs bypass the limit");
        let mut r = FactoryPanel::new(1);
        r.restocker = true;
        assert!(r.add_connection(key(1), Some(&src), false).is_err());
        g.cycle_arrow_bend();
        assert!(g.targeted_by.iter().all(|c| c.arrow_bend_mode == 1));
        for _ in 0..3 {
            g.cycle_arrow_bend();
        }
        assert!(g.targeted_by.iter().all(|c| c.arrow_bend_mode == 0));
        g.remove_connection(&key(1));
        assert!(g.targeted_by.iter().all(|c| c.from != key(1)));
    }
}
