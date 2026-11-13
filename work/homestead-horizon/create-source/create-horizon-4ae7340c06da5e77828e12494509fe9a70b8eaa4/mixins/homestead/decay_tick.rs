// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_decay_tick(game: &mut crate::game::Game<'_>, dimension: &str) {
    crate::game::homestead_decay::tick(game, dimension);
}