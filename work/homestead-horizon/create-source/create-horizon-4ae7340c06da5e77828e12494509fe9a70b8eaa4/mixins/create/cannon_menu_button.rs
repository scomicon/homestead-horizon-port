// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_cannon_menu_button(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut crate::containers::ModBlockEntity,
    id: i32,
) {
    if entity.id == crate::game::create_port_cannon::BLOCK {
        crate::game::create_port_cannon::menu_button(game, pos, entity, id);
    } else if entity.id == crate::game::create_port_schematic_table::BLOCK {
        crate::game::create_port_schematic_table::menu_button(game, pos, entity, id);
    }
}
