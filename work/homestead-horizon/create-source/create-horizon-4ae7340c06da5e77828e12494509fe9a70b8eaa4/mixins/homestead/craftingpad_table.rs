// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Head of `tick_craft_table`: a CraftingPad menu has no block to check, so the check is skipped.

fn homestead_craftingpad_table(game: &mut crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    if crate::game::homestead_craftingpad::keeps_open(game) {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
