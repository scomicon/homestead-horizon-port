// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_display_ticks(block: &str, ticks: &mut bool) {
    if block == crate::game::create_port_display_link::BLOCK
        || block == crate::game::create_port_display_board::BOARD
    {
        *ticks = true;
    }
}
