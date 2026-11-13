// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_stock_update_tag(
    entity: &crate::containers::ModBlockEntity,
    tag: &mut Option<horizon_world::nbt::Compound>,
) {
    if tag.is_some() {
        return;
    }
    if entity.id == crate::game::create_port_frogport::FROGPORT || entity.id == crate::game::create_port_frogport::POSTBOX {
        *tag = Some(crate::game::create_port_frogport::update_tag(entity));
    }
}
