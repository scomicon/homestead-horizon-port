// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_cannon_tick(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut crate::containers::ModBlockEntity,
) {
    if entity.id == crate::game::create_port_cannon::BLOCK {
        crate::game::create_port_cannon::tick(game, pos, entity);
    } else if entity.id == crate::game::create_port_schematic_table::BLOCK {
        crate::game::create_port_schematic_table::tick(game, pos, entity);
    }
}
