// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_skillet_update_tag(entity: &crate::containers::ModBlockEntity, tag: &mut Option<horizon_world::nbt::Compound>) {
    if entity.id == crate::game::homestead_skillet::BLOCK && tag.is_none() {
        *tag = Some(crate::game::homestead_skillet::update_tag(entity));
    }
}
