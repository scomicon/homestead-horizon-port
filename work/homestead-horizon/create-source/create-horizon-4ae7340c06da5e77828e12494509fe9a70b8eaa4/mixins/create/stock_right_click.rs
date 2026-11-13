// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_stock_right_click(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    canceled: &mut bool,
) {
    if !*canceled && crate::game::create_port_stock_link::right_click(game, pos) {
        game.swing_arm();
        *canceled = true;
        return;
    }
    if !*canceled && crate::game::create_port_frogport::right_click(game, pos) {
        game.swing_arm();
        *canceled = true;
    }
}
