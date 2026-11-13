// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

//! Packager and Repackager state machines after `PackagerBlockEntity`,
//! `RepackagerBlockEntity`, `PackagingRequest`, `PackageRepackageHelper`
//! and the default `UnpackingHandler` (Create 6.0.10). Inventories are
//! passed in as `ItemHandler`; no world access happens here.
//!
//! Parts of `attemptToSend` were reconstructed from a behavioural reading of
//! the Java (see docs/logistics.md, "Unverified"): the exact interleaving of
//! the slot scan is simplified to one extraction per package slot.
use crate::package::{
    self, BigStack, ItemHandler, ItemStack, OrderWithCrafts, Random, SLOTS, add_address, clear_address,
    contents_stacks, get_address, get_order_id, insert_stacked, is_package, set_order,
};
use crate::stock_network::InventorySummary;
use std::cell::Cell;
use std::rc::Rc;

/// Length of one packager animation in ticks.
pub const CYCLE: i32 = 20;
/// Cool-down after a redstone/button activation.
pub const BUTTON_COOLDOWN: i32 = 40;
/// Per-extraction item cap for requests and manual sends.
pub const EXTRACT_CAP: u32 = 64;
/// `performPackageRequests` calls `attemptToSend` at most this many times.
pub const MAX_SEND_PASSES: usize = 100;

pub const DEFAULT_STYLE: &str = "create:cardboard_package_12x12";

/// `LogisticallyLinkedBehaviour.RequestType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestType {
    Restock,
    Redstone,
    Player,
}

/// `PackagingRequest`. `final_link` and the counter are shared like Java's
/// `MutableBoolean`/`MutableInt`.
#[derive(Clone, Debug)]
pub struct PackagingRequest {
    pub item: ItemStack,
    pub count: u32,
    pub address: String,
    pub link_index: i32,
    pub final_link: Rc<Cell<bool>>,
    pub package_counter: i32,
    pub order_id: i32,
    pub context: Option<OrderWithCrafts>,
}

impl PackagingRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        item: ItemStack,
        count: u32,
        address: &str,
        link_index: i32,
        final_link: Rc<Cell<bool>>,
        package_count: i32,
        order_id: i32,
        context: Option<OrderWithCrafts>,
    ) -> Self {
        Self {
            item,
            count,
            address: address.to_owned(),
            link_index,
            final_link,
            package_counter: package_count,
            order_id,
            context,
        }
    }

    /// Saturating at zero (Java's count may go negative; it never does in
    /// practice because extraction is capped by the remaining count).
    pub fn subtract(&mut self, n: u32) {
        self.count = self.count.saturating_sub(n);
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

/// Result of a successful `attemptToSend`.
#[derive(Clone, Debug, PartialEq)]
pub struct SendReport {
    /// The package that left (or was queued to leave).
    pub package: ItemStack,
    /// Items taken out of the inventory (empty if an existing package was reused).
    pub extracted: Vec<ItemStack>,
    /// True when the box went to `queued_exiting` instead of `held_box`.
    pub queued: bool,
}

/// Packager block entity state.
#[derive(Clone, Debug)]
pub struct PackagerState {
    pub held_box: Option<ItemStack>,
    pub previously_unwrapped: Option<ItemStack>,
    pub animation_ticks: i32,
    pub animation_inward: bool,
    pub button_cooldown: i32,
    pub redstone_powered: bool,
    /// `LINKED` block state: a Stock Link is attached.
    pub linked: bool,
    pub sign_address: String,
    pub queued_exiting: Vec<(ItemStack, u32)>,
    /// Package item id used for freshly made boxes.
    pub style: String,
}

impl Default for PackagerState {
    fn default() -> Self {
        Self {
            held_box: None,
            previously_unwrapped: None,
            animation_ticks: 0,
            animation_inward: false,
            button_cooldown: 0,
            redstone_powered: false,
            linked: false,
            sign_address: String::new(),
            queued_exiting: Vec::new(),
            style: DEFAULT_STYLE.to_owned(),
        }
    }
}

/// What `tick` observed, for adapters to react to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TickReport {
    /// An animation just ended (Java: `wakeTheFrogs`).
    pub animation_finished: bool,
    /// A queued package started leaving.
    pub started_next: bool,
}

/// `PackagerBlockEntity.isTooBusyFor` thresholds.
pub fn busy_threshold(t: RequestType) -> usize {
    match t {
        RequestType::Player => 50,
        RequestType::Redstone => 20,
        RequestType::Restock => 10,
    }
}

/// Default `UnpackingHandler`: every content stack must fit the target.
/// Returns whether it did; with `simulate == false` the items are inserted.
pub fn unpack_default(target: &mut dyn ItemHandler, items: &[ItemStack], simulate: bool) -> bool {
    if target.is_bottomless() {
        if !simulate {
            for s in items {
                insert_stacked(target, s.clone(), false);
            }
        }
        return true;
    }
    let mut probe = target.duplicate();
    for s in items {
        if insert_stacked(probe.as_mut(), s.clone(), false).is_some() {
            return false;
        }
    }
    if !simulate {
        for s in items {
            insert_stacked(target, s.clone(), false);
        }
    }
    true
}

/// `getSign`: non-blank trimmed lines joined by spaces; front before back.
pub fn sign_text(front: &[&str], back: &[&str]) -> String {
    for side in [front, back] {
        let joined: Vec<&str> = side.iter().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
        if !joined.is_empty() {
            return joined.join(" ");
        }
    }
    String::new()
}

impl PackagerState {
    pub fn is_too_busy_for(&self, t: RequestType) -> bool {
        self.queued_exiting.len() >= busy_threshold(t)
    }

    /// `updateSignAddress`: the last non-blank side in iteration order wins.
    pub fn update_sign_address(&mut self, sides: &[String]) {
        self.sign_address.clear();
        for s in sides {
            if !s.trim().is_empty() {
                self.sign_address = s.clone();
            }
        }
    }

    pub fn redstone_mode_active(&self) -> bool {
        !self.linked
    }

    /// Animation timer plus the queued-package hand-off (`tick`).
    pub fn tick(&mut self) -> TickReport {
        let mut report = TickReport::default();
        if self.button_cooldown > 0 {
            self.button_cooldown -= 1;
        }
        if self.animation_ticks > 0 {
            self.animation_ticks -= 1;
            if self.animation_ticks == 0 {
                report.animation_finished = true;
            }
        }
        if self.animation_ticks == 0 {
            self.previously_unwrapped = None;
            if self.held_box.is_none() && !self.queued_exiting.is_empty() {
                let (pkg, count) = &mut self.queued_exiting[0];
                self.held_box = Some(pkg.clone());
                *count -= 1;
                if *count == 0 {
                    self.queued_exiting.remove(0);
                }
                self.animation_inward = false;
                self.animation_ticks = CYCLE;
                report.started_next = true;
            }
        }
        report
    }

    /// Box shown by the renderer (`getRenderedBox`).
    pub fn rendered_box(&self) -> Option<&ItemStack> {
        if self.animation_inward {
            if self.animation_ticks <= CYCLE / 2 { None } else { self.previously_unwrapped.as_ref() }
        } else if self.animation_ticks < CYCLE / 2 {
            None
        } else {
            self.held_box.as_ref()
        }
    }

    /// An adjacent consumer pulls the finished box out (only between animations).
    pub fn take_held_box(&mut self) -> Option<ItemStack> {
        if self.animation_ticks != 0 {
            return None;
        }
        self.held_box.take()
    }

    /// `unwrapBox`: false while animating or when the target refuses the contents.
    pub fn unwrap_box(&mut self, target: &mut dyn ItemHandler, boxed: &ItemStack, simulate: bool) -> bool {
        if self.animation_ticks > 0 {
            return false;
        }
        let items = contents_stacks(boxed);
        if items.is_empty() {
            return true;
        }
        if !unpack_default(target, &items, simulate) {
            return false;
        }
        if !simulate {
            self.previously_unwrapped = Some(boxed.clone());
            self.animation_inward = true;
            self.animation_ticks = CYCLE;
        }
        true
    }

    /// Redstone/button activation (`activate`). `sign_sides` are the six
    /// neighbouring sign texts. Returns the send report, if any.
    pub fn activate(
        &mut self,
        target: Option<&mut dyn ItemHandler>,
        sign_sides: &[String],
        rng: &mut dyn Random,
    ) -> Option<SendReport> {
        self.redstone_powered = true;
        if self.linked {
            return None;
        }
        self.update_sign_address(sign_sides);
        let report = self.attempt_to_send(target, None, rng);
        if self.button_cooldown <= 0 {
            self.button_cooldown = BUTTON_COOLDOWN;
        }
        report
    }

    /// `lazyTick`: re-reads the powered state and re-sends while powered.
    pub fn lazy_tick(
        &mut self,
        powered_now: bool,
        target: Option<&mut dyn ItemHandler>,
        sign_sides: &[String],
        rng: &mut dyn Random,
    ) -> Option<SendReport> {
        if !self.redstone_powered {
            return None;
        }
        self.redstone_powered = powered_now;
        if !self.redstone_mode_active() {
            return None;
        }
        self.update_sign_address(sign_sides);
        self.attempt_to_send(target, None, rng)
    }

    /// `attemptToSend`. `queued == None` is a manual (redstone/sign) send;
    /// otherwise it serves the first request of the queue.
    pub fn attempt_to_send(
        &mut self,
        target: Option<&mut dyn ItemHandler>,
        mut queued: Option<&mut Vec<PackagingRequest>>,
        _rng: &mut dyn Random,
    ) -> Option<SendReport> {
        if queued.is_none() && (self.held_box.is_some() || self.animation_ticks > 0 || self.button_cooldown > 0) {
            return None;
        }
        let target = target?;
        let mut extracted = package::SimpleInventory::new(SLOTS);
        let mut reused: Option<ItemStack> = None;
        let mut final_package_at_link = false;
        // index 0 of the queue is always the active request
        let mut used_request: Option<PackagingRequest> = None;

        'outer: for _ in 0..SLOTS {
            let active = queued.as_ref().and_then(|q| q.first().cloned());
            if queued.is_some() && active.is_none() {
                break;
            }
            // each pass takes from the first eligible slot; no eligible slot ends packing
            for slot in 0..target.slot_count() {
                let want = active.as_ref().map_or(EXTRACT_CAP, |r| EXTRACT_CAP.min(r.count));
                let Some(candidate) = target.extract(slot, want, true) else { continue };
                if let Some(r) = &active {
                    if !candidate.same_item(&r.item) {
                        continue;
                    }
                }
                let bulky = candidate.bulky;
                let has_items = extracted.slots.iter().flatten().any(|s| !s.is_empty());
                if has_items && bulky {
                    continue;
                }
                let leftover = insert_stacked(&mut extracted, candidate.clone(), true)
                    .map_or(0, |r| r.count);
                let transferred = candidate.count - leftover;
                if transferred == 0 {
                    continue;
                }
                let taken = target.extract(slot, transferred, false)?;
                insert_stacked(&mut extracted, taken.copy_with_count(transferred), false);
                if is_package(&taken) {
                    reused = Some(taken.copy_with_count(1));
                }
                if let Some(q) = queued.as_deref_mut() {
                    q[0].subtract(transferred);
                    used_request = Some(q[0].clone());
                    if q[0].is_empty() {
                        final_package_at_link = true;
                        let done = q.remove(0);
                        let continues = q.first().is_some_and(|n| {
                            n.address == done.address && n.order_id == done.order_id
                        });
                        if continues && !bulky {
                            q[0].package_counter = done.package_counter;
                            final_package_at_link = false;
                            continue 'outer;
                        }
                        // the box leaves; a continuing request would start a new box
                        break 'outer;
                    }
                }
                if bulky {
                    break 'outer;
                }
                continue 'outer;
            }
            break;
        }

        let packed: Vec<ItemStack> = extracted.slots.iter().flatten().filter(|s| !s.is_empty()).cloned().collect();
        if packed.is_empty() && reused.is_none() {
            if let Some(q) = queued.as_deref_mut() {
                if !q.is_empty() {
                    q.remove(0);
                }
            }
            return None;
        }
        let mut boxed = match &reused {
            Some(p) => p.clone(),
            None => package::containing(&self.style, &packed).0,
        };
        clear_address(&mut boxed);
        if let Some(req) = &used_request {
            add_address(&mut boxed, &req.address);
            // Java sets the order from the request that ended the box
            let mut counter = req.package_counter;
            // the counter lives in the request; advance the stored copy too
            if let Some(q) = queued.as_deref_mut() {
                if let Some(first) = q.first_mut() {
                    if first.order_id == req.order_id && first.link_index == req.link_index {
                        first.package_counter = counter + 1;
                    }
                }
            }
            if counter < 0 {
                counter = 0;
            }
            set_order(
                &mut boxed,
                req.order_id,
                req.link_index,
                req.final_link.get(),
                counter,
                final_package_at_link,
                req.context.clone(),
            );
        } else if !self.sign_address.trim().is_empty() {
            add_address(&mut boxed, &self.sign_address.clone());
        }
        let extracted_items = if reused.is_some() { Vec::new() } else { packed };
        let queued_flag = self.held_box.is_some() || self.animation_ticks > 0;
        if queued_flag {
            self.queued_exiting.push((boxed.clone(), 1));
        } else {
            self.held_box = Some(boxed.clone());
            self.animation_inward = false;
            self.animation_ticks = CYCLE;
        }
        Some(SendReport { package: boxed, extracted: extracted_items, queued: queued_flag })
    }

    /// `performPackageRequests` for one packager: up to 100 send passes.
    pub fn perform_requests(
        &mut self,
        target: &mut dyn ItemHandler,
        requests: Vec<PackagingRequest>,
        rng: &mut dyn Random,
    ) -> Vec<SendReport> {
        let mut queue = requests;
        let mut out = Vec::new();
        for _ in 0..MAX_SEND_PASSES {
            if queue.is_empty() {
                break;
            }
            if let Some(r) = self.attempt_to_send(Some(target), Some(&mut queue), rng) {
                out.push(r);
            }
        }
        out
    }
}

// ---------------------------------------------------------------------
// Repackager

/// `PackageRepackageHelper`.
#[derive(Clone, Debug, Default)]
pub struct RepackageHelper {
    /// Boxes per order id, in arrival order (kept sorted by first arrival).
    pub collected: Vec<(i32, Vec<ItemStack>)>,
}

impl RepackageHelper {
    pub fn clear(&mut self) {
        self.collected.clear();
    }

    pub fn is_fragmented(&self, stack: &ItemStack) -> bool {
        package::has_order_data(stack)
    }

    /// Returns the order id once every fragment of the order is present, else -1.
    pub fn add_package_fragment(&mut self, stack: &ItemStack) -> i32 {
        let id = get_order_id(stack);
        if id == -1 {
            return -1;
        }
        match self.collected.iter_mut().find(|(i, _)| *i == id) {
            Some((_, v)) => v.push(stack.clone()),
            None => self.collected.push((id, vec![stack.clone()])),
        }
        if self.is_order_complete(id) { id } else { -1 }
    }

    fn is_order_complete(&self, order_id: i32) -> bool {
        let Some((_, boxes)) = self.collected.iter().find(|(i, _)| *i == order_id) else { return false };
        let mut final_link_reached = false;
        for link in 0..1000 {
            if final_link_reached {
                return true;
            }
            for fragment in 0..1000 {
                let Some(b) = boxes
                    .iter()
                    .find(|b| package::get_link_index(b) == link && package::get_index(b) == fragment)
                else {
                    return false;
                };
                final_link_reached = package::is_final_link(b);
                if package::is_final(b) {
                    break;
                }
            }
        }
        true
    }

    /// `repack(orderId, r)`. Returns boxes with counts (recipe boxes can have more than one).
    pub fn repack(&self, order_id: i32, rng: &mut dyn Random, style: &str) -> Vec<BigStack> {
        let Some((_, boxes)) = self.collected.iter().find(|(i, _)| *i == order_id) else { return Vec::new() };
        let mut address = String::new();
        let mut order_context: Option<OrderWithCrafts> = None;
        let mut summary = InventorySummary::default();
        for b in boxes {
            address = get_address(b);
            if let Some(Some(ctx)) = package::has_order_data(b).then(|| package::get_order_context(b)) {
                if !ctx.is_empty() {
                    order_context = Some(ctx);
                }
            }
            for s in contents_stacks(b) {
                summary.add(&s, s.count);
            }
        }
        let mut exporting: Vec<BigStack> = Vec::new();
        let mut ordered_stacks: Vec<BigStack> = Vec::new();
        if let Some(ctx) = &order_context {
            exporting = repack_based_on_recipes(&mut summary, ctx, &address, rng, style);
            if exporting.is_empty() {
                ordered_stacks = ctx.stacks().to_vec();
            }
        }

        let mut entries: Vec<BigStack> = summary.entries().to_vec();
        let mut outputs: Vec<ItemStack> = Vec::new();
        loop {
            entries.retain(|e| e.count > 0);
            if entries.is_empty() {
                break;
            }
            let target = if ordered_stacks.is_empty() { None } else { Some(ordered_stacks.remove(0)) };
            let mut remaining = target.as_ref().map(|t| t.count);
            for e in entries.iter_mut() {
                if let Some(t) = &target {
                    if !e.stack.same_item(&t.stack) {
                        continue;
                    }
                }
                while e.count > 0 {
                    let mut chunk = e.stack.max_stack.max(1).min(e.count);
                    if let Some(r) = remaining {
                        if r == 0 {
                            break;
                        }
                        chunk = chunk.min(r);
                        remaining = Some(r - chunk);
                    }
                    e.count -= chunk;
                    outputs.push(e.stack.copy_with_count(chunk));
                }
                if remaining == Some(0) {
                    break;
                }
            }
            if target.is_none() {
                break;
            }
        }

        let mut slots: Vec<Option<ItemStack>> = vec![None; SLOTS];
        let mut filled = 0;
        let mut greedy: Vec<ItemStack> = Vec::new();
        for o in outputs {
            slots[filled] = Some(o);
            filled += 1;
            if filled == SLOTS {
                greedy.push(package::package_from_slots(style, std::mem::replace(&mut slots, vec![None; SLOTS])));
                filled = 0;
            }
        }
        if filled > 0 {
            greedy.push(package::package_from_slots(style, slots));
        }
        exporting.extend(greedy.into_iter().map(|p| BigStack::new(p, 1)));

        let last = exporting.len().saturating_sub(1);
        for (i, b) in exporting.iter_mut().enumerate() {
            add_address(&mut b.stack, &address);
            if get_order_id(&b.stack) == -1 {
                let ctx = if i == last { order_context.clone() } else { None };
                set_order(&mut b.stack, order_id, 0, true, 0, true, ctx);
            }
        }
        exporting
    }
}

/// `repackBasedOnRecipes`. Ingredients deducted before a failing ingredient
/// are not restored, exactly like the Java.
fn repack_based_on_recipes(
    summary: &mut InventorySummary,
    order: &OrderWithCrafts,
    _address: &str,
    rng: &mut dyn Random,
    style: &str,
) -> Vec<BigStack> {
    let mut out = Vec::new();
    for craft in &order.ordered_crafts {
        let mut packages = 0;
        'crafts: for _ in 0..craft.count {
            for ing in &craft.pattern.stacks {
                if ing.stack.is_empty() {
                    continue;
                }
                if summary.count_of(&ing.stack) == 0 {
                    break 'crafts;
                }
                summary.remove_one(&ing.stack);
            }
            packages += 1;
        }
        let mut slots: Vec<Option<ItemStack>> = vec![None; SLOTS];
        for (i, ing) in craft.pattern.stacks.iter().take(SLOTS).enumerate() {
            slots[i] = Some(ing.stack.copy_with_count(1));
        }
        let mut boxed = package::package_from_slots(style, slots);
        let pattern: Vec<BigStack> = craft.pattern.stacks.clone();
        set_order(&mut boxed, rng.next_int(), 0, true, 0, true, Some(OrderWithCrafts::single_recipe(pattern)));
        out.push(BigStack::new(boxed, packages));
    }
    out
}

/// Repackager block entity (extends the packager).
#[derive(Clone, Debug, Default)]
pub struct RepackagerState {
    pub base: PackagerState,
    pub helper: RepackageHelper,
    pub completed_order_id: i32,
}

impl RepackagerState {
    pub fn new() -> Self {
        Self { base: PackagerState::default(), helper: RepackageHelper::default(), completed_order_id: -1 }
    }

    /// `unwrapBox`: validates and starts the animation only; the target must
    /// take the whole box in one slot (or be bottomless).
    pub fn unwrap_box(&mut self, target: &mut dyn ItemHandler, boxed: &ItemStack, simulate: bool) -> bool {
        if self.base.animation_ticks > 0 {
            return false;
        }
        if !target.is_bottomless() {
            let fits = (0..target.slot_count()).any(|s| target.insert(s, boxed.clone(), true).is_none());
            if !fits {
                return false;
            }
        }
        if simulate {
            return true;
        }
        self.base.previously_unwrapped = Some(boxed.clone());
        self.base.animation_inward = true;
        self.base.animation_ticks = CYCLE;
        true
    }

    /// `attemptToSend` override: scan the target for whole packages or
    /// complete fragmented orders. Returns the packages queued/held.
    pub fn attempt_to_send(&mut self, target: &mut dyn ItemHandler, rng: &mut dyn Random) -> Vec<ItemStack> {
        let b = &self.base;
        if b.held_box.is_some() || b.animation_ticks > 0 || b.button_cooldown > 0 || !b.queued_exiting.is_empty() {
            return Vec::new();
        }
        let sent = self.attempt_to_repackage(target, rng);
        if self.base.held_box.is_some() && !self.base.sign_address.trim().is_empty() {
            let addr = self.base.sign_address.clone();
            if let Some(h) = self.base.held_box.as_mut() {
                add_address(h, &addr);
            }
        }
        sent
    }

    fn attempt_to_repackage(&mut self, target: &mut dyn ItemHandler, rng: &mut dyn Random) -> Vec<ItemStack> {
        self.helper.clear();
        self.completed_order_id = -1;
        for slot in 0..target.slot_count() {
            let Some(found) = target.extract(slot, 1, true) else { continue };
            if !is_package(&found) {
                continue;
            }
            if !self.helper.is_fragmented(&found) {
                let taken = target.extract(slot, 1, false).unwrap_or(found);
                self.base.held_box = Some(taken.clone());
                self.base.animation_inward = false;
                self.base.animation_ticks = CYCLE;
                return vec![taken];
            }
            let id = self.helper.add_package_fragment(&found);
            if id != -1 {
                self.completed_order_id = id;
                break;
            }
        }
        if self.completed_order_id == -1 {
            return Vec::new();
        }
        let id = self.completed_order_id;
        let boxes = self.helper.repack(id, rng, &self.base.style);
        for slot in 0..target.slot_count() {
            let matches = target.stack_in_slot(slot).is_some_and(|s| is_package(s) && get_order_id(s) == id);
            if matches {
                target.extract(slot, 1, false);
            }
        }
        let mut sent = Vec::new();
        for b in boxes {
            sent.push(b.stack.copy_with_count(1));
            self.base.queued_exiting.push((b.stack, b.count));
        }
        sent
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::{SimpleInventory, XorShift};

    fn it(id: &str, n: u32) -> ItemStack {
        ItemStack::new(id, n)
    }

    fn rng() -> XorShift {
        XorShift(7)
    }

    fn req(item: &ItemStack, n: u32, order: i32, link: i32, fin: &Rc<Cell<bool>>) -> PackagingRequest {
        PackagingRequest::create(item.clone(), n, "dest", link, fin.clone(), 0, order, None)
    }

    #[test]
    fn manual_send_packs_nine_slots_with_sign_address() {
        let mut inv = SimpleInventory::new(12);
        for i in 0..12 {
            inv.slots[i] = Some(it(&format!("m:i{i}"), 64));
        }
        let mut p = PackagerState::default();
        p.sign_address = "shop".into();
        let r = p.attempt_to_send(Some(&mut inv), None, &mut rng()).unwrap();
        assert_eq!(contents_stacks(&r.package).len(), 9);
        assert_eq!(get_address(&r.package), "shop");
        assert!(!r.queued);
        assert_eq!((p.animation_ticks, p.animation_inward), (CYCLE, false));
        assert!(p.held_box.is_some());
        assert_eq!(r.extracted.len(), 9);
        assert_eq!(inv.slots.iter().flatten().count(), 3);
        // gate: busy now
        assert!(p.attempt_to_send(Some(&mut inv), None, &mut rng()).is_none());
    }

    #[test]
    fn empty_inventory_sends_nothing() {
        let mut inv = SimpleInventory::new(3);
        let mut p = PackagerState::default();
        assert!(p.attempt_to_send(Some(&mut inv), None, &mut rng()).is_none());
        assert!(p.held_box.is_none());
    }

    #[test]
    fn bulky_item_travels_alone_and_not_mixed() {
        let mut inv = SimpleInventory::new(3);
        inv.slots[0] = Some(it("m:shulker", 1).with_max(1).bulky());
        inv.slots[1] = Some(it("m:iron", 10));
        let mut p = PackagerState::default();
        let r = p.attempt_to_send(Some(&mut inv), None, &mut rng()).unwrap();
        let c = contents_stacks(&r.package);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].key.id, "m:shulker");
        // next box takes the iron; a bulky item after an ordinary one is skipped
        let mut inv2 = SimpleInventory::new(3);
        inv2.slots[0] = Some(it("m:iron", 10));
        inv2.slots[1] = Some(it("m:shulker", 1).with_max(1).bulky());
        let mut p2 = PackagerState::default();
        let r2 = p2.attempt_to_send(Some(&mut inv2), None, &mut rng()).unwrap();
        assert_eq!(contents_stacks(&r2.package).len(), 1);
        assert!(inv2.slots[1].is_some());
    }

    #[test]
    fn existing_package_is_reused_without_extraction_report() {
        let (inner, _) = package::containing(DEFAULT_STYLE, &[it("m:iron", 3)]);
        let mut inv = SimpleInventory::new(2).with(0, inner.clone());
        let mut p = PackagerState::default();
        p.sign_address = "x".into();
        let r = p.attempt_to_send(Some(&mut inv), None, &mut rng()).unwrap();
        assert!(r.extracted.is_empty());
        assert_eq!(contents_stacks(&r.package), contents_stacks(&inner));
        assert_eq!(get_address(&r.package), "x");
        assert!(inv.is_empty());
    }

    #[test]
    fn request_fragments_orders_and_final_flags() {
        // 150 iron requested -> three requests passes of 64, 64, 22 into one package each?
        let iron = it("m:iron", 1);
        let mut inv = SimpleInventory::new(4);
        inv.slots[0] = Some(it("m:iron", 64));
        inv.slots[1] = Some(it("m:iron", 64));
        inv.slots[2] = Some(it("m:iron", 64));
        let fin = Rc::new(Cell::new(true));
        let mut p = PackagerState::default();
        let reports = p.perform_requests(&mut inv, vec![req(&iron, 150, 99, 0, &fin)], &mut rng());
        // all 150 fit in one nine-slot package (3 slots)
        assert_eq!(reports.len(), 1);
        let pk = &reports[0].package;
        assert_eq!(contents_stacks(pk).iter().map(|s| s.count).sum::<u32>(), 150);
        assert_eq!(get_order_id(pk), 99);
        assert_eq!(package::get_link_index(pk), 0);
        assert_eq!(package::get_index(pk), 0);
        assert!(package::is_final(pk) && package::is_final_link(pk));
        assert_eq!(get_address(pk), "dest");
        assert_eq!(inv.count_of(&iron), 42);
    }

    #[test]
    fn request_larger_than_a_package_splits_with_counter() {
        // 600 iron: nine slots * 64 = 576 per package
        let iron = it("m:iron", 1);
        let mut inv = SimpleInventory::new(10);
        for i in 0..10 {
            inv.slots[i] = Some(it("m:iron", 64));
        }
        let fin = Rc::new(Cell::new(true));
        let mut p = PackagerState::default();
        let reports = p.perform_requests(&mut inv, vec![req(&iron, 600, 5, 2, &fin)], &mut rng());
        assert_eq!(reports.len(), 2);
        let (a, b) = (&reports[0].package, &reports[1].package);
        assert_eq!(contents_stacks(a).iter().map(|s| s.count).sum::<u32>(), 576);
        assert_eq!(contents_stacks(b).iter().map(|s| s.count).sum::<u32>(), 24);
        assert_eq!((package::get_index(a), package::get_index(b)), (0, 1));
        assert!(!package::is_final(a) && package::is_final(b));
        assert_eq!(package::get_link_index(a), 2);
        // first box held, second queued
        assert!(!reports[0].queued && reports[1].queued);
        assert_eq!(p.queued_exiting.len(), 1);
    }

    #[test]
    fn next_request_same_order_continues_in_same_box() {
        let iron = it("m:iron", 1);
        let gold = it("m:gold", 1);
        let mut inv = SimpleInventory::new(4)
            .with(0, it("m:iron", 10))
            .with(1, it("m:gold", 5));
        let fin = Rc::new(Cell::new(true));
        let q = vec![req(&iron, 10, 1, 0, &fin), req(&gold, 5, 1, 0, &fin)];
        let mut p = PackagerState::default();
        let reports = p.perform_requests(&mut inv, q, &mut rng());
        assert_eq!(reports.len(), 1);
        assert_eq!(contents_stacks(&reports[0].package).len(), 2);
        assert!(package::is_final(&reports[0].package));
    }

    #[test]
    fn different_order_does_not_merge() {
        let iron = it("m:iron", 1);
        let gold = it("m:gold", 1);
        let mut inv = SimpleInventory::new(4)
            .with(0, it("m:iron", 10))
            .with(1, it("m:gold", 5));
        let fin = Rc::new(Cell::new(true));
        let q = vec![req(&iron, 10, 1, 0, &fin), req(&gold, 5, 2, 0, &fin)];
        let mut p = PackagerState::default();
        let reports = p.perform_requests(&mut inv, q, &mut rng());
        assert_eq!(reports.len(), 2);
        assert_eq!(get_order_id(&reports[0].package), 1);
        assert_eq!(get_order_id(&reports[1].package), 2);
    }

    #[test]
    fn request_without_matching_stock_is_dropped() {
        let iron = it("m:iron", 1);
        let mut inv = SimpleInventory::new(2).with(0, it("m:gold", 4));
        let fin = Rc::new(Cell::new(true));
        let mut p = PackagerState::default();
        let r = p.perform_requests(&mut inv, vec![req(&iron, 3, 1, 0, &fin)], &mut rng());
        assert!(r.is_empty());
    }

    #[test]
    fn tick_runs_queue_and_animation() {
        let mut p = PackagerState::default();
        let a = it("m:a", 1);
        p.queued_exiting.push((a.clone(), 2));
        let r = p.tick();
        assert!(r.started_next);
        assert_eq!(p.queued_exiting[0].1, 1);
        assert!(p.held_box.is_some());
        for _ in 0..(CYCLE - 1) {
            assert!(!p.tick().animation_finished);
        }
        assert!(p.tick().animation_finished);
        assert!(p.take_held_box().is_some());
        let r = p.tick();
        assert!(r.started_next);
        assert!(p.queued_exiting.is_empty());
    }

    #[test]
    fn unwrap_box_all_or_nothing() {
        let (b, _) = package::containing(DEFAULT_STYLE, &[it("m:iron", 64), it("m:gold", 64)]);
        let mut tiny = SimpleInventory::new(1);
        let mut p = PackagerState::default();
        assert!(!p.unwrap_box(&mut tiny, &b, false));
        assert!(tiny.is_empty() && p.animation_ticks == 0);
        let mut big = SimpleInventory::new(2);
        assert!(p.unwrap_box(&mut big, &b, true));
        assert!(big.is_empty());
        assert!(p.unwrap_box(&mut big, &b, false));
        assert_eq!(big.slots.iter().flatten().count(), 2);
        assert!(p.animation_inward && p.animation_ticks == CYCLE);
        assert!(!p.unwrap_box(&mut big, &b, false), "busy");
        // an empty package is accepted without animating
        let (empty, _) = package::containing(DEFAULT_STYLE, &[]);
        let mut p2 = PackagerState::default();
        assert!(p2.unwrap_box(&mut big, &empty, false));
        assert_eq!(p2.animation_ticks, 0);
    }

    #[test]
    fn busy_thresholds() {
        let mut p = PackagerState::default();
        for _ in 0..10 {
            p.queued_exiting.push((it("m:a", 1), 1));
        }
        assert!(p.is_too_busy_for(RequestType::Restock));
        assert!(!p.is_too_busy_for(RequestType::Redstone));
        for _ in 0..10 {
            p.queued_exiting.push((it("m:a", 1), 1));
        }
        assert!(p.is_too_busy_for(RequestType::Redstone));
        assert!(!p.is_too_busy_for(RequestType::Player));
    }

    #[test]
    fn activation_cooldown_and_linked_mode() {
        let mut inv = SimpleInventory::new(2).with(0, it("m:a", 3)).with(1, it("m:b", 3));
        let mut p = PackagerState::default();
        p.linked = true;
        assert!(p.activate(Some(&mut inv), &[], &mut rng()).is_none());
        assert!(p.redstone_powered && p.button_cooldown == 0);
        p.linked = false;
        let sides = vec![String::new(), "north".into(), String::new(), "south".into()];
        assert!(p.activate(Some(&mut inv), &sides, &mut rng()).is_some());
        assert_eq!(p.sign_address, "south", "last non-blank side wins");
        assert_eq!(p.button_cooldown, BUTTON_COOLDOWN);
    }

    #[test]
    fn sign_text_joins_lines() {
        assert_eq!(sign_text(&["a ", "", " b"], &["z"]), "a b");
        assert_eq!(sign_text(&["", " "], &["z", "y"]), "z y");
        assert_eq!(sign_text(&[], &[]), "");
    }

    #[test]
    fn lazy_tick_resends_while_powered() {
        let mut inv = SimpleInventory::new(2).with(0, it("m:a", 3));
        let mut p = PackagerState::default();
        assert!(p.lazy_tick(true, Some(&mut inv), &[], &mut rng()).is_none(), "not powered yet");
        p.redstone_powered = true;
        assert!(p.lazy_tick(true, Some(&mut inv), &[], &mut rng()).is_some());
        assert!(p.redstone_powered);
        p.held_box = None;
        p.animation_ticks = 0;
        assert!(p.lazy_tick(false, Some(&mut inv), &[], &mut rng()).is_none());
        assert!(!p.redstone_powered);
    }

    fn fragment(order: i32, link: i32, idx: i32, fin_link: bool, fin: bool, items: &[ItemStack]) -> ItemStack {
        let (mut b, _) = package::containing(DEFAULT_STYLE, items);
        set_order(&mut b, order, link, fin_link, idx, fin, None);
        add_address(&mut b, "dest");
        b
    }

    #[test]
    fn repackager_waits_for_all_fragments_then_merges() {
        let mut h = RepackageHelper::default();
        assert_eq!(h.add_package_fragment(&fragment(4, 0, 0, false, true, &[it("m:a", 2)])), -1);
        assert_eq!(h.add_package_fragment(&fragment(4, 1, 0, true, false, &[it("m:b", 3)])), -1);
        assert_eq!(h.add_package_fragment(&fragment(4, 1, 1, true, true, &[it("m:b", 4)])), 4);
        let out = h.repack(4, &mut rng(), DEFAULT_STYLE);
        assert_eq!(out.len(), 1);
        let c = contents_stacks(&out[0].stack);
        assert_eq!(c.iter().map(|s| (s.key.id.clone(), s.count)).collect::<Vec<_>>(), vec![("m:a".into(), 2), ("m:b".into(), 7)]);
        assert_eq!(get_address(&out[0].stack), "dest");
        assert_eq!(get_order_id(&out[0].stack), 4);
        assert!(package::is_final(&out[0].stack) && package::is_final_link(&out[0].stack));
    }

    #[test]
    fn repack_follows_ordered_stacks_and_chunks() {
        let ctx = OrderWithCrafts::simple(vec![BigStack::new(it("m:b", 1), 70), BigStack::new(it("m:a", 1), 2)]);
        let (mut f, _) = package::containing(DEFAULT_STYLE, &[it("m:a", 2), it("m:b", 64), it("m:b", 6)]);
        set_order(&mut f, 8, 0, true, 0, true, Some(ctx));
        add_address(&mut f, "x");
        let mut h = RepackageHelper::default();
        assert_eq!(h.add_package_fragment(&f), 8);
        let out = h.repack(8, &mut rng(), DEFAULT_STYLE);
        let c = contents_stacks(&out[0].stack);
        assert_eq!(c.iter().map(|s| (s.key.id.as_str(), s.count)).collect::<Vec<_>>(), vec![("m:b", 64), ("m:b", 6), ("m:a", 2)]);
        // the context rides on the last (here only) package
        assert!(package::get_order_context(&out[0].stack).is_some());
    }

    #[test]
    fn repack_makes_recipe_packages() {
        let pattern = vec![BigStack::new(it("m:a", 1), 1), BigStack::new(it("m:b", 1), 1)];
        let ctx = OrderWithCrafts { ordered_stacks: package::PackageOrder::new(vec![BigStack::new(it("m:a", 1), 3), BigStack::new(it("m:b", 1), 3)]), ordered_crafts: vec![crate::package::CraftingEntry { pattern: package::PackageOrder::new(pattern), count: 3 }] };
        let (mut f, _) = package::containing(DEFAULT_STYLE, &[it("m:a", 2), it("m:b", 5), it("m:z", 1)]);
        set_order(&mut f, 3, 0, true, 0, true, Some(ctx));
        let mut h = RepackageHelper::default();
        assert_eq!(h.add_package_fragment(&f), 3);
        let out = h.repack(3, &mut rng(), DEFAULT_STYLE);
        // 2 crafts possible (limited by m:a); then leftovers form a normal package
        assert_eq!(out[0].count, 2);
        assert_eq!(contents_stacks(&out[0].stack).len(), 2);
        assert!(package::get_order_context(&out[0].stack).unwrap().ordered_crafts.len() == 1);
        let rest = contents_stacks(&out[1].stack);
        assert_eq!(rest.iter().map(|s| (s.key.id.as_str(), s.count)).collect::<Vec<_>>(), vec![("m:b", 3), ("m:z", 1)]);
        assert_eq!(get_address(&out[0].stack), "");
    }

    #[test]
    fn repack_failed_craft_keeps_earlier_deductions() {
        // m:a present, m:b absent: the first ingredient is consumed and not restored
        let pattern = vec![BigStack::new(it("m:a", 1), 1), BigStack::new(it("m:b", 1), 1)];
        let ctx = OrderWithCrafts { ordered_stacks: package::PackageOrder::new(vec![BigStack::new(it("m:a", 1), 1), BigStack::new(it("m:b", 1), 1)]), ordered_crafts: vec![crate::package::CraftingEntry { pattern: package::PackageOrder::new(pattern), count: 1 }] };
        let (mut f, _) = package::containing(DEFAULT_STYLE, &[it("m:a", 2)]);
        set_order(&mut f, 3, 0, true, 0, true, Some(ctx));
        let mut h = RepackageHelper::default();
        h.add_package_fragment(&f);
        let out = h.repack(3, &mut rng(), DEFAULT_STYLE);
        assert_eq!(out[0].count, 0);
        assert_eq!(contents_stacks(&out[1].stack)[0].count, 1);
    }

    #[test]
    fn repackager_sends_plain_packages_and_completed_orders() {
        let mut inv = SimpleInventory::new(5);
        let plain = package::containing(DEFAULT_STYLE, &[it("m:p", 1)]).0;
        inv.slots[0] = Some(fragment(1, 0, 0, true, false, &[it("m:a", 1)]));
        inv.slots[1] = Some(plain.clone());
        let mut r = RepackagerState::new();
        let sent = r.attempt_to_send(&mut inv, &mut rng());
        assert_eq!(sent.len(), 1);
        assert_eq!(r.base.held_box.as_ref().unwrap(), &plain);
        assert!(inv.slots[0].is_some(), "incomplete order left alone");
        // complete the order
        let mut inv = SimpleInventory::new(5);
        inv.slots[0] = Some(fragment(1, 0, 0, true, false, &[it("m:a", 1)]));
        inv.slots[1] = Some(fragment(1, 0, 1, true, true, &[it("m:a", 2)]));
        let mut r = RepackagerState::new();
        let sent = r.attempt_to_send(&mut inv, &mut rng());
        assert_eq!(sent.len(), 1);
        assert!(inv.is_empty());
        assert_eq!(r.base.queued_exiting.len(), 1);
        assert_eq!(contents_stacks(&r.base.queued_exiting[0].0)[0].count, 3);
        // busy while something queued
        assert!(r.attempt_to_send(&mut inv, &mut rng()).is_empty());
    }

    #[test]
    fn repackager_unwrap_needs_a_slot_for_whole_box() {
        let b = package::containing(DEFAULT_STYLE, &[it("m:a", 1)]).0;
        let mut full = SimpleInventory::new(1).with(0, it("m:z", 1));
        let mut r = RepackagerState::new();
        assert!(!r.unwrap_box(&mut full, &b, false));
        let mut free = SimpleInventory::new(2);
        assert!(r.unwrap_box(&mut free, &b, true));
        assert_eq!(r.base.animation_ticks, 0);
        assert!(r.unwrap_box(&mut free, &b, false));
        assert!(r.base.animation_inward);
        full.bottomless = true;
        let mut r2 = RepackagerState::new();
        assert!(r2.unwrap_box(&mut full, &b, false));
    }

    #[test]
    fn rendered_box_phases() {
        let mut p = PackagerState::default();
        p.held_box = Some(it("m:a", 1));
        p.animation_ticks = CYCLE;
        assert!(p.rendered_box().is_some());
        p.animation_ticks = CYCLE / 2 - 1;
        assert!(p.rendered_box().is_none());
        p.animation_inward = true;
        p.previously_unwrapped = Some(it("m:b", 1));
        p.animation_ticks = CYCLE - 1;
        assert!(p.rendered_box().is_some());
        p.animation_ticks = CYCLE / 2;
        assert!(p.rendered_box().is_none());
    }
}
