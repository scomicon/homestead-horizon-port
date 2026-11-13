// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_cannon_ticks(block: &str, ticks: &mut bool) {
    crate::game::create_port_cannon::ticks(block, ticks);
    crate::game::create_port_schematic_table::ticks(block, ticks);
}
