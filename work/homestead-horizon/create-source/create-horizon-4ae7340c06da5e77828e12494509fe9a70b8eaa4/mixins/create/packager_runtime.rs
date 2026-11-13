// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

// Packager (`PackagerBlock` / `PackagerBlockEntity`): packs the items of the inventory
// behind it into a package, unpacks packages into that inventory, redstone triggered.
// The rules live in `create_engine::packager`; this file adapts them to the game.
//
// Block state: `facing` (the face the box leaves by; the inventory it works on is the
// block on the opposite side, `FACING.getOpposite()` in Create).
//
// How the core is fed: the inventory behind is read into a `Snapshot` (an
// `ItemHandler` over a copy of its slots); the core runs on the copy; the decreases
// are then applied to the real inventory with `native_extract_item`, and unpacked
// contents are inserted with `native_insert_item` (what does not fit is dropped).
//
// Container: two slots on every face. Slot 0 takes packages (hoppers, chutes, an arm,
// the player): they are unpacked into the inventory behind. Slot 1 shows the finished
// box when no animation runs; whatever takes it (hopper, arm, player) ends the hold.
// Differences from Create (see docs/logistics.md): a finished box is also pushed into
// an inventory in front, and dropped as an item after `EJECT_DELAY` ticks if the
// front is open air; Create keeps the box until something takes it or a Frogport
// does. No Stock Link, no request mode, `linked` is always false.
//
// A ticking block entity is out of the world map: this module reads and writes only
// the `entity` it is handed, and other block entities through `native_*` calls.

use super::{
    Game, create_port_contraption as contraption, create_port_package as package,
    native_block_entity_draw::BlockEntityDraw,
};
use crate::{
    containers::{BlockEntity, ModBlockEntity, ModContainer},
    items::ItemStack,
};
use create_engine::{
    package::{self as cp, ItemHandler, SimpleInventory, XorShift},
    packager::{self, PackagerState},
};
use horizon_assets::item_model::DisplayContext;
use horizon_core::{geometry::Direction as Face, random::RandomSource};
use horizon_items::item_nbt::{item_from_nbt, item_to_nbt};
use horizon_world::nbt::{Compound, Tag};

pub(super) const BLOCK: &str = "create:packager";
type Pos = [i32; 3];

const IN: usize = 0;
const OUT: usize = 1;
const SYNC_TICK: &str = "create_horizon:PackagerSync";
const MIRRORED: &str = "create_horizon:Mirrored";
const HELD_FOR: &str = "create_horizon:HeldFor";
/// Ticks a finished box waits in front of open air before it is dropped.
const EJECT_DELAY: i32 = 60;

const ALL: [Face; 6] = [Face::Down, Face::Up, Face::North, Face::South, Face::West, Face::East];

fn face_name(face: Face) -> &'static str {
    match face {
        Face::Down => "down",
        Face::Up => "up",
        Face::North => "north",
        Face::South => "south",
        Face::West => "west",
        Face::East => "east",
    }
}

fn face_named(name: &str) -> Option<Face> {
    ALL.into_iter().find(|face| face_name(*face) == name)
}

pub(super) fn step(pos: Pos, face: Face) -> Pos {
    let n = face.normal();
    [pos[0] + n[0], pos[1] + n[1], pos[2] + n[2]]
}

fn simulates(game: &Game<'_>) -> bool {
    game.serving.is_some() || game.options.dedicated.is_some()
}

// ------------------------------------------------------------------- NBT

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

pub(super) fn default_nbt() -> Compound {
    Compound::from([("AnimTicks".into(), Tag::Int(0))])
}

/// What clients need to draw the box animation.
pub(super) fn update_tag(entity: &ModBlockEntity) -> Compound {
    ["HeldBox", "PrevUnwrapped", "AnimTicks", "AnimInward", SYNC_TICK]
        .into_iter()
        .filter_map(|key| get(&entity.nbt, key).map(|tag| (key.into(), tag.clone())))
        .collect()
}

fn set_box(nbt: &mut Compound, key: &str, stack: Option<ItemStack>) {
    match stack {
        Some(stack) => {
            nbt.insert(key.into(), item_to_nbt(&stack));
        }
        None => {
            nbt.remove(&key.into());
        }
    }
}

fn load(reg: &mut package::Registry, nbt: &Compound, max: &dyn Fn(&str) -> u32) -> PackagerState {
    let core_box = |key: &str, reg: &mut package::Registry| {
        get(nbt, key).and_then(item_from_nbt).map(|stack| package::to_core(reg, &stack, max))
    };
    let mut st = PackagerState {
        held_box: core_box("HeldBox", reg),
        previously_unwrapped: core_box("PrevUnwrapped", reg),
        animation_ticks: int(nbt, "AnimTicks"),
        animation_inward: flag(nbt, "AnimInward"),
        button_cooldown: int(nbt, "Cooldown"),
        redstone_powered: flag(nbt, "RedstonePowered"),
        ..PackagerState::default()
    };
    if let Some(Tag::String(text)) = get(nbt, "SignAddress") {
        st.sign_address = text.to_string().unwrap_or_default();
    }
    if let Some(Tag::List(list)) = get(nbt, "Queue") {
        for tag in list {
            if let Some(stack) = item_from_nbt(tag) {
                st.queued_exiting.push((package::to_core(reg, &stack, max), 1));
            }
        }
    }
    st
}

fn save(reg: &package::Registry, st: &PackagerState, nbt: &mut Compound) {
    set_box(nbt, "HeldBox", st.held_box.as_ref().map(|b| package::to_game(reg, b)));
    set_box(nbt, "PrevUnwrapped", st.previously_unwrapped.as_ref().map(|b| package::to_game(reg, b)));
    nbt.insert("AnimTicks".into(), Tag::Int(st.animation_ticks));
    nbt.insert("AnimInward".into(), Tag::Byte(i8::from(st.animation_inward)));
    nbt.insert("Cooldown".into(), Tag::Int(st.button_cooldown));
    nbt.insert("RedstonePowered".into(), Tag::Byte(i8::from(st.redstone_powered)));
    nbt.insert("SignAddress".into(), Tag::String(st.sign_address.as_str().into()));
    let queue = st.queued_exiting.iter().map(|(b, _)| item_to_nbt(&package::to_game(reg, b))).collect();
    nbt.insert("Queue".into(), Tag::List(queue));
}

// -------------------------------------------------------------- inventory

/// A copy of the target inventory's slots, with the real slot ids and whether a
/// slot may be taken from through the working face.
#[derive(Clone)]
struct Snapshot {
    inv: SimpleInventory,
    ids: Vec<usize>,
    takeable: Vec<bool>,
}

impl ItemHandler for Snapshot {
    fn slot_count(&self) -> usize {
        self.inv.slot_count()
    }
    fn stack_in_slot(&self, slot: usize) -> Option<&cp::ItemStack> {
        self.inv.stack_in_slot(slot)
    }
    fn extract(&mut self, slot: usize, amount: u32, simulate: bool) -> Option<cp::ItemStack> {
        if !self.takeable.get(slot).copied().unwrap_or(false) {
            return None;
        }
        self.inv.extract(slot, amount, simulate)
    }
    fn insert(&mut self, slot: usize, stack: cp::ItemStack, simulate: bool) -> Option<cp::ItemStack> {
        self.inv.insert(slot, stack, simulate)
    }
    fn duplicate(&self) -> Box<dyn ItemHandler> {
        Box::new(self.clone())
    }
}

fn max_of<'a>(game: &'a Game<'_>) -> impl Fn(&str) -> u32 + 'a {
    move |id: &str| game.native_max_stack_size(id).unwrap_or(64)
}

/// Runs `f` on a snapshot of the inventory at `target` (read through its side `face`)
/// and applies the extractions it made. `f` gets `None` when there is no inventory.
fn with_target<R>(
    game: &mut Game<'_>,
    target: Pos,
    face: Face,
    reg: &mut package::Registry,
    f: impl FnOnce(Option<&mut dyn ItemHandler>) -> R,
) -> R {
    let dim = game.dim.id().to_owned();
    let Some(slots) = game.native_container_slots(&dim, target, face).filter(|s| !s.is_empty()) else {
        return f(None);
    };
    let mut snap = {
        let max = max_of(game);
        Snapshot {
            inv: SimpleInventory {
                slots: slots
                    .iter()
                    .map(|e| e.stack.as_ref().filter(|s| s.count > 0).map(|s| package::to_core(reg, s, &max)))
                    .collect(),
                limit: cp::DEFAULT_SLOT_LIMIT,
                bottomless: false,
            },
            ids: slots.iter().map(|e| e.slot).collect(),
            takeable: slots.iter().map(|e| e.can_take).collect(),
        }
    };
    let before: Vec<u32> = snap.inv.slots.iter().map(|s| s.as_ref().map_or(0, |s| s.count)).collect();
    let result = f(Some(&mut snap as &mut dyn ItemHandler));
    for (index, was) in before.iter().enumerate() {
        let now = snap.inv.slots[index].as_ref().map_or(0, |s| s.count);
        if *was > now {
            game.native_extract_item(&dim, target, face, snap.ids[index], *was - now);
        }
    }
    result
}

fn sign_sides(game: &Game<'_>, pos: Pos) -> Vec<String> {
    ALL.into_iter()
        .map(|face| {
            let at = step(pos, face);
            match game.world.block_entities.get(&(at[0], at[1], at[2])) {
                Some(BlockEntity::Sign(sign)) => {
                    let front: Vec<&str> = sign.front.lines.iter().map(String::as_str).collect();
                    let back: Vec<&str> = sign.back.lines.iter().map(String::as_str).collect();
                    packager::sign_text(&front, &back)
                }
                _ => String::new(),
            }
        })
        .collect()
}

fn facing_of(game: &Game<'_>, pos: Pos) -> Option<Face> {
    game.native_block_property(pos, "facing").as_deref().and_then(face_named)
}

fn center(pos: Pos) -> [f64; 3] {
    pos.map(|c| f64::from(c) + 0.5)
}

// ---------------------------------------------------------------- unpack

/// `unwrapBox` into the inventory at `target`. True when the package was taken (the
/// animation started and the contents went in; leftovers are dropped at `pos`).
fn unpack(
    game: &mut Game<'_>,
    pos: Pos,
    target: Pos,
    face: Face,
    reg: &mut package::Registry,
    st: &mut PackagerState,
    boxed: &cp::ItemStack,
) -> bool {
    let ok = with_target(game, target, face, reg, |tgt| match tgt {
        Some(t) => st.unwrap_box(t, boxed, true) && st.unwrap_box(t, boxed, false),
        None => false,
    });
    if !ok {
        return false;
    }
    for content in cp::contents_stacks(boxed) {
        let stack = package::to_game(reg, &content);
        if let Some(rest) = game.native_insert_item(target, face, stack) {
            game.spawn_item(rest, center(pos), [0.0, 0.2, 0.0], 10);
        }
    }
    true
}

// ------------------------------------------------------------------ tick

pub(super) fn tick(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity) {
    if entity.id != BLOCK || !simulates(game) || !game.native_on("create") || !contraption::resident(game, pos) {
        return;
    }
    let Some(facing) = facing_of(game, pos) else { return };
    let dim = game.dim.id().to_owned();
    let behind = step(pos, facing.opposite());
    let mut reg = package::Registry::default();
    let mut st = {
        let max = max_of(game);
        load(&mut reg, &entity.nbt, &max)
    };
    let before = (
        st.held_box.clone(),
        st.previously_unwrapped.clone(),
        st.animation_ticks,
        st.animation_inward,
        st.queued_exiting.len(),
    );
    entity.items.resize(2, None);
    // Something took the shown box.
    if flag(&entity.nbt, MIRRORED) && entity.items[OUT].is_none() && st.held_box.is_some() {
        st.held_box = None;
    }
    st.tick();

    // Packages in the input slot are unpacked; anything else is passed on to the
    // inventory behind.
    if let Some(stack) = entity.items[IN].clone().filter(|s| s.count > 0) {
        if package::is_package(&stack.item) {
            if st.animation_ticks == 0 {
                let boxed = {
                    let max = max_of(game);
                    package::to_core(&mut reg, &stack, &max)
                };
                if unpack(game, pos, behind, facing, &mut reg, &mut st, &boxed) {
                    entity.items[IN] = None;
                }
            }
        } else {
            entity.items[IN] = game.native_insert_item(behind, facing, stack);
        }
    }

    // Redstone: a rising edge packs (`activate`), a lazy tick re-sends while powered.
    let powered = game.native_neighbor_signal(&dim, pos) > 0;
    let seed = (game.world.game_time as u64) ^ ((pos[0] as u64) << 32) ^ (pos[2] as u64);
    let mut rng = XorShift(seed);
    let signs = sign_sides(game, pos);
    let lazy = (game.world.game_time + i64::from(pos[0] ^ pos[2])).rem_euclid(10) == 0;
    if powered && !st.redstone_powered {
        let report = with_target(game, behind, facing, &mut reg, |tgt| st.activate(tgt, &signs, &mut rng));
        let _ = report;
    } else if lazy && st.redstone_powered {
        let report = with_target(game, behind, facing, &mut reg, |tgt| st.lazy_tick(powered, tgt, &signs, &mut rng));
        let _ = report;
    }
    // New boxes get a random standard style, as `PackageStyles.getRandomBox` does.
    if st.held_box.as_ref().is_some_and(|b| b.key.id == packager::DEFAULT_STYLE) && st.animation_ticks == packager::CYCLE {
        let style = package::STYLES[(game.world.random.next_float() * 4.0) as usize % 4];
        if let Some(b) = st.held_box.as_mut() {
            b.key.id = style.to_owned();
        }
    }

    // Finished box: push it into an inventory in front, drop it after a delay in
    // open air, otherwise keep it for whatever takes it.
    let mut held_for = int(&entity.nbt, HELD_FOR);
    let idle_box = st.held_box.clone().filter(|_| st.animation_ticks == 0);
    if let Some(idle_box) = idle_box {
        let boxed = package::to_game(&reg, &idle_box);
        let front = step(pos, facing);
        let has_inventory = game
            .native_container_slots(&dim, front, facing.opposite())
            .is_some_and(|slots| !slots.is_empty());
        if has_inventory && game.native_insert_item(front, facing.opposite(), boxed.clone()).is_none() {
            st.held_box = None;
            held_for = 0;
        } else if !has_inventory && game.native_block(front) == "minecraft:air" {
            held_for += 1;
            if held_for >= EJECT_DELAY {
                let n = facing.normal();
                let at = [
                    f64::from(pos[0]) + 0.5 + f64::from(n[0]) * 0.7,
                    f64::from(pos[1]) + 0.5 + f64::from(n[1]) * 0.7,
                    f64::from(pos[2]) + 0.5 + f64::from(n[2]) * 0.7,
                ];
                game.spawn_item(boxed, at, [f64::from(n[0]) * 0.15, f64::from(n[1]) * 0.15 + 0.1, f64::from(n[2]) * 0.15], 20);
                st.held_box = None;
                held_for = 0;
            }
        } else {
            held_for = 0;
        }
    } else {
        held_for = 0;
    }
    if int(&entity.nbt, HELD_FOR) != held_for {
        entity.nbt.insert(HELD_FOR.into(), Tag::Int(held_for));
    }
    let shown = (st.animation_ticks == 0).then(|| st.held_box.as_ref().map(|b| package::to_game(&reg, b))).flatten();
    entity.nbt.insert(MIRRORED.into(), Tag::Byte(i8::from(shown.is_some())));
    entity.items[OUT] = shown;

    let after = (
        st.held_box.clone(),
        st.previously_unwrapped.clone(),
        st.animation_ticks,
        st.animation_inward,
        st.queued_exiting.len(),
    );
    save(&reg, &st, &mut entity.nbt);
    if before != after && (before.0 != after.0 || before.1 != after.1 || before.3 != after.3 || before.4 != after.4) {
        entity.nbt.insert(SYNC_TICK.into(), Tag::Int(game.world.game_time as i32));
        game.native_block_entity_sync(pos);
    }
}

// ----------------------------------------------------------- container

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

pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    if block == BLOCK && container.is_none() {
        *container = Some(ModContainer {
            slots: 2,
            faces: std::array::from_fn(|_| vec![IN, OUT]),
            inputs: vec![IN],
            outputs: vec![OUT],
            menu: None,
        });
    }
}

// --------------------------------------------------- interaction, placement

/// Facing for a freshly placed packager: away from an adjacent inventory (the box
/// leaves on the other side); otherwise toward the player.
pub(super) fn choose_facing(game: &mut Game<'_>, pos: Pos) -> Face {
    let dim = game.dim.id().to_owned();
    for dir in [Face::North, Face::South, Face::West, Face::East, Face::Down, Face::Up] {
        let neighbour = step(pos, dir);
        if game
            .native_container_slots(&dim, neighbour, dir.opposite())
            .is_some_and(|slots| !slots.is_empty())
        {
            return dir.opposite();
        }
    }
    let yaw = game.native_player().map_or(0.0, |p| p.yaw);
    // `Direction.fromYRot` is where the player looks; the box leaves toward them.
    match ((f64::from(yaw) / 90.0 + 0.5).floor() as i64) & 3 {
        0 => Face::North,
        1 => Face::East,
        2 => Face::South,
        _ => Face::West,
    }
}

pub(super) fn placed(game: &mut Game<'_>, pos: Pos) {
    if !simulates(game) || game.native_block(pos) != BLOCK {
        return;
    }
    let facing = choose_facing(game, pos);
    game.native_set_block_property(pos, "facing", face_name(facing));
    game.native_block_entity_sync(pos);
}

fn hand_over(game: &mut Game<'_>, stack: ItemStack, pos: Pos) {
    let uuid = game.native_acting_id();
    if let Some(rest) = game.native_give_stack(uuid, stack) {
        game.spawn_item(rest, center(pos), [0.0, 0.1, 0.0], 10);
    }
}

/// Empty hand takes the finished box; a package in hand goes into the input slot
/// (it is unpacked on the next tick); anything else is left to normal placement.
pub(super) fn right_click(game: &mut Game<'_>, pos: Pos) -> bool {
    if !simulates(game)
        || !game.native_on("create")
        || game.held_slot() != game.selected
        || !game.may_build()
        || game.native_block(pos) != BLOCK
    {
        return false;
    }
    let sneaking = game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft);
    let held = game.held().filter(|s| s.count > 0).cloned();
    match held {
        Some(stack) if package::is_package(&stack.item) && !sneaking => {
            let taken = game
                .native_with_mod_block_entity(pos, |e| {
                    e.items.resize(2, None);
                    if e.id == BLOCK && e.items[IN].is_none() {
                        let mut one = stack.clone();
                        one.count = 1;
                        e.items[IN] = Some(one);
                        true
                    } else {
                        false
                    }
                })
                .unwrap_or(false);
            if taken {
                game.consume_held();
            }
            taken
        }
        Some(_) => false,
        None if !sneaking => {
            let boxed = game
                .native_with_mod_block_entity(pos, |e| {
                    e.items.resize(2, None);
                    e.items[OUT].take()
                })
                .flatten();
            match boxed {
                Some(stack) => {
                    hand_over(game, stack, pos);
                    true
                }
                None => false,
            }
        }
        None => false,
    }
}

/// The engine drops the container's item slots; boxes still inside the animation
/// or queued are dropped here.
pub(super) fn destroyed(game: &mut Game<'_>, pos: Pos) {
    if game.native_block(pos) != BLOCK {
        return;
    }
    let mut dropped = Vec::new();
    game.native_with_mod_block_entity(pos, |e| {
        e.items.resize(2, None);
        if e.items[OUT].is_none() {
            if let Some(stack) = get(&e.nbt, "HeldBox").and_then(item_from_nbt) {
                dropped.push(stack);
            }
        }
        if let Some(Tag::List(list)) = get(&e.nbt, "Queue") {
            dropped.extend(list.iter().filter_map(item_from_nbt));
        }
        e.nbt.remove(&"HeldBox".into());
        e.nbt.remove(&"Queue".into());
    });
    for stack in dropped {
        game.spawn_item(stack, center(pos), [0.0, 0.2, 0.0], 10);
    }
}

/// `PackagerPoint.insert` for the Mechanical Arm: a package is unpacked into the
/// inventory behind, nothing else is taken. Returns how many items were accepted.
pub(super) fn arm_insert(game: &mut Game<'_>, pos: Pos, offered: &ItemStack, simulate: bool) -> u32 {
    if offered.count == 0 || !package::is_package(&offered.item) || game.native_block(pos) != BLOCK {
        return 0;
    }
    let Some(facing) = facing_of(game, pos) else { return 0 };
    let free = matches!(
        game.world.block_entities.get(&(pos[0], pos[1], pos[2])),
        Some(BlockEntity::Mod(e)) if e.id == BLOCK
            && e.items.get(IN).is_none_or(Option::is_none)
            && int(&e.nbt, "AnimTicks") == 0
    );
    if !free {
        return 0;
    }
    let behind = step(pos, facing.opposite());
    let mut reg = package::Registry::default();
    let boxed = {
        let max = max_of(game);
        package::to_core(&mut reg, offered, &max)
    };
    let mut st = PackagerState::default();
    let fits = with_target(game, behind, facing, &mut reg, |tgt| tgt.is_some_and(|t| st.unwrap_box(t, &boxed, true)));
    if !fits || simulate {
        return u32::from(fits);
    }
    let mut one = offered.clone();
    one.count = 1;
    let put = game
        .native_with_mod_block_entity(pos, |e| {
            e.items.resize(2, None);
            e.items[IN] = Some(one);
        })
        .is_some();
    u32::from(put)
}

// ------------------------------------------------------------------ draw

pub(super) fn draw(game: &Game<'_>, pos: Pos, entity: &ModBlockEntity, draw: &mut BlockEntityDraw) {
    let nbt = &entity.nbt;
    let ticks = int(nbt, "AnimTicks");
    let inward = flag(nbt, "AnimInward");
    let elapsed = match get(nbt, SYNC_TICK) {
        Some(Tag::Int(sample)) => (draw.game_time as i32).wrapping_sub(*sample).clamp(0, 64),
        _ => 0,
    };
    let remaining = (ticks as f32 - elapsed as f32 - draw.partial).max(0.0);
    let cycle = packager::CYCLE as f32;
    let (key, distance) = if ticks == 0 {
        ("HeldBox", 0.0)
    } else if inward {
        if remaining <= cycle / 2.0 {
            return;
        }
        ("PrevUnwrapped", (remaining - cycle / 2.0) / (cycle / 2.0) * 0.45)
    } else {
        if remaining < cycle / 2.0 {
            return;
        }
        ("HeldBox", (cycle - remaining) / (cycle / 2.0) * 0.45)
    };
    let Some(stack) = get(nbt, key).and_then(item_from_nbt) else { return };
    let n = facing_of(game, pos).map_or([0, 0, 0], |f| f.normal());
    let d = distance as f64;
    draw.push();
    draw.translate([
        (0.5 + f64::from(n[0]) * d) as f32,
        (0.5 + f64::from(n[1]) * d) as f32,
        (0.5 + f64::from(n[2]) * d) as f32,
    ]);
    draw.scale([0.5; 3]);
    game.native_draw_item(draw, &stack, DisplayContext::Fixed, None, 0);
    draw.pop();
}
