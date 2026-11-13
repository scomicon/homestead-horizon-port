// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_stack_refill(game: &mut crate::game::Game<'_>) {
    crate::game::homestead_stack_refill::tick(game);
}
