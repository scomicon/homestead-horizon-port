// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

//! Pure package data after `PackageItem`, `PackageOrderWithCrafts`,
//! `BigItemStack` and `PackageStyles` (Create 6.0.10), plus the small
//! inventory trait the logistics modules use instead of a game inventory.
//!
//! Items are plain data: identity is `id` plus an opaque `components` string
//! (the serialized data components), so two stacks are the same item exactly
//! when Java's `ItemStack.isSameItemSameComponents` would say so. A package
//! carries its contents, address and order data inline.

/// `PackageItem.SLOTS`: slots inside one package.
pub const SLOTS: usize = 9;
/// `BigItemStack.INF`.
pub const INF: u32 = 1_000_000_000;
/// Item handler slot limit used by default inventories.
pub const DEFAULT_SLOT_LIMIT: u32 = 64;

/// Identity of an item: registry id plus serialized components.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ItemKey {
    pub id: String,
    pub components: String,
}

impl ItemKey {
    pub fn new(id: &str) -> Self {
        Self { id: id.to_owned(), components: String::new() }
    }

    pub fn with_components(id: &str, components: &str) -> Self {
        Self { id: id.to_owned(), components: components.to_owned() }
    }
}

/// An item stack. `bulky` is `!Item.canFitInsideContainerItems()`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemStack {
    pub key: ItemKey,
    pub count: u32,
    pub max_stack: u32,
    pub bulky: bool,
    /// Present exactly when the stack is a package item.
    pub package: Option<Box<Package>>,
}

impl ItemStack {
    pub fn new(id: &str, count: u32) -> Self {
        Self { key: ItemKey::new(id), count, max_stack: 64, bulky: false, package: None }
    }

    pub fn with_max(mut self, max_stack: u32) -> Self {
        self.max_stack = max_stack;
        self
    }

    pub fn bulky(mut self) -> Self {
        self.bulky = true;
        self
    }

    pub fn with_components(mut self, components: &str) -> Self {
        self.key.components = components.to_owned();
        self
    }

    pub fn copy_with_count(&self, count: u32) -> Self {
        let mut copy = self.clone();
        copy.count = count;
        copy
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn is_package(&self) -> bool {
        self.package.is_some()
    }

    /// `ItemStack.isSameItemSameComponents` (package payload counts as a component).
    pub fn same_item(&self, other: &ItemStack) -> bool {
        self.key == other.key && self.package == other.package
    }
}

/// `BigItemStack`: a template stack with an unbounded count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BigStack {
    pub stack: ItemStack,
    pub count: u32,
}

impl BigStack {
    pub fn new(stack: ItemStack, count: u32) -> Self {
        Self { stack, count }
    }

    pub fn is_infinite(&self) -> bool {
        self.count >= INF
    }
}

/// `BigItemStack.comparator()`: larger counts first (stable).
pub fn sort_by_count_desc(stacks: &mut [BigStack]) {
    stacks.sort_by(|a, b| b.count.cmp(&a.count));
}

/// Record `PackageOrder`.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PackageOrder {
    pub stacks: Vec<BigStack>,
}

impl PackageOrder {
    pub fn new(stacks: Vec<BigStack>) -> Self {
        Self { stacks }
    }

    pub fn is_empty(&self) -> bool {
        self.stacks.is_empty()
    }
}

/// `PackageOrderWithCrafts.CraftingEntry`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CraftingEntry {
    pub pattern: PackageOrder,
    pub count: u32,
}

/// Record `PackageOrderWithCrafts`.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct OrderWithCrafts {
    pub ordered_stacks: PackageOrder,
    pub ordered_crafts: Vec<CraftingEntry>,
}

impl OrderWithCrafts {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn simple(stacks: Vec<BigStack>) -> Self {
        Self { ordered_stacks: PackageOrder::new(stacks), ordered_crafts: Vec::new() }
    }

    pub fn single_recipe(pattern: Vec<BigStack>) -> Self {
        Self {
            ordered_stacks: PackageOrder::default(),
            ordered_crafts: vec![CraftingEntry { pattern: PackageOrder::new(pattern), count: 1 }],
        }
    }

    /// `hasCraftingInformation`: exactly one ordered craft.
    pub fn has_crafting_information(context: Option<&OrderWithCrafts>) -> bool {
        context.is_some_and(|c| c.ordered_crafts.len() == 1)
    }

    /// `getCraftingInformation`; `None` where Java would throw.
    pub fn crafting_information(&self) -> Option<&[BigStack]> {
        self.ordered_crafts.first().map(|c| c.pattern.stacks.as_slice())
    }

    pub fn stacks(&self) -> &[BigStack] {
        &self.ordered_stacks.stacks
    }

    /// Ignores the crafts, like Java.
    pub fn is_empty(&self) -> bool {
        self.ordered_stacks.is_empty()
    }

    /// `orderedStacksMatchOrderedRecipes`.
    pub fn ordered_stacks_match_ordered_recipes(&self) -> bool {
        if self.ordered_crafts.is_empty() {
            return false;
        }
        let mut wanted: Vec<(ItemStack, u64)> = Vec::new();
        let mut crafted: Vec<(ItemStack, u64)> = Vec::new();
        for s in self.stacks() {
            add_counted(&mut wanted, &s.stack, s.count as u64);
        }
        for c in &self.ordered_crafts {
            for s in &c.pattern.stacks {
                add_counted(&mut crafted, &s.stack, s.count as u64 * c.count as u64);
            }
        }
        if wanted.len() != crafted.len() {
            return false;
        }
        wanted.iter().all(|(s, n)| {
            crafted.iter().find(|(o, _)| o.same_item(s)).is_some_and(|(_, m)| m == n)
        })
    }
}

fn add_counted(list: &mut Vec<(ItemStack, u64)>, stack: &ItemStack, count: u64) {
    if count == 0 {
        return;
    }
    match list.iter_mut().find(|(s, _)| s.same_item(stack)) {
        Some((_, n)) => *n += count,
        None => list.push((stack.clone(), count)),
    }
}

/// Record `PackageItem.PackageOrderData` (`PACKAGE_ORDER_DATA`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageOrderData {
    pub order_id: i32,
    pub link_index: i32,
    pub is_final_link: bool,
    pub fragment_index: i32,
    pub is_final: bool,
    pub order_context: Option<OrderWithCrafts>,
}

/// Static look of a package item (`PackageStyles.PackageStyle`): pixel size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackageStyle {
    pub name: &'static str,
    pub width: u32,
    pub height: u32,
    pub rare: bool,
}

/// The four standard cardboard styles. Pixel sizes follow the style names
/// (`<width>x<height>`); the rare creator-box styles are not listed (see
/// docs/logistics.md, gaps).
pub const STANDARD_STYLES: [PackageStyle; 4] = [
    PackageStyle { name: "cardboard_package_12x12", width: 12, height: 12, rare: false },
    PackageStyle { name: "cardboard_package_10x12", width: 10, height: 12, rare: false },
    PackageStyle { name: "cardboard_package_10x8", width: 10, height: 8, rare: false },
    PackageStyle { name: "cardboard_package_12x10", width: 12, height: 10, rare: false },
];

/// Package item state: `PACKAGE_CONTENTS`, `PACKAGE_ADDRESS`,
/// `PACKAGE_ORDER_DATA`, `PACKAGE_ORDER_CONTEXT`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Package {
    pub contents: Vec<Option<ItemStack>>,
    pub address: Option<String>,
    pub order_data: Option<PackageOrderData>,
    pub order_context: Option<OrderWithCrafts>,
}

impl Package {
    fn empty() -> Self {
        Self { contents: vec![None; SLOTS], address: None, order_data: None, order_context: None }
    }
}

/// `PackageItem.containing(List)`: stacked insertion into nine slots;
/// overflow is returned (Java drops it). `style_id` is the package item id.
pub fn containing(style_id: &str, stacks: &[ItemStack]) -> (ItemStack, Vec<ItemStack>) {
    let mut inv = SimpleInventory::new(SLOTS);
    let mut overflow = Vec::new();
    for s in stacks {
        if let Some(rest) = insert_stacked(&mut inv, s.clone(), false) {
            overflow.push(rest);
        }
    }
    (package_from_slots(style_id, inv.slots), overflow)
}

/// `PackageItem.containing(ItemStackHandler)`.
pub fn package_from_slots(style_id: &str, mut slots: Vec<Option<ItemStack>>) -> ItemStack {
    slots.resize(SLOTS, None);
    slots.truncate(SLOTS);
    let mut pkg = Package::empty();
    pkg.contents = slots;
    ItemStack {
        key: ItemKey::new(style_id),
        count: 1,
        max_stack: 1,
        bulky: true,
        package: Some(Box::new(pkg)),
    }
}

fn pkg(stack: &ItemStack) -> Option<&Package> {
    stack.package.as_deref()
}

fn pkg_mut(stack: &mut ItemStack) -> Option<&mut Package> {
    stack.package.as_deref_mut()
}

pub fn is_package(stack: &ItemStack) -> bool {
    stack.is_package()
}

/// `getContents`: the nine slots (empty vec for a non-package).
pub fn get_contents(stack: &ItemStack) -> Vec<Option<ItemStack>> {
    pkg(stack).map(|p| p.contents.clone()).unwrap_or_else(|| vec![None; SLOTS])
}

/// Non-empty contents in slot order.
pub fn contents_stacks(stack: &ItemStack) -> Vec<ItemStack> {
    get_contents(stack).into_iter().flatten().filter(|s| !s.is_empty()).collect()
}

pub fn add_address(stack: &mut ItemStack, address: &str) {
    if let Some(p) = pkg_mut(stack) {
        p.address = Some(address.to_owned());
    }
}

pub fn clear_address(stack: &mut ItemStack) {
    if let Some(p) = pkg_mut(stack) {
        p.address = None;
    }
}

/// `getAddress`: `""` when unset.
pub fn get_address(stack: &ItemStack) -> String {
    pkg(stack).and_then(|p| p.address.clone()).unwrap_or_default()
}

pub fn match_address_stack(stack: &ItemStack, address: &str) -> bool {
    match_address(&get_address(stack), address)
}

/// `PackageItem.matchAddress(boxAddress, address)`.
pub fn match_address(box_address: &str, address: &str) -> bool {
    if address.trim().is_empty() {
        return box_address.trim().is_empty();
    }
    if box_address == "*" || address == "*" {
        return true;
    }
    if box_address == address {
        return true;
    }
    glob_matches(box_address, address) || glob_matches(address, box_address)
}

/// `Glob.toRegexPattern` matching: `*` (not across `/`), `**`, `?`,
/// `[set]` / `[!set]`, `{a,b}` and `\` escapes; the whole text must match.
pub fn glob_matches(pattern: &str, text: &str) -> bool {
    let t: Vec<char> = text.chars().collect();
    expand_braces(&pattern.chars().collect::<Vec<_>>()).iter().any(|p| glob_core(p, &t))
}

fn expand_braces(p: &[char]) -> Vec<Vec<char>> {
    let mut i = 0;
    while i < p.len() {
        match p[i] {
            '\\' => i += 2,
            '[' => {
                // skip a class so braces inside stay literal
                let mut j = i + 1;
                while j < p.len() && p[j] != ']' {
                    j += 1;
                }
                i = j + 1;
            }
            '{' => {
                let mut depth = 0;
                let mut parts: Vec<Vec<char>> = vec![Vec::new()];
                let mut j = i + 1;
                while j < p.len() {
                    match p[j] {
                        '{' => {
                            depth += 1;
                            parts.last_mut().unwrap().push('{');
                        }
                        '}' if depth == 0 => break,
                        '}' => {
                            depth -= 1;
                            parts.last_mut().unwrap().push('}');
                        }
                        ',' if depth == 0 => parts.push(Vec::new()),
                        c => parts.last_mut().unwrap().push(c),
                    }
                    j += 1;
                }
                if j >= p.len() {
                    return vec![p.to_vec()]; // unmatched brace: leave literal
                }
                let mut out = Vec::new();
                for part in parts {
                    let mut candidate = p[..i].to_vec();
                    candidate.extend(part);
                    candidate.extend_from_slice(&p[j + 1..]);
                    out.extend(expand_braces(&candidate));
                }
                return out;
            }
            _ => i += 1,
        }
    }
    vec![p.to_vec()]
}

fn glob_core(p: &[char], t: &[char]) -> bool {
    let Some(&c) = p.first() else { return t.is_empty() };
    match c {
        '*' => {
            let cross = p.get(1) == Some(&'*');
            let rest = &p[if cross { 2 } else { 1 }..];
            for n in 0..=t.len() {
                if glob_core(rest, &t[n..]) {
                    return true;
                }
                if n < t.len() && !cross && t[n] == '/' {
                    return false;
                }
            }
            false
        }
        '?' => t.first().is_some_and(|&x| x != '/') && glob_core(&p[1..], &t[1..]),
        '\\' if p.len() > 1 => t.first() == Some(&p[1]) && glob_core(&p[2..], &t[1..]),
        '[' => {
            let Some(end) = p.iter().skip(2).position(|&x| x == ']').map(|e| e + 2) else {
                return t.first() == Some(&'[') && glob_core(&p[1..], &t[1..]);
            };
            let Some(&x) = t.first() else { return false };
            let (negate, set) = match p[1] {
                '!' | '^' => (true, &p[2..end]),
                _ => (false, &p[1..end]),
            };
            let mut hit = false;
            let mut k = 0;
            while k < set.len() {
                if k + 2 < set.len() && set[k + 1] == '-' {
                    hit |= set[k] <= x && x <= set[k + 2];
                    k += 3;
                } else {
                    hit |= set[k] == x;
                    k += 1;
                }
            }
            x != '/' && hit != negate && glob_core(&p[end + 1..], &t[1..])
        }
        _ => t.first() == Some(&c) && glob_core(&p[1..], &t[1..]),
    }
}

/// `PackageItem.setOrder`.
pub fn set_order(
    stack: &mut ItemStack,
    order_id: i32,
    link_index: i32,
    is_final_link: bool,
    fragment_index: i32,
    is_final: bool,
    order_context: Option<OrderWithCrafts>,
) {
    if let Some(p) = pkg_mut(stack) {
        p.order_data = Some(PackageOrderData {
            order_id,
            link_index,
            is_final_link,
            fragment_index,
            is_final,
            order_context,
        });
    }
}

fn order_data(stack: &ItemStack) -> Option<&PackageOrderData> {
    pkg(stack).and_then(|p| p.order_data.as_ref())
}

pub fn has_order_data(stack: &ItemStack) -> bool {
    order_data(stack).is_some()
}

/// `-1` when no order data.
pub fn get_order_id(stack: &ItemStack) -> i32 {
    order_data(stack).map_or(-1, |d| d.order_id)
}

/// Fragment index, `-1` when no order data.
pub fn get_index(stack: &ItemStack) -> i32 {
    order_data(stack).map_or(-1, |d| d.fragment_index)
}

pub fn is_final(stack: &ItemStack) -> bool {
    order_data(stack).is_some_and(|d| d.is_final)
}

pub fn get_link_index(stack: &ItemStack) -> i32 {
    order_data(stack).map_or(-1, |d| d.link_index)
}

pub fn is_final_link(stack: &ItemStack) -> bool {
    order_data(stack).is_some_and(|d| d.is_final_link)
}

/// `getOrderContext`: order data first, then the standalone component.
pub fn get_order_context(stack: &ItemStack) -> Option<OrderWithCrafts> {
    let p = pkg(stack)?;
    match &p.order_data {
        Some(d) => d.order_context.clone(),
        None => p.order_context.clone(),
    }
}

/// `addOrderContext`: sets only the standalone component.
pub fn add_order_context(stack: &mut ItemStack, context: OrderWithCrafts) {
    if let Some(p) = pkg_mut(stack) {
        p.order_context = Some(context);
    }
}

/// `PackageItem.getPackageVelocity`.
pub fn package_velocity(ticks: i32) -> f32 {
    let f = ticks as f32 / 20.0;
    ((f * f + f * 2.0) / 3.0).min(1.0)
}

// ---------------------------------------------------------------------
// Inventories

/// The small inventory interface (`IItemHandler`) the logistics logic uses.
pub trait ItemHandler {
    fn slot_count(&self) -> usize;
    fn stack_in_slot(&self, slot: usize) -> Option<&ItemStack>;
    /// Remove up to `amount` from a slot; returns what was (or would be) removed.
    fn extract(&mut self, slot: usize, amount: u32, simulate: bool) -> Option<ItemStack>;
    /// Insert into one slot; returns the remainder.
    fn insert(&mut self, slot: usize, stack: ItemStack, simulate: bool) -> Option<ItemStack>;
    fn slot_limit(&self, _slot: usize) -> u32 {
        DEFAULT_SLOT_LIMIT
    }
    /// Independent copy, used for all-or-nothing simulations.
    fn duplicate(&self) -> Box<dyn ItemHandler>;
    /// `BottomlessItemHandler` (creative crate) skips space checks.
    fn is_bottomless(&self) -> bool {
        false
    }
}

/// `ItemHandlerHelper.insertItemStacked`: matching stacks first, then empty slots.
pub fn insert_stacked(h: &mut dyn ItemHandler, stack: ItemStack, simulate: bool) -> Option<ItemStack> {
    if stack.is_empty() {
        return None;
    }
    let mut rest = Some(stack);
    for pass in 0..2 {
        for slot in 0..h.slot_count() {
            let Some(s) = rest.take() else { return None };
            let occupied = h.stack_in_slot(slot).is_some_and(|e| !e.is_empty());
            if (pass == 0) == occupied {
                rest = h.insert(slot, s, simulate);
            } else {
                rest = Some(s);
            }
            if rest.as_ref().is_none_or(|r| r.is_empty()) {
                return None;
            }
        }
    }
    rest
}

/// Plain slot list with one per-slot limit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimpleInventory {
    pub slots: Vec<Option<ItemStack>>,
    pub limit: u32,
    pub bottomless: bool,
}

impl SimpleInventory {
    pub fn new(slots: usize) -> Self {
        Self { slots: vec![None; slots], limit: DEFAULT_SLOT_LIMIT, bottomless: false }
    }

    pub fn with(mut self, slot: usize, stack: ItemStack) -> Self {
        self.slots[slot] = Some(stack);
        self
    }

    pub fn count_of(&self, key: &ItemStack) -> u32 {
        self.slots.iter().flatten().filter(|s| s.same_item(key)).map(|s| s.count).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(|s| s.as_ref().is_none_or(|s| s.is_empty()))
    }
}

impl ItemHandler for SimpleInventory {
    fn slot_count(&self) -> usize {
        self.slots.len()
    }

    fn stack_in_slot(&self, slot: usize) -> Option<&ItemStack> {
        self.slots.get(slot).and_then(|s| s.as_ref())
    }

    fn extract(&mut self, slot: usize, amount: u32, simulate: bool) -> Option<ItemStack> {
        let existing = self.slots.get_mut(slot)?.as_mut()?;
        if existing.is_empty() || amount == 0 {
            return None;
        }
        let n = amount.min(existing.count);
        let out = existing.copy_with_count(n);
        if !simulate {
            existing.count -= n;
            if existing.count == 0 {
                self.slots[slot] = None;
            }
        }
        Some(out)
    }

    fn insert(&mut self, slot: usize, stack: ItemStack, simulate: bool) -> Option<ItemStack> {
        let limit = self.limit.min(stack.max_stack);
        let Some(cell) = self.slots.get_mut(slot) else { return Some(stack) };
        match cell {
            Some(e) if !e.is_empty() => {
                if !e.same_item(&stack) {
                    return Some(stack);
                }
                let room = limit.saturating_sub(e.count);
                let n = room.min(stack.count);
                if !simulate {
                    e.count += n;
                }
                (stack.count > n).then(|| stack.copy_with_count(stack.count - n))
            }
            _ => {
                let n = limit.min(stack.count);
                if n == 0 {
                    return Some(stack);
                }
                if !simulate {
                    *cell = Some(stack.copy_with_count(n));
                }
                (stack.count > n).then(|| stack.copy_with_count(stack.count - n))
            }
        }
    }

    fn slot_limit(&self, _slot: usize) -> u32 {
        self.limit
    }

    fn duplicate(&self) -> Box<dyn ItemHandler> {
        Box::new(self.clone())
    }

    fn is_bottomless(&self) -> bool {
        self.bottomless
    }
}

/// Deterministic random source; Java uses an unseeded `java.util.Random`.
pub trait Random {
    fn next_int(&mut self) -> i32;
    /// Uniform in `0..bound` (`bound > 0`).
    fn next_below(&mut self, bound: usize) -> usize {
        (self.next_int() as u32 as usize) % bound.max(1)
    }
}

/// Seeded splitmix64 generator for tests and deterministic worlds.
#[derive(Clone, Debug)]
pub struct XorShift(pub u64);

impl Random for XorShift {
    fn next_int(&mut self) -> i32 {
        // splitmix64 step: well distributed even for tiny seeds
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 32) as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iron(n: u32) -> ItemStack {
        ItemStack::new("minecraft:iron_ingot", n)
    }

    #[test]
    fn address_matching_rules() {
        assert!(match_address("", ""));
        assert!(match_address("  ", ""));
        assert!(!match_address("home", ""));
        assert!(match_address("home", "home"));
        assert!(match_address("*", "anything"));
        assert!(match_address("anything", "*"));
        assert!(!match_address("home", "work"));
        // globs in either direction
        assert!(match_address("farm*", "farm-1"));
        assert!(match_address("farm-1", "farm*"));
        assert!(match_address("farm-?", "farm-7"));
        assert!(match_address("{a,b}c", "bc"));
        assert!(!match_address("{a,b}c", "cc"));
        assert!(match_address("[a-c]x", "bx"));
        assert!(!match_address("[!a-c]x", "bx"));
        assert!(!match_address("", "home"));
    }

    #[test]
    fn glob_star_does_not_cross_slash_but_double_star_does() {
        assert!(!glob_matches("a*", "a/b"));
        assert!(glob_matches("a**", "a/b"));
        assert!(glob_matches("a\\*b", "a*b"));
        assert!(!glob_matches("a\\*b", "axb"));
    }

    #[test]
    fn containing_stacks_and_overflows() {
        let (p, overflow) = containing("create:cardboard_package_12x12", &[iron(64), iron(30), iron(64)]);
        assert!(overflow.is_empty());
        assert!(p.is_package() && p.max_stack == 1 && p.bulky);
        let c = contents_stacks(&p);
        assert_eq!(c.iter().map(|s| s.count).collect::<Vec<_>>(), vec![64, 64, 30]);
        // 10 full stacks of distinct-by-count: only nine slots exist
        let many: Vec<_> = (0..10).map(|i| ItemStack::new(&format!("m:i{i}"), 1)).collect();
        let (p2, over) = containing("create:cardboard_package_10x8", &many);
        assert_eq!(contents_stacks(&p2).len(), SLOTS);
        assert_eq!(over.len(), 1);
    }

    #[test]
    fn order_data_defaults_and_accessors() {
        let (mut p, _) = containing("create:cardboard_package_12x12", &[iron(1)]);
        assert_eq!(get_order_id(&p), -1);
        assert_eq!(get_index(&p), -1);
        assert_eq!(get_link_index(&p), -1);
        assert!(!is_final(&p) && !is_final_link(&p) && !has_order_data(&p));
        assert_eq!(get_address(&p), "");
        add_address(&mut p, "base");
        assert_eq!(get_address(&p), "base");
        clear_address(&mut p);
        assert_eq!(get_address(&p), "");
        set_order(&mut p, 77, 2, true, 3, false, None);
        assert_eq!((get_order_id(&p), get_link_index(&p), get_index(&p)), (77, 2, 3));
        assert!(is_final_link(&p) && !is_final(&p) && has_order_data(&p));
    }

    #[test]
    fn order_context_precedence() {
        let (mut p, _) = containing("create:cardboard_package_12x12", &[iron(1)]);
        assert!(get_order_context(&p).is_none());
        let standalone = OrderWithCrafts::simple(vec![BigStack::new(iron(1), 5)]);
        add_order_context(&mut p, standalone.clone());
        assert_eq!(get_order_context(&p), Some(standalone));
        // order data wins even when its own context is null
        set_order(&mut p, 1, 0, true, 0, true, None);
        assert_eq!(get_order_context(&p), None);
        let own = OrderWithCrafts::single_recipe(vec![BigStack::new(iron(1), 1)]);
        set_order(&mut p, 1, 0, true, 0, true, Some(own.clone()));
        assert_eq!(get_order_context(&p), Some(own));
    }

    #[test]
    fn crafting_context_helpers() {
        assert!(!OrderWithCrafts::has_crafting_information(None));
        let single = OrderWithCrafts::single_recipe(vec![BigStack::new(iron(1), 2)]);
        assert!(OrderWithCrafts::has_crafting_information(Some(&single)));
        assert!(single.is_empty(), "isEmpty ignores crafts");
        assert_eq!(single.crafting_information().unwrap().len(), 1);
        assert!(OrderWithCrafts::empty().crafting_information().is_none());
    }

    #[test]
    fn ordered_stacks_match_recipes() {
        let mut o = OrderWithCrafts::single_recipe(vec![BigStack::new(iron(1), 3)]);
        o.ordered_crafts[0].count = 2;
        assert!(!o.ordered_stacks_match_ordered_recipes());
        o.ordered_stacks = PackageOrder::new(vec![BigStack::new(iron(1), 6)]);
        assert!(o.ordered_stacks_match_ordered_recipes());
        o.ordered_stacks = PackageOrder::new(vec![BigStack::new(iron(1), 5)]);
        assert!(!o.ordered_stacks_match_ordered_recipes());
        assert!(!OrderWithCrafts::simple(vec![BigStack::new(iron(1), 1)]).ordered_stacks_match_ordered_recipes());
    }

    #[test]
    fn velocity_curve() {
        assert_eq!(package_velocity(0), 0.0);
        assert!(package_velocity(10) < 0.5);
        assert_eq!(package_velocity(100), 1.0);
        assert!((package_velocity(20) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn simple_inventory_extract_insert() {
        let mut inv = SimpleInventory::new(2);
        assert!(insert_stacked(&mut inv, iron(100), false).is_none());
        assert_eq!(inv.slots[0].as_ref().unwrap().count, 64);
        assert_eq!(inv.slots[1].as_ref().unwrap().count, 36);
        let rest = insert_stacked(&mut inv, iron(100), true).unwrap();
        assert_eq!(rest.count, 100 - 28); // 28 space left, simulate only
        assert_eq!(inv.count_of(&iron(1)), 100);
        let out = inv.extract(0, 10, false).unwrap();
        assert_eq!(out.count, 10);
        assert_eq!(inv.extract(1, 99, true).unwrap().count, 36);
        assert_eq!(inv.count_of(&iron(1)), 90);
    }

    #[test]
    fn components_distinguish_items() {
        let a = iron(1).with_components("x");
        assert!(!a.same_item(&iron(1)));
        assert!(a.same_item(&iron(5).with_components("x")));
        let mut inv = SimpleInventory::new(1).with(0, iron(1));
        assert!(inv.insert(0, a, false).is_some());
    }

    #[test]
    fn xorshift_is_deterministic() {
        let mut a = XorShift(42);
        let mut b = XorShift(42);
        assert_eq!(a.next_int(), b.next_int());
        assert!(a.next_below(5) < 5);
    }
}
