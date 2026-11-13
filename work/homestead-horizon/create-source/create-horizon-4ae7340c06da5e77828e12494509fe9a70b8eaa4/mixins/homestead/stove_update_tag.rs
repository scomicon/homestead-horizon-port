// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_stove_update_tag(entity: &crate::containers::ModBlockEntity, tag: &mut Option<horizon_world::nbt::Compound>) {
    if entity.id == crate::game::homestead_stove::BLOCK && tag.is_none() {
        *tag = Some(crate::game::homestead_stove::update_tag(entity));
    }
}
