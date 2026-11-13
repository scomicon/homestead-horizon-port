// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_craftingpad_use(
    game: &mut crate::game::Game<'_>,
    stack: &crate::items::ItemStack,
    claimed: &mut bool,
) {
    if !*claimed && crate::game::homestead_craftingpad::use_item(game, stack) {
        *claimed = true;
    }
}
