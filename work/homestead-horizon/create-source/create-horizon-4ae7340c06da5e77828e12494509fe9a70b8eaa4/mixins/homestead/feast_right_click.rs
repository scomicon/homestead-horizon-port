// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_feast_right_click(game: &mut crate::game::Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    if !*canceled && crate::game::homestead_feast::right_click(game, pos) {
        game.swing_arm();
        *canceled = true;
    }
}
