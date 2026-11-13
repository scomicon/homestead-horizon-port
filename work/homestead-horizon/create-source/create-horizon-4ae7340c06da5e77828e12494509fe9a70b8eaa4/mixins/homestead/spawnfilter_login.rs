// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_spawnfilter_login(game: &mut crate::game::Game<'_>, player: u128) {
    crate::game::homestead_spawnfilter::logged_in(game, player);
}
