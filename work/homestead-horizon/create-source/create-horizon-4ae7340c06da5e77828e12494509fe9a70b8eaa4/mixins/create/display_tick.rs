// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_display_tick(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut crate::containers::ModBlockEntity,
) {
    if entity.id == crate::game::create_port_display_link::BLOCK {
        crate::game::create_port_display_link::tick(game, pos, entity);
    } else if entity.id == crate::game::create_port_display_board::BOARD {
        crate::game::create_port_display_board::tick(game, pos, entity);
    }
}
