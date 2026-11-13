// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

//! Frequency-linked stock networks after `InventorySummary`,
//! `LogisticsManager`, `LogisticallyLinkedBehaviour`, `PackagerLinkBlockEntity`,
//! `RequestPromiseQueue` and `GlobalLogisticsManager` (Create 6.0.10).
//!
//! A `StockLink` is the data a Stock Link needs from its world: its
//! frequency, the packager it points at, the packager's inventory identity
//! and the packager's cached `getAvailableItems()` summary. The adapter keeps
//! those up to date and executes the resulting packager requests.
use crate::package::{BigStack, INF, ItemKey, ItemStack, OrderWithCrafts, Random, sort_by_count_desc};
use crate::packager::{PackagingRequest, RequestType};
use std::cell::Cell;
use std::collections::BTreeMap;
use std::rc::Rc;

pub type FreqId = u128;
pub type PackagerId = u32;
pub type InventoryId = u64;
/// Redstone power at which a Stock Link is switched off.
pub const LINK_DISABLED_POWER: u8 = 15;
/// `LogisticsManager` summary cache lifetimes in ticks.
pub const SUMMARY_TTL: u64 = 20;
pub const ACCURATE_SUMMARY_TTL: u64 = 1;

/// `InventorySummary`. Entries keep first-insertion order (Java uses an
/// identity hash map, so its raw order is unspecified); `stacks_by_count`
/// gives the descending view and is never stale.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InventorySummary {
    entries: Vec<BigStack>,
    total: u64,
    pub contributing_links: u32,
}

impl InventorySummary {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, stack: &ItemStack, count: u32) {
        if count == 0 || stack.is_empty() {
            return;
        }
        if self.total < INF as u64 {
            self.total += count as u64;
        }
        if let Some(e) = self.entries.iter_mut().find(|e| e.stack.same_item(stack)) {
            if e.count < INF {
                e.count += count;
            }
            return;
        }
        let template = if stack.count > stack.max_stack { stack.copy_with_count(1) } else { stack.clone() };
        self.entries.push(BigStack::new(template, count));
    }

    pub fn add_big(&mut self, entry: &BigStack) {
        self.add(&entry.stack, entry.count);
    }

    pub fn add_summary(&mut self, other: &InventorySummary) {
        for e in &other.entries {
            self.add_big(e);
        }
        self.contributing_links += other.contributing_links;
    }

    /// Removes the whole entry for this item.
    pub fn erase(&mut self, stack: &ItemStack) -> bool {
        match self.entries.iter().position(|e| e.stack.same_item(stack)) {
            Some(i) => {
                let e = self.entries.remove(i);
                self.total = self.total.saturating_sub(e.count as u64);
                true
            }
            None => false,
        }
    }

    /// Takes up to `count` of the item (capped at what is there); drops the entry at zero.
    pub fn subtract(&mut self, stack: &ItemStack, count: u32) {
        if let Some(i) = self.entries.iter().position(|e| e.stack.same_item(stack)) {
            let take = count.min(self.entries[i].count);
            self.entries[i].count -= take;
            self.total = self.total.saturating_sub(take as u64);
            if self.entries[i].count == 0 {
                self.entries.remove(i);
            }
        }
    }

    /// Removes a single item, keeping a zero-count entry (used while repackaging).
    pub fn remove_one(&mut self, stack: &ItemStack) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.stack.same_item(stack)) {
            e.count = e.count.saturating_sub(1);
            self.total = self.total.saturating_sub(1);
        }
    }

    pub fn count_of(&self, stack: &ItemStack) -> u32 {
        self.entries.iter().find(|e| e.stack.same_item(stack)).map_or(0, |e| e.count)
    }

    pub fn total_count(&self) -> u64 {
        self.total
    }

    pub fn total_of_matching(&self, f: impl Fn(&ItemStack) -> bool) -> u64 {
        self.entries.iter().filter(|e| f(&e.stack)).map(|e| e.count as u64).sum()
    }

    pub fn entries(&self) -> &[BigStack] {
        &self.entries
    }

    pub fn stacks_by_count(&self) -> Vec<BigStack> {
        let mut v = self.entries.clone();
        sort_by_count_desc(&mut v);
        v
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// `RequestPromise`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestPromise {
    pub promised: BigStack,
    pub ticks_existed: u32,
}

impl RequestPromise {
    pub fn new(promised: BigStack) -> Self {
        Self { promised, ticks_existed: 0 }
    }
}

/// `RequestPromiseQueue`. `changes` counts the change callbacks Java fires
/// (the adapter marks its block entity dirty when it moves).
#[derive(Clone, Debug, Default)]
pub struct RequestPromiseQueue {
    // per-item lists in first-use order, insertion order inside each list
    lists: Vec<(String, Vec<RequestPromise>)>,
    pub changes: u32,
}

impl RequestPromiseQueue {
    pub fn add(&mut self, promise: RequestPromise) {
        let id = promise.promised.stack.key.id.clone();
        match self.lists.iter_mut().find(|(i, _)| *i == id) {
            Some((_, l)) => l.push(promise),
            None => self.lists.push((id, vec![promise])),
        }
        self.changes += 1;
    }

    pub fn is_empty(&self) -> bool {
        self.lists.iter().all(|(_, l)| l.is_empty())
    }

    /// `forceClear`: drops all promises of the same item and components.
    pub fn force_clear(&mut self, stack: &ItemStack) {
        let id = &stack.key.id;
        let Some(pos) = self.lists.iter().position(|(i, _)| i == id) else { return };
        let list = &mut self.lists[pos].1;
        let before = list.len();
        list.retain(|p| !p.promised.stack.same_item(stack));
        self.changes += (before - list.len()) as u32;
        if list.is_empty() {
            self.lists.remove(pos);
        }
    }

    /// `itemEnteredSystem`: delivered items reduce the oldest matching promises.
    pub fn item_entered_system(&mut self, stack: &ItemStack, mut amount: u32) {
        let id = &stack.key.id;
        let Some(pos) = self.lists.iter().position(|(i, _)| i == id) else { return };
        let mut removed = 0;
        let list = &mut self.lists[pos].1;
        list.retain_mut(|p| {
            if amount == 0 || !p.promised.stack.same_item(stack) {
                return true;
            }
            let take = amount.min(p.promised.count);
            p.promised.count -= take;
            amount -= take;
            if p.promised.count == 0 {
                removed += 1;
                return false;
            }
            true
        });
        self.changes += removed;
        if list.is_empty() {
            self.lists.remove(pos);
        }
    }

    /// `getTotalPromisedAndRemoveExpired`; `expiry == None` means never.
    pub fn total_promised_and_remove_expired(&mut self, stack: &ItemStack, expiry: Option<u32>) -> u32 {
        let id = &stack.key.id;
        let Some((_, list)) = self.lists.iter_mut().find(|(i, _)| i == id) else { return 0 };
        let mut total = 0;
        let mut removed = 0;
        list.retain(|p| {
            if !p.promised.stack.same_item(stack) {
                return true;
            }
            if expiry.is_some_and(|e| p.ticks_existed >= e) {
                removed += 1;
                return false;
            }
            total += p.promised.count;
            true
        });
        self.changes += removed;
        total
    }

    pub fn tick(&mut self) {
        for (_, l) in &mut self.lists {
            for p in l {
                p.ticks_existed += 1;
            }
        }
    }
}

/// Stock Link state seen by `LogisticallyLinkedBehaviour` + `PackagerLinkBlockEntity`.
#[derive(Clone, Debug)]
pub struct StockLink {
    pub id: u32,
    pub freq: FreqId,
    /// The packager this link points at (`None`: nothing attached, or a Repackager).
    pub packager: Option<PackagerId>,
    /// Identity of the packager's target inventory, when it has one.
    pub inventory: Option<InventoryId>,
    pub power: u8,
    /// Chunk loaded and block entity alive (`isValidLink`).
    pub loaded: bool,
    /// `packager.getAvailableItems()`.
    pub available: InventorySummary,
}

impl StockLink {
    pub fn new(id: u32, freq: FreqId, packager: PackagerId, inventory: InventoryId, available: InventorySummary) -> Self {
        Self { id, freq, packager: Some(packager), inventory: Some(inventory), power: 0, loaded: true, available }
    }

    /// `getPackager`: disabled by full redstone power.
    pub fn active_packager(&self) -> Option<PackagerId> {
        if self.power >= LINK_DISABLED_POWER { None } else { self.packager }
    }

    fn targets_same_inventory(&self, ignored: Option<InventoryId>) -> bool {
        ignored.is_some() && self.inventory == ignored
    }

    /// `fetchSummaryFromPackager`: `None` is `InventorySummary.EMPTY`.
    pub fn summary(&self, ignored: Option<InventoryId>) -> Option<&InventorySummary> {
        if self.active_packager().is_none() || self.targets_same_inventory(ignored) {
            None
        } else {
            Some(&self.available)
        }
    }

    /// `processRequest`: caps the amount by the packager's stock.
    #[allow(clippy::too_many_arguments)]
    pub fn process_request(
        &self,
        stack: &ItemStack,
        amount: u32,
        address: &str,
        link_index: i32,
        final_link: Rc<Cell<bool>>,
        order_id: i32,
        context: Option<OrderWithCrafts>,
        ignored: Option<InventoryId>,
    ) -> Option<(PackagerId, PackagingRequest)> {
        let packager = self.active_packager()?;
        if self.targets_same_inventory(ignored) {
            return None;
        }
        let available = self.available.count_of(stack);
        if available == 0 {
            return None;
        }
        let to_withdraw = amount.min(available);
        Some((
            packager,
            PackagingRequest::create(stack.clone(), to_withdraw, address, link_index, final_link, 0, order_id, context),
        ))
    }
}

fn present(links: &[StockLink], freq: FreqId) -> impl Iterator<Item = &StockLink> {
    links.iter().filter(move |l| l.freq == freq && l.loaded)
}

/// `createSummaryOfNetwork`: inventories behind several links count once.
pub fn summary_of_network(links: &[StockLink], freq: FreqId) -> InventorySummary {
    let mut out = InventorySummary::new();
    let mut seen: Vec<InventoryId> = Vec::new();
    for l in present(links, freq) {
        if let Some(inv) = l.inventory {
            if seen.contains(&inv) {
                continue;
            }
            seen.push(inv);
        }
        if let Some(s) = l.summary(None) {
            out.contributing_links += 1;
            let contributing = out.contributing_links;
            out.add_summary(s);
            out.contributing_links = contributing;
        }
    }
    out
}

/// `getStockOf`: uncached, no inventory de-duplication.
pub fn stock_of(links: &[StockLink], freq: FreqId, stack: &ItemStack, ignored: Option<InventoryId>) -> u32 {
    present(links, freq).filter_map(|l| l.summary(ignored)).map(|s| s.count_of(stack)).sum()
}

/// How many links of `freq` are in the world but not loaded is tracked by
/// `LogisticsNetworks`; this counts only the loaded ones.
pub fn loaded_link_count(links: &[StockLink], freq: FreqId) -> usize {
    present(links, freq).count()
}

/// Packager-grouped requests, in first-appearance order (the Java multimap).
pub type PackagerRequests = Vec<(PackagerId, Vec<PackagingRequest>)>;

pub fn group_by_packager(flat: Vec<(PackagerId, PackagingRequest)>) -> PackagerRequests {
    let mut out: PackagerRequests = Vec::new();
    for (p, r) in flat {
        match out.iter_mut().find(|(id, _)| *id == p) {
            Some((_, v)) => v.push(r),
            None => out.push((p, vec![r])),
        }
    }
    out
}

/// `findPackagersForRequest` (flattened, in request order).
pub fn find_packagers_for_request(
    links: &[StockLink],
    freq: FreqId,
    order: &OrderWithCrafts,
    ignored: Option<InventoryId>,
    address: &str,
    rng: &mut dyn Random,
) -> Vec<(PackagerId, PackagingRequest)> {
    let stacks: Vec<&BigStack> = order.stacks().iter().filter(|s| !s.stack.is_empty() && s.count > 0).collect();
    // links behind a shared inventory contribute one random representative
    let mut available: Vec<&StockLink> = Vec::new();
    let mut groups: Vec<(InventoryId, Vec<&StockLink>)> = Vec::new();
    for l in present(links, freq) {
        match l.inventory {
            Some(inv) => match groups.iter_mut().find(|(i, _)| *i == inv) {
                Some((_, g)) => g.push(l),
                None => groups.push((inv, vec![l])),
            },
            None => available.push(l),
        }
    }
    for (_, g) in &groups {
        available.push(g[rng.next_below(g.len())]);
    }

    let order_id = rng.next_int();
    let mut used: Vec<u32> = Vec::new();
    let mut final_tracker = Rc::new(Cell::new(false));
    let mut context = Some(order.clone());
    let mut requests: Vec<(PackagerId, PackagingRequest)> = Vec::new();
    for (i, entry) in stacks.iter().enumerate() {
        let mut remaining = entry.count;
        let final_entry = i + 1 == stacks.len();
        for link in &available {
            let used_index = used.iter().position(|&u| u == link.id);
            let link_index = used_index.unwrap_or(used.len());
            let is_final_link = if link_index + 1 == used.len() {
                Rc::clone(&final_tracker)
            } else {
                Rc::new(Cell::new(false))
            };
            let Some((packager, request)) = link.process_request(
                &entry.stack,
                remaining,
                address,
                link_index as i32,
                Rc::clone(&is_final_link),
                order_id,
                context.clone(),
                ignored,
            ) else {
                continue;
            };
            let processed = request.count;
            requests.push((packager, request));
            if processed > 0 && used_index.is_none() {
                context = None;
                used.push(link.id);
                final_tracker = is_final_link;
            }
            remaining -= processed;
            if remaining > 0 {
                continue;
            }
            if final_entry {
                final_tracker.set(true);
            }
            break;
        }
    }
    requests
}

/// `LogisticsManager.broadcastPackageRequest`: `None` when refused (empty
/// order or a packager is too busy for this request type); otherwise the
/// requests the adapter must now run through `PackagerState::perform_requests`
/// (which may be empty, as in Java where an unfillable order still returns true).
#[allow(clippy::too_many_arguments)]
pub fn broadcast_package_request(
    links: &[StockLink],
    freq: FreqId,
    ty: RequestType,
    order: &OrderWithCrafts,
    ignored: Option<InventoryId>,
    address: &str,
    too_busy: &dyn Fn(PackagerId, RequestType) -> bool,
    rng: &mut dyn Random,
) -> Option<PackagerRequests> {
    if order.is_empty() {
        return None;
    }
    let grouped = group_by_packager(find_packagers_for_request(links, freq, order, ignored, address, rng));
    if grouped.iter().any(|(p, _)| too_busy(*p, ty)) {
        return None;
    }
    Some(grouped)
}

/// `TickBasedCache` for network summaries.
#[derive(Clone, Debug)]
pub struct SummaryCache {
    ttl: u64,
    map: BTreeMap<FreqId, (u64, InventorySummary)>,
}

impl SummaryCache {
    pub fn new(ttl: u64) -> Self {
        Self { ttl, map: BTreeMap::new() }
    }

    pub fn get(&mut self, freq: FreqId, now: u64, compute: impl FnOnce() -> InventorySummary) -> InventorySummary {
        if let Some((t, s)) = self.map.get(&freq) {
            if now.saturating_sub(*t) < self.ttl {
                return s.clone();
            }
        }
        let s = compute();
        self.map.insert(freq, (now, s.clone()));
        s
    }

    pub fn invalidate(&mut self, freq: FreqId) {
        self.map.remove(&freq);
    }

    /// `deductFromAccurateSummary`: subtract a sent package's contents, capped.
    pub fn deduct(&mut self, freq: FreqId, items: &[ItemStack]) {
        if let Some((_, s)) = self.map.get_mut(&freq) {
            for i in items {
                s.subtract(i, i.count);
            }
        }
    }
}

/// One `LogisticsNetwork`.
#[derive(Clone, Debug, Default)]
pub struct LogisticsNetwork {
    pub owner: Option<u128>,
    pub locked: bool,
    pub total_links: std::collections::BTreeSet<LinkPos>,
    pub loaded_links: std::collections::BTreeSet<LinkPos>,
    pub panel_promises: RequestPromiseQueue,
}

/// `GlobalPos` of a link.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct LinkPos {
    pub dimension: String,
    pub pos: [i32; 3],
}

/// `GlobalLogisticsManager`.
#[derive(Clone, Debug, Default)]
pub struct LogisticsNetworks {
    pub networks: BTreeMap<FreqId, LogisticsNetwork>,
    pub dirty: bool,
}

impl LogisticsNetworks {
    pub fn link_added(&mut self, freq: FreqId, pos: LinkPos, owned_by: Option<u128>) {
        let n = self.networks.entry(freq).or_default();
        n.total_links.insert(pos);
        if n.owner.is_none() {
            n.owner = owned_by;
        }
        self.dirty = true;
    }

    pub fn link_loaded(&mut self, freq: FreqId, pos: LinkPos) {
        self.networks.entry(freq).or_default().loaded_links.insert(pos);
    }

    pub fn link_removed(&mut self, freq: FreqId, pos: &LinkPos) {
        let Some(n) = self.networks.get_mut(&freq) else { return };
        n.total_links.remove(pos);
        n.loaded_links.remove(pos);
        if n.total_links.is_empty() {
            self.networks.remove(&freq);
        }
        self.dirty = true;
    }

    pub fn link_invalidated(&mut self, freq: FreqId, pos: &LinkPos) {
        if let Some(n) = self.networks.get_mut(&freq) {
            n.loaded_links.remove(pos);
        }
    }

    pub fn unloaded_link_count(&self, freq: FreqId) -> usize {
        self.networks.get(&freq).map_or(0, |n| n.total_links.len().saturating_sub(n.loaded_links.len()))
    }

    pub fn may_interact(&self, freq: FreqId, player: u128) -> bool {
        self.networks.get(&freq).is_none_or(|n| n.owner.is_none_or(|o| !n.locked || o == player))
    }

    pub fn may_administrate(&self, freq: FreqId, player: u128) -> bool {
        self.networks.get(&freq).is_none_or(|n| n.owner.is_none_or(|o| o == player))
    }

    pub fn is_lockable(&self, freq: FreqId) -> bool {
        self.networks.contains_key(&freq)
    }

    pub fn is_locked(&self, freq: FreqId) -> bool {
        self.networks.get(&freq).is_some_and(|n| n.locked)
    }

    pub fn queued_promises(&mut self, freq: FreqId) -> Option<&mut RequestPromiseQueue> {
        self.networks.get_mut(&freq).map(|n| &mut n.panel_promises)
    }

    pub fn has_queued_promises(&self, freq: FreqId) -> bool {
        self.networks.get(&freq).is_some_and(|n| !n.panel_promises.is_empty())
    }

    /// Overworld tick: age every network's promises.
    pub fn tick(&mut self) {
        for n in self.networks.values_mut() {
            n.panel_promises.tick();
        }
    }
}

/// `ItemKey` helper for callers building summaries by id.
pub fn summary_of(items: &[(&str, u32)]) -> InventorySummary {
    let mut s = InventorySummary::new();
    for (id, n) in items {
        let st = ItemStack { key: ItemKey::new(id), count: 1, max_stack: 64, bulky: false, package: None };
        s.add(&st, *n);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::XorShift;

    fn it(id: &str) -> ItemStack {
        ItemStack::new(id, 1)
    }

    fn link(id: u32, packager: u32, inv: u64, items: &[(&str, u32)]) -> StockLink {
        StockLink::new(id, 1, packager, inv, summary_of(items))
    }

    #[test]
    fn summary_adds_and_counts() {
        let mut s = InventorySummary::new();
        s.add(&it("a"), 5);
        s.add(&ItemStack::new("a", 200), 3);
        s.add(&it("b"), 9);
        s.add(&it("c"), 0);
        assert_eq!(s.count_of(&it("a")), 8);
        assert_eq!(s.total_count(), 17);
        assert_eq!(s.entries().len(), 2);
        assert_eq!(s.stacks_by_count()[0].stack.key.id, "b");
        assert!(s.erase(&it("b")));
        assert!(!s.erase(&it("b")));
        assert_eq!(s.total_count(), 8);
        s.subtract(&it("a"), 100);
        assert!(s.is_empty());
        let mut inf = InventorySummary::new();
        inf.add(&it("a"), INF);
        inf.add(&it("a"), 5);
        assert_eq!(inf.count_of(&it("a")), INF, "stops growing at INF");
    }

    #[test]
    fn components_split_summary_entries() {
        let mut s = InventorySummary::new();
        s.add(&it("a"), 1);
        s.add(&it("a").with_components("ench"), 2);
        assert_eq!(s.entries().len(), 2);
        assert_eq!(s.count_of(&it("a").with_components("ench")), 2);
    }

    #[test]
    fn network_summary_dedupes_shared_inventory() {
        let links = vec![
            link(1, 10, 100, &[("a", 5)]),
            link(2, 11, 100, &[("a", 5)]), // same inventory: counted once
            link(3, 12, 101, &[("a", 2), ("b", 1)]),
        ];
        let s = summary_of_network(&links, 1);
        assert_eq!(s.count_of(&it("a")), 7);
        assert_eq!(s.contributing_links, 2);
        // getStockOf does not dedupe
        assert_eq!(stock_of(&links, 1, &it("a"), None), 12);
        assert_eq!(stock_of(&links, 1, &it("a"), Some(101)), 10);
        assert_eq!(summary_of_network(&links, 2).contributing_links, 0);
    }

    #[test]
    fn disabled_and_unloaded_links_do_not_contribute() {
        let mut l = link(1, 10, 100, &[("a", 5)]);
        l.power = 15;
        assert!(l.active_packager().is_none());
        assert!(summary_of_network(&[l.clone()], 1).is_empty());
        l.power = 14;
        l.loaded = false;
        assert_eq!(loaded_link_count(&[l], 1), 0);
    }

    #[test]
    fn single_link_request_is_final_and_capped() {
        let links = vec![link(1, 10, 100, &[("a", 5)])];
        let order = OrderWithCrafts::simple(vec![BigStack::new(it("a"), 8)]);
        let mut rng = XorShift(3);
        let r = find_packagers_for_request(&links, 1, &order, None, "home", &mut rng);
        assert_eq!(r.len(), 1);
        let (p, req) = &r[0];
        assert_eq!(*p, 10);
        assert_eq!(req.count, 5, "capped by availability");
        assert_eq!((req.link_index, req.address.as_str()), (0, "home"));
        assert!(!req.final_link.get(), "shortfall: nothing marked final");
        assert!(req.context.is_some());
    }

    #[test]
    fn two_links_split_and_mark_final_on_last() {
        let links = vec![link(1, 10, 100, &[("a", 5)]), link(2, 11, 101, &[("a", 5), ("b", 2)])];
        let order = OrderWithCrafts::simple(vec![BigStack::new(it("a"), 8), BigStack::new(it("b"), 2)]);
        let mut rng = XorShift(3);
        let r = find_packagers_for_request(&links, 1, &order, None, "x", &mut rng);
        let summary: Vec<(u32, u32, i32)> = r.iter().map(|(p, q)| (*p, q.count, q.link_index)).collect();
        assert_eq!(summary, vec![(10, 5, 0), (11, 3, 1), (11, 2, 1)]);
        // all requests share one order id; context goes to the first used link only
        assert!(r.iter().all(|(_, q)| q.order_id == r[0].1.order_id));
        assert!(r[0].1.context.is_some() && r[1].1.context.is_none());
        // the last link's flag is true, the first link's is not
        assert!(!r[0].1.final_link.get());
        assert!(r[1].1.final_link.get() && r[2].1.final_link.get());
    }

    #[test]
    fn requested_stack_that_needs_both_links_for_last_entry() {
        let links = vec![link(1, 10, 100, &[("a", 2)]), link(2, 11, 101, &[("a", 2)])];
        let order = OrderWithCrafts::simple(vec![BigStack::new(it("a"), 3)]);
        let r = find_packagers_for_request(&links, 1, &order, None, "x", &mut XorShift(1));
        assert_eq!(r.iter().map(|(_, q)| q.count).collect::<Vec<_>>(), vec![2, 1]);
        assert!(r[1].1.final_link.get());
    }

    #[test]
    fn shared_inventory_uses_one_random_representative() {
        let links = vec![link(1, 10, 100, &[("a", 4)]), link(2, 11, 100, &[("a", 4)])];
        let order = OrderWithCrafts::simple(vec![BigStack::new(it("a"), 4)]);
        let mut seen = std::collections::BTreeSet::new();
        for seed in 1..40 {
            let r = find_packagers_for_request(&links, 1, &order, None, "x", &mut XorShift(seed));
            assert_eq!(r.len(), 1);
            seen.insert(r[0].0);
        }
        assert_eq!(seen.len(), 2, "both representatives get picked over seeds");
        // deterministic for a fixed seed
        let a = find_packagers_for_request(&links, 1, &order, None, "x", &mut XorShift(9));
        let b = find_packagers_for_request(&links, 1, &order, None, "x", &mut XorShift(9));
        assert_eq!(a[0].0, b[0].0);
    }

    #[test]
    fn ignored_inventory_is_skipped() {
        let links = vec![link(1, 10, 100, &[("a", 4)]), link(2, 11, 101, &[("a", 4)])];
        let order = OrderWithCrafts::simple(vec![BigStack::new(it("a"), 4)]);
        let r = find_packagers_for_request(&links, 1, &order, Some(100), "x", &mut XorShift(1));
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].0, 11);
    }

    #[test]
    fn empty_stacks_and_zero_counts_are_filtered() {
        let links = vec![link(1, 10, 100, &[("a", 4)])];
        let order = OrderWithCrafts::simple(vec![BigStack::new(it("a"), 0), BigStack::new(ItemStack::new("a", 0), 3)]);
        assert!(find_packagers_for_request(&links, 1, &order, None, "x", &mut XorShift(1)).is_empty());
    }

    #[test]
    fn broadcast_refuses_empty_and_busy() {
        let links = vec![link(1, 10, 100, &[("a", 4)])];
        let order = OrderWithCrafts::simple(vec![BigStack::new(it("a"), 2)]);
        let mut rng = XorShift(1);
        assert!(broadcast_package_request(&links, 1, RequestType::Player, &OrderWithCrafts::empty(), None, "", &|_, _| false, &mut rng).is_none());
        assert!(broadcast_package_request(&links, 1, RequestType::Restock, &order, None, "", &|p, t| p == 10 && t == RequestType::Restock, &mut rng).is_none());
        let ok = broadcast_package_request(&links, 1, RequestType::Player, &order, None, "", &|_, _| false, &mut rng).unwrap();
        assert_eq!(ok.len(), 1);
        // an unfillable order is still accepted, with nothing to run
        let none = OrderWithCrafts::simple(vec![BigStack::new(it("zzz"), 2)]);
        assert!(broadcast_package_request(&links, 1, RequestType::Player, &none, None, "", &|_, _| false, &mut rng).unwrap().is_empty());
    }

    #[test]
    fn grouping_keeps_first_appearance() {
        let f = Rc::new(Cell::new(false));
        let mk = |n| PackagingRequest::create(it("a"), n, "", 0, f.clone(), 0, 1, None);
        let g = group_by_packager(vec![(5, mk(1)), (3, mk(2)), (5, mk(3))]);
        assert_eq!(g.iter().map(|(p, v)| (*p, v.len())).collect::<Vec<_>>(), vec![(5, 2), (3, 1)]);
    }

    #[test]
    fn promise_queue_rules() {
        let mut q = RequestPromiseQueue::default();
        q.add(RequestPromise::new(BigStack::new(it("a"), 5)));
        q.add(RequestPromise::new(BigStack::new(it("a"), 3)));
        q.add(RequestPromise::new(BigStack::new(it("b"), 1)));
        assert_eq!(q.total_promised_and_remove_expired(&it("a"), None), 8);
        q.item_entered_system(&it("a"), 6);
        assert_eq!(q.total_promised_and_remove_expired(&it("a"), None), 2);
        // partial reduction fires no callback; the removal of the first fired one
        assert_eq!(q.changes, 3 + 1);
        for _ in 0..10 {
            q.tick();
        }
        assert_eq!(q.total_promised_and_remove_expired(&it("a"), Some(11)), 2);
        assert_eq!(q.total_promised_and_remove_expired(&it("a"), Some(10)), 0);
        q.force_clear(&it("b"));
        assert!(q.is_empty() || !q.has_a());
    }

    impl RequestPromiseQueue {
        fn has_a(&self) -> bool {
            self.lists.iter().any(|(i, l)| i == "a" && !l.is_empty())
        }
    }

    #[test]
    fn force_clear_is_component_aware() {
        let mut q = RequestPromiseQueue::default();
        q.add(RequestPromise::new(BigStack::new(it("a"), 5)));
        q.add(RequestPromise::new(BigStack::new(it("a").with_components("x"), 5)));
        q.force_clear(&it("a"));
        assert_eq!(q.total_promised_and_remove_expired(&it("a"), None), 0);
        assert_eq!(q.total_promised_and_remove_expired(&it("a").with_components("x"), None), 5);
    }

    #[test]
    fn global_manager_ownership_and_unloaded_counts() {
        let mut g = LogisticsNetworks::default();
        let pos = |x| LinkPos { dimension: "overworld".into(), pos: [x, 0, 0] };
        assert!(g.may_interact(9, 1) && g.may_administrate(9, 1));
        g.link_added(9, pos(0), Some(42));
        g.link_added(9, pos(1), Some(43));
        assert_eq!(g.networks[&9].owner, Some(42), "first owner sticks");
        g.link_loaded(9, pos(0));
        assert_eq!(g.unloaded_link_count(9), 1);
        g.link_loaded(9, pos(1));
        assert_eq!(g.unloaded_link_count(9), 0);
        g.link_invalidated(9, &pos(1));
        assert_eq!(g.unloaded_link_count(9), 1);
        g.networks.get_mut(&9).unwrap().locked = true;
        assert!(!g.may_interact(9, 7));
        assert!(g.may_interact(9, 42));
        assert!(!g.may_administrate(9, 7));
        assert!(g.is_lockable(9) && g.is_locked(9));
        g.link_removed(9, &pos(0));
        assert!(g.networks.contains_key(&9));
        g.link_removed(9, &pos(1));
        assert!(!g.networks.contains_key(&9));
        assert_eq!(g.unloaded_link_count(9), 0);
    }

    #[test]
    fn global_manager_promises_tick() {
        let mut g = LogisticsNetworks::default();
        g.link_added(1, LinkPos { dimension: "d".into(), pos: [0; 3] }, None);
        g.queued_promises(1).unwrap().add(RequestPromise::new(BigStack::new(it("a"), 1)));
        assert!(g.has_queued_promises(1));
        g.tick();
        assert_eq!(g.queued_promises(1).unwrap().total_promised_and_remove_expired(&it("a"), Some(1)), 0);
        assert!(g.queued_promises(2).is_none());
    }

    #[test]
    fn summary_cache_ttl_and_deduction() {
        let mut c = SummaryCache::new(ACCURATE_SUMMARY_TTL);
        let s = c.get(1, 0, || summary_of(&[("a", 10)]));
        assert_eq!(s.count_of(&it("a")), 10);
        c.deduct(1, &[ItemStack::new("a", 4)]);
        assert_eq!(c.get(1, 0, || panic!("cached")).count_of(&it("a")), 6);
        assert_eq!(c.get(1, 1, || summary_of(&[("a", 1)])).count_of(&it("a")), 1);
        c.deduct(1, &[ItemStack::new("a", 99)]);
        assert!(c.get(1, 1, || panic!("cached")).is_empty());
        c.invalidate(1);
        let mut slow = SummaryCache::new(SUMMARY_TTL);
        slow.get(1, 0, || summary_of(&[("a", 1)]));
        assert_eq!(slow.get(1, 19, || summary_of(&[])).count_of(&it("a")), 1);
        assert!(slow.get(1, 20, || summary_of(&[])).is_empty());
    }
}
