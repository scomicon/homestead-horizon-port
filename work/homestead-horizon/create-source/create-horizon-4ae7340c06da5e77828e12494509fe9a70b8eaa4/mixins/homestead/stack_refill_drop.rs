// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_stack_refill_drop(player: u128, count: u32) {
    let _ = (player, count);
    crate::game::homestead_stack_refill::dropped();
}
