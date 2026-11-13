// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_right_click(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    face: [i32; 3],
    canceled: &mut bool,
) {
    if !*canceled && crate::game::homestead_stove::right_click(game, pos) {
        game.swing_arm();
        *canceled = true;
        return;
    }
    if !*canceled && crate::game::homestead_skillet::right_click(game, pos) {
        game.swing_arm();
        *canceled = true;
        return;
    }
    if !*canceled && crate::game::homestead_cooking_pot::right_click(game, pos) {
        game.swing_arm();
        *canceled = true;
        return;
    }
    if !*canceled && crate::game::homestead_cutting_board::right_click(game, pos) {
        game.swing_arm();
        *canceled = true;
        return;
    }
    if !*canceled && crate::game::homestead_harvest::right_click(game, pos, face) {
        game.swing_arm();
        *canceled = true;
    }
}
