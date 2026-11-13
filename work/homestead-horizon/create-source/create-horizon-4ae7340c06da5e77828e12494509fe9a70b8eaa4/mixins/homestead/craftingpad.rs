// Copyright (c) StrikerRocker and Guntram Blohm / CraftingPad contributors (MIT)
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// CraftingPad: right-clicking the item opens a crafting table's 3x3 menu anywhere.
//
// The real thing, not a drawn imitation: the menu is the game's own crafting table screen
// (`craft_size = 3`, the same `craft_grid`, recipes, recipe book, shift-click and result slot), opened
// the way a click on a crafting table opens it. What a table does and a pad cannot is to be a block:
// `tick_craft_table` closes the menu once the block is gone or out of reach, so the pad's menu is
// marked with a position no block can have (`SENTINEL`) and a head injection on `tick_craft_table`
// leaves a marked menu alone. Closing works as for a table (`close_container` returns the grid to the
// inventory and clears the mark), so Escape, E and walking away on purpose all behave.
//
// Not done: the original is also usable from a server (the menu there is the server's, and
// `remote` menus are not opened from here); the pad's `stillValid` is "the player holds it" in
// the original as far as it could be recalled; here the menu stays until it is closed.

use super::{Game, Screen};
use crate::items::ItemStack;

pub(super) const ITEM: &str = "craftingpad:craftingpad";
/// The mark of a pad menu in `craft_table`: no world has a block at this position.
pub(super) const SENTINEL: [i32; 3] = [i32::MIN, i32::MIN, i32::MIN];

/// A right click of `stack` in the air: opens the crafting menu. True when it did.
pub(super) fn use_item(game: &mut Game<'_>, stack: &ItemStack) -> bool {
    if &*stack.item != ITEM || game.remote.is_some() || game.screen != Screen::Playing {
        return false;
    }
    game.craft_size = 3;
    game.craft_table = Some(SENTINEL);
    game.screen = Screen::Inventory;
    game.capture(false);
    true
}

/// A pad menu is open: `tick_craft_table` must not close it.
pub(super) fn keeps_open(game: &Game<'_>) -> bool {
    game.craft_size == 3 && game.craft_table == Some(SENTINEL)
}
