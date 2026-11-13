// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_link_source(game: &crate::game::GameLevel<'_>, state: horizon_world::registry::BlockStateId, source: &mut bool) {
    crate::game::create_port_link::signal_source(game, state, source);
}
