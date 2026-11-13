// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

// Mechanical Arm: block entity state, kinetic tick, item transfers between
// neighbouring inventories, point selection with the Arm item, and the pose.
// The rules live in `create_engine::arm`; this file only adapts them to the game.
// Supported point types for transfers: Basin, Belt (deposit), Depot, Millstone,
// Saw, Crushing Wheels (deposit), ordinary inventories (`Container`), Funnel
// (deposit), Chute, Mechanical Crafter, Deployer, Blaze Burner (fuel), Campfire,
// Composter, Jukebox, Respawn Anchor and Packager (insert unpacks a package; extract reads the output slot). Belts take items and give them up again.

use super::{
    Game, create_port as kinetics, create_port_belt as belt, native_block_entity_draw::BlockEntityDraw,
};
use crate::{
    containers::{BlockEntity, Jukebox, ModBlockEntity},
    items::ItemStack,
};
use create_engine::arm::{
    self, Arm, ArmWorld, BlockProbe, Direction as ArmDirection, Event, Mode, Phase, Point,
    PointKind, Selection, SelectionMode, Stack,
};
use create_engine::burner::{Burner, Candidate, Fuel, Heat};
use horizon_assets::item_model::DisplayContext;
use horizon_client::text::Text;
use horizon_core::{geometry::Direction as Face, position::BlockPos, random::RandomSource};
use horizon_items::item_nbt::{encode_items, item_from_nbt, item_to_nbt};
use horizon_world::{
    attachments::AttachmentType,
    nbt::{Compound, Tag},
};

pub(super) const BLOCK: &str = "create:mechanical_arm";
type Pos = [i32; 3];
const SYNC_TICK: &str = "create_horizon:ArmSync";

/// Points picked with the Arm item and not yet given to a placed arm.
const SELECTION: AttachmentType<Vec<Point>> = AttachmentType {
    namespace: "create",
    name: "arm_selection",
    default: Vec::new,
    codec: None,
    copy_on_death: false,
    sync: false,
};

// ------------------------------------------------------------------- NBT

fn get<'a>(nbt: &'a Compound, key: &str) -> Option<&'a Tag> {
    nbt.get(&key.into())
}

fn int(nbt: &Compound, key: &str, default: i32) -> i32 {
    match get(nbt, key) {
        Some(Tag::Int(v)) => *v,
        _ => default,
    }
}

fn string(nbt: &Compound, key: &str) -> Option<String> {
    match get(nbt, key) {
        Some(Tag::String(v)) => v.to_string().ok(),
        _ => None,
    }
}

fn flag(nbt: &Compound, key: &str) -> bool {
    matches!(get(nbt, key), Some(Tag::Byte(v)) if *v != 0)
}

pub(super) fn default_nbt() -> Compound {
    Compound::from([
        ("Phase".into(), Tag::String(Phase::SearchInputs.name().into())),
        ("SelectionMode".into(), Tag::String(SelectionMode::RoundRobin.name().into())),
        ("Powered".into(), Tag::Byte(0)),
        ("Points".into(), Tag::List(Vec::new())),
    ])
}

fn selection_mode(name: &str) -> SelectionMode {
    [SelectionMode::RoundRobin, SelectionMode::ForcedRoundRobin, SelectionMode::PreferFirst]
        .into_iter()
        .find(|mode| mode.name() == name)
        .unwrap_or(SelectionMode::RoundRobin)
}

fn write_point(point: &Point, anchor: Pos) -> Tag {
    let record = point.serialize(anchor);
    Tag::Compound(Compound::from([
        ("Type".into(), Tag::String(record.kind.as_str().into())),
        ("Pos".into(), Tag::IntArray(record.pos.to_vec())),
        ("Mode".into(), Tag::String(record.mode.as_str().into())),
    ]))
}

fn read_point(tag: &Tag, anchor: Pos) -> Option<Point> {
    let Tag::Compound(c) = tag else { return None };
    let Some(Tag::IntArray(rel)) = get(c, "Pos") else { return None };
    if rel.len() != 3 {
        return None;
    }
    Some(Point {
        kind: PointKind::from_name(&string(c, "Type")?)?,
        pos: [anchor[0] + rel[0], anchor[1] + rel[1], anchor[2] + rel[2]],
        mode: Mode::parse(&string(c, "Mode")?)?,
    })
}

/// Saved points keep the stored order (inputs first, then outputs).
fn load(nbt: &Compound, anchor: Pos) -> Arm<ItemStack> {
    let mut arm = Arm::new();
    let points: Vec<Point> = match get(nbt, "Points") {
        Some(Tag::List(list)) => list.iter().filter_map(|tag| read_point(tag, anchor)).collect(),
        _ => Vec::new(),
    };
    arm.set_points(points);
    arm.events.clear();
    arm.phase = string(nbt, "Phase")
        .and_then(|name| Phase::parse(&name))
        .unwrap_or(Phase::SearchInputs);
    arm.chased_index = int(nbt, "TargetPointIndex", 0);
    arm.progress = kinetics::number(nbt, "MovementProgress");
    arm.last_input = int(nbt, "LastInput", -1);
    arm.last_output = int(nbt, "LastOutput", -1);
    arm.selection_mode = selection_mode(&string(nbt, "SelectionMode").unwrap_or_default());
    arm.redstone_locked = flag(nbt, "Powered");
    arm.goggles = flag(nbt, "Goggles");
    arm.held = get(nbt, "HeldItem")
        .and_then(item_from_nbt)
        .filter(|item| item.count > 0)
        .map(|mut item| {
            let count = item.count;
            item.count = 1;
            Stack { item, count }
        });
    if !arm.progress.is_finite() {
        arm.progress = 0.0;
    }
    arm
}

fn store(arm: &Arm<ItemStack>, nbt: &mut Compound, anchor: Pos) {
    let points: Vec<Tag> =
        arm.inputs.iter().chain(&arm.outputs).map(|point| write_point(point, anchor)).collect();
    nbt.insert("Points".into(), Tag::List(points));
    nbt.insert("Phase".into(), Tag::String(arm.phase.name().into()));
    nbt.insert("TargetPointIndex".into(), Tag::Int(arm.chased_index));
    nbt.insert("MovementProgress".into(), Tag::Float(arm.progress.to_bits()));
    nbt.insert("LastInput".into(), Tag::Int(arm.last_input));
    nbt.insert("LastOutput".into(), Tag::Int(arm.last_output));
    nbt.insert("SelectionMode".into(), Tag::String(arm.selection_mode.name().into()));
    nbt.insert("Powered".into(), Tag::Byte(i8::from(arm.redstone_locked)));
    nbt.insert("Goggles".into(), Tag::Byte(i8::from(arm.goggles)));
    match &arm.held {
        Some(held) => {
            let mut item = held.item.clone();
            item.count = held.count;
            nbt.insert("HeldItem".into(), item_to_nbt(&item));
        }
        None => {
            nbt.remove(&"HeldItem".into());
        }
    }
}

/// What the client needs to draw the pose; progress is extrapolated from the sync tick.
pub(super) fn update_tag(entity: &ModBlockEntity) -> Compound {
    [
        "Phase", "TargetPointIndex", "MovementProgress", "Points", "HeldItem", "Goggles", "Speed",
        "NetworkStatus", "PrevPoint", SYNC_TICK, "create_horizon",
    ]
    .into_iter()
    .filter_map(|key| get(&entity.nbt, key).map(|tag| (key.into(), tag.clone())))
    .collect()
}

// ------------------------------------------------------------------ probe

fn direction(name: &str) -> Option<ArmDirection> {
    Some(match name {
        "down" => ArmDirection::Down,
        "up" => ArmDirection::Up,
        "north" => ArmDirection::North,
        "south" => ArmDirection::South,
        "west" => ArmDirection::West,
        "east" => ArmDirection::East,
        _ => return None,
    })
}

fn face(direction: ArmDirection) -> Face {
    match direction {
        ArmDirection::Down => Face::Down,
        ArmDirection::Up => Face::Up,
        ArmDirection::North => Face::North,
        ArmDirection::South => Face::South,
        ArmDirection::West => Face::West,
        ArmDirection::East => Face::East,
    }
}

fn probe(game: &mut Game<'_>, pos: Pos) -> BlockProbe {
    let block = game.native_block(pos);
    let property = |game: &Game<'_>, key: &str| game.native_block_property(pos, key);
    let mut probe = BlockProbe {
        facing: property(game, "facing").as_deref().and_then(direction),
        ..BlockProbe::default()
    };
    match block.as_str() {
        "create:basin" => probe.basin = true,
        "create:belt" => {
            probe.belt = true;
            probe.belt_can_transport_objects =
                !matches!(property(game, "slope").as_deref(), Some("vertical" | "sideways"));
        }
        "create:blaze_burner" | "create:lit_blaze_burner" => probe.blaze_burner = true,
        "create:depot" => probe.depot = true,
        "create:millstone" => probe.millstone = true,
        "create:deployer" => probe.deployer = true,
        "create:andesite_funnel" | "create:brass_funnel" => {
            probe.funnel = true;
            probe.funnel_extracting = property(game, "extracting").as_deref() == Some("true");
        }
        "minecraft:campfire" | "minecraft:soul_campfire" => probe.campfire = true,
        "minecraft:composter" => probe.composter = true,
        "minecraft:jukebox" => probe.jukebox = true,
        "minecraft:respawn_anchor" => probe.respawn_anchor = true,
        b if b == super::create_port_chute::BLOCK || b == super::create_port_chute::SMART => {
            probe.chute = true;
        }
        b if b == super::create_port_packager::BLOCK => probe.packager = true,
        b if b == super::create_port_drill::SAW => {
            probe.saw = true;
            probe.saw_facing_up = probe.facing == Some(ArmDirection::Up);
            probe.saw_speed = match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
                Some(crate::containers::BlockEntity::Mod(entity)) => {
                    Some(kinetics::number(&entity.nbt, "Speed"))
                }
                _ => None,
            };
        }
        b if b == super::create_port_saw_processing::crushing::CONTROLLER => {
            probe.crushing_wheel_controller = true;
        }
        b if b == super::create_port_saw_processing::crafter::BLOCK => {
            probe.mechanical_crafter = true;
        }
        _ => {
            let dim = game.dim.id().to_owned();
            probe.container = block != "minecraft:air"
                && game
                    .native_container_slots(&dim, pos, Face::Up)
                    .is_some_and(|slots| !slots.is_empty());
        }
    }
    probe
}

// ------------------------------------------------------------- the world

struct World<'a, 'g> {
    game: &'a mut Game<'g>,
}

fn single(stack: &ItemStack) -> ItemStack {
    let mut item = stack.clone();
    item.count = 1;
    item
}

fn with_count(item: &ItemStack, count: u32) -> ItemStack {
    let mut stack = item.clone();
    stack.count = count;
    stack
}

impl ArmWorld<ItemStack> for World<'_, '_> {
    fn is_valid(&mut self, point: &Point) -> bool {
        super::create_port_contraption::resident(self.game, point.pos) && {
            let probe = probe(self.game, point.pos);
            point.kind.can_create(&probe)
        }
    }

    fn slot_count(&mut self, point: &Point) -> usize {
        // A belt segment, a crafter and a deployer hand each show one slot.
        if matches!(point.kind, PointKind::Belt | PointKind::Crafter | PointKind::Deployer) {
            return 1;
        }
        let dim = self.game.dim.id().to_owned();
        self.game
            .native_container_slots(&dim, point.pos, Face::Up)
            .map_or(0, |slots| slots.len())
    }

    fn extract(
        &mut self,
        point: &Point,
        slot: usize,
        amount: u32,
        simulate: bool,
    ) -> Option<Stack<ItemStack>> {
        match point.kind {
            PointKind::Belt => return self.belt_extract(point.pos, amount, simulate),
            PointKind::Crafter => return self.crafter_extract(point.pos, simulate),
            PointKind::Deployer => return self.deployer_extract(point.pos, amount, simulate),
            PointKind::Jukebox => return self.jukebox_extract(point.pos, simulate),
            PointKind::Composter => return self.composter_extract(point.pos, slot, simulate),
            _ => {}
        }
        let dim = self.game.dim.id().to_owned();
        let slots = self.game.native_container_slots(&dim, point.pos, Face::Up)?;
        let entry = slots.get(slot)?;
        if !entry.can_take {
            return None;
        }
        let stack = entry.stack.as_ref()?;
        let count = amount.min(stack.count);
        if count == 0 {
            return None;
        }
        if simulate {
            return Some(Stack { item: single(stack), count });
        }
        let taken = self.game.native_extract_item(&dim, point.pos, Face::Up, entry.slot, count)?;
        if point.kind == PointKind::Depot {
            self.game.native_with_mod_block_entity(point.pos, |entity| {
                if entity.items.first().is_none_or(Option::is_none) {
                    entity.nbt.remove(&"HeldItem".into());
                }
                kinetics::depot_data(entity);
            });
            self.game.native_block_entity_sync(point.pos);
        }
        Some(Stack { item: single(&taken), count: taken.count })
    }

    fn insert(
        &mut self,
        point: &Point,
        stack: Stack<ItemStack>,
        simulate: bool,
    ) -> Option<Stack<ItemStack>> {
        let offered = with_count(&stack.item, stack.count);
        let pos = point.pos;
        let moved: u32 = match point.kind {
            PointKind::Belt => belt::insert_from_container(self.game, pos, &offered, simulate),
            PointKind::Depot => belt::insert_depot_from_chute(self.game, pos, &offered, simulate),
            PointKind::Millstone => {
                super::create_port_millstone::insert_from_transport(self.game, pos, &offered, simulate)
            }
            PointKind::Saw => super::create_port_saw_processing::insert_from_transport(
                self.game, pos, &offered, simulate,
            ),
            PointKind::CrushingWheels => {
                super::create_port_saw_processing::crushing::insert(self.game, pos, &offered, simulate)
            }
            PointKind::Container => self
                .game
                .chute_insert((pos[0], pos[1], pos[2]), &offered, simulate)
                .unwrap_or(0),
            PointKind::Basin => {
                if simulate {
                    self.basin_capacity(pos, &offered)
                } else {
                    let rest = self.game.native_insert_item(pos, Face::Up, offered.clone());
                    stack.count - rest.map_or(0, |rest| rest.count)
                }
            }
            PointKind::Funnel => self.funnel_insert(pos, &offered, simulate),
            PointKind::Chute => self.chute_put(pos, &offered, simulate),
            PointKind::Crafter => self.crafter_insert(pos, &offered, simulate),
            PointKind::Deployer => self.deployer_insert(pos, &offered, simulate),
            PointKind::BlazeBurner => self.burner_insert(pos, &offered, simulate),
            PointKind::Campfire => self.campfire_insert(pos, &offered, simulate),
            PointKind::Composter => self.composter_insert(pos, &offered, simulate),
            PointKind::Jukebox => self.jukebox_insert(pos, &offered, simulate),
            PointKind::RespawnAnchor => self.anchor_insert(pos, &offered, simulate),
            // `PackagerPoint.insert`: a package is unpacked into the inventory behind.
            PointKind::Packager => {
                super::create_port_packager::arm_insert(self.game, pos, &offered, simulate)
            }
        };
        let remainder = stack.count.saturating_sub(moved);
        (remainder > 0).then(|| Stack { item: stack.item, count: remainder })
    }

    /// `BeltPoint.keepAlive`: the first item within reach stays on the segment.
    fn keep_alive(&mut self, point: &Point) {
        if point.kind == PointKind::Belt {
            self.belt_lock(point.pos);
        }
    }

    fn jukebox_has_record(&mut self, point: &Point) -> bool {
        self.game.native_block_property(point.pos, "has_record").as_deref() == Some("true")
    }
}

impl World<'_, '_> {
    /// How many of `stack` the basin's input slots could take.
    fn basin_capacity(&mut self, pos: Pos, stack: &ItemStack) -> u32 {
        let maximum = kinetics::stack_maximum(self.game, stack);
        let Some(crate::containers::BlockEntity::Mod(entity)) =
            self.game.world.block_entities.get(&(pos[0], pos[1], pos[2]))
        else {
            return 0;
        };
        let mut room = 0;
        for slot in 0..9 {
            if !super::create_port_basin::can_place(&entity.items, slot, stack, maximum) {
                continue;
            }
            room += match entity.items.get(slot).and_then(Option::as_ref) {
                Some(existing) => maximum.saturating_sub(existing.count),
                None => maximum,
            };
        }
        room.min(stack.count)
    }
}

// ------------------------------------------------- per-kind transfers

fn key(pos: Pos) -> (i32, i32, i32) {
    (pos[0], pos[1], pos[2])
}

fn property_is(game: &Game<'_>, pos: Pos, name: &str, value: &str) -> bool {
    game.native_block_property(pos, name).as_deref() == Some(value)
}

/// `FilteringBehaviour.test` for an ordinary filter item, as the funnel and the
/// smart chute test it (`accepts_filter`): no filter accepts everything.
fn filter_accepts(game: &Game<'_>, pos: Pos, stack: &ItemStack) -> bool {
    match game.world.block_entities.get(&key(pos)) {
        Some(BlockEntity::Mod(entity)) => {
            super::create_port_saw_processing::read_filter(game, entity)
                .is_some_and(|filter| filter.is_none_or(|filter| filter.item == stack.item))
        }
        _ => false,
    }
}

fn crafter_idle(entity: &ModBlockEntity) -> bool {
    use create_engine::crafter::Phase as CrafterPhase;
    string(&entity.nbt, "Phase")
        .and_then(|name| CrafterPhase::from_name(&name))
        .unwrap_or_default()
        == CrafterPhase::Idle
}

/// The crafter's single slot, written the way its own `save_inventory` does.
fn crafter_save(entity: &mut ModBlockEntity, item: Option<ItemStack>) {
    entity.items.resize(1, None);
    let mut rows = Vec::new();
    if let Some(Tag::Compound(mut row)) = item.as_ref().map(item_to_nbt) {
        row.insert("Slot".into(), Tag::Int(0));
        rows.push(Tag::Compound(row));
    }
    entity.items[0] = item;
    entity.nbt.insert(
        "Inventory".into(),
        Tag::Compound(Compound::from([
            ("Size".into(), Tag::Int(1)),
            ("Items".into(), Tag::List(rows)),
        ])),
    );
}

/// The deployer's hand, written the way its own `write_hand` does.
fn deployer_write(entity: &mut ModBlockEntity, hand: Option<ItemStack>) {
    entity.items.resize(1, None);
    entity.nbt.insert(
        "HeldItem".into(),
        hand.as_ref().map_or_else(|| Tag::Compound(Compound::new()), item_to_nbt),
    );
    entity.nbt.insert("Inventory".into(), encode_items(std::slice::from_ref(&hand)));
    entity.items[0] = hand;
}

fn heat_name(heat: Heat) -> &'static str {
    match heat {
        Heat::None => "none",
        Heat::Smouldering => "smouldering",
        Heat::Fading => "fading",
        Heat::Kindled => "kindled",
        Heat::Seething => "seething",
    }
}

/// `BlazeBurnerBlockEntity` state of a lit burner; None for anything else.
fn burner_state(game: &Game<'_>, pos: Pos) -> Option<Burner> {
    if game.native_block(pos) != super::create_port_burner::BLOCK {
        return None;
    }
    let heat = super::create_port_burner::heat(&game.native_block_property(pos, "blaze")?)?;
    let Some(BlockEntity::Mod(entity)) = game.world.block_entities.get(&key(pos)) else {
        return None;
    };
    if entity.id != super::create_port_burner::ENTITY {
        return None;
    }
    let number = |name| kinetics::integer(&entity.nbt, name);
    Some(Burner {
        fuel: match number("fuelLevel") {
            0 => Fuel::None,
            1 => Fuel::Normal,
            2 => Fuel::Special,
            _ => return None,
        },
        remaining: number("burnTimeRemaining"),
        creative: number("isCreative") != 0,
        heat,
    })
}

fn store_burner(nbt: &mut Compound, state: Burner) {
    if state.creative {
        nbt.remove(&"fuelLevel".into());
        nbt.remove(&"burnTimeRemaining".into());
        nbt.insert("isCreative".into(), Tag::Byte(1));
    } else {
        nbt.remove(&"isCreative".into());
        nbt.insert("fuelLevel".into(), Tag::Int(state.fuel as i32));
        nbt.insert("burnTimeRemaining".into(), Tag::Int(state.remaining));
    }
}

/// The server's fuel definitions, as the burner's own `candidate` reads them.
fn fuel_candidate(game: &Game<'_>, stack: &ItemStack) -> Candidate {
    if game.remote.is_some() {
        return Candidate { fuel: Fuel::None, burn_time: 0 };
    }
    let data = |id: &str| {
        game.native_data_map("minecraft:item", id, &stack.item)
            .and_then(|v| v.as_i64().or_else(|| v["burn_time"].as_i64()))
            .and_then(|v| i32::try_from(v).ok())
            .filter(|v| *v > 0)
    };
    let tag = |name: &str| game.native_server_item_in_tag(stack, name).unwrap_or(false);
    Candidate::resolve(
        data("create:superheated_blaze_burner_fuels"),
        data("create:regular_blaze_burner_fuels"),
        tag("create:blaze_burner_fuel/special"),
        game.res.providers().burn_time(game.items(), stack, false),
        tag("create:blaze_burner_fuel/regular"),
    )
}

/// How far from the segment centre a belt item still counts as on the segment
/// (`applyToEachWithin(index + .5, .55)`).
const BELT_REACH: f32 = 0.55;

/// The controller, its saved `Inventory` and the centre of the segment at `pos`.
fn belt_target(game: &Game<'_>, pos: Pos) -> Option<(Pos, Compound, f32)> {
    let (controller, index) = belt::node_data(game, pos)?;
    let Some(BlockEntity::Mod(entity)) = game.world.block_entities.get(&key(controller)) else {
        return None;
    };
    let Some(Tag::Compound(inventory)) = get(&entity.nbt, "Inventory") else {
        return None;
    };
    Some((controller, inventory.clone(), index as f32 + 0.5))
}

fn belt_items(inventory: &Compound) -> Vec<Tag> {
    match get(inventory, "Items") {
        Some(Tag::List(items)) => items.clone(),
        _ => Vec::new(),
    }
}

/// The first transported item within reach of the segment centre.
fn belt_find(game: &Game<'_>, items: &[Tag], centre: f32) -> Option<usize> {
    items.iter().position(|tag| {
        belt::read_item(game, tag)
            .is_some_and(|moving| moving.stack.count > 0 && (centre - moving.position).abs() < BELT_REACH)
    })
}

impl World<'_, '_> {
    /// `FunnelPoint.insert`: not powered, not extracting, the filter accepts; the
    /// item goes into the inventory behind the funnel.
    fn funnel_insert(&mut self, pos: Pos, offered: &ItemStack, simulate: bool) -> u32 {
        let block = self.game.native_block(pos);
        let Some(facing) = self.game.native_block_property(pos, "facing").as_deref().and_then(direction)
        else {
            return 0;
        };
        let powered = property_is(self.game, pos, "powered", "true");
        let blocked = ["extracting", "waterlogged"]
            .into_iter()
            .any(|name| property_is(self.game, pos, name, "true"));
        let filtered = block == super::create_port_chute::funnel::BRASS
            && !filter_accepts(self.game, pos, offered);
        if !arm::funnel_accepts(powered, !blocked, !filtered) {
            return 0;
        }
        let behind = facing.normal();
        let behind = [pos[0] - behind[0], pos[1] - behind[1], pos[2] - behind[2]];
        self.game.chute_insert(key(behind), offered, simulate).unwrap_or(0)
    }

    /// `ChuteItemHandler.insertItem`: an empty chute takes the whole stack.
    fn chute_put(&mut self, pos: Pos, offered: &ItemStack, simulate: bool) -> u32 {
        let smart = self.game.native_block(pos) == super::create_port_chute::SMART;
        let free = matches!(
            self.game.world.block_entities.get(&key(pos)),
            Some(BlockEntity::Mod(entity))
                if entity.items.first().is_none_or(|slot| slot.as_ref().is_none_or(|s| s.count == 0))
        );
        let open = if smart {
            !property_is(self.game, pos, "powered", "true") && filter_accepts(self.game, pos, offered)
        } else {
            property_is(self.game, pos, "waterlogged", "false")
        };
        if !free || !open || offered.count == 0 {
            return 0;
        }
        let room = offered.count.min(kinetics::stack_maximum(self.game, offered));
        if simulate {
            return room;
        }
        let rest = self.game.native_insert_item(pos, Face::Up, with_count(offered, room));
        room - rest.map_or(0, |rest| rest.count)
    }

    /// The crafter's slot takes one item while it is idle, uncovered and empty.
    /// Its own ticker starts the craft from the filled slot.
    fn crafter_insert(&mut self, pos: Pos, offered: &ItemStack, simulate: bool) -> u32 {
        let ready = matches!(
            self.game.world.block_entities.get(&key(pos)),
            Some(BlockEntity::Mod(entity))
                if entity.id == super::create_port_saw_processing::crafter::BLOCK
                    && crafter_idle(entity)
                    && kinetics::integer(&entity.nbt, "Cover") == 0
                    && entity.items.first().is_none_or(Option::is_none)
        );
        if !ready || offered.count == 0 {
            return 0;
        }
        if simulate {
            return 1;
        }
        let one = single(offered);
        if self.game.native_with_mod_block_entity(pos, |entity| crafter_save(entity, Some(one))).is_none() {
            return 0;
        }
        self.game.native_block_entity_sync(pos);
        1
    }

    /// `CrafterPoint.extract`: the idle crafter gives up its slot.
    fn crafter_extract(&mut self, pos: Pos, simulate: bool) -> Option<Stack<ItemStack>> {
        let item = match self.game.world.block_entities.get(&key(pos)) {
            Some(BlockEntity::Mod(entity))
                if entity.id == super::create_port_saw_processing::crafter::BLOCK
                    && crafter_idle(entity) =>
            {
                entity.items.first().cloned().flatten().filter(|item| item.count > 0)?
            }
            _ => return None,
        };
        if !simulate {
            self.game.native_with_mod_block_entity(pos, |entity| crafter_save(entity, None))?;
            self.game.native_block_entity_sync(pos);
        }
        Some(Stack { item: single(&item), count: item.count })
    }

    fn deployer_hand(&self, pos: Pos) -> Option<Option<ItemStack>> {
        match self.game.world.block_entities.get(&key(pos)) {
            Some(BlockEntity::Mod(entity)) if entity.id == super::create_port_deployer::BLOCK => {
                Some(entity.items.first().cloned().flatten().filter(|item| item.count > 0))
            }
            _ => None,
        }
    }

    /// `DeployerItemHandler.insertItem`: the hand takes more of the item it holds,
    /// or any item when it is empty, up to the stack maximum.
    fn deployer_insert(&mut self, pos: Pos, offered: &ItemStack, simulate: bool) -> u32 {
        let Some(hand) = self.deployer_hand(pos) else { return 0 };
        let maximum = kinetics::stack_maximum(self.game, offered);
        let (held, room) = match &hand {
            None => (0, maximum),
            Some(held) if held.same_item_same_components(offered) => {
                (held.count, maximum.saturating_sub(held.count))
            }
            Some(_) => return 0,
        };
        let moved = offered.count.min(room);
        if moved == 0 || simulate {
            return moved;
        }
        let merged = with_count(offered, held + moved);
        if self.game.native_with_mod_block_entity(pos, |entity| deployer_write(entity, Some(merged))).is_none() {
            return 0;
        }
        self.game.native_block_entity_sync(pos);
        moved
    }

    /// `DeployerItemHandler.extractItem`: the hand gives up its stack unless it
    /// holds the filtered item.
    fn deployer_extract(&mut self, pos: Pos, amount: u32, simulate: bool) -> Option<Stack<ItemStack>> {
        let hand = self.deployer_hand(pos)??;
        let filtered = match self.game.world.block_entities.get(&key(pos)) {
            Some(BlockEntity::Mod(entity)) => {
                super::create_port_saw_processing::read_filter(self.game, entity)
                    .is_none_or(|filter| filter.is_some_and(|filter| filter.item == hand.item))
            }
            _ => true,
        };
        let count = amount.min(hand.count);
        if filtered || count == 0 {
            return None;
        }
        if !simulate {
            let rest = (hand.count > count).then(|| with_count(&hand, hand.count - count));
            self.game.native_with_mod_block_entity(pos, |entity| deployer_write(entity, rest))?;
            self.game.native_block_entity_sync(pos);
        }
        Some(Stack { item: single(&hand), count })
    }

    /// `BlazeBurnerPoint.insert`: `BlazeBurnerBlock.tryInsert` without forced
    /// overflow; one item is used on success.
    fn burner_insert(&mut self, pos: Pos, offered: &ItemStack, simulate: bool) -> u32 {
        let Some(mut state) = burner_state(self.game, pos) else { return 0 };
        let accepted = state.insert(fuel_candidate(self.game, offered), false, true, |_| {});
        let moved = offered.count - arm_consumed(accepted, offered.count);
        if moved == 0 || simulate {
            return moved;
        }
        let old = state.heat;
        state.insert(fuel_candidate(self.game, offered), false, false, |_| {});
        let at = pos.map(|c| f64::from(c) + 0.5);
        let volume = 0.125 + self.game.world.random.next_float() * 0.125;
        let pitch = 0.75 - self.game.world.random.next_float() * 0.25;
        self.game.native_play_sound_in_place("minecraft:entity.blaze.shoot", at, 4, volume, pitch);
        if state.heat != old {
            if self.game.native_set_block_property(pos, "blaze", heat_name(state.heat)) {
                self.game.native_block_entity_sync(pos);
            }
            let volume = 0.125 + self.game.world.random.next_float() * 0.125;
            let pitch = 1.15 - self.game.world.random.next_float() * 0.25;
            self.game.native_play_sound_in_place("minecraft:entity.blaze.ambient", at, 4, volume, pitch);
        }
        self.game.native_with_mod_block_entity(pos, |entity| store_burner(&mut entity.nbt, state));
        1
    }

    /// `CampfirePoint.insert`: a campfire cooking recipe and a free slot.
    fn campfire_insert(&mut self, pos: Pos, offered: &ItemStack, simulate: bool) -> u32 {
        let time = self
            .game
            .recipes()
            .cook("minecraft:campfire_cooking", offered)
            .map(|recipe| recipe.time as i32);
        let free = matches!(
            self.game.world.block_entities.get(&key(pos)),
            Some(BlockEntity::Campfire(campfire)) if campfire.items.iter().any(Option::is_none)
        );
        let moved = offered.count - arm::campfire_insert(offered.count, time.is_some(), free);
        let Some(time) = time else { return 0 };
        if moved == 0 || simulate {
            return moved;
        }
        let placed = match self.game.world.block_entities.get_mut(&key(pos)) {
            Some(BlockEntity::Campfire(campfire)) => campfire.place_food(&mut single(offered), time),
            _ => false,
        };
        if !placed {
            return 0;
        }
        self.game.native_campfire_start(pos, &offered.item);
        self.game.mark_block_entity_updated(key(pos));
        1
    }

    /// Composter input container: one compostable item from above while below level 7.
    fn composter_insert(&mut self, pos: Pos, offered: &ItemStack, simulate: bool) -> u32 {
        let compostable = self
            .game
            .items()
            .get(&offered.item)
            .is_some_and(|info| info.compostable.is_some());
        if !self.game.composter_level(pos).is_some_and(|level| level < 7) || !compostable || offered.count == 0 {
            return 0;
        }
        if simulate {
            return 1;
        }
        let rest = self.game.composter_container_insert(pos, offered.clone(), true);
        offered.count - rest.map_or(0, |rest| rest.count)
    }

    /// Composter output container (below): a full composter gives its bone meal
    /// from the second of the two slots.
    fn composter_extract(&mut self, pos: Pos, slot: usize, simulate: bool) -> Option<Stack<ItemStack>> {
        if slot != 1 || self.game.composter_level(pos) != Some(8) {
            return None;
        }
        if !simulate {
            let at = BlockPos::new(pos[0], pos[1], pos[2]);
            self.game.with_level(|b, level, ticks| b.composter_empty(level, ticks, at));
            let _ = self.game.process_level_events();
            self.game.world.modified.insert((pos[0] >> 4, pos[2] >> 4));
        }
        Some(Stack { item: ItemStack::new("minecraft:bone_meal", 1), count: 1 })
    }

    /// `JukeboxPoint.insert`: a playable record into a jukebox without one.
    fn jukebox_insert(&mut self, pos: Pos, offered: &ItemStack, simulate: bool) -> u32 {
        let playable = self
            .game
            .items()
            .get(&offered.item)
            .is_some_and(|info| info.jukebox_song.is_some());
        let has_record = self.game.native_block_property(pos, "has_record").map(|v| v == "true");
        let empty = match self.game.world.block_entities.get(&key(pos)) {
            Some(BlockEntity::Jukebox(jukebox)) => jukebox.record().is_none(),
            Some(_) => false,
            None => true,
        };
        let moved = offered.count - arm::jukebox_insert(playable, has_record, empty, offered.count);
        if moved == 0 || simulate {
            return moved;
        }
        if self.game.world.block_entities.get(&key(pos)).is_none() {
            self.game.world.block_entities.insert(
                key(pos),
                BlockEntity::Jukebox(Jukebox {
                    items: [None],
                    ticks: 0,
                    playing: false,
                    song_clock_present: false,
                }),
            );
        }
        self.game.jukebox_set_item(pos, Some(single(offered)));
        1
    }

    /// `JukeboxPoint.extract`: the record comes out and the song stops.
    fn jukebox_extract(&mut self, pos: Pos, simulate: bool) -> Option<Stack<ItemStack>> {
        let has_record = self.game.native_block_property(pos, "has_record").map(|v| v == "true");
        let record = match self.game.world.block_entities.get(&key(pos)) {
            Some(BlockEntity::Jukebox(jukebox)) => jukebox.record().cloned(),
            _ => None,
        };
        if !arm::jukebox_can_extract(has_record, record.is_some()) {
            return None;
        }
        let record = record?;
        if !simulate {
            self.game.jukebox_set_item(pos, None);
        }
        Some(Stack { item: single(&record), count: 1 })
    }

    /// `RespawnAnchorPoint.insert`: glowstone below charge 4, one item per charge.
    fn anchor_insert(&mut self, pos: Pos, offered: &ItemStack, simulate: bool) -> u32 {
        let charge = self
            .game
            .native_block_property(pos, "charges")
            .and_then(|value| value.parse::<u32>().ok());
        let rest = arm::respawn_anchor_insert(
            offered.item.as_ref() == "minecraft:glowstone",
            charge,
            offered.count,
        );
        let moved = offered.count - rest;
        let Some(charge) = charge else { return 0 };
        if moved == 0 || simulate {
            return moved;
        }
        if !self.game.native_set_block_property(pos, "charges", &(charge + 1).to_string()) {
            return 0;
        }
        let at = pos.map(|c| f64::from(c) + 0.5);
        self.game.native_play_sound_in_place("minecraft:block.respawn_anchor.charge", at, 4, 1.0, 1.0);
        1
    }

    /// `BeltPoint.extract`: the first item on the segment, taken from the
    /// controller's saved `Inventory` (the belt re-reads it on its next tick).
    fn belt_extract(&mut self, pos: Pos, amount: u32, simulate: bool) -> Option<Stack<ItemStack>> {
        let (controller, mut inventory, centre) = belt_target(self.game, pos)?;
        let mut items = belt_items(&inventory);
        let at = belt_find(self.game, &items, centre)?;
        let mut moving = belt::read_item(self.game, &items[at])?;
        let count = amount.min(moving.stack.count);
        if count == 0 {
            return None;
        }
        let item = single(&belt::game_stack(&moving.stack));
        if simulate {
            return Some(Stack { item, count });
        }
        moving.stack.count -= count;
        if moving.stack.count == 0 {
            items.remove(at);
        } else {
            items[at] = belt::write_item(&moving);
        }
        inventory.insert("Items".into(), Tag::List(items));
        self.game.native_with_mod_block_entity(controller, |entity| {
            entity.nbt.insert("Inventory".into(), Tag::Compound(inventory));
        })?;
        self.game.native_block_entity_sync(controller);
        Some(Stack { item, count })
    }

    /// `BeltPoint.keepAlive`: flag the first item within reach as externally
    /// locked; the belt clears the flag when it honours it, so this repeats while
    /// the arm travels to the segment. Clients do not need the flag.
    fn belt_lock(&mut self, pos: Pos) {
        let Some((controller, mut inventory, centre)) = belt_target(self.game, pos) else {
            return;
        };
        let mut items = belt_items(&inventory);
        let Some(at) = belt_find(self.game, &items, centre) else { return };
        let Some(mut moving) = belt::read_item(self.game, &items[at]) else { return };
        if moving.externally_locked {
            return;
        }
        moving.externally_locked = true;
        items[at] = belt::write_item(&moving);
        inventory.insert("Items".into(), Tag::List(items));
        self.game.native_with_mod_block_entity(controller, |entity| {
            entity.nbt.insert("Inventory".into(), Tag::Compound(inventory));
        });
    }
}

/// Items left over after a single-item use: all of them when it was refused.
fn arm_consumed(accepted: bool, count: u32) -> u32 {
    if accepted { count.saturating_sub(1) } else { count }
}

// ------------------------------------------------------------------ tick

/// Chunks must be resident before the arm touches its points.
fn loaded(game: &Game<'_>, arm: &Arm<ItemStack>) -> bool {
    arm.inputs
        .iter()
        .chain(&arm.outputs)
        .all(|point| super::create_port_contraption::resident(game, point.pos))
}

pub(super) fn tick(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity, rpm: f32) -> bool {
    let mut arm = load(&entity.nbt, pos);
    if !loaded(game, &arm) {
        return false;
    }
    let before = (arm.phase, arm.chased_index, arm.held.clone(), arm.inputs.len() + arm.outputs.len());
    let previous_target = arm.targeted().copied();
    let dim = game.dim.id().to_owned();
    let powered = game.native_neighbor_signal(&dim, pos) != 0;
    let mut world = World { game: &mut *game };
    if powered != arm.redstone_locked {
        arm.redstone_update(powered, &mut world);
    }
    arm.tick(rpm, &mut world);
    if (game_time(world.game) + i64::from(pos[0] ^ pos[2])).rem_euclid(10) == 0 {
        arm.lazy_tick(&mut world);
    }
    for event in arm.drain_events() {
        if event == Event::PickupSound {
            let pitch = arm::pickup_pitch((game.world.game_time.rem_euclid(97) as f32) / 97.0);
            game.native_play_sound_in_place(
                "minecraft:entity.item.pickup",
                pos.map(|c| f64::from(c) + 0.5),
                4,
                0.125,
                pitch,
            );
        }
    }
    let after = (arm.phase, arm.chased_index, arm.held.clone(), arm.inputs.len() + arm.outputs.len());
    let discrete = before != after;
    if discrete {
        if let Some(target) = previous_target {
            entity.nbt.insert("PrevPoint".into(), write_point(&target, pos));
        }
        entity
            .nbt
            .insert(SYNC_TICK.into(), Tag::Int(game.world.game_time as i32));
    }
    store(&arm, &mut entity.nbt, pos);
    discrete
}

fn game_time(game: &Game<'_>) -> i64 {
    game.world.game_time
}

// ------------------------------------------------------- Arm item, placement

fn holds_arm(game: &Game<'_>) -> bool {
    game.stack_in(game.selected)
        .is_some_and(|stack| stack.item.as_ref() == BLOCK)
}

fn selection(game: &mut Game<'_>) -> Selection {
    Selection {
        points: game.player_attachments.get_existing(&SELECTION).ok().flatten().unwrap_or_default(),
    }
}

fn status(game: &mut Game<'_>, key: &str) {
    let player = game.native_acting_id();
    game.native_action_bar(player, Text::translate(key, &[]));
}

/// Right click with the Arm item on a valid point block selects it instead of
/// placing the arm. Sneaking places the arm as usual.
pub(super) fn right_click(game: &mut Game<'_>, pos: Pos) -> bool {
    if !game.native_on("create")
        || game.held_slot() != game.selected
        || !holds_arm(game)
        || !game.may_build()
        || game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft)
    {
        return false;
    }
    let probe = probe(game, pos);
    let mut selection = selection(game);
    let Some(outcome) = selection.right_click(&probe, pos) else {
        return false;
    };
    let _ = game.player_attachments.set(&SELECTION, selection.points);
    status(
        game,
        match outcome.mode {
            Mode::Take => "create.mechanical_arm.extract_from",
            Mode::Deposit => "create.mechanical_arm.deposit_to",
        },
    );
    true
}

/// Left click with the Arm item removes a selected point.
pub(super) fn left_click(game: &mut Game<'_>, pos: Pos) -> bool {
    if !game.native_on("create") || !holds_arm(game) || !game.may_build() {
        return false;
    }
    let mut selection = selection(game);
    if !selection.left_click(pos) {
        return false;
    }
    let _ = game.player_attachments.set(&SELECTION, selection.points);
    status(game, "create.mechanical_arm.deselect");
    true
}

/// A freshly placed arm takes the selection that lies within its range.
pub(super) fn placed(game: &mut Game<'_>, pos: Pos) {
    let mut selection = selection(game);
    let flushed = selection.flush(pos, arm::DEFAULT_RANGE);
    let _ = game.player_attachments.remove(&SELECTION);
    let mut arm = Arm::<ItemStack>::new();
    arm.set_points(flushed.points);
    game.native_with_mod_block_entity(pos, |entity| {
        store(&arm, &mut entity.nbt, pos);
    });
    game.native_block_entity_sync(pos);
}

// ------------------------------------------------------------------ pose

fn ceiling(game: &Game<'_>, pos: Pos) -> bool {
    game.native_block_property(pos, "ceiling").as_deref() == Some("true")
}

fn target_angles(game: &Game<'_>, arm_pos: Pos, point: &Point, ceiling: bool) -> arm::AngleTarget {
    let facing = game
        .native_block_property(point.pos, "facing")
        .as_deref()
        .and_then(direction);
    point.angle_target(arm_pos, facing, ceiling)
}

pub(super) fn draw(game: &Game<'_>, pos: Pos, entity: &ModBlockEntity, draw: &mut BlockEntityDraw) {
    let arm = load(&entity.nbt, pos);
    let ceiling = ceiling(game, pos);
    // Every NBT read has a default, so a missing or half-synced tag draws the rest pose.
    let rpm = Some(kinetics::number(&entity.nbt, "Speed")).filter(|rpm| rpm.is_finite()).unwrap_or(0.0);
    let moving = matches!(arm.phase, Phase::MoveToInput | Phase::MoveToOutput);
    let progress = if moving {
        let elapsed = match get(&entity.nbt, SYNC_TICK) {
            Some(Tag::Int(sample)) => (draw.game_time as i32).wrapping_sub(*sample).clamp(0, 512),
            _ => 0,
        };
        (arm.progress + arm::progress_step(rpm) * (elapsed as f32 + draw.partial)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let current = arm
        .targeted()
        .map(|point| target_angles(game, pos, point, ceiling));
    let previous = get(&entity.nbt, "PrevPoint")
        .and_then(|tag| read_point(tag, pos))
        .map(|point| target_angles(game, pos, &point, ceiling));
    let pose = arm::pose(progress, previous.map_or(0.0, |t| t.base), previous, current);

    draw.push();
    if ceiling {
        draw.rotate_about([1.0, 0.0, 0.0], 180.0, [0.5; 3]);
    }
    let shaft = create_engine::visual::angle(
        draw.game_time as f64 + f64::from(draw.partial),
        rpm,
        create_engine::visual::rotation_offset(
            pos,
            create_engine::kinetics::Axis::Y,
            false,
        ),
    );
    draw.push();
    draw.rotate_about([0.0, 1.0, 0.0], shaft, [0.5; 3]);
    game.native_draw_model(draw, "create:block/mechanical_arm/cog", 0xFFFFFF);
    draw.pop();

    draw.rotate_about([0.0, 1.0, 0.0], pose.base, [0.5; 3]);
    game.native_draw_model(draw, "create:block/mechanical_arm/base", 0xFFFFFF);
    // Shoulder at 6/16 above the floor; segment lengths 14/16 and 15/16.
    let shoulder = [0.5, 6.0 / 16.0, 0.5];
    draw.push();
    draw.rotate_about([1.0, 0.0, 0.0], pose.lower_arm - 90.0, shoulder);
    game.native_draw_model(draw, "create:block/mechanical_arm/lower_body", 0xFFFFFF);
    let elbow = [0.5, 6.0 / 16.0 + 14.0 / 16.0, 0.5];
    draw.translate([0.0, 14.0 / 16.0, 0.0]);
    draw.rotate_about([1.0, 0.0, 0.0], pose.upper_arm - 90.0, elbow);
    game.native_draw_model(draw, "create:block/mechanical_arm/upper_body", 0xFFFFFF);
    draw.translate([0.0, 15.0 / 16.0, 0.0]);
    draw.rotate_about([1.0, 0.0, 0.0], pose.head, [0.5, 6.0 / 16.0 + 29.0 / 16.0, 0.5]);
    let claw = if arm.goggles {
        "create:block/mechanical_arm/claw_base_goggles"
    } else {
        "create:block/mechanical_arm/claw_base"
    };
    game.native_draw_model(draw, claw, 0xFFFFFF);
    game.native_draw_model(draw, "create:block/mechanical_arm/lower_claw_grip", 0xFFFFFF);
    game.native_draw_model(draw, "create:block/mechanical_arm/upper_claw_grip", 0xFFFFFF);
    if let Some(held) = arm.held.as_ref().filter(|held| held.count > 0) {
        // `ArmRenderer.renderItem`: the stack hangs between the grips (FIXED, half size).
        // The offset below the wrist is the source's 11/16 for flat items; the
        // block-item variant is not told apart yet.
        let stack = with_count(&held.item, held.count);
        draw.push();
        draw.translate([0.5, 6.0 / 16.0 + 29.0 / 16.0 + 11.0 / 16.0, 0.5]);
        draw.scale([0.5; 3]);
        game.native_draw_item(draw, &stack, DisplayContext::Fixed, None, 0);
        draw.pop();
    }
    draw.pop();
    draw.pop();
}

/// The held stack drops when the arm block is destroyed (`ArmBlockEntity` is a
/// `SmartBlockEntity`; the block drops what it carries). Call before the block
/// is removed, from the destroy hook (`destroy.rs`, `create_destroy`).
pub(super) fn destroyed(game: &mut Game<'_>, pos: Pos) {
    let held = game
        .native_with_mod_block_entity(pos, |entity| {
            let held = get(&entity.nbt, "HeldItem")
                .and_then(item_from_nbt)
                .filter(|item| item.count > 0);
            entity.nbt.remove(&"HeldItem".into());
            held
        })
        .flatten();
    if let Some(stack) = held {
        game.spawn_item(stack, pos.map(|c| f64::from(c) + 0.5), [0.0, 0.2, 0.0], 10);
    }
}
