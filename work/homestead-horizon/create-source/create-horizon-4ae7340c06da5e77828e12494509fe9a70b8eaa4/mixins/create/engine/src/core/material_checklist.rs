// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Pure material checklist after MaterialChecklist and the ItemRequirement
// vocabulary it consumes (Create 6.0.10, content/schematics/cannon and
// content/schematics/requirement). Rendering to a written book or clipboard
// component is an adapter concern; this module produces the exact page layout
// (sorting, page size, page breaks, amounts) those adapters must realise.
use std::collections::BTreeMap;

/// Written book: entries per page (`MAX_ENTRIES_PER_PAGE`).
pub const MAX_ENTRIES_PER_BOOK_PAGE: usize = 5;
/// Clipboard: entries per page (`MAX_ENTRIES_PER_CLIPBOARD_PAGE`).
pub const MAX_ENTRIES_PER_CLIPBOARD_PAGE: usize = 7;
/// Item stack size used for the "stacks + remainder" figure (hardcoded 64).
pub const STACK_SIZE: i32 = 64;

pub const AIR: &str = "minecraft:air";

/// `ItemUseType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UseKind {
    /// Item is consumed.
    Consume,
    /// One point of durability is spent (tools such as shears).
    Damage,
    /// Anything else; ignored by the checklist.
    Other,
}

/// `ItemRequirement.StackRequirement` (the item matcher is the adapter's).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StackRequirement {
    pub item: String,
    pub count: i32,
    pub usage: UseKind,
}

impl StackRequirement {
    pub fn consume(item: &str, count: i32) -> Self {
        Self { item: item.to_owned(), count, usage: UseKind::Consume }
    }
    pub fn damage(item: &str, count: i32) -> Self {
        Self { item: item.to_owned(), count, usage: UseKind::Damage }
    }
}

/// `ItemRequirement`: NONE (empty), INVALID, or a list of stack requirements.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemRequirement {
    Empty,
    Invalid,
    Items(Vec<StackRequirement>),
}

impl ItemRequirement {
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Empty => true,
            Self::Invalid => false,
            Self::Items(v) => v.is_empty(),
        }
    }
    pub fn is_invalid(&self) -> bool {
        matches!(self, Self::Invalid)
    }
    pub fn items(&self) -> &[StackRequirement] {
        match self {
            Self::Items(v) => v,
            _ => &[],
        }
    }
}

/// Where a page continues, mirroring the coloured ">>>" separators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Continuation {
    /// Break inside the unfinished pass (blue in the book, dark grey on clipboards).
    Unfinished,
    /// Break inside the completed pass (dark green).
    Completed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChecklistEntry {
    pub item: String,
    /// Remaining amount for unfinished entries, the full requirement for completed ones.
    pub amount: i32,
    pub completed: bool,
}

impl ChecklistEntry {
    /// `amount / 64` and `amount % 64` as shown after the `|` separator.
    pub fn stacks_and_remainder(&self) -> (i32, i32) {
        (self.amount / STACK_SIZE, self.amount % STACK_SIZE)
    }
    /// Second line of an entry, e.g. ` x150` (the book appends the gray part).
    pub fn amount_text(&self) -> String {
        format!(" x{}", self.amount)
    }
    /// Gray part, e.g. ` | 2▤ +22`.
    pub fn stack_text(&self) -> String {
        let (s, r) = self.stacks_and_remainder();
        format!(" | {s}\u{25A4} +{r}")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChecklistPage {
    pub entries: Vec<ChecklistEntry>,
    pub continued: Option<Continuation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChecklistLayout {
    /// `blocksNotLoaded`: book gets a leading red page, clipboard a red entry.
    pub blocks_not_loaded_notice: bool,
    /// The final page is always present, possibly empty.
    pub pages: Vec<ChecklistPage>,
}

#[derive(Clone, Debug, Default)]
pub struct MaterialChecklist {
    pub gathered: BTreeMap<String, i32>,
    pub required: BTreeMap<String, i32>,
    pub damage_required: BTreeMap<String, i32>,
    pub blocks_not_loaded: bool,
}

fn put_or_increment(map: &mut BTreeMap<String, i32>, item: &str, count: i32) {
    if item == AIR {
        return;
    }
    let slot = map.entry(item.to_owned()).or_insert(0);
    *slot = slot.wrapping_add(count);
}

impl MaterialChecklist {
    pub fn new() -> Self {
        Self::default()
    }

    /// `warnBlockNotLoaded`.
    pub fn warn_block_not_loaded(&mut self) {
        self.blocks_not_loaded = true;
    }

    /// `require(ItemRequirement)`: empty/invalid requirements are ignored.
    pub fn require(&mut self, requirement: &ItemRequirement) {
        if requirement.is_empty() || requirement.is_invalid() {
            return;
        }
        for stack in requirement.items() {
            match stack.usage {
                UseKind::Damage => put_or_increment(&mut self.damage_required, &stack.item, stack.count),
                UseKind::Consume => put_or_increment(&mut self.required, &stack.item, stack.count),
                UseKind::Other => {}
            }
        }
    }

    /// `collect(ItemStack)`: only counts items the schematic needs.
    pub fn collect(&mut self, item: &str, count: i32) {
        if self.required.contains_key(item) || self.damage_required.contains_key(item) {
            let slot = self.gathered.entry(item.to_owned()).or_insert(0);
            *slot = slot.wrapping_add(count);
        }
    }

    /// The `checklist.required/damageRequired.clear()` part of `updateChecklist`.
    pub fn clear_requirements(&mut self) {
        self.required.clear();
        self.damage_required.clear();
        self.blocks_not_loaded = false;
    }

    /// `getRequiredAmount`: consumed count plus damage points rounded up to whole
    /// items. A zero `max_damage` follows Java's float division and saturating
    /// int cast (inf -> i32::MAX, NaN -> 0).
    pub fn required_amount(&self, item: &str, max_damage: i32) -> i32 {
        let mut amount = self.required.get(item).copied().unwrap_or(0);
        if let Some(d) = self.damage_required.get(item) {
            let items = (*d as f32 / max_damage as f32).ceil() as i32;
            amount = amount.saturating_add(items);
        }
        amount
    }

    /// Union of required and damage-required items sorted ascending by
    /// lower-cased display name (`String.compareTo`, stable on ties).
    pub fn sorted_items(&self, display_name: &dyn Fn(&str) -> String) -> Vec<String> {
        let mut keys: Vec<String> = self.required.keys().cloned().collect();
        for k in self.damage_required.keys() {
            if !self.required.contains_key(k) {
                keys.push(k.clone());
            }
        }
        keys.sort_by_key(|k| display_name(k).to_lowercase());
        keys
    }

    /// Shared page builder of `createWrittenBook` / `createWrittenClipboard`.
    /// Unfinished items (remaining > 0) come first, then completed items; the
    /// per-page counter is not reset between the two passes.
    pub fn layout(
        &self,
        max_per_page: usize,
        display_name: &dyn Fn(&str) -> String,
        max_damage: &dyn Fn(&str) -> i32,
    ) -> ChecklistLayout {
        let mut pages = Vec::new();
        let mut current: Vec<ChecklistEntry> = Vec::new();
        let mut written = 0usize;
        let mut completed: Vec<(String, i32)> = Vec::new();

        for item in self.sorted_items(display_name) {
            let required = self.required_amount(&item, max_damage(&item));
            let gathered = self.gathered.get(&item).copied().unwrap_or(0);
            let amount = required.wrapping_sub(gathered);
            if amount <= 0 {
                completed.push((item, required));
                continue;
            }
            if written == max_per_page {
                pages.push(ChecklistPage {
                    entries: std::mem::take(&mut current),
                    continued: Some(Continuation::Unfinished),
                });
                written = 0;
            }
            written += 1;
            current.push(ChecklistEntry { item, amount, completed: false });
        }
        for (item, required) in completed {
            if written == max_per_page {
                pages.push(ChecklistPage {
                    entries: std::mem::take(&mut current),
                    continued: Some(Continuation::Completed),
                });
                written = 0;
            }
            written += 1;
            current.push(ChecklistEntry { item, amount: required, completed: true });
        }
        pages.push(ChecklistPage { entries: current, continued: None });
        ChecklistLayout { blocks_not_loaded_notice: self.blocks_not_loaded, pages }
    }

    pub fn book_layout(
        &self,
        display_name: &dyn Fn(&str) -> String,
        max_damage: &dyn Fn(&str) -> i32,
    ) -> ChecklistLayout {
        self.layout(MAX_ENTRIES_PER_BOOK_PAGE, display_name, max_damage)
    }

    pub fn clipboard_layout(
        &self,
        display_name: &dyn Fn(&str) -> String,
        max_damage: &dyn Fn(&str) -> i32,
    ) -> ChecklistLayout {
        self.layout(MAX_ENTRIES_PER_CLIPBOARD_PAGE, display_name, max_damage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(id: &str) -> String {
        id.trim_start_matches("minecraft:").replace('_', " ")
    }
    fn dur(id: &str) -> i32 {
        if id.ends_with("shears") { 238 } else { 0 }
    }

    #[test]
    fn require_ignores_empty_invalid_air_and_other() {
        let mut c = MaterialChecklist::new();
        c.require(&ItemRequirement::Empty);
        c.require(&ItemRequirement::Invalid);
        c.require(&ItemRequirement::Items(vec![
            StackRequirement::consume(AIR, 3),
            StackRequirement { item: "x".into(), count: 1, usage: UseKind::Other },
            StackRequirement::consume("minecraft:stone", 2),
            StackRequirement::damage("minecraft:shears", 5),
        ]));
        assert_eq!(c.required.len(), 1);
        assert_eq!(c.required["minecraft:stone"], 2);
        assert_eq!(c.damage_required["minecraft:shears"], 5);
    }

    #[test]
    fn collect_counts_only_needed_items() {
        let mut c = MaterialChecklist::new();
        c.require(&ItemRequirement::Items(vec![StackRequirement::consume("minecraft:stone", 10)]));
        c.collect("minecraft:stone", 4);
        c.collect("minecraft:stone", 3);
        c.collect("minecraft:dirt", 9);
        assert_eq!(c.gathered["minecraft:stone"], 7);
        assert!(!c.gathered.contains_key("minecraft:dirt"));
    }

    #[test]
    fn damage_rounds_up_to_whole_items() {
        let mut c = MaterialChecklist::new();
        c.require(&ItemRequirement::Items(vec![StackRequirement::damage("minecraft:shears", 239)]));
        assert_eq!(c.required_amount("minecraft:shears", 238), 2);
        c.damage_required.insert("minecraft:shears".into(), 238);
        assert_eq!(c.required_amount("minecraft:shears", 238), 1);
        c.required.insert("minecraft:shears".into(), 3);
        assert_eq!(c.required_amount("minecraft:shears", 238), 4);
    }

    #[test]
    fn zero_durability_follows_java_cast() {
        let mut c = MaterialChecklist::new();
        c.damage_required.insert("a".into(), 5);
        assert_eq!(c.required_amount("a", 0), i32::MAX);
        c.damage_required.insert("b".into(), 0);
        assert_eq!(c.required_amount("b", 0), 0);
    }

    #[test]
    fn sorted_by_lowercase_display_name_with_union() {
        let mut c = MaterialChecklist::new();
        c.required.insert("minecraft:oak_planks".into(), 1);
        c.required.insert("minecraft:cobblestone".into(), 1);
        c.damage_required.insert("minecraft:shears".into(), 1);
        c.damage_required.insert("minecraft:cobblestone".into(), 1);
        let s = c.sorted_items(&name);
        assert_eq!(s, ["minecraft:cobblestone", "minecraft:oak_planks", "minecraft:shears"]);
    }

    #[test]
    fn entry_text_matches_documented_example() {
        let e = ChecklistEntry { item: "a".into(), amount: 150, completed: false };
        assert_eq!(e.amount_text(), " x150");
        assert_eq!(e.stack_text(), " | 2\u{25A4} +22");
    }

    #[test]
    fn unfinished_first_then_completed_sharing_counter() {
        let mut c = MaterialChecklist::new();
        for (i, n) in ["a", "b", "c", "d", "e", "f", "g"].iter().enumerate() {
            c.required.insert(format!("minecraft:{n}"), 10 + i as i32);
        }
        // a and b already satisfied
        c.gathered.insert("minecraft:a".into(), 10);
        c.gathered.insert("minecraft:b".into(), 20);
        let l = c.book_layout(&name, &dur);
        assert!(!l.blocks_not_loaded_notice);
        // 5 unfinished (c..g) fill page 1; completed go to page 2 after a break.
        assert_eq!(l.pages.len(), 2);
        assert_eq!(l.pages[0].entries.len(), 5);
        assert!(l.pages[0].entries.iter().all(|e| !e.completed));
        // the break happens when the first completed entry finds the page full
        assert_eq!(l.pages[0].continued, Some(Continuation::Completed));
        assert_eq!(l.pages[0].entries[0].amount, 12);
        assert_eq!(l.pages[1].entries.len(), 2);
        assert!(l.pages[1].entries.iter().all(|e| e.completed));
        // completed entries show the full requirement
        assert_eq!(l.pages[1].entries[0].amount, 10);
        assert_eq!(l.pages[1].entries[1].amount, 11);
        assert_eq!(l.pages[1].continued, None);
    }

    #[test]
    fn completed_can_share_page_with_unfinished() {
        let mut c = MaterialChecklist::new();
        c.required.insert("minecraft:a".into(), 1);
        c.required.insert("minecraft:b".into(), 1);
        c.gathered.insert("minecraft:a".into(), 1);
        let l = c.book_layout(&name, &dur);
        assert_eq!(l.pages.len(), 1);
        assert!(!l.pages[0].entries[0].completed);
        assert!(l.pages[0].entries[1].completed);
    }

    #[test]
    fn completed_overflow_uses_completed_separator() {
        let mut c = MaterialChecklist::new();
        for n in ["a", "b", "c", "d", "e", "f"] {
            c.required.insert(format!("minecraft:{n}"), 1);
            c.gathered.insert(format!("minecraft:{n}"), 1);
        }
        let l = c.book_layout(&name, &dur);
        assert_eq!(l.pages.len(), 2);
        assert_eq!(l.pages[0].continued, Some(Continuation::Completed));
        assert_eq!(l.pages[1].entries.len(), 1);
    }

    #[test]
    fn empty_checklist_has_one_empty_page_and_notice_flag() {
        let mut c = MaterialChecklist::new();
        c.warn_block_not_loaded();
        let l = c.clipboard_layout(&name, &dur);
        assert!(l.blocks_not_loaded_notice);
        assert_eq!(l.pages.len(), 1);
        assert!(l.pages[0].entries.is_empty());
    }

    #[test]
    fn clipboard_pages_hold_seven() {
        let mut c = MaterialChecklist::new();
        for i in 0..8 {
            c.required.insert(format!("minecraft:i{i}"), 1);
        }
        let l = c.clipboard_layout(&name, &dur);
        assert_eq!(l.pages[0].entries.len(), 7);
        assert_eq!(l.pages[1].entries.len(), 1);
    }

    #[test]
    fn clear_requirements_resets_not_loaded_flag_but_keeps_gathered() {
        let mut c = MaterialChecklist::new();
        c.required.insert("a".into(), 1);
        c.gathered.insert("a".into(), 1);
        c.warn_block_not_loaded();
        c.clear_requirements();
        assert!(c.required.is_empty() && !c.blocks_not_loaded);
        assert_eq!(c.gathered.len(), 1);
    }
}
