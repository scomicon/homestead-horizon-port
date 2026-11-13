// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_logistics_draw(
    game: &crate::game::Game<'_>,
    pos: [i32; 3],
    block: &str,
    entity: &crate::containers::ModBlockEntity,
    draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw,
) {
    if block == crate::game::create_port_packager::BLOCK {
        crate::game::create_port_packager::draw(game, pos, entity, draw);
    } else if block == crate::game::create_port_gauge::BLOCK {
        crate::game::create_port_gauge::draw(game, pos, entity, draw);
    }
}
