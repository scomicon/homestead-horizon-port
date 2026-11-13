// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

// Factory Gauge, local monitor only (`FactoryPanelBlock` / `FactoryPanelBehaviour`).
// The rules (storage monitor, satisfied / promised-satisfied / waiting, demand in
// items or stacks) live in `create_engine::factory_gauge`; this file adapts them.
//
// What exists: one gauge per block, attached to the inventory behind it (`facing` points
// away from it). It reads the item it counts (right click with the item), compares the
// stock with a target amount and drives the `satisfied` state, which powers its
// neighbours with 15 while satisfied (polarity unverified, see docs/logistics.md).
// The promise queue is empty: with no Stock Link network there is nothing to promise,
// no request is ever made, `waiting` means "no inventory behind".
//
// Interaction (the value-settings screen is not ported):
//   right click with an item   -> sets the counted item
//   right click, empty hand    -> target amount +10 (0, 10 .. 100, then 0 again)
//   sneak + empty hand         -> toggles items / stacks
//
// A ticking block entity is out of the world map; the entity is only touched through
// the `entity` argument while it ticks.

use super::{
    Game, GameLevel, create_port_contraption as contraption, create_port_package as package,
    create_port_packager as packager, native_block_entity_draw::BlockEntityDraw,
};
use crate::containers::{BlockEntity, ModBlockEntity};
use create_engine::{
    factory_gauge::{self as gauge, FactoryPanel, GaugeEnv, GaugeEvent, PanelKey, SourcePanel},
    package::{self as cp, OrderWithCrafts},
    packager::RequestType,
    stock_network::{FreqId, InventorySummary, PackagerRequests, RequestPromiseQueue},
};
use horizon_assets::item_model::DisplayContext;
use horizon_client::text::Text;
use horizon_core::{geometry::Direction as Face, position::BlockPos};
use horizon_items::item_nbt::{item_from_nbt, item_to_nbt};
use horizon_world::{
    nbt::{Compound, Tag},
    registry::BlockStateId,
};

pub(super) const BLOCK: &str = "create:factory_gauge";
type Pos = [i32; 3];
const PERIOD: i64 = 5;

fn get<'a>(nbt: &'a Compound, key: &str) -> Option<&'a Tag> {
    nbt.get(&key.into())
}

fn int(nbt: &Compound, key: &str) -> i32 {
    match get(nbt, key) {
        Some(Tag::Int(v)) => *v,
        Some(Tag::Byte(v)) => i32::from(*v),
        _ => 0,
    }
}

fn truth(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

pub(super) fn default_nbt() -> Compound {
    Compound::from([
        ("Count".into(), Tag::Int(0)),
        ("UpTo".into(), Tag::Byte(1)),
        ("Satisfied".into(), Tag::Byte(0)),
    ])
}

pub(super) fn entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if block == BLOCK && entity.is_none() {
        *entity = Some((block.into(), default_nbt()));
    }
}

pub(super) fn ticks(block: &str, ticks: &mut bool) {
    if block == BLOCK {
        *ticks = true;
    }
}

pub(super) fn update_tag(entity: &ModBlockEntity) -> Compound {
    ["Filter", "Count", "UpTo", "Satisfied", "Waiting"]
        .into_iter()
        .filter_map(|key| get(&entity.nbt, key).map(|tag| (key.into(), tag.clone())))
        .collect()
}

/// The world as the gauge sees it: the inventory behind it stands in for the whole
/// network; nothing can be requested.
struct Local {
    summary: InventorySummary,
    inventory: bool,
}

impl GaugeEnv for Local {
    fn summary(&mut self, _freq: FreqId, _accurate: bool) -> InventorySummary {
        self.summary.clone()
    }
    fn invalidate_summary(&mut self, _freq: FreqId) {}
    fn unloaded_link_count(&self, _freq: FreqId) -> usize {
        usize::from(!self.inventory)
    }
    fn source(&self, _key: &PanelKey) -> Option<SourcePanel> {
        None
    }
    fn find_packagers(&mut self, _freq: FreqId, _order: &OrderWithCrafts, _address: &str) -> PackagerRequests {
        Vec::new()
    }
    fn is_too_busy(&self, _packager: u32, _ty: RequestType) -> bool {
        false
    }
    fn perform(&mut self, _requests: PackagerRequests) {}
    fn network_promises(&mut self, _freq: FreqId) -> Option<&mut RequestPromiseQueue> {
        None
    }
    fn restocked_packager(&mut self) -> Option<InventorySummary> {
        None
    }
    fn stock_of(&mut self, _freq: FreqId, _stack: &cp::ItemStack) -> u32 {
        0
    }
    fn broadcast_restock(&mut self, _freq: FreqId, _order: &OrderWithCrafts, _address: &str) -> bool {
        false
    }
}

fn facing_of(game: &Game<'_>, pos: Pos) -> Option<Face> {
    let name = game.native_block_property(pos, "facing")?;
    [Face::Down, Face::Up, Face::North, Face::South, Face::West, Face::East]
        .into_iter()
        .find(|face| match face {
            Face::Down => name == "down",
            Face::Up => name == "up",
            Face::North => name == "north",
            Face::South => name == "south",
            Face::West => name == "west",
            Face::East => name == "east",
        })
}

pub(super) fn tick(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity) {
    if entity.id != BLOCK
        || !(game.serving.is_some() || game.options.dedicated.is_some())
        || !game.native_on("create")
        || !contraption::resident(game, pos)
        || (game.world.game_time + i64::from(pos[0] ^ pos[2])).rem_euclid(PERIOD) != 0
    {
        return;
    }
    let Some(facing) = facing_of(game, pos) else { return };
    let dim = game.dim.id().to_owned();
    let behind = packager::step(pos, facing.opposite());
    let mut reg = package::Registry::default();
    let max = |id: &str| game.native_max_stack_size(id).unwrap_or(64);
    let filter = get(&entity.nbt, "Filter")
        .and_then(item_from_nbt)
        .map(|stack| package::to_core(&mut reg, &stack, &max));
    let slots = game.native_container_slots(&dim, behind, facing);
    let mut env = Local { summary: InventorySummary::new(), inventory: slots.is_some() };
    for slot in slots.iter().flatten() {
        if let Some(stack) = slot.stack.as_ref().filter(|s| s.count > 0) {
            let counted = package::to_core(&mut reg, stack, &max);
            env.summary.add(&counted, stack.count);
        }
    }
    let was_satisfied = int(&entity.nbt, "Satisfied") != 0;
    let mut panel = FactoryPanel::new(0);
    panel.filter = filter;
    panel.count = int(&entity.nbt, "Count").max(0) as u32;
    panel.up_to = int(&entity.nbt, "UpTo") != 0;
    panel.satisfied = was_satisfied;
    panel.promised_satisfied = int(&entity.nbt, "PromisedSatisfied") != 0;
    panel.waiting_for_network = int(&entity.nbt, "Waiting") != 0;
    panel.tick(&mut env);
    if panel.filter.is_none() {
        // An unconfigured gauge stays dark instead of counting as satisfied.
        panel.satisfied = false;
        panel.promised_satisfied = false;
    }

    let events = std::mem::take(&mut panel.events);
    let changed = (panel.satisfied != was_satisfied)
        || panel.promised_satisfied != (int(&entity.nbt, "PromisedSatisfied") != 0)
        || panel.waiting_for_network != (int(&entity.nbt, "Waiting") != 0);
    if !changed {
        return;
    }
    for (key, value) in [
        ("Satisfied", panel.satisfied),
        ("PromisedSatisfied", panel.promised_satisfied),
        ("Waiting", panel.waiting_for_network),
    ] {
        entity.nbt.insert(key.into(), Tag::Byte(i8::from(value)));
    }
    if events.contains(&GaugeEvent::ConfirmSounds) {
        // Stand-in for `AllSoundEvents.CONFIRM` (two notes), whose file was not read.
        let at = pos.map(|c| f64::from(c) + 0.5);
        game.native_play_sound_in_place("minecraft:block.note_block.chime", at, 4, 0.5, 1.0);
        game.native_play_sound_in_place("minecraft:block.note_block.chime", at, 4, 0.5, 1.5);
    }
    if events.contains(&GaugeEvent::RedstoneOutputsChanged) {
        game.native_set_block_property(pos, "satisfied", truth(panel.satisfied));
        game.native_update_signal_neighbors(&dim, pos);
    }
    game.native_block_entity_sync(pos);
}

// ----------------------------------------------------------- interaction

fn status(game: &mut Game<'_>, text: String) {
    let player = game.native_acting_id();
    game.native_action_bar(player, Text::literal(&text));
}

pub(super) fn right_click(game: &mut Game<'_>, pos: Pos) -> bool {
    if !(game.serving.is_some() || game.options.dedicated.is_some())
        || !game.native_on("create")
        || game.held_slot() != game.selected
        || !game.may_build()
        || game.native_block(pos) != BLOCK
    {
        return false;
    }
    let sneaking = game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft);
    let held = game.held().filter(|s| s.count > 0).cloned();
    let line = match held {
        Some(_) if sneaking => return false,
        Some(mut stack) => {
            stack.count = 1;
            let name = stack.item.to_string();
            game.native_with_mod_block_entity(pos, |e| {
                e.nbt.insert("Filter".into(), item_to_nbt(&stack));
                // Re-evaluate from scratch: the new item may or may not be in stock.
                e.nbt.insert("Satisfied".into(), Tag::Byte(0));
            });
            game.native_set_block_property(pos, "satisfied", "false");
            format!("Factory Gauge: {name}")
        }
        None => {
            let result = game.native_with_mod_block_entity(pos, |e| {
                if sneaking {
                    let up_to = int(&e.nbt, "UpTo") == 0;
                    e.nbt.insert("UpTo".into(), Tag::Byte(i8::from(up_to)));
                } else {
                    let count = int(&e.nbt, "Count");
                    let step = gauge::TARGET_STEP as i32;
                    let max = gauge::MAX_TARGET_AMOUNT as i32;
                    e.nbt.insert("Count".into(), Tag::Int(if count >= max { 0 } else { count + step }));
                }
                (int(&e.nbt, "Count"), int(&e.nbt, "UpTo") != 0)
            });
            let Some((count, up_to)) = result else { return false };
            format!("Factory Gauge: {count} {}", if up_to { "items" } else { "stacks" })
        }
    };
    status(game, line);
    game.native_block_entity_sync(pos);
    true
}

pub(super) fn placed(game: &mut Game<'_>, pos: Pos) {
    if !(game.serving.is_some() || game.options.dedicated.is_some()) || game.native_block(pos) != BLOCK {
        return;
    }
    let facing = packager::choose_facing(game, pos);
    let name = match facing {
        Face::Down => "down",
        Face::Up => "up",
        Face::North => "north",
        Face::South => "south",
        Face::West => "west",
        Face::East => "east",
    };
    game.native_set_block_property(pos, "facing", name);
    game.native_block_entity_sync(pos);
}

// -------------------------------------------------------------- signal

/// `block_signal` hook: 15 on every side while satisfied.
pub(super) fn signal(
    game: &GameLevel<'_>,
    _pos: BlockPos,
    state: BlockStateId,
    _face: Face,
    direct: bool,
    signal: &mut i32,
) {
    let Some(block) = game.registry.state(state).filter(|block| &*block.name == BLOCK) else {
        return;
    };
    let lit = block.properties.get("satisfied").is_some_and(|v| &**v == "true");
    *signal = if lit && !direct { 15 } else { 0 };
}

/// `is_signal_source` hook.
pub(super) fn signal_source(game: &GameLevel<'_>, state: BlockStateId, source: &mut bool) {
    let Some(block) = game.registry.state(state).filter(|block| &*block.name == BLOCK) else {
        return;
    };
    *source = block.properties.get("satisfied").is_some_and(|v| &**v == "true");
}

// ----------------------------------------------------------------- draw

/// The counted item lies on the plate (drawn for `up`, then turned to `facing`).
pub(super) fn draw(game: &Game<'_>, pos: Pos, entity: &ModBlockEntity, draw: &mut BlockEntityDraw) {
    let Some(stack) = get(&entity.nbt, "Filter").and_then(item_from_nbt) else { return };
    draw.push();
    match game.native_block_property(pos, "facing").as_deref() {
        Some("down") => draw.rotate_about([1.0, 0.0, 0.0], 180.0, [0.5; 3]),
        Some("north") => draw.rotate_about([1.0, 0.0, 0.0], 90.0, [0.5; 3]),
        Some("south") => {
            draw.rotate_about([0.0, 1.0, 0.0], 180.0, [0.5; 3]);
            draw.rotate_about([1.0, 0.0, 0.0], 90.0, [0.5; 3]);
        }
        Some("west") => {
            draw.rotate_about([0.0, 1.0, 0.0], 270.0, [0.5; 3]);
            draw.rotate_about([1.0, 0.0, 0.0], 90.0, [0.5; 3]);
        }
        Some("east") => {
            draw.rotate_about([0.0, 1.0, 0.0], 90.0, [0.5; 3]);
            draw.rotate_about([1.0, 0.0, 0.0], 90.0, [0.5; 3]);
        }
        _ => {}
    }
    draw.translate([0.5, 0.2, 0.5]);
    draw.rotate_about([1.0, 0.0, 0.0], 90.0, [0.0; 3]);
    draw.scale([0.4; 3]);
    game.native_draw_item(draw, &stack, DisplayContext::Fixed, None, 0);
    draw.pop();
}

/// Read helper for acceptance tooling.
#[allow(dead_code, reason = "read helper for acceptance tooling")]
pub(super) fn satisfied(game: &Game<'_>, pos: Pos) -> Option<bool> {
    match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
        Some(BlockEntity::Mod(e)) if e.id == BLOCK => Some(int(&e.nbt, "Satisfied") != 0),
        _ => None,
    }
}
