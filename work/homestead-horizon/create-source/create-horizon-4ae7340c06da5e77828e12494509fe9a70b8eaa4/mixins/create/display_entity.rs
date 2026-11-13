// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_display_entity(block: &str, entity: &mut Option<(String, horizon_world::nbt::Compound)>) {
    if entity.is_some() {
        return;
    }
    if block == crate::game::create_port_display_link::BLOCK {
        *entity = Some((block.into(), crate::game::create_port_display_link::default_nbt()));
    } else if block == crate::game::create_port_display_board::BOARD {
        *entity = Some((block.into(), crate::game::create_port_display_board::default_nbt()));
    }
}
