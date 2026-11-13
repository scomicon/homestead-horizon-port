// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_stock_entity(block: &str, entity: &mut Option<(String, horizon_world::nbt::Compound)>) {
    crate::game::create_port_stock_link::entity(block, entity);
    crate::game::create_port_frogport::entity(block, entity);
}
