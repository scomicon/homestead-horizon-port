// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_ctip_register(commands: &mut Vec<crate::mod_api::NativeCommand>) {
    crate::game::homestead_client_tooltips::register(commands);
}
