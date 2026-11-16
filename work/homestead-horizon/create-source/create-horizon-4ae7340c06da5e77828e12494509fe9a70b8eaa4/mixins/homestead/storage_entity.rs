// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_storage_entity(block: &str, entity: &mut Option<(String, horizon_world::nbt::Compound)>) {
    crate::game::homestead_storage::entity(block, entity);
}
