// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_crops_use(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    face: [i32; 3],
    canceled: &mut bool,
) {
    if !*canceled && crate::game::homestead_crops::right_click(game, pos, face) {
        game.swing_arm();
        *canceled = true;
    }
}
