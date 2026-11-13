// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_logistics_update_tag(
    entity: &crate::containers::ModBlockEntity,
    tag: &mut Option<horizon_world::nbt::Compound>,
) {
    if tag.is_some() {
        return;
    }
    if entity.id == crate::game::create_port_packager::BLOCK {
        *tag = Some(crate::game::create_port_packager::update_tag(entity));
    } else if entity.id == crate::game::create_port_gauge::BLOCK {
        *tag = Some(crate::game::create_port_gauge::update_tag(entity));
    }
}
