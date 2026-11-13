// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_cannon_quick_move(
    game: &mut crate::game::Game<'_>,
    entity: &mut crate::containers::ModBlockEntity,
    container_slots: usize,
    slot: usize,
    stack: &mut crate::items::ItemStack,
    handled: &mut bool,
) {
    if *handled {
        return;
    }
    crate::game::create_port_cannon::quick_move(&*game, entity, container_slots, slot, stack, handled);
    crate::game::create_port_schematic_table::quick_move(&*game, entity, container_slots, slot, stack, handled);
}
