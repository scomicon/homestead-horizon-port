// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Create 6.0.10 BasinBlockEntity/BasinInventory/BasinRenderer adapter.
// Heat-aware downward basin; two canonical water/lava tank halves.
// Unknown tank data, complex filters and side spouts remain opaque.
use super::create_port_block_entity_revision::EntityRevision as _;
use super::{
    Game, GameLevel, create_port_basin_recipes::item_drain, create_port_fluid_codec as fluids,
    native_block_entity_draw::BlockEntityDraw,
};
use crate::{
    containers::{BlockEntity, ModBlockEntity, ModContainer, ModRevision},
    items::ItemStack,
};
use create_engine::{basin as rules, transport::SessionCache};
use horizon_client::mod_api::{
    NativeFluidAction, NativeFluidDrain, NativeFluidRenderOptions, NativeFluidStack,
    NativeFluidTankView,
};
use horizon_core::random::{LegacyRandom, RandomSource};
use horizon_world::nbt::{Compound, Tag};
use std::{cell::RefCell, collections::BTreeMap};

pub(super) const ID: &str = "create:basin";
type Pos = [i32; 3];

#[derive(Clone, Debug)]
pub(super) struct Key(pub ItemStack);
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.0.same_item_same_components(&other.0)
    }
}
impl Eq for Key {}
pub(super) type Basin = rules::Basin<Key, fluids::Key>;

#[derive(Default)]
struct Motion {
    owner: Option<ModRevision>,
    tick: Option<i64>,
    previous: f32,
    angle: f32,
    speed: f32,
    lazy: i32,
    moving: bool,
    fluid_levels: [rules::FluidLevel; 4],
    received_fluids: [Option<fluids::Level>; 4],
    rendered_fluids: [Option<rules::Fluid<fluids::Key>>; 4],
}
struct Server {
    owner: Option<ModRevision>,
    initialized: bool,
    contents_changed: bool,
    tick: Option<i64>,
    lazy: i32,
    backup: i32,
    heat: rules::HeatCache,
}
impl Default for Server {
    fn default() -> Self {
        Self {
            owner: None,
            initialized: false,
            contents_changed: false,
            tick: None,
            lazy: 10,
            backup: 20,
            heat: rules::HeatCache::default(),
        }
    }
}
impl Server {
    fn bind(&mut self, owner: ModRevision) {
        if self
            .owner
            .is_some_and(|old| !old.is_same_create_entity(owner))
        {
            *self = Self::default();
        }
        self.owner = Some(owner);
    }
}
thread_local! {
    static MOTION: RefCell<SessionCache<BTreeMap<Pos, Motion>>> = RefCell::new(SessionCache::default());
    static SERVER: RefCell<SessionCache<BTreeMap<Pos, Server>>> = RefCell::new(SessionCache::default());
}

pub(super) fn reset(reload: bool) {
    if !reload {
        MOTION.with(|m| m.borrow_mut().clear());
        SERVER.with(|m| m.borrow_mut().clear());
        tank::reset();
        pump::reset();
        crate::game::create_port_pipe_net::reset();
    }
}

/// BasinInventory.whenContentsChanged/notifyChangeOfContents only raises a
/// flag. The next ordinary Basin tick schedules its operator checker.
pub(super) fn notify_contents(game: &Game<'_>, pos: Pos) {
    let Some(BlockEntity::Mod(entity)) = game.world.block_entities.get(&(pos[0], pos[1], pos[2]))
    else {
        return;
    };
    if entity.id != ID || !crate::game::create_port::inventory_supported(entity) {
        return;
    }
    SERVER.with(|cache| {
        let mut cache = cache.borrow_mut();
        let server = cache
            .dimension(std::ptr::from_ref(&game.world) as usize, game.dim.id())
            .entry(pos)
            .or_default();
        server.bind(entity.revision());
        server.contents_changed = true;
    });
}

pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    if block == ID && container.is_none() {
        *container = Some(ModContainer {
            slots: 18,
            faces: std::array::from_fn(|_| (0..18).collect()),
            inputs: (0..9).collect(),
            // CombinedInvWrapper exposes BOTH input and output to extraction.
            outputs: (0..18).collect(),
            menu: None,
        });
    }
}

fn inventory(slots: &[Option<ItemStack>]) -> Tag {
    Tag::Compound(Compound::from([
        ("Size".into(), Tag::Int(9)),
        (
            "Items".into(),
            Tag::List(
                slots
                    .iter()
                    .enumerate()
                    .filter_map(|(i, s)| {
                        let stack = s.as_ref().filter(|s| s.count > 0)?;
                        let Tag::Compound(mut nbt) = horizon_items::item_nbt::item_to_nbt(stack)
                        else {
                            return None;
                        };
                        nbt.insert("Slot".into(), Tag::Int(i as i32));
                        Some(Tag::Compound(nbt))
                    })
                    .collect(),
            ),
        ),
    ]))
}
fn restore(entity: &mut ModBlockEntity) {
    // Canonical inventory is decoded before publication. Never re-read an old
    // bridge snapshot after ordinary automation has already changed live slots.
    if crate::game::create_port::inventory_supported(entity) {
        entity.items.resize(18, None);
    }
}
pub(super) fn data(entity: &mut ModBlockEntity) -> bool {
    if entity.id != ID || !crate::game::create_port::inventory_supported(entity) {
        return false;
    }
    entity.items.resize(18, None);
    let mut changed = false;
    for (key, range) in [("InputItems", 0..9), ("OutputItems", 9..18)] {
        let Tag::Compound(updated) = inventory(&entity.items[range]) else {
            unreachable!();
        };
        let mut value = match entity.nbt.get(&key.into()) {
            Some(Tag::Compound(existing)) => existing.clone(),
            _ => Compound::new(),
        };
        value.extend(updated);
        let value = Tag::Compound(value);
        changed |= entity.nbt.get(&key.into()) != Some(&value);
        entity.nbt.insert(key.into(), value.clone());
    }
    changed
}
fn empty_tanks() -> Tag {
    let segment = Tag::Compound(Compound::from([
        (
            "Level".into(),
            Tag::Compound(Compound::from([
                ("Force".into(), Tag::Byte(1)),
                ("Speed".into(), Tag::Float(0.25f32.to_bits())),
                ("Target".into(), Tag::Float(0.0f32.to_bits())),
                ("Value".into(), Tag::Float(0.0f32.to_bits())),
            ])),
        ),
        ("TankContent".into(), Tag::Compound(Compound::new())),
    ]));
    Tag::List(vec![segment.clone(), segment])
}
pub(super) fn initialize(entity: &mut ModBlockEntity) {
    if entity.id != ID || !crate::game::create_port::inventory_supported(entity) {
        return;
    }
    restore(entity);
    for key in ["InputTanks", "OutputTanks"] {
        entity.nbt.entry(key.into()).or_insert_with(empty_tanks);
    }
    for key in ["DisabledSpoutput", "Overflow", "FluidOverflow"] {
        entity
            .nbt
            .entry(key.into())
            .or_insert_with(|| Tag::List(Vec::new()));
    }
    entity
        .nbt
        .entry("Filter".into())
        .or_insert_with(|| Tag::Compound(Compound::new()));
    entity
        .nbt
        .entry("FilterAmount".into())
        .or_insert(Tag::Int(64));
    entity.nbt.entry("UpTo".into()).or_insert(Tag::Byte(1));
    data(entity);
}

fn nonempty(tag: Option<&Tag>) -> bool {
    match tag {
        None => false,
        Some(Tag::Compound(c)) => !c.is_empty(),
        Some(Tag::List(l)) => !l.is_empty(),
        _ => true,
    }
}
pub(super) fn unsupported(
    game: &Game<'_>,
    pos: Pos,
    entity: &ModBlockEntity,
) -> Option<&'static str> {
    if !crate::game::create_port::inventory_supported(entity) {
        return Some("basin inventory sizes are not supported");
    }
    if game.native_block_property(pos, "facing").as_deref() != Some("down") {
        return Some("basin side spouts are not implemented");
    }
    if super::create_port_basin_filter::read(game, entity).is_none() {
        return Some("basin recipe filter data is not supported");
    }
    for key in ["InputTanks", "OutputTanks"] {
        if let Err(fluids::Unsupported(reason)) = read_tanks(entity, key) {
            return Some(reason);
        }
    }
    if [
        "Overflow",
        "FluidOverflow",
        "PreferredSpoutput",
        "DisabledSpoutput",
    ]
    .into_iter()
    .any(|key| nonempty(entity.nbt.get(&key.into())))
    {
        return Some("basin spout buffers are not implemented");
    }
    None
}
fn component_maximum(stack: &ItemStack) -> Option<u32> {
    super::create_port::stack_maximum_override(stack)
}
pub(super) fn max_stack(game: &Game<'_>, stack: &ItemStack) -> u32 {
    super::create_port::stack_maximum(game, stack)
}
pub(super) fn has_analog(
    game: &GameLevel<'_>,
    state: horizon_world::registry::BlockStateId,
    output: &mut bool,
) {
    if matches!(
        game.facts.get(state).name.as_ref(),
        ID | item_drain::BLOCK | tank::BLOCK | super::create_port_spout::BLOCK
    ) {
        *output = true;
    }
}
pub(super) fn analog(
    game: &GameLevel<'_>,
    pos: horizon_core::position::BlockPos,
    state: horizon_world::registry::BlockStateId,
    output: &mut i32,
) {
    let block = game.facts.get(state).name.as_ref();
    if !matches!(
        block,
        ID | item_drain::BLOCK | tank::BLOCK | super::create_port_spout::BLOCK
    ) {
        return;
    }
    *output = 0;
    let Some(BlockEntity::Mod(e)) = game.world.block_entities.get(&(pos.x, pos.y, pos.z)) else {
        return;
    };
    if block == item_drain::BLOCK {
        *output = item_drain::comparator(e);
        return;
    }
    if block == tank::BLOCK {
        *output = tank::comparator(e);
        return;
    }
    if block == super::create_port_spout::BLOCK {
        *output = super::create_port_spout::comparator(e);
        return;
    }
    // Unsupported native-created inventories may not have canonical slots yet.
    // Keep their payload opaque and fail closed before any comparator indexing.
    if !crate::game::create_port::inventory_supported(e) {
        return;
    }
    let mut fullness = 0.0f32;
    let mut occupied = false;
    for stack in e.items.iter().take(9).flatten().filter(|s| s.count > 0) {
        let maximum = component_maximum(stack)
            .unwrap_or_else(|| game.items.get(&stack.item).map_or(64, |i| i.max_stack))
            .min(64);
        fullness += stack.count as f32 / maximum as f32;
        occupied = true;
    }
    *output = (fullness / 9.0 * 14.0).floor() as i32 + i32::from(occupied);
}
pub(super) fn can_survive(
    game: &GameLevel<'_>,
    pos: horizon_core::position::BlockPos,
    state: horizon_world::registry::BlockStateId,
    survives: &mut Option<bool>,
) {
    let block = game.facts.get(state).name.as_ref();
    if block == "create:mechanical_mixer" {
        *survives = Some(
            !matches!(game.world.block_entities.get(&(pos.x,pos.y-1,pos.z)),Some(BlockEntity::Mod(e)) if e.id == ID),
        );
        return;
    }
    if block != ID {
        return;
    }
    *survives = Some(
        !matches!(game.world.block_entities.get(&(pos.x,pos.y+1,pos.z)),Some(BlockEntity::Mod(e)) if matches!(e.id.as_str(),"create:mechanical_mixer"|"create:mechanical_press")),
    );
}
pub(super) fn core_stack(game: &Game<'_>, stack: &ItemStack) -> rules::Item<Key> {
    let mut key = stack.clone();
    key.count = 1;
    rules::Item {
        key: Key(key),
        count: stack.count,
        maximum: max_stack(game, stack),
    }
}
pub(super) fn native_stack(item: &rules::Item<Key>) -> ItemStack {
    ItemStack {
        count: item.count,
        ..item.key.0.clone()
    }
}
pub(super) fn read(game: &Game<'_>, pos: Pos) -> Option<Basin> {
    let BlockEntity::Mod(entity) = game.world.block_entities.get(&(pos[0], pos[1], pos[2]))? else {
        return None;
    };
    if entity.id != ID || unsupported(game, pos, entity).is_some() {
        return None;
    }
    Some(Basin {
        input: std::array::from_fn(|i| {
            entity
                .items
                .get(i)
                .and_then(Option::as_ref)
                .filter(|s| s.count > 0)
                .map(|s| core_stack(game, s))
        }),
        output: std::array::from_fn(|i| {
            entity
                .items
                .get(9 + i)
                .and_then(Option::as_ref)
                .filter(|s| s.count > 0)
                .map(|s| core_stack(game, s))
        }),
        input_fluids: read_tanks(entity, "InputTanks").ok()?,
        output_fluids: read_tanks(entity, "OutputTanks").ok()?,
        ..Basin::default()
    })
}
pub(super) fn heat(game: &Game<'_>, pos: Pos) -> rules::Heat {
    let Some(BlockEntity::Mod(entity)) = game.world.block_entities.get(&(pos[0], pos[1], pos[2]))
    else {
        return rules::Heat::None;
    };
    if entity.id != ID {
        return rules::Heat::None;
    }
    SERVER.with(|cache| {
        let mut cache = cache.borrow_mut();
        let server = cache
            .dimension(std::ptr::from_ref(&game.world) as usize, game.dim.id())
            .entry(pos)
            .or_default();
        server.bind(entity.revision());
        server.heat.get_or_read(|| {
            let state = game.world.block(pos[0], pos[1] - 1, pos[2]);
            let Some(state) = game.pack.assets.registry.state(state) else {
                return Some(rules::Heat::None);
            };
            let blaze = state
                .properties
                .get("blaze")
                .map(|value| match value.as_str() {
                    "smouldering" => rules::Heat::Smouldering,
                    "fading" => rules::Heat::Fading,
                    "kindled" => rules::Heat::Kindled,
                    "seething" => rules::Heat::Seething,
                    _ => rules::Heat::None,
                });
            // Same source priority and short-circuit: a real blaze level
            // wins before passive tags or campfire lit state are tested.
            if let Some(blaze) = blaze {
                return Some(blaze);
            }
            let passive = game
                .native_server_block_in_tag(state, "create:passive_boiler_heaters")
                .unwrap_or(false);
            let campfire = passive
                && game
                    .native_server_block_in_tag(state, "minecraft:campfires")
                    .unwrap_or(false);
            let lit = state.properties.get("lit").map(|v| v == "true");
            Some(rules::heat_for_block(blaze, passive, campfire, lit))
        })
    })
}
pub(super) fn write(game: &mut Game<'_>, pos: Pos, basin: &Basin) -> bool {
    let Some(changed) = game.native_with_mod_block_entity(pos, |entity| {
        if entity.id != ID || !crate::game::create_port::inventory_supported(entity) {
            return false;
        }
        // Preflight both canonical halves before any item/fluid write. Unsupported
        // original data is retained; it cannot become an empty tank by accident.
        let Ok((input, output)) = replacement_tanks(entity, basin) else {
            return false;
        };
        let fluid_changed = set_tanks(entity, input, output);
        entity.items = basin
            .input
            .iter()
            .chain(&basin.output)
            .map(|s| s.as_ref().map(native_stack))
            .collect();
        data(entity) || fluid_changed
    }) else {
        return false;
    };
    if changed {
        game.native_block_entity_sync(pos);
        notify_contents(game, pos);
    }
    changed
}

fn read_tanks(entity: &ModBlockEntity, key: &str) -> Result<fluids::Tanks, fluids::Unsupported> {
    // A fresh factory-created entity has not yet had its first initialization
    // tick. Reading/simulating that default must not write or dirty the entity.
    let empty = empty_tanks();
    fluids::read_half(Some(entity.nbt.get(&key.into()).unwrap_or(&empty)))
}
fn replacement_tanks(
    entity: &ModBlockEntity,
    basin: &Basin,
) -> Result<(Tag, Tag), fluids::Unsupported> {
    let empty = empty_tanks();
    Ok((
        fluids::write_half(
            Some(entity.nbt.get(&"InputTanks".into()).unwrap_or(&empty)),
            &basin.input_fluids,
        )?,
        fluids::write_half(
            Some(entity.nbt.get(&"OutputTanks".into()).unwrap_or(&empty)),
            &basin.output_fluids,
        )?,
    ))
}
fn set_tanks(entity: &mut ModBlockEntity, input: Tag, output: Tag) -> bool {
    let changed = entity.nbt.get(&"InputTanks".into()) != Some(&input)
        || entity.nbt.get(&"OutputTanks".into()) != Some(&output);
    if changed {
        entity.nbt.insert("InputTanks".into(), input);
        entity.nbt.insert("OutputTanks".into(), output);
    }
    changed
}
fn fluid_state(game: &Game<'_>, pos: Pos) -> Option<Basin> {
    if game.options.execution_role.is_client() {
        return None;
    }
    let BlockEntity::Mod(entity) = game.world.block_entities.get(&(pos[0], pos[1], pos[2]))? else {
        return None;
    };
    if entity.id != ID || !crate::game::create_port::inventory_supported(entity) {
        return None;
    }
    Some(Basin {
        input_fluids: read_tanks(entity, "InputTanks").ok()?,
        output_fluids: read_tanks(entity, "OutputTanks").ok()?,
        ..Basin::default()
    })
}
fn commit_fluids(game: &mut Game<'_>, pos: Pos, state: &Basin) {
    let changed = game
        .native_with_mod_block_entity(pos, |entity| {
            if entity.id != ID || !crate::game::create_port::inventory_supported(entity) {
                return false;
            }
            let Ok((input, output)) = replacement_tanks(entity, state) else {
                return false;
            };
            set_tanks(entity, input, output)
        })
        .unwrap_or(false);
    if changed {
        game.native_block_entity_sync(pos);
        notify_contents(game, pos);
    }
}
fn core_fluid(stack: &NativeFluidStack) -> Option<rules::Fluid<fluids::Key>> {
    (stack.amount > 0 && matches!(stack.fluid.as_str(), "minecraft:water" | "minecraft:lava")).then(
        || rules::Fluid {
            key: fluids::Key {
                id: stack.fluid.clone(),
                components: stack.components.clone(),
            },
            amount: stack.amount as u32,
        },
    )
}
fn native_fluid(stack: &rules::Fluid<fluids::Key>) -> NativeFluidStack {
    NativeFluidStack {
        fluid: stack.key.id.clone(),
        amount: stack.amount as i32,
        components: stack.key.components.clone(),
    }
}
/// Use the shared Horizon fluid helpers for sided automation.
pub(super) fn custom_fluids(block: &str, custom: &mut bool) {
    if matches!(
        block,
        ID | tank::BLOCK | item_drain::BLOCK | super::create_port_spout::BLOCK
    ) {
        *custom = true;
    }
}
pub(super) fn fluid_tanks(
    game: &Game<'_>,
    pos: Pos,
    side: Option<horizon_core::geometry::Direction>,
    out: &mut Option<Vec<NativeFluidTankView>>,
) {
    if game.native_block(pos) == item_drain::BLOCK {
        *out = item_drain::view(game, pos, side);
        return;
    }
    if game.native_block(pos) == super::create_port_spout::BLOCK {
        *out = super::create_port_spout::view(game, pos, side);
        return;
    }
    if game.native_block(pos) == tank::BLOCK {
        *out = tank::view(game, pos);
        return;
    }
    let Some(state) = fluid_state(game, pos) else {
        return;
    };
    *out = Some(
        (0..4)
            .map(|tank| NativeFluidTankView {
                tank,
                content: state.fluid(tank).map(native_fluid),
                capacity: rules::TANK_CAPACITY as i32,
                can_fill: tank >= 2,
                can_drain: true,
            })
            .collect(),
    );
}
pub(super) fn fluid_fill(
    game: &mut Game<'_>,
    pos: Pos,
    side: Option<horizon_core::geometry::Direction>,
    resource: &NativeFluidStack,
    action: NativeFluidAction,
    out: &mut Option<i32>,
) {
    if game.native_block(pos) == item_drain::BLOCK {
        *out = item_drain::fill(game, pos, side, resource, action);
        return;
    }
    if game.native_block(pos) == super::create_port_spout::BLOCK {
        *out = super::create_port_spout::fill(game, pos, side, resource, action);
        return;
    }
    if game.native_block(pos) == tank::BLOCK {
        *out = tank::fill(game, pos, resource, action);
        return;
    }
    let Some(mut state) = fluid_state(game, pos) else {
        return;
    };
    *out = Some(0);
    let Some(resource) = core_fluid(resource) else {
        return;
    };
    // Injected into the defining crate today; reject future external variants.
    #[allow(unreachable_patterns)]
    let simulate = match action {
        NativeFluidAction::Simulate => true,
        NativeFluidAction::Execute => false,
        _ => return,
    };
    *out = Some(fill_input(game, pos, &mut state, &resource, simulate) as i32);
}
fn fill_input(
    game: &mut Game<'_>,
    pos: Pos,
    state: &mut Basin,
    resource: &rules::Fluid<fluids::Key>,
    simulate: bool,
) -> u32 {
    let amount = state.fill_input_fluid(resource, simulate);
    if !simulate && amount > 0 {
        commit_fluids(game, pos, state);
    }
    amount
}
pub(super) fn fluid_drain(
    game: &mut Game<'_>,
    pos: Pos,
    side: Option<horizon_core::geometry::Direction>,
    request: &NativeFluidDrain,
    action: NativeFluidAction,
    out: &mut Option<NativeFluidStack>,
) {
    if game.native_block(pos) == item_drain::BLOCK {
        *out = item_drain::drain(game, pos, side, request, action);
        return;
    }
    if game.native_block(pos) == super::create_port_spout::BLOCK {
        *out = super::create_port_spout::drain(game, pos, side, request, action);
        return;
    }
    if game.native_block(pos) == tank::BLOCK {
        *out = tank::drain(game, pos, request, action);
        return;
    }
    let Some(mut state) = fluid_state(game, pos) else {
        return;
    };
    let selected;
    let request = match request {
        NativeFluidDrain::Any(maximum) if *maximum > 0 => rules::FluidRequest::Any(*maximum as u32),
        NativeFluidDrain::Exact(stack) => {
            let Some(stack) = core_fluid(stack) else {
                return;
            };
            selected = stack;
            rules::FluidRequest::Exact(&selected)
        }
        _ => return,
    };
    // Injected into the defining crate today; reject future external variants.
    #[allow(unreachable_patterns)]
    let simulate = match action {
        NativeFluidAction::Simulate => true,
        NativeFluidAction::Execute => false,
        _ => return,
    };
    *out = drain(game, pos, &mut state, request, simulate)
        .as_ref()
        .map(native_fluid);
}
fn drain(
    game: &mut Game<'_>,
    pos: Pos,
    state: &mut Basin,
    request: rules::FluidRequest<'_, fluids::Key>,
    simulate: bool,
) -> Option<rules::Fluid<fluids::Key>> {
    let report = state.drain_fluid(request, simulate);
    if !simulate && report.consumed > 0 {
        commit_fluids(game, pos, state);
    }
    report.returned
}
/// `BasinInventory.insertItem`: unique component-bearing stack per half and
/// only its first empty slot. Normal hopper transfers are one item at a time.
pub(super) fn can_place(
    slots: &[Option<ItemStack>],
    local: usize,
    stack: &ItemStack,
    maximum: u32,
) -> bool {
    let maximum = component_maximum(stack).unwrap_or(maximum);
    if local >= 9 || slots.len() < 9 || stack.count == 0 {
        return false;
    }
    let input = &slots[..9];
    if input.iter().enumerate().any(|(i, s)| {
        i != local
            && s.as_ref()
                .is_some_and(|s| s.count > 0 && s.same_item_same_components(stack))
    }) {
        return false;
    }
    match input[local].as_ref().filter(|s| s.count > 0) {
        Some(held) => held.count < maximum.min(64) && held.same_item_same_components(stack),
        None => {
            input
                .iter()
                .position(|s| s.as_ref().is_none_or(|s| s.count == 0))
                == Some(local)
        }
    }
}

pub(super) fn tick(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity) -> bool {
    if entity.id != ID || !crate::game::create_port::inventory_supported(entity) {
        return false;
    }
    restore(entity);
    let changed = data(entity);
    initialize(entity);
    let owner = entity.revision();
    let already_ticked = SERVER.with(|cache| {
        let mut cache = cache.borrow_mut();
        let server = cache
            .dimension(std::ptr::from_ref(&game.world) as usize, game.dim.id())
            .entry(pos)
            .or_default();
        server.bind(owner);
        server.tick == Some(game.world.game_time)
    });
    if already_ticked {
        return changed;
    }
    SERVER.with(|cache| {
        cache
            .borrow_mut()
            .dimension(std::ptr::from_ref(&game.world) as usize, game.dim.id())
            .entry(pos)
            .or_default()
            .heat
            .reset()
    });
    for key in ["InputTanks", "OutputTanks"] {
        if let Some(tanks) = entity.nbt.get_mut(&key.into()) {
            let _ = fluids::tick_half(tanks);
        }
    }
    let scheduled = SERVER.with(|cache| {
        let mut cache = cache.borrow_mut();
        let s = cache
            .dimension(std::ptr::from_ref(&game.world) as usize, game.dim.id())
            .entry(pos)
            .or_default();
        if s.tick == Some(game.world.game_time) {
            return false;
        }
        if s.tick.is_some_and(|t| t > game.world.game_time) {
            *s = Server {
                owner: Some(owner),
                ..Server::default()
            };
        }
        // The duplicate-tick return above deliberately leaves this flag set.
        let mut scheduled = std::mem::take(&mut s.contents_changed) || !s.initialized;
        if !s.initialized {
            s.initialized = true;
            s.backup -= 1;
        }
        if s.lazy <= 0 {
            s.lazy = 10;
            if s.backup <= 0 {
                s.backup = 20;
                scheduled |= entity.items.iter().flatten().any(|s| s.count > 0)
                    || ["InputTanks", "OutputTanks"].into_iter().any(|key| {
                        read_tanks(entity, key)
                            .is_ok_and(|tanks| tanks.iter().flatten().any(|f| f.amount > 0))
                    });
            } else {
                s.backup -= 1;
            }
        } else {
            s.lazy -= 1;
        }
        s.tick = Some(game.world.game_time);
        scheduled
    });
    // Canonical serialization may normalize a supported loaded inventory's
    // order/numeric tags. Basin.read does not notify contents: only genuine
    // inventory/fluid events, initialization and the lazy backup wake it.
    if scheduled {
        crate::game::create_port_mixer::schedule(game, [pos[0], pos[1] + 2, pos[2]]);
        crate::game::create_port_press_basin::schedule(game, [pos[0], pos[1] + 2, pos[2]]);
    }
    if changed {
        let dim = game.dim.id().to_owned();
        game.native_update_output_neighbors(&dim, pos);
    }
    let reason = unsupported(game, pos, entity);
    match reason {
        Some(reason) => {
            entity.nbt.insert(
                "create_horizon:Unsupported".into(),
                Tag::String(reason.into()),
            );
        }
        None => {
            entity.nbt.remove(&"create_horizon:Unsupported".into());
        }
    }
    // Falling-item intake deliberately waits for the entity-sensitive basin
    // collision API; no guessed contact height or direct teleport is used.
    changed
}
pub(super) fn reacts(game: &Game<'_>, pos: Pos) -> bool {
    game.native_on("create")
        && game.native_block(pos) == ID
        && game.held().is_none_or(|s| {
            s.count == 0
                || matches!(
                    s.item.as_ref(),
                    "minecraft:air"
                        | "minecraft:water_bucket"
                        | "minecraft:lava_bucket"
                        | "minecraft:bucket"
                        | "minecraft:sponge"
                )
        })
}
pub(super) fn give_container(game: &mut Game<'_>, pos: Pos, stack: ItemStack) {
    let player = game.native_acting_id();
    if let Some(rest) = game.native_give_stack(player, stack) {
        if let Some(eye) = game.eye_position() {
            game.throw_from_eyes(rest, eye, false);
        } else {
            game.spawn_item(rest, pos.map(|v| f64::from(v) + 0.5), [0.0, 0.2, 0.0], 10);
        }
    }
}
fn bucket_click(game: &mut Game<'_>, pos: Pos, held: &ItemStack) -> bool {
    match held.item.as_ref() {
        "minecraft:water_bucket" | "minecraft:lava_bucket" => {
            let Some((fluid, amount, remainder)) = game.native_item_fluid_drain(held, 1000) else {
                return true;
            };
            let Some(mut state) = fluid_state(game, pos) else {
                return true;
            };
            let fluid = rules::Fluid {
                key: fluids::Key {
                    id: fluid,
                    components: Compound::new(),
                },
                amount: amount as u32,
            };
            if fill_input(game, pos, &mut state, &fluid, true) != fluid.amount {
                return true;
            }
            fill_input(game, pos, &mut state, &fluid, false);
            if !game.creative {
                if held.count == 1 {
                    *game.held_mut() = Some(remainder);
                } else {
                    if let Some(s) = game.held_mut() {
                        s.count -= 1;
                    }
                    give_container(game, pos, remainder);
                }
            }
            true
        }
        "minecraft:bucket" => {
            let Some(mut state) = fluid_state(game, pos) else {
                return true;
            };
            for tank in 0..4 {
                let Some(mut fluid) = state.fluid(tank).cloned() else {
                    continue;
                };
                // API item containers cannot carry an opaque fluid component
                // patch. Refuse it explicitly rather than flattening it.
                if !fluid.key.components.is_empty() || fluid.amount < 1000 {
                    continue;
                }
                let Some((amount, filled)) = game.native_item_fluid_fill(held, &fluid.key.id, 1000)
                else {
                    continue;
                };
                if amount != 1000 {
                    continue;
                }
                fluid.amount = amount as u32;
                drain(
                    game,
                    pos,
                    &mut state,
                    rules::FluidRequest::Exact(&fluid),
                    false,
                );
                if !game.creative {
                    game.consume_held();
                    give_container(game, pos, filled);
                }
                return true;
            }
            true // GenericItemFilling claims even a refused/insufficient bucket.
        }
        "minecraft:sponge" => fluid_state(game, pos).is_some_and(|mut state| {
            drain(
                game,
                pos,
                &mut state,
                rules::FluidRequest::Any(i32::MAX as u32),
                false,
            )
            .is_some()
        }),
        _ => false,
    }
}
pub(super) fn right_click(game: &mut Game<'_>, pos: Pos) -> bool {
    if crate::game::create_port::inventory_blocked(game, pos) {
        return true;
    }
    if !reacts(game, pos) {
        return false;
    }
    if let Some(held) = game
        .held()
        .filter(|s| s.count > 0 && s.item.as_ref() != "minecraft:air")
        .cloned()
    {
        return bucket_click(game, pos, &held);
    }
    let stacks = game
        .native_with_mod_block_entity(pos, |entity| {
            restore(entity);
            let stacks = entity
                .items
                .iter_mut()
                .filter_map(Option::take)
                .collect::<Vec<_>>();
            data(entity);
            stacks
        })
        .unwrap_or_default();
    let player = game.native_acting_id();
    let success = !stacks.is_empty();
    for stack in stacks {
        if let Some(rest) = game.native_give_stack(player, stack) {
            if let Some(eye) = game.eye_position() {
                game.throw_from_eyes(rest, eye, false);
            } else {
                game.spawn_item(rest, pos.map(|v| f64::from(v) + 0.5), [0.0, 0.2, 0.0], 10);
            }
        }
    }
    if success {
        let pitch = 1.0 + game.world.random.next_float();
        game.with_level(|_, level, _| {
            level.native_play_sound(
                "minecraft:entity.item.pickup",
                pos.map(|v| f64::from(v) + 0.5),
                7,
                0.2,
                pitch,
            );
        });
    }
    game.native_block_entity_sync(pos);
    crate::game::create_port_mixer::basin_removed(game, [pos[0], pos[1] + 2, pos[2]]);
    crate::game::create_port_press_basin::basin_removed(game, [pos[0], pos[1] + 2, pos[2]]);
    true
}

/// FluidTankBlockEntity / ConnectivityHandler adapter: the multiblock Fluid
/// Tank (width 1..=3, height 1..=32, 8000 mB per block). Rules live in
/// `create_engine::tank_multiblock::TankWorld`; this module loads the block
/// entities around a change into a `TankWorld`, runs the operation and writes
/// the differences back (see docs/fluid-tank-multiblock.md for the NBT format).
/// Boiler, whistles and moving tanks are not handled: a tank with a non-zero
/// Boiler.Engines/Whistles is opaque and never touched.
pub(super) mod tank {
    use super::*;
    use create_engine::tank_multiblock::{self as mb, TankConfig, TankWorld};
    use horizon_world::level::PlacementContext;
    use std::collections::{BTreeSet, VecDeque};

    pub(in crate::game) const BLOCK: &str = "create:fluid_tank";
    /// `fluidTankCapacity` per block, mB.
    const PER_BLOCK: u32 = 8000;
    const BUCKET: i32 = 1000;
    /// Fluid-render geometry of `FluidTankRenderer`: cap height and puddle.
    const CAP_HEIGHT: f32 = 0.25;
    const MIN_PUDDLE: f32 = 1.0 / 16.0;
    /// Largest block set one reconcile loads; bigger clusters defer.
    const MAX_CELLS: usize = 8192;
    /// Server tick cadence of the self-check / deferred formation retry.
    const CADENCE: i64 = 5;
    type Fluid = rules::Fluid<fluids::Key>;

    #[derive(Default)]
    struct RenderLevel {
        owner: Option<ModRevision>,
        tick: Option<i64>,
        chase: rules::FluidLevel,
    }

    thread_local! {
        static RENDER: RefCell<SessionCache<BTreeMap<Pos, RenderLevel>>> = RefCell::new(SessionCache::default());
    }

    pub(super) fn reset() {
        RENDER.with(|cache| cache.borrow_mut().clear());
    }

    pub(in crate::game) fn default_nbt() -> Compound {
        Compound::from([
            (
                "Boiler".into(),
                Tag::Compound(Compound::from([
                    ("ActiveHeat".into(), Tag::Int(0)),
                    ("Engines".into(), Tag::Int(0)),
                    ("PassiveHeat".into(), Tag::Byte(0)),
                    ("Supply".into(), Tag::Float(0)),
                    ("Update".into(), Tag::Byte(0)),
                    ("Whistles".into(), Tag::Int(0)),
                ])),
            ),
            ("Height".into(), Tag::Int(1)),
            ("Size".into(), Tag::Int(1)),
            ("Luminosity".into(), Tag::Int(0)),
            ("Window".into(), Tag::Byte(1)),
            ("TankContent".into(), Tag::Compound(Compound::new())),
        ])
    }

    // ----- records -----

    /// The saved topology of one tank block (absolute controller position).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Layout {
        controller: Pos,
        width: i32,
        height: i32,
        window: bool,
    }

    impl Layout {
        fn capacity(&self) -> u32 {
            (self.width * self.width * self.height) as u32 * PER_BLOCK
        }
    }

    fn footprint(controller: Pos, width: i32, height: i32) -> Vec<Pos> {
        let mut out = Vec::with_capacity((width * width * height) as usize);
        for y in 0..height {
            for x in 0..width {
                for z in 0..width {
                    out.push([controller[0] + x, controller[1] + y, controller[2] + z]);
                }
            }
        }
        out
    }

    /// Parse the topology keys; None for anything unsupported (boiler data,
    /// malformed or out of range values). `Controller` is the relative offset
    /// to the controller and is absent on a controller.
    fn layout(pos: Pos, nbt: &Compound) -> Option<Layout> {
        let int = |key: &str| match nbt.get(&key.into()) {
            Some(Tag::Int(value)) => Some(*value),
            _ => None,
        };
        let width = int("Size")?;
        let height = int("Height")?;
        if !(1..=mb::MAX_WIDTH).contains(&width)
            || !(1..=mb::DEFAULT_MAX_HEIGHT).contains(&height)
        {
            return None;
        }
        let window = match nbt.get(&"Window".into())? {
            Tag::Byte(value @ (0 | 1)) => *value != 0,
            _ => return None,
        };
        let Tag::Compound(boiler) = nbt.get(&"Boiler".into())? else {
            return None;
        };
        // Attachment counts are written by `boiler_step` on the controller.
        if ["Engines", "Whistles"]
            .iter()
            .any(|key| !matches!(boiler.get(&(*key).into()), Some(Tag::Int(v)) if *v >= 0))
        {
            return None;
        }
        let controller = match nbt.get(&"Controller".into()) {
            None => pos,
            Some(Tag::IntArray(offset)) if offset.len() == 3 => {
                let [dx, dy, dz] = [offset[0], offset[1], offset[2]];
                if dx > 0
                    || dy > 0
                    || dz > 0
                    || (dx, dy, dz) == (0, 0, 0)
                    || -dx >= width
                    || -dz >= width
                    || -dy >= height
                {
                    return None;
                }
                [pos[0] + dx, pos[1] + dy, pos[2] + dz]
            }
            Some(_) => return None,
        };
        Some(Layout { controller, width, height, window })
    }

    /// `Some(content)` when the pooled tank content is readable.
    fn content(nbt: &Compound, capacity: u32) -> Option<Option<Fluid>> {
        fluids::read_content(nbt.get(&"TankContent".into()), capacity).ok()
    }

    fn tank_at<'a>(game: &'a Game<'_>, [x, y, z]: Pos) -> Option<&'a ModBlockEntity> {
        match game.world.block_entities.get(&(x, y, z))? {
            BlockEntity::Mod(entity) if entity.id == BLOCK => Some(entity),
            _ => None,
        }
    }

    fn any_at<'a>(game: &'a Game<'_>, [x, y, z]: Pos) -> Option<&'a ModBlockEntity> {
        match game.world.block_entities.get(&(x, y, z))? {
            BlockEntity::Mod(entity) => Some(entity),
            _ => None,
        }
    }

    fn is_tank(game: &Game<'_>, p: Pos) -> bool {
        game.pack
            .assets
            .registry
            .state(game.world.block(p[0], p[1], p[2]))
            .is_some_and(|state| state.name == BLOCK)
    }

    fn wanted_state(layout: &Layout, pos: Pos) -> mb::TankState {
        let c = layout.controller;
        mb::TankState {
            bottom: pos[1] == c[1],
            top: pos[1] == c[1] + layout.height - 1,
            shape: mb::window_shape(layout.window, layout.width, pos[0] - c[0], pos[2] - c[2]),
        }
    }

    /// The block-state properties that differ from `want`.
    fn state_diff(game: &Game<'_>, pos: Pos, want: &mb::TankState) -> Vec<(&'static str, String)> {
        let wanted = [
            ("top", want.top.to_string()),
            ("bottom", want.bottom.to_string()),
            ("shape", want.shape.name().to_string()),
        ];
        let Some(state) = game
            .pack
            .assets
            .registry
            .state(game.world.block(pos[0], pos[1], pos[2]))
        else {
            return Vec::new();
        };
        wanted
            .into_iter()
            .filter(|(key, value)| {
                state.properties.get(*key).map(String::as_str) != Some(value.as_str())
            })
            .collect()
    }

    // ----- the pool (controller) behind any member -----

    struct Pool {
        ctrl: Pos,
        layout: Layout,
        fluid: Option<Fluid>,
    }

    /// Server-side view of the multiblock `pos` belongs to. None on clients,
    /// for unsupported or inconsistent records (the tick repairs the latter).
    fn pool(game: &Game<'_>, pos: Pos) -> Option<Pool> {
        if game.options.execution_role.is_client() || !game.native_on("create") {
            return None;
        }
        if !is_tank(game, pos) {
            return None;
        }
        let own = layout(pos, &tank_at(game, pos)?.nbt)?;
        let ctrl = tank_at(game, own.controller)?;
        let cl = layout(own.controller, &ctrl.nbt)?;
        if cl.controller != own.controller || (cl.width, cl.height) != (own.width, own.height) {
            return None;
        }
        let fluid = content(&ctrl.nbt, cl.capacity())?;
        Some(Pool { ctrl: own.controller, layout: cl, fluid })
    }

    pub(in crate::game) fn placement(_game: &GameLevel<'_>, _ctx: &PlacementContext) -> bool {
        // Stacks and wide tanks are formed by `reconcile` after placement.
        true
    }

    // ----- derived per-block fields -----

    fn comparator_of(nbt: &Compound) -> i32 {
        if let Some(Tag::Float(bits)) = nbt.get(&"Fill".into()) {
            return i32::from(mb::comparator_level(f64::from(f32::from_bits(*bits))));
        }
        // Legacy single tank without the derived `Fill`.
        let Some(l) = layout([0, 0, 0], nbt) else { return 0 };
        if nbt.contains_key(&"Controller".into()) {
            return 0;
        }
        let amount = content(nbt, l.capacity())
            .flatten()
            .map_or(0, |fluid| fluid.amount);
        i32::from(mb::comparator_level(f64::from(
            amount as f32 / l.capacity() as f32,
        )))
    }

    pub(super) fn comparator(entity: &ModBlockEntity) -> i32 {
        if entity.id != BLOCK {
            return 0;
        }
        comparator_of(&entity.nbt)
    }

    /// Everything a reconcile writes into one block entity.
    struct Write {
        pos: Pos,
        layout: Layout,
        content: Tag,
        fill: f32,
        luminosity: i32,
        state: mb::TankState,
    }

    fn put(nbt: &mut Compound, key: &str, tag: Tag) -> bool {
        if nbt.get(&key.into()) == Some(&tag) {
            return false;
        }
        nbt.insert(key.into(), tag);
        true
    }

    /// Returns (anything changed, comparator level changed).
    fn write_cell(nbt: &mut Compound, w: &Write) -> (bool, bool) {
        let before = comparator_of(nbt);
        let l = &w.layout;
        let mut changed = put(nbt, "Size", Tag::Int(l.width));
        changed |= put(nbt, "Height", Tag::Int(l.height));
        changed |= put(nbt, "Window", Tag::Byte(i8::from(l.window)));
        if l.controller == w.pos {
            changed |= nbt.remove(&"Controller".into()).is_some();
        } else {
            let offset = (0..3).map(|i| l.controller[i] - w.pos[i]).collect();
            changed |= put(nbt, "Controller", Tag::IntArray(offset));
        }
        changed |= put(nbt, "TankContent", w.content.clone());
        changed |= put(nbt, "Fill", Tag::Float(w.fill.to_bits()));
        changed |= put(nbt, "Luminosity", Tag::Int(w.luminosity));
        if l.controller != w.pos {
            // `splitMultiAndInvalidate` / merge: only a controller is a boiler.
            changed |= clear_boiler(nbt);
        }
        (changed, before != comparator_of(nbt))
    }

    // ----- reconcile: load, run the core, write back -----

    enum Op {
        /// A tank appeared: `form_multi` from these seeds.
        Form(Vec<Pos>),
        /// Repair after something vanished or disagrees; no new formation.
        Check(Pos),
        /// `onRemove` of a block that is still in the world.
        Remove(Pos),
        /// Wrench.
        Toggle(Pos),
    }

    type Rec = Option<(Layout, Option<Fluid>)>;

    fn entity_or_own<'a>(
        game: &'a Game<'_>,
        own: Option<(Pos, &'a ModBlockEntity)>,
        p: Pos,
    ) -> Option<&'a ModBlockEntity> {
        match own {
            Some((at, entity)) if at == p => Some(entity),
            _ => tank_at(game, p),
        }
    }

    /// Collect the tank blocks around the seeds. None defers: a chunk next to
    /// the cluster (or inside a recorded footprint) is not loaded, or the
    /// cluster is too large.
    fn load(
        game: &Game<'_>,
        seeds: &[Pos],
        own: Option<(Pos, &ModBlockEntity)>,
    ) -> Option<BTreeMap<Pos, Rec>> {
        let mut cells = BTreeMap::new();
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::new();
        for &seed in seeds {
            if is_tank(game, seed) && seen.insert(seed) {
                queue.push_back(seed);
            }
        }
        while let Some(p) = queue.pop_front() {
            if cells.len() >= MAX_CELLS {
                return None;
            }
            let rec: Rec = entity_or_own(game, own, p).and_then(|entity| {
                let l = layout(p, &entity.nbt)?;
                let fluid = if l.controller == p {
                    content(&entity.nbt, l.capacity())?
                } else {
                    None
                };
                Some((l, fluid))
            });
            let mut next: Vec<Pos> = [
                [1, 0, 0],
                [-1, 0, 0],
                [0, 1, 0],
                [0, -1, 0],
                [0, 0, 1],
                [0, 0, -1],
            ]
            .iter()
            .map(|d| [p[0] + d[0], p[1] + d[1], p[2] + d[2]])
            .collect();
            if let Some((l, _)) = &rec {
                next.push(l.controller);
                if l.controller == p {
                    next.extend(footprint(p, l.width, l.height));
                }
            }
            cells.insert(p, rec);
            for n in next {
                if !game.world.loaded(n[0], n[2]) {
                    return None;
                }
                if is_tank(game, n) && seen.insert(n) {
                    queue.push_back(n);
                }
            }
        }
        Some(cells)
    }

    #[derive(Default)]
    struct Keys(Vec<fluids::Key>);
    impl Keys {
        fn id(&mut self, key: &fluids::Key) -> u32 {
            let at = self.0.iter().position(|k| k == key).unwrap_or_else(|| {
                self.0.push(key.clone());
                self.0.len() - 1
            });
            at as u32 + 1
        }
        fn key(&self, id: u32) -> &fluids::Key {
            &self.0[id as usize - 1]
        }
    }

    /// Run the core on the loaded cells and describe every resulting write.
    fn build(cells: &BTreeMap<Pos, Rec>, op: &Op) -> Option<Vec<Write>> {
        let mut keys = Keys::default();
        let mut tw = TankWorld::new(TankConfig::default());
        let mut claimed = BTreeSet::new();
        let mut ghosts = Vec::new();
        // Saved multiblocks first. A footprint block that is not a tank
        // agreeing with the record is a ghost: it is broken below, which
        // splits the pool exactly like a break event would.
        for (&p, rec) in cells {
            let Some((l, fluid)) = rec else { continue };
            if l.controller != p {
                continue;
            }
            let content = fluid.as_ref().map(|f| (keys.id(&f.key), i64::from(f.amount)));
            let fp = footprint(p, l.width, l.height);
            if fp.iter().any(|q| claimed.contains(q)) {
                // Overlapping records: keep the block as a single.
                if claimed.insert(p) {
                    let single = content.map(|(id, a)| (id, a.min(i64::from(PER_BLOCK))));
                    tw.restore(p, p, 1, 1, single, l.window);
                }
                continue;
            }
            for &q in &fp {
                claimed.insert(q);
                let agrees = q == p
                    || matches!(cells.get(&q), Some(Some((ql, _)))
                        if ql.controller == p && ql.width == l.width && ql.height == l.height);
                tw.restore(q, p, l.width, l.height, if q == p { content } else { None }, l.window);
                if !agrees {
                    ghosts.push(q);
                }
            }
        }
        tw.remove_all(&ghosts);
        // Everything else (orphan members, overlap losers, new tanks).
        for (&p, rec) in cells {
            let Some((l, fluid)) = rec else { continue };
            if tw.contains(p) {
                continue;
            }
            let single = fluid
                .as_ref()
                .map(|f| (keys.id(&f.key), i64::from(f.amount.min(PER_BLOCK))));
            tw.restore(p, p, 1, 1, single, l.window);
        }
        match op {
            Op::Form(seeds) => {
                for &s in seeds {
                    if tw.contains(s) {
                        tw.form_multi(s);
                    }
                }
                tw.update_connectivity();
            }
            Op::Check(_) => {}
            Op::Remove(p) => {
                if tw.contains(*p) {
                    tw.remove(*p);
                }
            }
            Op::Toggle(p) => {
                tw.toggle_windows(*p);
            }
        }
        let mut out = Vec::new();
        for q in tw.positions() {
            let s = tw.snapshot(q)?;
            let c = tw.snapshot(s.controller)?;
            let state = tw.block_state(q)?;
            let layout = Layout {
                controller: s.controller,
                width: c.width,
                height: c.height,
                window: c.window,
            };
            let capacity = layout.capacity();
            let fluid = c
                .content
                .map(|(id, amount)| Fluid { key: keys.key(id).clone(), amount: amount as u32 });
            let fill = fluid.as_ref().map_or(0.0, |f| f.amount as f32 / capacity as f32);
            let lava = fluid.as_ref().is_some_and(|f| f.key.id == "minecraft:lava");
            let luminosity = if lava && layout.window {
                i32::from(mb::layer_luminosity(fill, layout.height, 15, false)
                    [(q[1] - s.controller[1]) as usize])
            } else {
                0
            };
            let tag = if s.controller == q {
                fluids::write_content(fluid.as_ref(), capacity).ok()?
            } else {
                Tag::Compound(Compound::new())
            };
            out.push(Write { pos: q, layout, content: tag, fill, luminosity, state });
        }
        Some(out)
    }

    fn write_back(game: &mut Game<'_>, writes: &[Write], mut own: Option<(Pos, &mut ModBlockEntity)>) {
        for w in writes {
            // Block-state properties first; the NBT write follows.
            for (key, value) in state_diff(game, w.pos, &w.state) {
                game.native_set_block_property_with(
                    w.pos,
                    key,
                    &value,
                    horizon_world::level::flags::UPDATE_CLIENTS,
                );
            }
            let (changed, level) = match own.as_mut() {
                Some((at, entity)) if *at == w.pos => write_cell(&mut entity.nbt, w),
                _ => game
                    .native_with_mod_block_entity(w.pos, |entity| write_cell(&mut entity.nbt, w))
                    .unwrap_or((false, false)),
            };
            if changed {
                game.native_block_entity_sync(w.pos);
            }
            if level {
                game.native_update_output_neighbors(game.dim.id(), w.pos);
            }
        }
    }

    /// Returns false when nothing was done (client, deferred, unsupported).
    fn reconcile(
        game: &mut Game<'_>,
        op: Op,
        own: Option<(Pos, &mut ModBlockEntity)>,
    ) -> bool {
        if game.options.execution_role.is_client() || !game.native_on("create") {
            return false;
        }
        let seeds = match &op {
            Op::Form(seeds) => seeds.clone(),
            Op::Check(p) | Op::Remove(p) | Op::Toggle(p) => vec![*p],
        };
        let writes = {
            let own_ref = own.as_ref().map(|(p, e)| (*p, &**e));
            let Some(cells) = load(game, &seeds, own_ref) else {
                return false;
            };
            let Some(writes) = build(&cells, &op) else {
                return false;
            };
            writes
        };
        write_back(game, &writes, own);
        true
    }

    /// A tank block was placed at `pos` (call after the block exists). The
    /// tick forms it anyway within `CADENCE` ticks; this is the immediate path.
    pub(in crate::game) fn placed(game: &mut Game<'_>, pos: Pos) {
        if game.native_block(pos) == BLOCK {
            reconcile(game, Op::Form(vec![pos]), None);
        }
    }

    /// The tank at `pos` is about to be broken (block still present): split
    /// the multiblock with the original pool redistribution.
    pub(in crate::game) fn destroyed(game: &mut Game<'_>, pos: Pos) {
        if game.native_block(pos) == BLOCK {
            reconcile(game, Op::Remove(pos), None);
        }
    }

    fn consistent(game: &Game<'_>, pos: Pos, l: &Layout) -> bool {
        if !state_diff(game, pos, &wanted_state(l, pos)).is_empty() {
            return false;
        }
        if l.controller != pos {
            return tank_at(game, l.controller)
                .and_then(|e| layout(l.controller, &e.nbt))
                .is_some_and(|c| {
                    c.controller == l.controller
                        && (c.width, c.height, c.window) == (l.width, l.height, l.window)
                });
        }
        footprint(pos, l.width, l.height).into_iter().skip(1).all(|q| {
            tank_at(game, q)
                .and_then(|e| layout(q, &e.nbt))
                .is_some_and(|m| {
                    m.controller == pos && (m.width, m.height) == (l.width, l.height)
                })
        })
    }

    /// First tick forms the multiblock (`LastKnownPos` marks it done); later
    /// ticks only verify the group and repair it when a member vanished.
    pub(in crate::game) fn tick(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity) {
        let slot = i64::from(
            (pos[0].wrapping_mul(31) ^ pos[1].wrapping_mul(7) ^ pos[2].wrapping_mul(17))
                .rem_euclid(CADENCE as i32),
        );
        if (game.world.game_time + slot).rem_euclid(CADENCE) != 0 {
            return;
        }
        let Some(l) = layout(pos, &entity.nbt) else {
            return;
        };
        let first = !entity.nbt.contains_key(&"LastKnownPos".into());
        let op = if first {
            Op::Form(vec![pos])
        } else if !consistent(game, pos, &l) {
            Op::Check(pos)
        } else {
            if l.controller == pos {
                let phase = (game.world.game_time + slot).div_euclid(CADENCE);
                boiler_step(game, pos, entity, &l, phase);
            }
            return;
        };
        if reconcile(game, op, Some((pos, &mut *entity))) && first {
            entity
                .nbt
                .insert("LastKnownPos".into(), Tag::IntArray(pos.to_vec()));
            game.world.modified.insert((pos[0] >> 4, pos[2] >> 4));
        }
    }

    // ----- boiler (BoilerData adapter; rules in create_engine::boiler) -----
    //
    // The controller's `Boiler` compound holds the persisted `BoilerData`:
    // ActiveHeat Int, PassiveHeat Byte, Engines Int, Whistles Int, Supply Float,
    // Update Byte (heat refresh / re-count requested), plus, only while the
    // boiler is active: Level Int, Efficiency Float, Gathered Int, Ring List<Float>,
    // RingIndex Int, SampleIn Int. An inactive tank keeps the pristine compound.

    use create_engine::{
        boiler::{self as bo, Boiler, Sampling},
        burner::Heat as BurnerHeat,
        placement::Direction as Dir,
    };

    const WATER: &str = "minecraft:water";
    /// Debug switch. Create never stores boiler water: only fluid that flows in
    /// while the boiler is active counts. When true, a water pool in the tank
    /// also counts as a steady supply of `POOL_SUPPLY` mB per tick.
    const POOL_FEEDS_BOILER: bool = false;
    const POOL_SUPPLY: i32 = 200;

    fn bint(c: &Compound, key: &str) -> i32 {
        match c.get(&key.into()) {
            Some(Tag::Int(v)) => *v,
            Some(Tag::Byte(v)) => i32::from(*v),
            _ => 0,
        }
    }

    fn bfloat(c: &Compound, key: &str) -> f32 {
        match c.get(&key.into()) {
            Some(Tag::Float(bits)) => f32::from_bits(*bits),
            _ => 0.0,
        }
    }

    fn boiler_compound(nbt: &Compound) -> Option<&Compound> {
        match nbt.get(&"Boiler".into()) {
            Some(Tag::Compound(c)) => Some(c),
            _ => None,
        }
    }

    fn pristine_boiler() -> Compound {
        Compound::from([
            ("ActiveHeat".into(), Tag::Int(0)),
            ("Engines".into(), Tag::Int(0)),
            ("PassiveHeat".into(), Tag::Byte(0)),
            ("Supply".into(), Tag::Float(0)),
            ("Update".into(), Tag::Byte(0)),
            ("Whistles".into(), Tag::Int(0)),
        ])
    }

    /// Reset a non-controller (or deactivated) block to the pristine compound.
    fn clear_boiler(nbt: &mut Compound) -> bool {
        put(nbt, "Boiler", Tag::Compound(pristine_boiler()))
    }

    fn read_boiler(nbt: &Compound) -> Boiler {
        let mut b = Boiler::default();
        let Some(c) = boiler_compound(nbt) else {
            return b;
        };
        b.active_heat = bint(c, "ActiveHeat");
        b.passive = bint(c, "PassiveHeat") != 0;
        b.attached_engines = bint(c, "Engines");
        b.attached_whistles = bint(c, "Whistles");
        b.water_supply = bfloat(c, "Supply");
        b.needs_heat_update = bint(c, "Update") != 0;
        let mut ring = [0.0; bo::RING];
        if let Some(Tag::List(list)) = c.get(&"Ring".into()) {
            for (slot, tag) in ring.iter_mut().zip(list) {
                if let Tag::Float(bits) = tag {
                    *slot = f32::from_bits(*bits);
                }
            }
        }
        b.set_sampling(Sampling {
            ring,
            ticks_until_next_sample: bint(c, "SampleIn"),
            index: bint(c, "RingIndex").max(0) as usize,
            gathered: bint(c, "Gathered"),
        });
        b
    }

    fn boiler_tag(b: &Boiler, size: i32) -> Compound {
        let s = b.sampling();
        Compound::from([
            ("ActiveHeat".into(), Tag::Int(b.active_heat)),
            ("Engines".into(), Tag::Int(b.attached_engines)),
            ("PassiveHeat".into(), Tag::Byte(i8::from(b.passive))),
            ("Supply".into(), Tag::Float(b.water_supply.to_bits())),
            ("Update".into(), Tag::Byte(i8::from(b.needs_heat_update))),
            ("Whistles".into(), Tag::Int(b.attached_whistles)),
            ("Level".into(), Tag::Int(b.level(size))),
            ("Efficiency".into(), Tag::Float(b.engine_efficiency(size).to_bits())),
            ("Gathered".into(), Tag::Int(s.gathered)),
            (
                "Ring".into(),
                Tag::List(s.ring.iter().map(|v| Tag::Float(v.to_bits())).collect()),
            ),
            ("RingIndex".into(), Tag::Int(s.index as i32)),
            ("SampleIn".into(), Tag::Int(s.ticks_until_next_sample)),
        ])
    }

    /// True while engines or whistles are attached to the controller `ctrl`.
    fn boiler_active(game: &Game<'_>, ctrl: Pos) -> bool {
        tank_at(game, ctrl)
            .and_then(|e| boiler_compound(&e.nbt))
            .is_some_and(|c| bint(c, "Engines") > 0 || bint(c, "Whistles") > 0)
    }

    /// `BoilerFluidHandler.fill`: the amount only feeds the water supply.
    fn gather(game: &mut Game<'_>, ctrl: Pos, amount: i32) {
        game.native_with_mod_block_entity(ctrl, |e| {
            if let Some(Tag::Compound(c)) = e.nbt.get_mut(&"Boiler".into()) {
                let total = bint(c, "Gathered").saturating_add(amount);
                c.insert("Gathered".into(), Tag::Int(total));
            }
        });
    }

    /// Pipe transfer into an active boiler: drain the source, count the supply.
    fn feed_boiler(
        game: &mut Game<'_>,
        source: Pos,
        ctrl: Pos,
        mut from: Fluid,
        kind: create_engine::fluid_transport::Fluid,
        rate: i32,
    ) {
        if from.key.id != kind.id() || from.key.id != WATER || !from.key.components.is_empty() {
            return;
        }
        let moved = (rate as u32).min(from.amount);
        if moved == 0 {
            return;
        }
        from.amount -= moved;
        let Some(fields) = replacement(game, source, (from.amount > 0).then_some(&from)) else {
            return;
        };
        let mut notes = Vec::new();
        if !apply(game, source, fields, &mut notes) {
            return;
        }
        gather(game, ctrl, moved as i32);
        notes.sort_unstable();
        notes.dedup();
        for at in notes {
            notify(game, at);
        }
        notify_endpoint(game, source);
    }

    /// What a Steam Engine needs from the boiler it sits on.
    pub(in crate::game) struct Steam {
        pub efficiency: f32,
        pub level: i32,
        pub active: bool,
    }

    fn controller_of(game: &Game<'_>, pos: Pos) -> Option<Pos> {
        if !is_tank(game, pos) {
            return None;
        }
        Some(layout(pos, &tank_at(game, pos)?.nbt)?.controller)
    }

    /// Boiler values of the multiblock containing the tank block `pos`
    /// (written by `boiler_step`, at most five ticks old).
    pub(in crate::game) fn steam(game: &Game<'_>, pos: Pos) -> Option<Steam> {
        let ctrl = controller_of(game, pos)?;
        let c = boiler_compound(&tank_at(game, ctrl)?.nbt)?;
        Some(Steam {
            efficiency: bfloat(c, "Efficiency").clamp(0.0, 1.0),
            level: bint(c, "Level"),
            active: bint(c, "Engines") > 0 || bint(c, "Whistles") > 0,
        })
    }

    /// An engine or whistle was placed or removed: re-count at the next step.
    pub(in crate::game) fn request_update(game: &mut Game<'_>, pos: Pos) {
        let Some(ctrl) = controller_of(game, pos) else {
            return;
        };
        let pending = tank_at(game, ctrl)
            .and_then(|e| boiler_compound(&e.nbt))
            .is_none_or(|c| bint(c, "Update") == 0);
        if pending {
            game.native_with_mod_block_entity(ctrl, |e| {
                if let Some(Tag::Compound(c)) = e.nbt.get_mut(&"Boiler".into()) {
                    c.insert("Update".into(), Tag::Byte(1));
                }
            });
        }
    }

    /// `BoilerHeaters`: heat of the block `p` (-1 none, 0 passive, 1 or 2 active).
    fn heater(game: &Game<'_>, p: Pos) -> i32 {
        if !game.world.loaded(p[0], p[2]) {
            return bo::NO_HEAT;
        }
        let Some(state) = game
            .pack
            .assets
            .registry
            .state(game.world.block(p[0], p[1], p[2]))
        else {
            return bo::NO_HEAT;
        };
        if let Some(blaze) = state.properties.get("blaze") {
            return bo::blaze_burner_heat(match blaze.as_str() {
                "smouldering" => BurnerHeat::Smouldering,
                "fading" => BurnerHeat::Fading,
                "kindled" => BurnerHeat::Kindled,
                "seething" => BurnerHeat::Seething,
                _ => BurnerHeat::None,
            });
        }
        let passive = game
            .native_server_block_in_tag(state, "create:passive_boiler_heaters")
            .unwrap_or(false);
        if !passive {
            return bo::NO_HEAT;
        }
        let campfire = game
            .native_server_block_in_tag(state, "minecraft:campfires")
            .unwrap_or(false);
        let lit = state.properties.get("lit").map(|v| v == "true");
        bo::passive_heat(true, !rules::is_not_unheated(campfire, lit, None))
    }

    struct Probe<'a, 'b>(&'a Game<'b>);

    impl bo::AttachmentProbe for Probe<'_, '_> {
        fn engine_facing(&self, p: Pos) -> Option<Dir> {
            if !self.0.world.loaded(p[0], p[2]) {
                return None;
            }
            let state = self
                .0
                .pack
                .assets
                .registry
                .state(self.0.world.block(p[0], p[1], p[2]))?;
            crate::game::create_port_steam::engine_direction(state)
        }

        fn whistle(&self, p: Pos) -> Option<(Dir, i32)> {
            if !self.0.world.loaded(p[0], p[2]) {
                return None;
            }
            let state = self
                .0
                .pack
                .assets
                .registry
                .state(self.0.world.block(p[0], p[1], p[2]))?;
            crate::game::create_port_steam::whistle_attachment(state).map(|d| (d, 0))
        }
    }

    /// One `BoilerData.tick` worth of work per `CADENCE` ticks on a controller:
    /// recount attachments, read the heat under the footprint, sample the water
    /// supply, store the result and force the windows off while active.
    fn boiler_step(
        game: &mut Game<'_>,
        pos: Pos,
        entity: &mut ModBlockEntity,
        l: &Layout,
        phase: i64,
    ) {
        let size = l.width * l.width * l.height;
        let mut b = read_boiler(&entity.nbt);
        let dirty = b.needs_heat_update;
        // Idle tanks poll slowly; the engine requests an immediate re-count.
        if dirty || b.is_active() || phase.rem_euclid(4) == 0 {
            let blocks = footprint(pos, l.width, l.height);
            b.evaluate(&blocks, &Probe(&*game));
        }
        if !b.is_active() {
            if boiler_compound(&entity.nbt) != Some(&pristine_boiler())
                && clear_boiler(&mut entity.nbt)
            {
                game.native_block_entity_sync(pos);
            }
            return;
        }
        if POOL_FEEDS_BOILER
            && let Some(Some(fluid)) = content(&entity.nbt, l.capacity())
            && fluid.key.id == WATER
            && fluid.amount > 0
        {
            b.fill_water(POOL_SUPPLY * bo::SAMPLE_RATE, false);
        }
        let capacity = i64::from(l.capacity());
        let world: &Game<'_> = &*game;
        let mut heaters = || {
            let mut out = Vec::with_capacity((l.width * l.width) as usize);
            for x in 0..l.width {
                for z in 0..l.width {
                    out.push(heater(world, [pos[0] + x, pos[1] - 1, pos[2] + z]));
                }
            }
            out
        };
        // One cadence is five ticks: run the five sampling steps it stands for.
        for _ in 0..CADENCE {
            b.tick_server(size, capacity, false, &mut heaters);
        }
        let before = boiler_compound(&entity.nbt)
            .cloned()
            .unwrap_or_else(Compound::new);
        let new = boiler_tag(&b, size);
        let public = |c: &Compound| {
            ["ActiveHeat", "Engines", "Whistles", "Level", "PassiveHeat"].map(|k| bint(c, k))
        };
        let visible_change = public(&before) != public(&new)
            || bfloat(&before, "Efficiency") != b.engine_efficiency(size);
        put(&mut entity.nbt, "Boiler", Tag::Compound(new));
        game.world.modified.insert((pos[0] >> 4, pos[2] >> 4));
        if visible_change {
            game.native_block_entity_sync(pos);
        }
        if l.window {
            // Activating a boiler forces the windows off.
            reconcile(game, Op::Toggle(pos), Some((pos, &mut *entity)));
        }
    }

    // ----- fluid access (always through the controller) -----

    pub(super) fn view(game: &Game<'_>, pos: Pos) -> Option<Vec<NativeFluidTankView>> {
        let pool = pool(game, pos)?;
        Some(vec![NativeFluidTankView {
            tank: 0,
            content: pool.fluid.as_ref().map(native_fluid),
            capacity: pool.layout.capacity() as i32,
            can_fill: true,
            can_drain: true,
        }])
    }

    fn notify(game: &mut Game<'_>, pos: Pos) {
        game.native_block_entity_sync(pos);
        game.native_update_output_neighbors(game.dim.id(), pos);
    }

    /// Store `fluid` as the pool of the multiblock of `pos` and refresh the
    /// derived fields of every member. Returns the blocks to notify (the
    /// controller, plus members whose comparator level changed).
    fn set_pool(game: &mut Game<'_>, pos: Pos, fluid: Option<&Fluid>) -> Option<Vec<Pos>> {
        let pool = pool(game, pos)?;
        let l = pool.layout;
        let members = footprint(pool.ctrl, l.width, l.height);
        if members.iter().any(|&q| tank_at(game, q).is_none()) {
            return None;
        }
        let capacity = l.capacity();
        let content = fluids::write_content(fluid, capacity).ok()?;
        let fill = fluid.map_or(0.0, |f| f.amount as f32 / capacity as f32);
        let lava = fluid.is_some_and(|f| f.key.id == "minecraft:lava");
        let layers = mb::layer_luminosity(fill, l.height, 15, false);
        let level = i32::from(mb::comparator_level(f64::from(fill)));
        let mut notes = Vec::new();
        for q in members {
            let luminosity = if lava && l.window {
                i32::from(layers[(q[1] - pool.ctrl[1]) as usize])
            } else {
                0
            };
            let is_ctrl = q == pool.ctrl;
            let level_changed = game.native_with_mod_block_entity(q, |entity| {
                let before = comparator_of(&entity.nbt);
                entity.nbt.insert("Fill".into(), Tag::Float(fill.to_bits()));
                entity.nbt.insert("Luminosity".into(), Tag::Int(luminosity));
                if is_ctrl {
                    entity.nbt.insert("TankContent".into(), content.clone());
                }
                before != level
            })?;
            if is_ctrl || level_changed {
                notes.push(q);
            }
        }
        Some(notes)
    }

    fn commit(game: &mut Game<'_>, pos: Pos, fluid: Option<&Fluid>) -> bool {
        let Some(notes) = set_pool(game, pos, fluid) else {
            return false;
        };
        for at in notes {
            notify(game, at);
        }
        true
    }

    pub(super) fn endpoint(game: &Game<'_>, pos: Pos) -> bool {
        pool(game, pos)
            .is_some_and(|pool| pool.fluid.is_none_or(|fluid| fluid.key.components.is_empty()))
    }

    // Endpoints share a strict canonical fluid key; item cargo remains owned by
    // the Drain. A pipe transfer never edits HeldItem or its processing phase.
    fn stored(game: &Game<'_>, pos: Pos) -> Option<Option<Fluid>> {
        match game.native_block(pos).as_str() {
            item_drain::BLOCK => item_drain::stored(game, pos),
            crate::game::create_port_spout::BLOCK => {
                crate::game::create_port_spout::stored(game, pos)
            }
            BLOCK => pool(game, pos).map(|pool| pool.fluid),
            _ => None,
        }
    }

    pub(super) fn sample(
        game: &Game<'_>,
        pos: Pos,
    ) -> Option<create_engine::fluid_transport::Fluid> {
        let fluid = stored(game, pos)??;
        if !fluid.key.components.is_empty() {
            return None;
        }
        create_engine::fluid_transport::Fluid::from_id(&fluid.key.id)
    }

    pub(super) fn amount(game: &Game<'_>, pos: Pos) -> Option<u32> {
        Some(stored(game, pos)?.map_or(0, |fluid| fluid.amount))
    }

    enum Fields {
        Tank(Option<Fluid>),
        Drain(Tag),
        Spout(Tag),
    }

    fn replacement(game: &Game<'_>, pos: Pos, fluid: Option<&Fluid>) -> Option<Fields> {
        match game.native_block(pos).as_str() {
            item_drain::BLOCK => item_drain::replacement(game, pos, fluid).map(Fields::Drain),
            crate::game::create_port_spout::BLOCK => {
                crate::game::create_port_spout::replacement(game, pos, fluid).map(Fields::Spout)
            }
            BLOCK => {
                let pool = pool(game, pos)?;
                // Preflight the codec so `apply` cannot fail on the content.
                fluids::write_content(fluid, pool.layout.capacity()).ok()?;
                Some(Fields::Tank(fluid.cloned()))
            }
            _ => None,
        }
    }

    /// `notes` collects the tank blocks `notify_endpoint` must notify.
    fn apply(game: &mut Game<'_>, pos: Pos, fields: Fields, notes: &mut Vec<Pos>) -> bool {
        match fields {
            Fields::Tank(fluid) => match set_pool(game, pos, fluid.as_ref()) {
                Some(list) => {
                    notes.extend(list);
                    true
                }
                None => false,
            },
            Fields::Drain(tanks) => {
                game.native_with_mod_block_entity(pos, |e| item_drain::set_raw(e, tanks))
                    == Some(true)
            }
            Fields::Spout(tanks) => {
                game.native_with_mod_block_entity(pos, |e| {
                    crate::game::create_port_spout::set_raw(e, tanks)
                }) == Some(true)
            }
        }
    }

    fn notify_endpoint(game: &mut Game<'_>, pos: Pos) {
        match game.native_block(pos).as_str() {
            item_drain::BLOCK => item_drain::notify_fluid(game, pos),
            crate::game::create_port_spout::BLOCK => {
                crate::game::create_port_spout::notify_fluid(game, pos)
            }
            _ => {}
        }
    }

    pub(super) fn transfer_pair(
        game: &mut Game<'_>,
        source: Pos,
        target: Pos,
        kind: create_engine::fluid_transport::Fluid,
        rate: i32,
    ) {
        if source == target
            || rate <= 0
            || !matches!(
                game.native_block(source).as_str(),
                BLOCK | item_drain::BLOCK
            )
        {
            return;
        }
        let capacity = match game.native_block(target).as_str() {
            BLOCK => match pool(game, target) {
                Some(pool) => pool.layout.capacity(),
                None => return,
            },
            crate::game::create_port_spout::BLOCK => crate::game::create_port_spout::CAPACITY,
            _ => return, // The original Item Drain forbids fluid insertion.
        };
        // Two members of one multiblock share a pool: nothing to move.
        if game.native_block(source) == BLOCK
            && game.native_block(target) == BLOCK
            && matches!((pool(game, source), pool(game, target)),
                (Some(a), Some(b)) if a.ctrl == b.ctrl)
        {
            return;
        }
        let Some(Some(mut from)) = stored(game, source) else {
            return;
        };
        let Some(to) = stored(game, target) else {
            return;
        };
        if game.native_block(source) == BLOCK
            && pool(game, source).is_some_and(|p| boiler_active(game, p.ctrl))
        {
            return; // an active boiler cannot be drained
        }
        if game.native_block(target) == BLOCK
            && let Some(p) = pool(game, target)
            && boiler_active(game, p.ctrl)
        {
            feed_boiler(game, source, p.ctrl, from, kind, rate);
            return;
        }
        if from.key.id != kind.id()
            || !from.key.components.is_empty()
            || to.as_ref().is_some_and(|fluid| fluid.key != from.key)
        {
            return;
        }
        let received = to.as_ref().map_or(0, |fluid| fluid.amount);
        let moved = (rate as u32)
            .min(from.amount)
            .min(capacity.saturating_sub(received));
        if moved == 0 {
            return;
        }
        let Some(original) = any_at(game, source).map(|e| e.nbt.clone()) else {
            return;
        };
        let before = from.clone();
        let into = Fluid {
            key: from.key.clone(),
            amount: received + moved,
        };
        from.amount -= moved;
        let (Some(source_fields), Some(target_fields)) = (
            replacement(game, source, (from.amount > 0).then_some(&from)),
            replacement(game, target, Some(&into)),
        ) else {
            return;
        };
        // Preflight both owned records. No callbacks or notifications run
        // between writes. Restore the source if the receiver disappears.
        let mut notes = Vec::new();
        if !apply(game, source, source_fields, &mut notes) {
            return;
        }
        if !apply(game, target, target_fields, &mut notes) {
            if game.native_block(source) == BLOCK {
                set_pool(game, source, Some(&before));
            } else {
                game.native_with_mod_block_entity(source, |e| {
                    e.nbt = original;
                });
            }
            return;
        }
        notes.sort_unstable();
        notes.dedup();
        for at in notes {
            notify(game, at);
        }
        notify_endpoint(game, source);
        notify_endpoint(game, target);
    }

    pub(super) fn fill(
        game: &mut Game<'_>,
        pos: Pos,
        resource: &NativeFluidStack,
        action: NativeFluidAction,
    ) -> Option<i32> {
        let pool = pool(game, pos)?;
        let Some(mut fluid) = core_fluid(resource) else {
            return Some(0);
        };
        if boiler_active(game, pool.ctrl) {
            // BoilerFluidHandler.fill: only water, always accepted, counted as
            // supply and never stored.
            if fluid.key.id != WATER || !fluid.key.components.is_empty() {
                return Some(0);
            }
            if matches!(action, NativeFluidAction::Execute) {
                gather(game, pool.ctrl, fluid.amount as i32);
            }
            return Some(fluid.amount as i32);
        }
        let stored = match pool.fluid {
            Some(current) if current.key == fluid.key => current.amount,
            Some(_) => return Some(0),
            None => 0,
        };
        let accepted = fluid.amount.min(pool.layout.capacity().saturating_sub(stored));
        #[allow(unreachable_patterns)]
        match action {
            NativeFluidAction::Simulate => {}
            NativeFluidAction::Execute => {
                fluid.amount = stored + accepted;
                if accepted != 0 && !commit(game, pos, Some(&fluid)) {
                    return Some(0);
                }
            }
            _ => return Some(0),
        }
        Some(accepted as i32)
    }

    pub(super) fn drain(
        game: &mut Game<'_>,
        pos: Pos,
        request: &NativeFluidDrain,
        action: NativeFluidAction,
    ) -> Option<NativeFluidStack> {
        let pool = pool(game, pos)?;
        if boiler_active(game, pool.ctrl) {
            return None; // BoilerFluidHandler.drain is empty.
        }
        let mut fluid = pool.fluid?;
        let maximum = match request {
            NativeFluidDrain::Any(maximum) => *maximum,
            NativeFluidDrain::Exact(wanted)
                if wanted.fluid == fluid.key.id && wanted.components == fluid.key.components =>
            {
                wanted.amount
            }
            _ => return None,
        };
        if maximum <= 0 {
            return None;
        }
        let drained = fluid.amount.min(maximum as u32);
        #[allow(unreachable_patterns)]
        match action {
            NativeFluidAction::Simulate => {}
            NativeFluidAction::Execute => {
                fluid.amount -= drained;
                if !commit(game, pos, (fluid.amount > 0).then_some(&fluid)) {
                    return None;
                }
            }
            _ => return None,
        }
        fluid.amount = drained;
        Some(native_fluid(&fluid))
    }

    pub(in crate::game) fn predicts_use(game: &Game<'_>, pos: Pos) -> bool {
        game.native_on("create")
            && game.native_block(pos) == BLOCK
            && !game.native_sneaking()
            && game
                .held()
                .filter(|held| held.count > 0)
                .is_some_and(|held| {
                    game.may_build() && crate::game::create_port_chassis::wrench(game, held)
                        || game.creative
                            && matches!(
                                held.item.as_ref(),
                                "minecraft:bucket"
                                    | "minecraft:water_bucket"
                                    | "minecraft:lava_bucket"
                            )
                })
    }

    pub(in crate::game) fn right_click(game: &mut Game<'_>, pos: Pos) -> bool {
        if !predicts_use(game, pos) {
            return false;
        }
        let Some(pool) = pool(game, pos) else {
            return true;
        };
        let before = pool.fluid;
        let Some(held) = game.held().cloned() else {
            return false;
        };
        if crate::game::create_port_chassis::wrench(game, &held) {
            // toggleWindows: every block of the multiblock changes shape,
            // except while the boiler is active.
            if boiler_active(game, pool.ctrl) {
                return true;
            }
            reconcile(game, Op::Toggle(pos), None);
            return true;
        }
        let (fluid, amount, emptying) = if held.item.as_ref() == "minecraft:bucket" {
            let Some(mut resource) = before
                .as_ref()
                .map(native_fluid)
                .filter(|f| f.components.is_empty())
            else {
                return true;
            };
            if resource.amount < BUCKET
                || game
                    .native_item_fluid_fill(&held, &resource.fluid, BUCKET)
                    .is_none_or(|(amount, _)| amount != BUCKET)
            {
                return true;
            }
            let remaining = resource.amount - BUCKET;
            resource.amount = BUCKET;
            let request = NativeFluidDrain::Exact(resource);
            let mut drained = None;
            game.native_custom_fluid_drain(
                pos,
                None,
                &request,
                NativeFluidAction::Execute,
                &mut drained,
            );
            let Some(drained) = drained else {
                return true;
            };
            (drained.fluid, remaining, false)
        } else {
            let Some((fluid, amount, _)) = game.native_item_fluid_drain(&held, BUCKET) else {
                return true;
            };
            let resource = NativeFluidStack {
                fluid,
                amount,
                components: Compound::new(),
            };
            let mut accepted = None;
            game.native_custom_fluid_fill(
                pos,
                None,
                &resource,
                NativeFluidAction::Simulate,
                &mut accepted,
            );
            if accepted != Some(amount) {
                return true;
            }
            accepted = None;
            game.native_custom_fluid_fill(
                pos,
                None,
                &resource,
                NativeFluidAction::Execute,
                &mut accepted,
            );
            if accepted != Some(amount) {
                return true;
            }
            (
                resource.fluid,
                before.as_ref().map_or(0, |f| f.amount as i32) + amount,
                true,
            )
        };
        play_bucket(game, pos, &fluid, amount, emptying);
        true
    }

    fn play_bucket(game: &mut Game<'_>, pos: Pos, fluid: &str, amount: i32, emptying: bool) {
        let event = match (emptying, fluid) {
            (true, "minecraft:lava") => "minecraft:item.bucket.empty_lava",
            (false, "minecraft:lava") => "minecraft:item.bucket.fill_lava",
            (true, _) => "minecraft:item.bucket.empty",
            (false, _) => "minecraft:item.bucket.fill",
        };
        game.with_level(|_, level, _| {
            let Some(random) = level.native_level_random() else {
                return;
            };
            let pitch = mb::interaction_pitch(i64::from(amount), i64::from(PER_BLOCK))
                + (random.next_float() - 0.5) / 4.0;
            level.native_play_sound(
                event,
                pos.map(|coordinate| f64::from(coordinate) + 0.5),
                4,
                0.5,
                pitch,
            );
        });
    }

    // ----- client: rendering -----

    /// What the renderer needs from a controller's block entity.
    struct View {
        width: i32,
        height: i32,
        window: bool,
        fluid: Option<Fluid>,
    }

    fn view_of(entity: &ModBlockEntity) -> Option<View> {
        if entity.id != BLOCK || entity.nbt.contains_key(&"Controller".into()) {
            return None;
        }
        let l = layout([0, 0, 0], &entity.nbt)?;
        Some(View {
            width: l.width,
            height: l.height,
            window: l.window,
            fluid: content(&entity.nbt, l.capacity())?,
        })
    }

    fn fraction(view: &View) -> f32 {
        let capacity = (view.width * view.width * view.height) as f32 * PER_BLOCK as f32;
        view.fluid.as_ref().map_or(0.0, |f| f.amount as f32 / capacity)
    }

    /// `getRenderBoundingBox`: a controller draws the whole multiblock.
    pub(in crate::game) fn render_bounds(entity: &ModBlockEntity, bounds: &mut [f32; 6]) {
        if let Some(view) = view_of(entity) {
            *bounds = [0.0, 0.0, 0.0, view.width as f32, view.height as f32, view.width as f32];
        }
    }

    pub(super) fn client_tick(
        session: usize,
        dimension: &str,
        time: i64,
        entries: &[(Pos, &ModBlockEntity)],
    ) {
        RENDER.with(|cache| {
            let mut cache = cache.borrow_mut();
            let cache = cache.dimension(session, dimension);
            let mut live = std::collections::BTreeSet::new();
            for &(pos, entity) in entries.iter().filter(|(_, e)| e.id == BLOCK) {
                let Some(view) = view_of(entity) else {
                    continue;
                };
                live.insert(pos);
                let owner = entity.revision();
                let target = fraction(&view);
                let motion = cache.entry(pos).or_default();
                if motion.tick.is_some_and(|old| old > time)
                    || motion
                        .owner
                        .is_none_or(|old| !old.is_same_create_entity(owner))
                {
                    *motion = RenderLevel {
                        owner: Some(owner),
                        tick: None,
                        chase: rules::FluidLevel {
                            previous: target,
                            value: target,
                            target,
                            speed: 0.5,
                        },
                    };
                }
                motion.chase.target = target;
                if motion.tick != Some(time) {
                    motion.chase.tick();
                    motion.tick = Some(time);
                }
            }
            cache.retain(|pos, _| live.contains(pos));
        });
    }

    pub(in crate::game) fn draw(
        game: &Game<'_>,
        pos: Pos,
        entity: &ModBlockEntity,
        draw: &mut BlockEntityDraw,
    ) {
        let Some(view) = view_of(entity) else {
            return;
        };
        let (true, Some(fluid)) = (view.window, view.fluid.as_ref()) else {
            return;
        };
        let level = RENDER.with(|cache| {
            cache
                .borrow_mut()
                .dimension(std::ptr::from_ref(&game.world) as usize, game.dim.id())
                .get(&pos)
                .filter(|motion| {
                    motion
                        .owner
                        .is_some_and(|old| old.is_same_create_entity(entity.revision()))
                })
                .map_or_else(|| fraction(&view), |motion| motion.chase.at(draw.partial))
        });
        // FluidTankRenderer: the fluid column spans the inside of every layer.
        let total_height = view.height as f32 - 2.0 * CAP_HEIGHT - MIN_PUDDLE;
        if level < 1.0 / (512.0 * total_height) {
            return;
        }
        let hull = 1.0 / 16.0 + 1.0 / 128.0;
        let floor = CAP_HEIGHT + MIN_PUDDLE;
        let height = (level * total_height).clamp(0.0, total_height);
        let far = view.width as f32 - hull;
        game.native_draw_fluid_box(
            draw,
            &native_fluid(fluid),
            [hull, floor, hull, far, floor + height, far],
            NativeFluidRenderOptions {
                argb: None,
                render_bottom: false,
                uv_shrink_ratio: 4.0 / 2048.0,
            },
        );
    }
}

pub(super) fn removed(game: &mut Game<'_>, pos: Pos) {
    SERVER.with(|cache| {
        cache
            .borrow_mut()
            .dimension(std::ptr::from_ref(&game.world) as usize, game.dim.id())
            .remove(&pos);
    });
    crate::game::create_port_mixer::basin_removed(game, [pos[0], pos[1] + 2, pos[2]]);
    crate::game::create_port_press_basin::basin_removed(game, [pos[0], pos[1] + 2, pos[2]]);
}
pub(super) fn maintenance(game: &mut Game<'_>) {
    SERVER.with(|cache| {
        cache
            .borrow_mut()
            .dimension(std::ptr::from_ref(&game.world) as usize, game.dim.id())
            .retain(|p, _| game.world.loaded(p[0], p[2]) && game.native_block(*p) == ID);
    });
}
/// FluidTransportBehaviour adapter for one dry, closed, straight line.
pub(super) mod pump {
    use super::*;
    use create_engine::{fluid_transport as flow, placement::Direction};
    use horizon_world::{chunk::BlockState, level::PlacementContext, registry::BlockStateId};
    use std::rc::Rc;

    pub(in crate::game) const PUMP: &str = "create:mechanical_pump";
    pub(in crate::game) const PIPE: &str = "create:fluid_pipe";
    pub(in crate::game) const VALVE: &str = "create:fluid_valve";

    struct Line {
        pump: Pos,
        front: Direction,
        source: Pos,
        target: Pos,
        devices: Vec<Pos>,
        valve: Option<(Pos, bool)>,
    }

    #[derive(Default)]
    struct TickState {
        owner: Option<ModRevision>,
        line: Option<(Pos, Direction, Pos, Pos)>,
        valve: Option<(Pos, bool)>,
        pressure: f32,
        phase: u8,
        source_ticks: u8,
    }

    #[derive(Default)]
    struct Lines {
        tick: Option<i64>,
        by_device: BTreeMap<Pos, Option<Rc<Line>>>,
        live: BTreeMap<Pos, TickState>,
    }

    thread_local! {
        static NETWORKS: RefCell<SessionCache<Lines>> = RefCell::new(SessionCache::default());
    }

    pub(super) fn reset() {
        NETWORKS.with(|networks| networks.borrow_mut().clear());
    }

    fn relative(pos: Pos, direction: Direction, distance: i32) -> Pos {
        let offset = direction.vector();
        std::array::from_fn(|i| pos[i] + offset[i] * distance)
    }

    fn dry(state: &BlockState) -> bool {
        state
            .properties
            .get("waterlogged")
            .is_some_and(|value| value == "false")
    }

    fn front(state: &BlockState) -> Option<Direction> {
        let name = state.properties.get("facing")?;
        Direction::ALL.into_iter().find(|side| side.name() == name)
    }

    fn axis(state: &BlockState) -> Option<create_engine::kinetics::Axis> {
        let mut sides = Direction::ALL.into_iter().filter(|side| {
            state
                .properties
                .get(side.name())
                .is_some_and(|v| v == "true")
        });
        let first = sides.next()?;
        (sides.next()? == first.opposite() && sides.next().is_none()).then_some(first.axis())
    }

    fn enabled(state: &BlockState) -> bool {
        state
            .properties
            .get("enabled")
            .is_some_and(|value| value == "true")
    }

    fn valve_axis(state: &BlockState, shaft: bool) -> Option<create_engine::kinetics::Axis> {
        let first = state.properties.get("axis_along_first")? == "true";
        Some(create_engine::placement::gauge_axis(
            front(state)?,
            first == shaft,
        ))
    }

    fn state<'a>(game: &'a Game<'_>, pos: Pos) -> Option<&'a BlockState> {
        let [x, y, z] = pos;
        if !game.world.loaded(x, z) {
            return None;
        }
        game.pack.assets.registry.state(game.world.block(x, y, z))
    }

    fn device_axis(state: &BlockState) -> Option<create_engine::kinetics::Axis> {
        if !dry(state) {
            return None;
        }
        match state.name.as_str() {
            PIPE => axis(state),
            PUMP => front(state).map(Direction::axis),
            VALVE => valve_axis(state, false),
            _ => None,
        }
    }

    fn pump_at(game: &Game<'_>, pos: Pos, origin: &BlockState) -> Option<Pos> {
        if origin.name == PUMP {
            return Some(pos);
        }
        let axis = device_axis(origin)?;
        for side in Direction::ALL
            .into_iter()
            .filter(|side| side.axis() == axis)
        {
            for distance in 1..=flow::MAX_PIPES_PER_SIDE {
                let neighbor = relative(pos, side, distance);
                let Some(state) = state(game, neighbor) else {
                    break;
                };
                if device_axis(state) != Some(axis) {
                    break;
                }
                if state.name == PUMP {
                    return Some(neighbor);
                }
            }
        }
        None
    }

    fn find_line(game: &Game<'_>, pos: Pos, origin: &BlockState) -> Option<Line> {
        let pump = pump_at(game, pos, origin)?;
        let block = state(game, pump)?;
        let direction = front(block)?;
        if !dry(block) {
            return None;
        }
        let scan = |side| {
            let mut devices = Vec::new();
            let mut valve = None;
            for distance in 1..=flow::MAX_PIPES_PER_SIDE + 1 {
                let at = relative(pump, side, distance);
                let state = state(game, at)?;
                if state.name == tank::BLOCK && tank::endpoint(game, at) {
                    return Some((at, devices, valve));
                }
                if state.name == item_drain::BLOCK
                    && item_drain::endpoint(game, at, side.opposite().vector())
                {
                    return Some((at, devices, valve));
                }
                if state.name == crate::game::create_port_spout::BLOCK
                    && crate::game::create_port_spout::endpoint(game, at, side.opposite().vector())
                {
                    return Some((at, devices, valve));
                }
                if !matches!(state.name.as_str(), PIPE | VALVE)
                    || device_axis(state) != Some(direction.axis())
                {
                    return None;
                }
                if state.name == VALVE && valve.replace((at, enabled(state))).is_some() {
                    return None;
                }
                devices.push(at);
            }
            None
        };
        let (source, mut behind, inlet_valve) = scan(direction.opposite())?;
        let (target, ahead, outlet_valve) = scan(direction)?;
        // Straight networks accept extraction from a Tank or Item Drain;
        // insertion into a Drain and extraction from Spouts remain unsupported.
        if !matches!(
            game.native_block(source).as_str(),
            tank::BLOCK | item_drain::BLOCK
        ) || game.native_block(target) == item_drain::BLOCK
        {
            return None;
        }
        if inlet_valve.is_some() && outlet_valve.is_some() {
            return None;
        }
        behind.reverse();
        behind.push(pump);
        behind.extend(ahead);
        Some(Line {
            pump,
            front: direction,
            source,
            target,
            devices: behind,
            valve: inlet_valve.or(outlet_valve),
        })
    }

    fn line(game: &Game<'_>, pos: Pos, origin: &BlockState) -> Option<Rc<Line>> {
        NETWORKS.with(|cache| {
            let mut cache = cache.borrow_mut();
            let index = cache.dimension(std::ptr::from_ref(&game.world) as usize, game.dim.id());
            let now = game.world.game_time;
            if index.tick != Some(now) {
                index.by_device.clear();
                index.tick = Some(now);
                if now % 20 == 0 {
                    index.live.retain(|p, _| {
                        state(game, *p)
                            .is_some_and(|s| matches!(s.name.as_str(), PIPE | PUMP | VALVE))
                    });
                }
            }
            if let Some(line) = index.by_device.get(&pos) {
                return line.clone();
            }
            let found = find_line(game, pos, origin).map(Rc::new);
            if let Some(line) = &found {
                for &device in &line.devices {
                    index.by_device.insert(device, Some(line.clone()));
                }
            } else {
                index.by_device.insert(pos, None);
            }
            found
        })
    }

    fn stored(entity: &ModBlockEntity, side: Direction) -> Option<Option<(flow::Flow, bool)>> {
        let Some(tag) = entity.nbt.get(&side.name().into()) else {
            return Some(None);
        };
        let Tag::Compound(connection) = tag else {
            return None;
        };
        if connection
            .keys()
            .any(|key| !matches!(key.to_string_lossy().as_str(), "Pressure" | "Flow"))
        {
            return None;
        }
        if let Some(pressure) = connection.get(&"Pressure".into()) {
            let Tag::List(values) = pressure else {
                return None;
            };
            let [Tag::Float(inward), Tag::Float(outward)] = values.as_slice() else {
                return None;
            };
            let inward = f32::from_bits(*inward);
            let outward = f32::from_bits(*outward);
            if !inward.is_finite()
                || !outward.is_finite()
                || inward < 0.0
                || outward < 0.0
                || inward > 0.0 && outward > 0.0
            {
                return None;
            }
        }
        let Some(tag) = connection.get(&"Flow".into()) else {
            return Some(None);
        };
        let Tag::Compound(data) = tag else {
            return None;
        };
        if data
            .keys()
            .any(|key| !matches!(key.to_string_lossy().as_str(), "Fluid" | "In" | "Progress"))
        {
            return None;
        }
        let Tag::Compound(fluid) = data.get(&"Fluid".into())? else {
            return None;
        };
        if fluid.len() != 2 || !matches!(fluid.get(&"amount".into()), Some(Tag::Int(1))) {
            return None;
        }
        let Tag::String(id) = fluid.get(&"id".into())? else {
            return None;
        };
        let Tag::Byte(inbound @ (0 | 1)) = data.get(&"In".into())? else {
            return None;
        };
        let progress = match data.get(&"Progress".into()) {
            None => 1.0,
            Some(Tag::Compound(fields))
                if fields.keys().all(|key| {
                    matches!(
                        key.to_string_lossy().as_str(),
                        "Value" | "Target" | "Speed" | "Force"
                    )
                }) =>
            {
                if !matches!(fields.get(&"Target".into()), Some(Tag::Float(0)))
                    || !matches!(fields.get(&"Speed".into()), Some(Tag::Float(0)))
                {
                    return None;
                }
                if fields
                    .get(&"Force".into())
                    .is_some_and(|tag| !matches!(tag, Tag::Byte(1)))
                {
                    return None;
                }
                let Tag::Float(value) = fields.get(&"Value".into())? else {
                    return None;
                };
                let value = f32::from_bits(*value);
                if !(0.0..1.0).contains(&value) {
                    return None;
                }
                value
            }
            _ => return None,
        };
        Some(Some((
            flow::Flow {
                fluid: flow::Fluid::from_id(&id.to_string_lossy())?,
                progress,
            },
            *inbound != 0,
        )))
    }

    fn faces(entity: &ModBlockEntity, front: Direction) -> Option<flow::Faces> {
        let inlet = stored(entity, front.opposite())?;
        let outlet = stored(entity, front)?;
        if inlet.is_none_or(|(_, inward)| inward) && outlet.is_none_or(|(_, inward)| !inward) {
            Some(flow::Faces {
                inlet: inlet.map(|(f, _)| f),
                outlet: outlet.map(|(f, _)| f),
            })
        } else if inlet.is_none_or(|(_, inward)| !inward) && outlet.is_none_or(|(_, inward)| inward)
        {
            Some(
                flow::Faces {
                    inlet: outlet.map(|(f, _)| f),
                    outlet: inlet.map(|(f, _)| f),
                }
                .reversed(),
            )
        } else {
            None
        }
    }

    fn tag_number(n: f32) -> Tag {
        Tag::Float(n.to_bits())
    }

    #[derive(Clone, Copy, Default, PartialEq)]
    struct Pointer {
        value: f32,
        target: f32,
        speed: f32,
    }

    impl Pointer {
        fn read(nbt: &Compound) -> Option<Self> {
            let Some(tag) = nbt.get(&"Pointer".into()) else {
                return Some(Self::default());
            };
            let Tag::Compound(fields) = tag else {
                return None;
            };
            let number = |key: &str| match fields.get(&key.into()) {
                Some(Tag::Float(bits)) => Some(f32::from_bits(*bits)),
                _ => None,
            };
            let value = number("Value")?;
            let target = number("Target")?;
            let speed = number("Speed")?;
            ((0.0..=1.0).contains(&value)
                && matches!(target, 0.0 | 1.0)
                && (0.0..=1.0).contains(&speed))
            .then_some(Self {
                value,
                target,
                speed,
            })
        }

        fn tick(&mut self, rpm: f32) {
            self.target = if rpm > 0.0 { 1.0 } else { 0.0 };
            self.speed = (rpm.abs() / 16.0 / 20.0).clamp(0.0, 1.0);
            if self.value < self.target {
                self.value = (self.value + self.speed).min(self.target);
            } else if self.value > self.target {
                self.value = (self.value - self.speed).max(self.target);
            }
        }

        fn save(self, nbt: &mut Compound) {
            if nbt.get(&"Pointer".into()).is_none() {
                nbt.insert("Pointer".into(), Tag::Compound(Compound::new()));
            }
            if let Some(Tag::Compound(fields)) = nbt.get_mut(&"Pointer".into()) {
                fields.insert("Value".into(), tag_number(self.value));
                fields.insert("Target".into(), tag_number(self.target));
                fields.insert("Speed".into(), tag_number(self.speed));
            }
        }
    }

    pub(in crate::game) fn valve_nbt() -> Compound {
        let mut nbt = Compound::new();
        Pointer::default().save(&mut nbt);
        nbt
    }

    pub(in crate::game) fn tick_valve(
        game: &mut Game<'_>,
        pos: Pos,
        entity: &mut ModBlockEntity,
        rpm: f32,
    ) -> bool {
        let mut changed = tick(game, pos, entity, rpm);
        let Some(mut pointer) = Pointer::read(&entity.nbt) else {
            return changed;
        };
        let previous = pointer;
        pointer.tick(rpm);
        if pointer != previous || entity.nbt.get(&"Pointer".into()).is_none() {
            pointer.save(&mut entity.nbt);
            changed = true;
        }
        let value = match pointer.value {
            0.0 => Some("false"),
            1.0 => Some("true"),
            _ => None,
        };
        if let Some(value) = value {
            changed |= game.native_set_block_property(pos, "enabled", value);
        }
        if trace_enabled() {
            log::info!(
                "Create fluid valve: {}",
                serde_json::json!({
                    "time": game.world.game_time, "pos": pos, "rpm": rpm,
                    "enabled": state(game, pos).is_some_and(enabled),
                    "pointer": [pointer.value, pointer.target, pointer.speed],
                })
            );
        }
        changed
    }

    fn save_face(
        entity: &mut ModBlockEntity,
        side: Direction,
        pressure: f32,
        inbound: bool,
        fluid: Option<flow::Flow>,
        force: bool,
    ) -> bool {
        let mut connection = Compound::new();
        if pressure != 0.0 {
            let values = if inbound {
                [pressure, 0.0]
            } else {
                [0.0, pressure]
            };
            connection.insert("Pressure".into(), Tag::List(values.map(tag_number).into()));
        }
        if let Some(flow) = fluid {
            let mut data = Compound::from([
                (
                    "Fluid".into(),
                    Tag::Compound(Compound::from([
                        ("id".into(), Tag::String(flow.fluid.id().into())),
                        ("amount".into(), Tag::Int(1)),
                    ])),
                ),
                ("In".into(), Tag::Byte(i8::from(inbound))),
            ]);
            if flow.progress != 1.0 {
                let mut progress = Compound::from([
                    ("Value".into(), tag_number(flow.progress)),
                    ("Target".into(), tag_number(0.0)),
                    ("Speed".into(), tag_number(0.0)),
                ]);
                if force {
                    progress.insert("Force".into(), Tag::Byte(1));
                }
                data.insert("Progress".into(), Tag::Compound(progress));
            }
            connection.insert("Flow".into(), Tag::Compound(data));
        }
        let value = Tag::Compound(connection);
        let key = side.name().into();
        if entity.nbt.get(&key) == Some(&value) {
            return false;
        }
        entity.nbt.insert(key, value);
        true
    }

    fn save(
        entity: &mut ModBlockEntity,
        front: Direction,
        pressure: f32,
        before: flow::Faces,
        after: flow::Faces,
    ) -> bool {
        let incoming = save_face(
            entity,
            front.opposite(),
            pressure,
            true,
            after.inlet,
            before.inlet.is_none(),
        );
        let outgoing = save_face(
            entity,
            front,
            pressure,
            false,
            after.outlet,
            before.outlet.is_none(),
        );
        incoming || outgoing
    }

    fn other_faces(game: &Game<'_>, pos: Pos, front: Direction) -> Option<flow::Faces> {
        let BlockEntity::Mod(entity) = game.world.block_entities.get(&(pos[0], pos[1], pos[2]))?
        else {
            return None;
        };
        matches!(entity.id.as_str(), PIPE | PUMP | VALVE)
            .then(|| faces(entity, front))
            .flatten()
    }

    fn controls(
        game: &Game<'_>,
        pos: Pos,
        owner: ModRevision,
        line: Option<&Line>,
        pressure: f32,
        source_flow: bool,
    ) -> (u8, bool) {
        let valve = line.and_then(|line| line.valve);
        let line = line.map(|line| (line.pump, line.front, line.source, line.target));
        NETWORKS.with(|cache| {
            let mut cache = cache.borrow_mut();
            let state = cache
                .dimension(std::ptr::from_ref(&game.world) as usize, game.dim.id())
                .live
                .entry(pos)
                .or_default();
            if state
                .owner
                .is_none_or(|old| !old.is_same_create_entity(owner))
                || state.line != line
                || state.valve != valve
                || state.pressure != pressure
            {
                *state = TickState {
                    owner: Some(owner),
                    line,
                    valve,
                    pressure,
                    phase: 2,
                    source_ticks: 0,
                };
            }
            if state.phase != 0 {
                let phase = state.phase;
                state.phase -= 1;
                return (phase, false);
            }
            state.source_ticks = if source_flow {
                (state.source_ticks + 1).min(3)
            } else {
                0
            };
            (0, state.source_ticks == 3)
        })
    }

    pub(in crate::game) fn tick(
        game: &mut Game<'_>,
        pos: Pos,
        entity: &mut ModBlockEntity,
        rpm: f32,
    ) -> bool {
        if !matches!(entity.id.as_str(), PIPE | PUMP | VALVE) {
            return false;
        }
        let Some(origin) = state(game, pos) else {
            return false;
        };
        let Some(axis) = device_axis(origin) else {
            return false;
        };
        let Some(line) = line(game, pos, origin) else {
            // An open, unsupported or unloaded path must not discard saved
            // connections. It has no authority to drain either endpoint.
            controls(game, pos, entity.revision(), None, 0.0, false);
            return false;
        };
        let front = line.front;
        let Some(before) = faces(entity, front) else {
            return false;
        };
        let pressure = if pos == line.pump {
            rpm.abs()
        } else {
            let p = line.pump;
            match game.world.block_entities.get(&(p[0], p[1], p[2])) {
                Some(BlockEntity::Mod(entity)) if entity.id == PUMP => {
                    crate::game::create_port::running_speed(game, &entity.nbt).abs()
                }
                _ => 0.0,
            }
        };
        let is_source = line.devices.first() == Some(&pos);
        let valve_closed_here = line.valve == Some((pos, false));
        let input = if pressure == 0.0 || valve_closed_here {
            None
        } else if is_source {
            tank::sample(game, line.source)
        } else {
            other_faces(game, relative(pos, front.opposite(), 1), front)
                .and_then(|faces| faces.outlet?.complete())
        };
        let source_flow = is_source
            && input.is_some_and(|fluid| before.inlet.is_some_and(|inlet| inlet.fluid == fluid));
        let (phase, ready) = controls(
            game,
            pos,
            entity.revision(),
            Some(&line),
            pressure,
            source_flow,
        );
        if phase == 2 {
            return false;
        }
        if phase == 1 {
            return save(entity, front, pressure, before, before);
        }
        if ready
            && let Some(fluid) = input
            && line.valve.is_none_or(|(at, open)| {
                open && state(game, at).is_some_and(|s| s.name == VALVE && dry(s))
            })
            && before.complete(fluid)
            && line.devices.iter().skip(1).all(|&device| {
                state(game, device).is_some_and(|s| {
                    if device == line.pump {
                        front_of_pump(s) == Some(line.front)
                    } else {
                        matches!(s.name.as_str(), PIPE | VALVE) && device_axis(s) == Some(axis)
                    }
                }) && other_faces(game, device, front).is_some_and(|pair| pair.complete(fluid))
            })
        {
            tank::transfer_pair(
                game,
                line.source,
                line.target,
                fluid,
                flow::transfer_rate(pressure),
            );
        }
        let mut after = before;
        if pressure > 0.0 {
            after.tick(input, pressure);
        } else {
            after = flow::Faces::default();
        }
        trace(game, pos, &line, pressure, after, is_source);
        before != after && save(entity, front, pressure, before, after)
    }

    fn trace(
        game: &Game<'_>,
        pos: Pos,
        line: &Line,
        pressure: f32,
        faces: flow::Faces,
        source: bool,
    ) {
        if !trace_enabled() {
            return;
        }
        let flow = |face: Option<flow::Flow>| face.map(|f| (f.fluid.id(), f.progress));
        log::info!(
            "Create fluid pump: {}",
            serde_json::json!({
                "time": game.world.game_time, "pos": pos, "pressure": pressure, "front": line.front.name(),
                "inlet": flow(faces.inlet), "outlet": flow(faces.outlet),
                "storage": source.then(|| [tank::amount(game, line.source), tank::amount(game, line.target)]),
            })
        );
    }

    fn trace_enabled() -> bool {
        static TRACE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *TRACE.get_or_init(|| std::env::var_os("CREATE_TRACE_FLUID_PUMP").is_some())
    }

    fn trace_attachment(pos: Pos, model: &'static str) {
        if !trace_enabled() {
            return;
        }
        thread_local! {
            static DRAWN: RefCell<std::collections::BTreeSet<(Pos, &'static str)>> = const { RefCell::new(std::collections::BTreeSet::new()) };
        }
        if DRAWN.with(|drawn| drawn.borrow_mut().insert((pos, model))) {
            log::info!(
                "Create fluid attachment: {}",
                serde_json::json!({"pos": pos, "model": model})
            );
        }
    }

    fn front_of_pump(state: &BlockState) -> Option<Direction> {
        (state.name == PUMP && dry(state))
            .then(|| front(state))
            .flatten()
    }

    pub(in crate::game) fn reacts(game: &Game<'_>, pos: Pos) -> bool {
        game.native_on("create")
            && game.native_block(pos) == PUMP
            && game.may_build()
            && !game.native_sneaking()
            && game
                .held()
                .is_some_and(|held| crate::game::create_port_chassis::wrench(game, held))
    }

    pub(in crate::game) fn right_click(game: &mut Game<'_>, pos: Pos) -> bool {
        if !reacts(game, pos) {
            return false;
        }
        if let Some(front) = state(game, pos).and_then(front_of_pump)
            && game.native_set_block_property(pos, "facing", front.opposite().name())
        {
            crate::game::create_port_sounds::cog_wrench(game, pos);
        }
        true
    }

    // The attachment and placement path follows each original fluid capability's
    // sided geometry even when its current saved contents are unsupported.
    fn storage_connection(state: &BlockState, face: Direction) -> bool {
        match state.name.as_str() {
            tank::BLOCK => true,
            item_drain::BLOCK => face != Direction::Up,
            crate::game::create_port_spout::BLOCK => face != Direction::Down,
            _ => false,
        }
    }
    pub(in crate::game) fn placement(
        game: &GameLevel<'_>,
        mut state: BlockStateId,
        ctx: &PlacementContext,
        block: &str,
    ) -> Option<BlockStateId> {
        let pos = [ctx.pos.x, ctx.pos.y, ctx.pos.z];
        let target = game.native_state_block(game.world.block(pos[0], pos[1], pos[2]))?;
        if matches!(target, "minecraft:water" | "minecraft:lava") {
            return None;
        }
        let look = crate::game::create_port::nearest(ctx.yaw, ctx.pitch);
        if block == VALVE {
            use create_engine::kinetics::Axis;
            let facing = if ctx.sneaking { look } else { look.opposite() };
            let ports = crate::game::create_port_placement::level_ports(game, pos);
            let has_shaft = |side: Direction| ports[side as usize];
            let has_pipe = |side: Direction| {
                let at = relative(pos, side, 1);
                game.registry
                    .state(game.world.block(at[0], at[1], at[2]))
                    .is_some_and(|s| {
                        storage_connection(s, side.opposite())
                            || device_axis(s) == Some(side.axis())
                    })
            };
            let first = match facing.axis() {
                axis @ (Axis::X | Axis::Z) => {
                    let perpendicular = if axis == Axis::X {
                        Direction::South
                    } else {
                        Direction::East
                    };
                    let vertical_shaft = has_shaft(Direction::Up) || has_shaft(Direction::Down);
                    let horizontal_pipe =
                        has_pipe(perpendicular) || has_pipe(perpendicular.opposite());
                    (axis == Axis::X) == (vertical_shaft || horizontal_pipe)
                }
                Axis::Y => {
                    let prefer = |s: Direction| has_shaft(s) || has_pipe(s.clockwise(Axis::Y));
                    let x = prefer(Direction::East) || prefer(Direction::West);
                    let z = prefer(Direction::North) || prefer(Direction::South);
                    if x != z {
                        x
                    } else {
                        crate::game::create_port_placement::horizontal(ctx.yaw).axis() == Axis::X
                    }
                }
            };
            let state = game.native_state_with_property(state, "facing", facing.name())?;
            return game.native_state_with_property(
                state,
                "axis_along_first",
                if first { "true" } else { "false" },
            );
        }
        let joined: Vec<_> = Direction::ALL
            .into_iter()
            .filter(|&side| {
                let at = relative(pos, side, 1);
                let Some(adjacent) = game.registry.state(game.world.block(at[0], at[1], at[2]))
                else {
                    return false;
                };
                storage_connection(adjacent, side.opposite())
                    || device_axis(adjacent) == Some(side.axis())
            })
            .collect();
        if block == PUMP {
            let want = if ctx.sneaking { look } else { look.opposite() };
            let closest = joined.iter().rev().min_by_key(|&&side| {
                if side == want {
                    0
                } else if side == want.opposite() {
                    2
                } else {
                    1
                }
            });
            let base = if ctx.sneaking {
                want
            } else {
                create_engine::placement::preferred(
                    crate::game::create_port_placement::level_ports(game, pos),
                )
                .map_or(want, Direction::opposite)
            };
            let front = closest
                .filter(|&&side| side.axis() != want.axis() && !ctx.sneaking)
                .copied()
                .unwrap_or(base);
            return game.native_state_with_property(state, "facing", front.name());
        }
        let axis = joined.first().map_or(look.axis(), |side| side.axis());
        if joined.iter().any(|side| side.axis() != axis) {
            return None;
        }
        for side in Direction::ALL {
            state = game.native_state_with_property(
                state,
                side.name(),
                if side.axis() == axis { "true" } else { "false" },
            )?;
        }
        Some(state)
    }

    pub(in crate::game) fn draw(
        game: &Game<'_>,
        pos: Pos,
        entity: &ModBlockEntity,
        draw: &mut BlockEntityDraw,
    ) {
        let Some(block) = state(game, pos) else {
            return;
        };
        let Some(axis) = device_axis(block) else {
            return;
        };
        if block.name == PUMP {
            let angle = create_engine::visual::angle(
                draw.game_time as f64 + f64::from(draw.partial),
                crate::game::create_port::number(&entity.nbt, "Speed"),
                create_engine::visual::rotation_offset(pos, axis, false),
            );
            draw.push();
            draw.rotate_about(create_engine::visual::positive_axis(axis), angle, [0.5; 3]);
            crate::game::create_port::face_south(
                draw,
                [Direction::East, Direction::Up, Direction::South][axis.index()],
            );
            game.native_draw_model(draw, "create:block/mechanical_pump/cog", 0xFFFFFF);
            draw.pop();
        }
        if block.name == VALVE {
            let (Some(facing), Some(shaft)) = (front(block), valve_axis(block, true)) else {
                return;
            };
            let angle = create_engine::visual::angle(
                draw.game_time as f64 + f64::from(draw.partial),
                crate::game::create_port::number(&entity.nbt, "Speed"),
                create_engine::visual::rotation_offset(pos, shaft, false),
            );
            draw.push();
            crate::game::create_port::orient(draw, shaft);
            draw.rotate_about([0.0, 1.0, 0.0], angle, [0.5; 3]);
            game.native_draw_model(draw, "create:block/shaft", 0xFFFFFF);
            draw.pop();
            let angle = Pointer::read(&entity.nbt).unwrap_or_default().value * -90.0;
            let offset = if axis.index() == 1 || shaft.index() == 0 {
                90.0
            } else {
                0.0
            };
            let yaw = match facing {
                Direction::North => 180.0,
                Direction::East => 270.0,
                Direction::West => 90.0,
                _ => 0.0,
            };
            let pitch = match facing {
                Direction::Up => 0.0,
                Direction::Down => 180.0,
                _ => 90.0,
            };
            draw.push();
            draw.rotate_about([0.0, 1.0, 0.0], yaw, [0.5; 3]);
            draw.rotate_about([1.0, 0.0, 0.0], pitch, [0.5; 3]);
            draw.rotate_about([0.0, 1.0, 0.0], offset + angle, [0.5; 3]);
            if game.native_draw_model(draw, "create:block/fluid_valve/pointer", 0xFFFFFF) {
                trace_attachment(pos, "create:block/fluid_valve/pointer");
            }
            draw.pop();
        }
        if !crate::game::create_port::original_assets() {
            return;
        }
        // Straight PipeAttachmentModel quads supply the tube beyond its core.
        use create_assets::PipeAttachment::{Connection, Drain, Rim, RimConnector};
        for side in Direction::ALL
            .into_iter()
            .filter(|side| side.axis() == axis)
        {
            let neighbor = state(game, relative(pos, side, 1));
            let models: &[create_assets::PipeAttachment] = if block.name == VALVE {
                if neighbor.is_some_and(|state| storage_connection(state, side.opposite())) {
                    &[Drain]
                } else {
                    &[]
                }
            } else if neighbor.is_some_and(|state| storage_connection(state, side.opposite())) {
                &[RimConnector, Drain]
            } else if block.name == PUMP {
                &[]
            } else if let Some(state) = neighbor.filter(|state| device_axis(state) == Some(axis)) {
                if state.name == PUMP {
                    &[RimConnector]
                } else {
                    &[Connection]
                }
            } else {
                &[RimConnector, Rim]
            };
            for part in models {
                let model = part.model(side as usize);
                if game.native_draw_model(draw, model, 0xFFFFFF) {
                    trace_attachment(pos, model);
                }
            }
        }
    }
}

pub(super) fn client_tick(game: &mut Game<'_>) {
    if !game.native_window() || game.remote.is_none() || !game.native_on("create") {
        return;
    }
    super::create_port_spout::client_tick(game);
    let time = game.world.game_time;
    let session = std::ptr::from_ref(&game.world) as usize;
    let dimension = game.dim.id().to_owned();
    let entries: Vec<_> = game
        .world
        .block_entities
        .iter()
        .filter_map(|(&(x, y, z), e)| match e {
            BlockEntity::Mod(e) if matches!(e.id.as_str(), ID | tank::BLOCK) => {
                Some(([x, y, z], e.as_ref()))
            }
            _ => None,
        })
        .collect();
    tank::client_tick(session, &dimension, time, &entries);
    let entries = entries
        .into_iter()
        .filter_map(|(pos, e)| (e.id == ID).then(|| (pos, e.revision())));
    let entries: Vec<_> = entries.collect();
    let live: std::collections::BTreeSet<_> = entries.iter().map(|(pos, _)| *pos).collect();
    MOTION.with(|cache| {
        let mut cache = cache.borrow_mut();
        let cache = cache.dimension(session, &dimension);
        cache.retain(|p, _| live.contains(p));
        for (pos, owner) in entries {
            let m = cache.entry(pos).or_default();
            if m.tick.is_some_and(|t| t > time)
                || m.owner.is_some_and(|old| !old.is_same_create_entity(owner))
            {
                *m = Motion::default();
            }
            m.owner = Some(owner);
            if m.tick == Some(time) { continue; }
            // read(clientPacket) updates the chase target; the lazy tick is a
            // fallback for absent/removed mixers, as in BasinBlockEntity.
            let mixer = game.world.block_entities.get(&(pos[0],pos[1]+2,pos[2]));
            let moving = matches!(mixer, Some(BlockEntity::Mod(e)) if e.id == "create:mechanical_mixer" && crate::game::create_port::integer(&e.nbt,"Running") != 0 && crate::game::create_port::integer(&e.nbt,"Ticks") <= 20);
            if m.tick.is_none() || m.lazy <= 0 || m.moving != moving {
                m.moving = moving;
                m.lazy = 10;
            } else { m.lazy -= 1; }
            m.previous = m.angle;
            m.speed += ((if m.moving {20.0} else {0.0}) - m.speed) * 0.1;
            m.angle += m.speed;
            if let Some(BlockEntity::Mod(entity))=game.world.block_entities.get(&(pos[0],pos[1],pos[2])) {
                for (half,key) in ["InputTanks","OutputTanks"].into_iter().enumerate() {
                    let Ok(levels)=fluids::read_levels(entity.nbt.get(&key.into())) else {continue;};
                    let Ok(contents)=read_tanks(entity,key) else {continue;};
                    for (i,level) in levels.into_iter().enumerate() {
                        let index=half*2+i;
                        if let Some(fluid)=contents[i].as_ref(){m.rendered_fluids[index]=Some(fluid.clone());}
                        let chase=&mut m.fluid_levels[index];
                        if m.received_fluids[index]!=Some(level) {
                            if level.force {chase.value=level.value;chase.previous=level.value;}
                            chase.target=level.target;chase.speed=level.speed;
                            m.received_fluids[index]=Some(level);
                        }
                        chase.tick();
                    }
                }
            }
            m.tick = Some(time);
        }
    });
}
fn draw_fluids(
    game: &Game<'_>,
    pos: Pos,
    entity: &ModBlockEntity,
    draw: &mut BlockEntityDraw,
) -> f32 {
    let rendered = MOTION.with(|cache| {
        let mut cache = cache.borrow_mut();
        let motion = cache
            .dimension(std::ptr::from_ref(&game.world) as usize, game.dim.id())
            .get(&pos);
        std::array::from_fn::<_, 4, _>(|i| {
            if let Some(m) = motion {
                return (
                    m.rendered_fluids[i].clone(),
                    m.fluid_levels[i].at(draw.partial) * rules::TANK_CAPACITY as f32,
                );
            }
            let key = if i < 2 { "InputTanks" } else { "OutputTanks" };
            let fluid = read_tanks(entity, key).ok().and_then(|t| t[i % 2].clone());
            let units = fluids::read_levels(entity.nbt.get(&key.into()))
                .map_or(0.0, |l| l[i % 2].value * rules::TANK_CAPACITY as f32);
            (fluid, units)
        })
    });
    let total: f32 = rendered.iter().map(|(_, units)| units).sum();
    if total < 1.0 {
        return 0.0;
    }
    let ratio = (total / 2000.0).clamp(0.0, 1.0);
    let height = 2.0 / 16.0 + 12.0 / 16.0 * (1.0 - (1.0 - ratio) * (1.0 - ratio));
    let mut x = 2.0 / 16.0;
    for (fluid, units) in rendered {
        let Some(fluid) = fluid else {
            continue;
        };
        if units < 1.0 {
            continue;
        }
        let next = x + (units / total).clamp(0.0, 1.0) * 12.0 / 16.0;
        game.native_draw_fluid_box(
            draw,
            &native_fluid(&fluid),
            [x, 2.0 / 16.0, 2.0 / 16.0, next, height, 14.0 / 16.0],
            NativeFluidRenderOptions {
                argb: None,
                render_bottom: false,
                // Actual source fixture's stitched blocks atlas is 2048x1024.
                // This is explicit because Horizon stores block sprites in arrays.
                uv_shrink_ratio: 4.0 / 2048.0,
            },
        );
        x = next;
    }
    height
}
pub(super) fn draw(
    game: &Game<'_>,
    pos: Pos,
    entity: &ModBlockEntity,
    draw: &mut BlockEntityDraw,
) -> bool {
    if entity.id != ID {
        return false;
    }
    super::create_port_basin_filter::draw(game, pos, entity, draw);
    // Retained motion belongs to this live BE, including ordinary NBT changes.
    MOTION.with(|cache| {
        let mut cache = cache.borrow_mut();
        let motions = cache.dimension(std::ptr::from_ref(&game.world) as usize, game.dim.id());
        if motions.get(&pos).is_some_and(|m| {
            m.owner
                .is_some_and(|old| !old.is_same_create_entity(entity.revision()))
        }) {
            motions.remove(&pos);
        }
    });
    // restore() truncates supported inventories to18, while opaque unsupported
    // inventories retain every live slot. Borrow the same visible range.
    let end = if crate::game::create_port::inventory_supported(entity) {
        entity.items.len().min(18)
    } else {
        entity.items.len()
    };
    let items = &entity.items[..end];
    let fluid_height = draw_fluids(game, pos, entity, draw);
    let item_height = (fluid_height - 0.3).clamp(0.125, 0.6);
    let mut count = items.iter().flatten().filter(|s| s.count > 0).count();
    if count == 0 {
        return true;
    }
    let rotation = MOTION.with(|cache| {
        let mut cache = cache.borrow_mut();
        cache
            .dimension(std::ptr::from_ref(&game.world) as usize, game.dim.id())
            .get(&pos)
            .map_or(0.0, |m| m.previous + (m.angle - m.previous) * draw.partial)
    });
    // BlockPos.hashCode and RandomSource.create preserve stable item pile offsets.
    let hash = pos[1]
        .wrapping_add(pos[2].wrapping_mul(31))
        .wrapping_mul(31)
        .wrapping_add(pos[0]);
    let mut random = LegacyRandom::new(i64::from(hash));
    let partition = 360.0 / count as f32;
    let radius = if count == 1 { 0.0 } else { 0.125 };
    draw.push();
    draw.translate([0.5, 0.2, 0.5]);
    draw.rotate_about([0.0, 1.0, 0.0], rotation, [0.0; 3]);
    for item in items.iter().flatten().filter(|s| s.count > 0) {
        let angle = partition * count as f32;
        let at =
            crate::game::create_port_mixer::rotate_y([radius, f64::from(item_height), 0.0], angle);
        draw.push();
        if fluid_height > 0.0 {
            let phase = (draw.game_time as f32 + draw.partial) / 12.0 + angle;
            let bob = (crate::game::create_port_mixer::sin(phase) + 1.5) / 32.0;
            draw.translate([0.0, bob, 0.0]);
        }
        draw.translate(at.map(|v| v as f32));
        draw.rotate_about([0.0, 1.0, 0.0], angle + 35.0, [0.0; 3]);
        draw.rotate_about([1.0, 0.0, 0.0], 65.0, [0.0; 3]);
        for _ in 0..=item.count / 8 {
            draw.push();
            draw.translate(std::array::from_fn(|_| {
                (random.next_float() - 0.5) * 2.0 / 16.0
            }));
            game.native_draw_item(
                draw,
                item,
                horizon_assets::item_model::DisplayContext::Ground,
                None,
                0,
            );
            draw.pop();
        }
        draw.pop();
        count -= 1;
    }
    draw.pop();
    true
}
