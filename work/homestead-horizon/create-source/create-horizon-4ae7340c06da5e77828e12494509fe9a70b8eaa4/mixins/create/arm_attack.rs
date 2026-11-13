// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn arm_attack(game: &mut crate::game::Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    if crate::game::create_port_arm::left_click(game, pos) {
        *canceled = true;
    }
}
