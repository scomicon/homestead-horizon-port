// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_display_bounds(
    game: &crate::game::Game<'_>,
    pos: [i32; 3],
    block: &str,
    entity: &crate::containers::ModBlockEntity,
    bounds: &mut [f32; 6],
) {
    if block == crate::game::create_port_display_board::BOARD {
        crate::game::create_port_display_board::bounds(entity, game, pos, bounds);
    }
}
