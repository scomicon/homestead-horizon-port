// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_logistics_ticks(block: &str, ticks: &mut bool) {
    crate::game::create_port_packager::ticks(block, ticks);
    crate::game::create_port_gauge::ticks(block, ticks);
}
