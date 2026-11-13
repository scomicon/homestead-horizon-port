// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_board_update_tag(entity: &crate::containers::ModBlockEntity, tag: &mut Option<horizon_world::nbt::Compound>) {
    if entity.id == crate::game::homestead_cutting_board::BLOCK && tag.is_none() {
        *tag = Some(crate::game::homestead_cutting_board::update_tag(entity));
    }
}
