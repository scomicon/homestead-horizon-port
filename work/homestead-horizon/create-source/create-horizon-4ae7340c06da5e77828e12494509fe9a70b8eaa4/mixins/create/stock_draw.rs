// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_stock_draw(
    game: &crate::game::Game<'_>,
    pos: [i32; 3],
    block: &str,
    entity: &crate::containers::ModBlockEntity,
    draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw,
) {
    if block == crate::game::create_port_frogport::FROGPORT || block == crate::game::create_port_frogport::POSTBOX {
        crate::game::create_port_frogport::draw(game, pos, entity, draw);
    }
}
