// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_logistics_right_click(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    canceled: &mut bool,
) {
    if !*canceled && crate::game::create_port_packager::right_click(game, pos) {
        game.swing_arm();
        *canceled = true;
        return;
    }
    if !*canceled && crate::game::create_port_gauge::right_click(game, pos) {
        game.swing_arm();
        *canceled = true;
    }
}
