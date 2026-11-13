// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_cannon_update_tag(
    entity: &crate::containers::ModBlockEntity,
    tag: &mut Option<horizon_world::nbt::Compound>,
) {
    if tag.is_some() {
        return;
    }
    if entity.id == crate::game::create_port_cannon::BLOCK {
        *tag = Some(crate::game::create_port_cannon::update_tag(entity));
    } else if entity.id == crate::game::create_port_schematic_table::BLOCK {
        *tag = Some(crate::game::create_port_schematic_table::update_tag(entity));
    }
}
