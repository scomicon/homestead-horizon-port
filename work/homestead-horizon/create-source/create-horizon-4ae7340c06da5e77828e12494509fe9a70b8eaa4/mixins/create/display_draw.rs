// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_display_draw(
    game: &crate::game::Game<'_>,
    pos: [i32; 3],
    block: &str,
    state: horizon_world::registry::BlockStateId,
    entity: &crate::containers::ModBlockEntity,
    draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw,
) {
    let _ = state;
    if block == crate::game::create_port_display_board::BOARD {
        crate::game::create_port_display_board::draw(game, pos, entity, draw);
    }
}
