// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_display_update_tag(
    entity: &crate::containers::ModBlockEntity,
    tag: &mut Option<horizon_world::nbt::Compound>,
) {
    if tag.is_some() {
        return;
    }
    if entity.id == crate::game::create_port_display_link::BLOCK {
        *tag = Some(crate::game::create_port_display_link::update_tag(entity));
    } else if entity.id == crate::game::create_port_display_board::BOARD {
        *tag = Some(crate::game::create_port_display_board::update_tag(entity));
    }
}
