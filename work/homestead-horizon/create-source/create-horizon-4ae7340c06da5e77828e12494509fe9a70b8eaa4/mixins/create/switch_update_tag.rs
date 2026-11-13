// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_switch_update_tag(
    entity: &crate::containers::ModBlockEntity,
    tag: &mut Option<horizon_world::nbt::Compound>,
) {
    crate::game::create_port_switch::update_tag(entity, tag);
}
