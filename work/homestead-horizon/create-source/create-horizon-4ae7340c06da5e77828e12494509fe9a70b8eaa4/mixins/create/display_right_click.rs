// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_display_right_click(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    face: [i32; 3],
    canceled: &mut bool,
) {
    let _ = face;
    if *canceled {
        return;
    }
    if crate::game::create_port_display_link::right_click(game, pos) {
        game.swing_arm();
        *canceled = true;
    }
}
