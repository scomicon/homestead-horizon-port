// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_logistics_tick(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut crate::containers::ModBlockEntity,
) {
    if entity.id == crate::game::create_port_packager::BLOCK {
        crate::game::create_port_packager::tick(game, pos, entity);
    } else if entity.id == crate::game::create_port_gauge::BLOCK {
        crate::game::create_port_gauge::tick(game, pos, entity);
    }
}
