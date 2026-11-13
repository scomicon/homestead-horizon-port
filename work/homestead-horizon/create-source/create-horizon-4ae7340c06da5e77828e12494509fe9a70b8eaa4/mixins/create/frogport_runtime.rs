// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

// Package Port blocks (`FrogportBlock` / `PostboxBlock`, `PackagePortBlockEntity`):
// an 18-slot package inventory with an address filter that pushes packages into the
// inventory below it, pulls packages from it, and ships packages to other ports. The
// state machine (animation, slot filter, address acceptance) lives in
// `create_engine::package_post`; this file adapts it to the game.
//
// Blocks: `create:package_frogport` and `create:package_postbox`, both without block
// state. The port acts on the block below it (a chest, a barrel, a Packager).
//
// Routing: Create sends a Frogport's packages along chain conveyors to another
// Frogport, and a Postbox's packages to trains. Neither chain conveyors nor trains are
// in this port yet, so the chain/station is replaced by a virtual network: every other
// port of the same kind within `RANGE` blocks is reachable, the nearest one that accepts
// the package's address and has room and is idle receives it. A Frogport only talks to
// Frogports and a Postbox only to Postboxes (`PackageTarget::can_support`). The target
// handed to the core is a placeholder of the matching variant.
//
// Differences from Create: no chain conveyor or train, no frog model or tongue, the
// delivery animation is a package rising or sinking above the port, no GUI (the address
// is set with `create stock address`), all pushes and pulls happen on a 10 tick period.
//
// A ticking block entity is out of the world map: this module reads and writes only the
// `entity` it is handed while it ticks, and other ports through `native_*` calls.

use super::{
    Game, create_port_contraption as contraption, create_port_package as package, create_port_packager as pk,
    native_block_entity_draw::BlockEntityDraw,
};
use crate::{
    containers::{BlockEntity, ModBlockEntity, ModContainer},
    items::ItemStack,
};
use create_engine::{
    package::{self as cp},
    package_post::{ANIMATION_SPEED, PORT_SLOTS, PackagePort, PackageSink, PackageTarget, PortEvent},
};
use horizon_assets::item_model::DisplayContext;
use horizon_client::text::Text;
use horizon_core::geometry::Direction as Face;
use horizon_items::item_nbt::{item_from_nbt, item_to_nbt};
use horizon_world::nbt::{Compound, Tag};

pub(super) const FROGPORT: &str = "create:package_frogport";
pub(super) const POSTBOX: &str = "create:package_postbox";
type Pos = [i32; 3];

const SLOTS: usize = PORT_SLOTS;
/// Reach of the virtual network, in blocks.
const RANGE: i32 = 32;
/// Period of pushing into / pulling from the inventory below.
const LAZY: i64 = 10;

fn is_port(id: &str) -> bool {
    id == FROGPORT || id == POSTBOX
}

fn simulates(game: &Game<'_>) -> bool {
    game.serving.is_some() || game.options.dedicated.is_some()
}

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

fn flag(nbt: &Compound, key: &str) -> bool {
    int(nbt, key) != 0
}

fn text(nbt: &Compound, key: &str) -> String {
    match get(nbt, key) {
        Some(Tag::String(value)) => value.to_string().unwrap_or_default(),
        _ => String::new(),
    }
}

fn center(pos: Pos) -> [f64; 3] {
    pos.map(|c| f64::from(c) + 0.5)
}

fn max_of<'a>(game: &'a Game<'_>) -> impl Fn(&str) -> u32 + 'a {
    move |id: &str| game.native_max_stack_size(id).unwrap_or(64)
}

// ------------------------------------------------------------ block entity

pub(super) fn default_nbt() -> Compound {
    Compound::from([("Accepts".into(), Tag::Byte(1))])
}

pub(super) fn entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if is_port(block) && entity.is_none() {
        *entity = Some((block.into(), default_nbt()));
    }
}

pub(super) fn ticks(block: &str, ticks: &mut bool) {
    if is_port(block) {
        *ticks = true;
    }
}

pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    if is_port(block) && container.is_none() {
        *container = Some(ModContainer {
            slots: SLOTS as _,
            faces: std::array::from_fn(|_| (0..SLOTS).collect()),
            inputs: (0..SLOTS).collect(),
            outputs: (0..SLOTS).collect(),
            menu: None,
        });
    }
}

/// What clients need to draw the package animation.
pub(super) fn update_tag(entity: &ModBlockEntity) -> Compound {
    ["Filter", "Accepts", "Anim", "AnimPkg", "Prog", "Depositing", "AnimStart"]
        .into_iter()
        .filter_map(|key| get(&entity.nbt, key).map(|tag| (key.into(), tag.clone())))
        .collect()
}

// -------------------------------------------------------------- state (NBT)

fn target_of(postbox: bool) -> PackageTarget {
    if postbox {
        PackageTarget::TrainStation { relative_pos: [0, 0, 0] }
    } else {
        PackageTarget::ChainConveyor { relative_pos: [0, 0, 0], chain_pos: 0.0, connection: None, flipped: false }
    }
}

fn load(reg: &mut package::Registry, entity: &ModBlockEntity, max: &dyn Fn(&str) -> u32, postbox: bool) -> PackagePort {
    let nbt = &entity.nbt;
    let mut port = PackagePort::new(postbox);
    for (index, slot) in entity.items.iter().enumerate().take(SLOTS) {
        port.inventory.slots[index] = slot
            .as_ref()
            .filter(|s| s.count > 0 && package::is_package(&s.item))
            .map(|s| package::to_core(reg, s, max));
    }
    port.address_filter = text(nbt, "Filter");
    port.accepts_packages = get(nbt, "Accepts").is_none() || flag(nbt, "Accepts");
    port.target = Some(target_of(postbox));
    port.animating = flag(nbt, "Anim");
    port.animation_progress = int(nbt, "Prog") as f32 / 1000.0;
    port.currently_depositing = flag(nbt, "Depositing");
    port.animated_package = get(nbt, "AnimPkg").and_then(item_from_nbt).map(|s| package::to_core(reg, &s, max));
    port
}

fn save(entity: &mut ModBlockEntity, reg: &package::Registry, port: &PackagePort, started_at: Option<i32>) {
    entity.items.resize(SLOTS, None);
    for (index, slot) in port.inventory.slots.iter().enumerate() {
        entity.items[index] = slot.as_ref().filter(|s| s.count > 0).map(|s| package::to_game(reg, s));
    }
    let nbt = &mut entity.nbt;
    nbt.insert("Anim".into(), Tag::Byte(i8::from(port.animating)));
    nbt.insert("Prog".into(), Tag::Int((port.animation_progress * 1000.0).round() as i32));
    nbt.insert("Depositing".into(), Tag::Byte(i8::from(port.currently_depositing)));
    match &port.animated_package {
        Some(p) => {
            nbt.insert("AnimPkg".into(), item_to_nbt(&package::to_game(reg, p)));
        }
        None => {
            nbt.remove(&"AnimPkg".into());
        }
    }
    if let Some(time) = started_at {
        nbt.insert("AnimStart".into(), Tag::Int(time));
    }
}

// ------------------------------------------------------------ virtual network

/// The nearest other port of the same kind that takes `package` right now: it accepts
/// the address, has a free slot and is not animating.
fn find_destination(game: &Game<'_>, from: Pos, postbox: bool, package: &cp::ItemStack) -> Option<Pos> {
    let kind = if postbox { POSTBOX } else { FROGPORT };
    let mut best: Option<(i64, Pos)> = None;
    for (key, entity) in game.world.block_entities.iter() {
        let BlockEntity::Mod(e) = entity else { continue };
        let at = [key.0, key.1, key.2];
        if e.id != kind || at == from {
            continue;
        }
        let d = [at[0] - from[0], at[1] - from[1], at[2] - from[2]];
        let dist = d.iter().map(|c| i64::from(*c) * i64::from(*c)).sum::<i64>();
        if dist > i64::from(RANGE) * i64::from(RANGE) {
            continue;
        }
        let accepts = get(&e.nbt, "Accepts").is_none() || flag(&e.nbt, "Accepts");
        if !accepts || !cp::match_address_stack(package, &text(&e.nbt, "Filter")) || flag(&e.nbt, "Anim") {
            continue;
        }
        let used = e.items.iter().flatten().filter(|s| s.count > 0).count();
        if used >= SLOTS {
            continue;
        }
        if best.is_none_or(|(d0, p0)| (dist, at) < (d0, p0)) {
            best = Some((dist, at));
        }
    }
    best.map(|(_, at)| at)
}

fn deliverable(game: &Game<'_>, from: Pos, postbox: bool, package: &cp::ItemStack) -> bool {
    find_destination(game, from, postbox, package).is_some()
}

/// `PackagePortTarget.export` for the virtual network: starts the receiving animation
/// on the destination port, which inserts the package when it ends.
struct Net<'a, 'g> {
    game: &'a mut Game<'g>,
    from: Pos,
    postbox: bool,
    reg: &'a package::Registry,
}

impl PackageSink for Net<'_, '_> {
    fn export(&mut self, _target: &PackageTarget, package: &cp::ItemStack, simulate: bool) -> bool {
        let Some(dest) = find_destination(self.game, self.from, self.postbox, package) else { return false };
        if simulate {
            return true;
        }
        let stack = package::to_game(self.reg, package);
        let nbt = item_to_nbt(&stack);
        let now = self.game.world.game_time as i32;
        let wrote = self
            .game
            .native_with_mod_block_entity(dest, |e| {
                e.nbt.insert("Anim".into(), Tag::Byte(1));
                e.nbt.insert("Prog".into(), Tag::Int(0));
                e.nbt.insert("Depositing".into(), Tag::Byte(0));
                e.nbt.insert("AnimPkg".into(), nbt);
                e.nbt.insert("AnimStart".into(), Tag::Int(now));
            })
            .is_some();
        if wrote {
            self.game.native_block_entity_sync(dest);
        }
        wrote
    }
}

// ------------------------------------------------------------------- tick

fn hand_over(game: &mut Game<'_>, stack: ItemStack, pos: Pos) {
    let uuid = game.native_acting_id();
    if let Some(rest) = game.native_give_stack(uuid, stack) {
        game.spawn_item(rest, center(pos), [0.0, 0.1, 0.0], 10);
    }
}

fn blip(game: &mut Game<'_>, pos: Pos) {
    game.native_play_sound_in_place("minecraft:entity.item_frame.add_item", center(pos), 4, 0.5, 1.2);
}

/// `tryPushingToAdjacentInventories` then `tryPullingFromOwnAndAdjacentInventories`
/// against the block below.
fn lazy(game: &mut Game<'_>, pos: Pos, postbox: bool, port: &mut PackagePort, reg: &mut package::Registry) {
    let dim = game.dim.id().to_owned();
    let below = pk::step(pos, Face::Down);
    let below_is_packager = game.native_block(below) == pk::BLOCK;
    let has_container = below_is_packager
        || game.native_container_slots(&dim, below, Face::Up).is_some_and(|slots| !slots.is_empty());

    // Push: every package goes into the inventory below (a Packager unpacks it).
    if has_container {
        for slot in 0..SLOTS {
            let Some(core) = port.inventory.slots[slot].clone() else { continue };
            let stack = package::to_game(reg, &core);
            let moved = if below_is_packager {
                pk::arm_insert(game, below, &stack, true) > 0 && pk::arm_insert(game, below, &stack, false) > 0
            } else {
                game.native_insert_item(below, Face::Up, stack).is_none()
            };
            if moved {
                port.inventory.slots[slot] = None;
            }
        }
    }
    if port.animating {
        return;
    }

    // Pull: one package that has somewhere to go. Own slots first (the ones this port
    // would not keep), then the inventory below.
    let filter = port.filter_string().map(str::to_owned);
    let mut picked: Option<cp::ItemStack> = None;
    for slot in 0..SLOTS {
        let Some(candidate) = port.inventory.slots[slot].clone() else { continue };
        let foreign = filter.as_deref().is_none_or(|f| !cp::match_address(&cp::get_address(&candidate), f));
        if foreign && deliverable(game, pos, postbox, &candidate) {
            port.inventory.slots[slot] = None;
            picked = Some(candidate);
            break;
        }
    }
    if picked.is_none() && has_container {
        let slots = game.native_container_slots(&dim, below, Face::Up).unwrap_or_default();
        for entry in slots {
            if !entry.can_take {
                continue;
            }
            let Some(stack) = entry.stack.clone().filter(|s| s.count > 0 && package::is_package(&s.item)) else {
                continue;
            };
            let core = {
                let max = max_of(game);
                package::to_core(reg, &stack, &max)
            };
            let foreign = below_is_packager
                || filter.as_deref().is_none_or(|f| !cp::match_address(&cp::get_address(&core), f));
            if !foreign || !deliverable(game, pos, postbox, &core) {
                continue;
            }
            if game.native_extract_item(&dim, below, Face::Up, entry.slot, 1).is_some() {
                picked = Some(core);
                break;
            }
        }
    }
    if let Some(core) = picked {
        let started = {
            let mut sink = Net { game: &mut *game, from: pos, postbox, reg: &*reg };
            port.start_animation(core.clone(), true, &mut sink)
        };
        if !started {
            if let Some(rest) = port.insert_package(core, false) {
                let stack = package::to_game(reg, &rest);
                game.spawn_item(stack, center(pos), [0.0, 0.2, 0.0], 10);
            }
        }
    }
}

pub(super) fn tick(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity) {
    if !is_port(&entity.id) || !simulates(game) || !game.native_on("create") || !contraption::resident(game, pos) {
        return;
    }
    let postbox = entity.id == POSTBOX;
    entity.items.resize(SLOTS, None);
    // Only packages belong in a port; anything else a hopper pushed in is spat out.
    for slot in entity.items.iter_mut() {
        if slot.as_ref().is_some_and(|s| s.count > 0 && !package::is_package(&s.item)) {
            if let Some(stray) = slot.take() {
                game.spawn_item(stray, center(pos), [0.0, 0.2, 0.0], 10);
            }
        }
    }
    let mut reg = package::Registry::default();
    let mut port = {
        let max = max_of(game);
        load(&mut reg, entity, &max, postbox)
    };
    let was = (port.inventory.slots.clone(), port.animating, port.currently_depositing);

    if port.animating {
        let events = {
            let mut sink = Net { game: &mut *game, from: pos, postbox, reg: &reg };
            port.tick(&mut sink)
        };
        for event in events {
            match event {
                PortEvent::PackageSent(_) | PortEvent::PackageReceived(_) => blip(game, pos),
                PortEvent::Dropped(boxed) => {
                    // A failed hand-over goes back into the port; a full port spills it.
                    if let Some(rest) = port.insert_package(boxed, false) {
                        let stack = package::to_game(&reg, &rest);
                        game.spawn_item(stack, center(pos), [0.0, 0.2, 0.0], 10);
                    }
                }
            }
        }
    }
    if !port.animating && (game.world.game_time + i64::from(pos[0] ^ pos[2])).rem_euclid(LAZY) == 0 {
        lazy(game, pos, postbox, &mut port, &mut reg);
    }

    let started = (!was.1 && port.animating).then_some(game.world.game_time as i32);
    save(entity, &reg, &port, started);
    let changed = was.0 != port.inventory.slots || was.1 != port.animating || was.2 != port.currently_depositing;
    if changed {
        game.native_block_entity_sync(pos);
    }
}

// ----------------------------------------------------------- interaction

fn announce(game: &mut Game<'_>, line: String) {
    let player = game.native_acting_id();
    game.native_action_bar(player, Text::literal(&line));
}

/// One-line state of the port at `pos` (None when there is none).
pub(super) fn status_line(game: &Game<'_>, pos: Pos) -> Option<String> {
    match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
        Some(BlockEntity::Mod(e)) if is_port(&e.id) => {
            let packages = e.items.iter().flatten().filter(|s| s.count > 0).count();
            let filter = if get(&e.nbt, "Accepts").is_some_and(|_| !flag(&e.nbt, "Accepts")) {
                "accepts nothing".to_owned()
            } else {
                format!("address filter \"{}\"", text(&e.nbt, "Filter"))
            };
            let kind = if e.id == POSTBOX { "Package Postbox" } else { "Package Frogport" };
            Some(format!("{kind}: {filter}, {packages}/{SLOTS} package(s){}", if flag(&e.nbt, "Anim") { ", animating" } else { "" }))
        }
        _ => None,
    }
}

/// `create stock address`: `none` stops acceptance, `clear` empties the filter, `all` is
/// `*`, a `+` is a `*`.
pub(super) fn set_address(game: &mut Game<'_>, pos: Pos, word: &str) -> Result<String, String> {
    let (accepts, filter) = match word {
        "none" => (false, None),
        "clear" => (true, Some(String::new())),
        "all" => (true, Some("*".to_owned())),
        other => (true, Some(other.replace('+', "*"))),
    };
    let done = game
        .native_with_mod_block_entity(pos, |e| {
            if !is_port(&e.id) {
                return false;
            }
            e.nbt.insert("Accepts".into(), Tag::Byte(i8::from(accepts)));
            if let Some(filter) = filter {
                e.nbt.insert("Filter".into(), Tag::String(filter.as_str().into()));
            }
            true
        })
        .unwrap_or(false);
    if !done {
        return Err("no Frogport or Postbox there".into());
    }
    game.native_block_entity_sync(pos);
    status_line(game, pos).ok_or_else(|| "no Frogport or Postbox there".to_owned())
}

/// Right click on a port. A package in hand goes into the first free slot; an empty hand
/// takes the last package; sneaking with an empty hand shows the port's state.
pub(super) fn right_click(game: &mut Game<'_>, pos: Pos) -> bool {
    if !simulates(game)
        || !game.native_on("create")
        || game.held_slot() != game.selected
        || !game.may_build()
        || !is_port(&game.native_block(pos))
    {
        return false;
    }
    let sneaking = game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft);
    let held = game.held().filter(|s| s.count > 0).cloned();
    match held {
        Some(stack) if package::is_package(&stack.item) && !sneaking => {
            let mut one = stack.clone();
            one.count = 1;
            let inserted = game
                .native_with_mod_block_entity(pos, |e| {
                    e.items.resize(SLOTS, None);
                    match e.items.iter_mut().find(|slot| slot.is_none()) {
                        Some(slot) => {
                            *slot = Some(one);
                            true
                        }
                        None => false,
                    }
                })
                .unwrap_or(false);
            if inserted {
                game.consume_held();
                game.native_block_entity_sync(pos);
            }
            inserted
        }
        Some(_) => false,
        None if sneaking => {
            let line = status_line(game, pos).unwrap_or_default();
            announce(game, line);
            true
        }
        None => {
            let taken = game
                .native_with_mod_block_entity(pos, |e| {
                    e.items.resize(SLOTS, None);
                    e.items.iter_mut().rev().find_map(|slot| slot.take())
                })
                .flatten();
            match taken {
                Some(stack) => {
                    hand_over(game, stack, pos);
                    game.native_block_entity_sync(pos);
                    true
                }
                None => false,
            }
        }
    }
}

/// The engine drops the container's item slots; a package in mid-air is dropped here
/// (not for a Postbox that already handed it over).
pub(super) fn destroyed(game: &mut Game<'_>, pos: Pos) {
    let block = game.native_block(pos);
    if !is_port(&block) {
        return;
    }
    let mut dropped: Option<ItemStack> = None;
    game.native_with_mod_block_entity(pos, |e| {
        let handed_over = e.id == POSTBOX && flag(&e.nbt, "Depositing");
        if flag(&e.nbt, "Anim") && !handed_over {
            dropped = get(&e.nbt, "AnimPkg").and_then(item_from_nbt);
        }
        e.nbt.remove(&"AnimPkg".into());
        e.nbt.insert("Anim".into(), Tag::Byte(0));
    });
    if let Some(stack) = dropped {
        game.spawn_item(stack, center(pos), [0.0, 0.2, 0.0], 10);
    }
}

// ------------------------------------------------------------------- draw

/// The package in flight rises above the port when it leaves and sinks onto it when it arrives.
pub(super) fn draw(game: &Game<'_>, _pos: Pos, entity: &ModBlockEntity, draw: &mut BlockEntityDraw) {
    let nbt = &entity.nbt;
    if !flag(nbt, "Anim") {
        return;
    }
    let Some(stack) = get(nbt, "AnimPkg").and_then(item_from_nbt) else { return };
    let elapsed = (draw.game_time as i32).wrapping_sub(int(nbt, "AnimStart")).clamp(0, 64) as f32;
    let progress = ((elapsed + draw.partial) * ANIMATION_SPEED).clamp(0.0, 1.0);
    let depositing = flag(nbt, "Depositing");
    if depositing && progress > 0.5 {
        return;
    }
    let y = if depositing { 0.75 + progress * 1.2 } else { 1.35 - progress * 0.6 };
    draw.push();
    draw.translate([0.5, y, 0.5]);
    draw.scale([0.5; 3]);
    game.native_draw_item(draw, &stack, DisplayContext::Fixed, None, 0);
    draw.pop();
}
