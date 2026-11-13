// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Stack Refill (Homestead pack): when the stack in the selected hotbar slot or in the off hand is
// used up, the next stack of the same item moves into that slot, so a build, a meal or a pickaxe
// never leaves the hand empty.
//
// The mod's source was not available here (it is not in `pack/mods` and no jar was supplied), so
// the rules are those of the mod as it is described, written down in `docs/logic-c.md`:
//
// * "used up" is a stack that vanished from the hand between two ticks while the player was
//   playing (no screen) and the whole inventory holds less of that item than a tick before. A stack
//   that merely moved (hands swapped, a hotbar key pressed, an inventory click) keeps the total
//   and is left alone, and so is a stack that was thrown (`item_dropped` is heard first),
// * the source is the first exactly equal stack (same item, same components) in the main
//   inventory, then in the rest of the hotbar, and the off hand for the main hand; a tool that
//   broke (a damageable item) falls back to the first stack of the same item with other damage,
// * the whole source stack moves; nothing is split, nothing is crafted.
//
// Only the local player of the window game is served (no remote server: its inventory is the
// server's), once per tick from `level_tick_pre`.

use super::{Game, Screen};
use crate::items::ItemStack;
use std::cell::RefCell;

/// `Inventory.SLOT_OFFHAND`.
const OFFHAND: usize = 40;
/// Slots of the main inventory and the hotbar: 0-8 hotbar, 9-35 inventory.
const SLOTS: usize = 36;

struct Snap {
    stack: ItemStack,
    /// How many of this exact item the inventory held when the snapshot was taken.
    total: u64,
}

#[derive(Default)]
struct Watch {
    selected: usize,
    main: Option<Snap>,
    off: Option<Snap>,
    dropped: bool,
}

thread_local! {
    static WATCH: RefCell<Watch> = RefCell::new(Watch::default());
}

fn count(stack: &ItemStack) -> u64 {
    stack.count as u64
}

fn total(game: &Game<'_>, stack: &ItemStack) -> u64 {
    (0..SLOTS)
        .chain(std::iter::once(OFFHAND))
        .filter_map(|slot| game.stack_in(slot))
        .filter(|other| other.same_item_same_components(stack))
        .map(count)
        .sum()
}

fn snap(game: &Game<'_>, slot: usize) -> Option<Snap> {
    let stack = game.stack_in(slot).filter(|stack| count(stack) > 0)?;
    Some(Snap { total: total(game, stack), stack: stack.clone() })
}

fn empty(game: &Game<'_>, slot: usize) -> bool {
    game.stack_in(slot).is_none_or(|stack| count(stack) == 0)
}

fn damageable(game: &Game<'_>, stack: &ItemStack) -> bool {
    game.items().get(&stack.item).is_some_and(|info| info.max_damage as i32 > 0)
}

/// The slot a refill for `slot` takes from: the main inventory first, then the rest of the hotbar
/// (and the off hand for the main hand), exactly equal stacks before other damage values.
fn source(game: &Game<'_>, slot: usize, prev: &ItemStack) -> Option<usize> {
    let mut order: Vec<usize> = (9..SLOTS).chain(0..9).filter(|other| *other != slot).collect();
    order.push(if slot == OFFHAND { game.selected } else { OFFHAND });
    let found = |exact: bool| {
        order.iter().copied().find(|other| {
            game.stack_in(*other).is_some_and(|stack| {
                count(stack) > 0
                    && if exact {
                        stack.same_item_same_components(prev)
                    } else {
                        stack.item == prev.item
                    }
            })
        })
    };
    found(true).or_else(|| if damageable(game, prev) { found(false) } else { None })
}

fn refill(game: &mut Game<'_>, slot: usize, prev: &Snap) -> bool {
    if !empty(game, slot) || total(game, &prev.stack) >= prev.total {
        return false;
    }
    let Some(from) = source(game, slot, &prev.stack) else {
        return false;
    };
    let moved = game.stack_in_mut(from).take();
    *game.stack_in_mut(slot) = moved;
    true
}

/// `item_dropped`: the player threw something, so an empty hand is not a used-up stack.
pub(super) fn dropped() {
    WATCH.with_borrow_mut(|watch| watch.dropped = true);
}

/// Once per tick for the local player.
pub(super) fn tick(game: &mut Game<'_>) {
    let mut watch = WATCH.with_borrow_mut(std::mem::take);
    let selected = game.selected;
    let serve = game.player.is_some() && game.remote.is_none() && game.screen == Screen::Playing;
    thread_local! {
        static CALLS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    }
    let calls = CALLS.with(|calls| {
        calls.set(calls.get().wrapping_add(1));
        calls.get()
    });
    if calls % 200 == 1 {
        log::info!(
            "Stack refill tick #{calls}: serve={serve} player={} remote={} playing={} selected={selected} main={} dropped={}",
            game.player.is_some(),
            game.remote.is_some(),
            game.screen == Screen::Playing,
            watch.main.is_some(),
            watch.dropped
        );
    }
    if serve && !std::mem::take(&mut watch.dropped) {
        if watch.selected == selected && let Some(prev) = &watch.main {
            refill(game, selected, prev);
        }
        if let Some(prev) = &watch.off {
            refill(game, OFFHAND, prev);
        }
    }
    watch.dropped = false;
    watch.selected = selected;
    watch.main = snap(game, selected);
    watch.off = snap(game, OFFHAND);
    WATCH.with_borrow_mut(|slot| *slot = watch);
}
