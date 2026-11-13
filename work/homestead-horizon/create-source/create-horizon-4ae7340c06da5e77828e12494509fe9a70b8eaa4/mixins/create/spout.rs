// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// SpoutBlockEntity/FillingBySpout/SpoutRenderer, Create 6.0.10 ac0c444d.
// Bounded deterministic recipes and vanilla buckets, water/lava, depot/belt.
use super::create_port_block_entity_revision::EntityRevision as _;
use super::{Game, create_port_belt as belt, create_port_fluid_codec as fluids};
use crate::{
    containers::{BlockEntity, ModBlockEntity, ModRevision},
    items::ItemStack,
};
use create_engine::{
    basin::{Fluid, FluidLevel},
    spout::{self, Context, Cycle, Decision},
    transport::{ProcessingResult, ProcessingUpdate, SessionCache, Transported, TransportedResult},
};
use horizon_client::{
    crafting::Ingredient,
    mod_api::{
        NativeFluidAction, NativeFluidDrain, NativeFluidRenderOptions, NativeFluidStack,
        NativeFluidTankView,
    },
};
use horizon_core::{
    geometry::Direction,
    random::{LegacyRandom, RandomSource},
};
use horizon_world::nbt::{Compound, Tag};
use serde_json::Value;
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

pub(super) const BLOCK: &str = "create:spout";
pub(super) const CAPACITY: u32 = 1000;
type Pos = [i32; 3];
type Cargo = Transported<ItemStack>;

#[derive(Clone)]
struct State {
    cycle: Cycle,
    fluid: Option<Fluid<fluids::Key>>,
    level: FluidLevel,
    force_level: bool,
}

pub(super) fn default_nbt() -> Compound {
    let segment = Compound::from([
        ("TankContent".into(), Tag::Compound(Compound::new())),
        (
            "Level".into(),
            Tag::Compound(Compound::from([
                ("Value".into(), Tag::Float(0.0f32.to_bits())),
                ("Target".into(), Tag::Float(0.0f32.to_bits())),
                ("Speed".into(), Tag::Float(0.25f32.to_bits())),
            ])),
        ),
    ]);
    Compound::from([
        ("Tanks".into(), Tag::List(vec![Tag::Compound(segment)])),
        ("ProcessingTicks".into(), Tag::Int(-1)),
    ])
}

fn read(entity: &ModBlockEntity) -> Option<State> {
    if entity.id != BLOCK {
        return None;
    }
    let Tag::Int(ticks) = entity.nbt.get(&"ProcessingTicks".into())? else {
        return None;
    };
    let (fluid, level) = fluids::read_single(entity.nbt.get(&"Tanks".into()), CAPACITY).ok()?;
    if fluid.as_ref().is_some_and(|f| !f.key.components.is_empty()) {
        return None;
    }
    Some(State {
        cycle: Cycle::from_ticks(*ticks)?,
        fluid,
        level: FluidLevel {
            previous: level.value,
            value: level.value,
            target: level.target,
            speed: level.speed,
        },
        force_level: level.force,
    })
}

fn entity<'a>(game: &'a Game<'_>, pos: Pos) -> Option<&'a ModBlockEntity> {
    let BlockEntity::Mod(entity) = game.world.block_entities.get(&(pos[0], pos[1], pos[2]))? else {
        return None;
    };
    (game.native_block(pos) == BLOCK && entity.id == BLOCK).then_some(entity.as_ref())
}

fn state(game: &Game<'_>, pos: Pos) -> Option<State> {
    read(entity(game, pos)?)
}
pub(super) fn available(game: &Game<'_>, pos: Pos) -> bool {
    entity(game, pos).is_some()
}
pub(super) fn endpoint(game: &Game<'_>, pos: Pos, face: Pos) -> bool {
    face != [0, -1, 0] && state(game, pos).is_some()
}
pub(super) fn stored(game: &Game<'_>, pos: Pos) -> Option<Option<Fluid<fluids::Key>>> {
    Some(state(game, pos)?.fluid)
}

fn native(fluid: &Fluid<fluids::Key>) -> NativeFluidStack {
    NativeFluidStack {
        fluid: fluid.key.id.clone(),
        amount: fluid.amount as i32,
        components: fluid.key.components.clone(),
    }
}

/// Preflight a replacement without writes, preserving the canonical level record.
/// The pump commits this owned tag together with its source tank before callbacks.
pub(super) fn replacement(
    game: &Game<'_>,
    pos: Pos,
    fluid: Option<&Fluid<fluids::Key>>,
) -> Option<Tag> {
    let original = entity(game, pos)?;
    read(original)?;
    replacement_tag(original, fluid)
}

fn replacement_tag(entity: &ModBlockEntity, fluid: Option<&Fluid<fluids::Key>>) -> Option<Tag> {
    let mut tanks = entity.nbt.get(&"Tanks".into())?.clone();
    let Tag::List(segments) = &mut tanks else {
        return None;
    };
    let [Tag::Compound(segment)] = segments.as_mut_slice() else {
        return None;
    };
    segment.insert(
        "TankContent".into(),
        fluids::write_content(fluid, CAPACITY).ok()?,
    );
    let Tag::Compound(level) = segment.get_mut(&"Level".into())? else {
        return None;
    };
    let target = fluid.map_or(0.0, |f| f.amount as f32 / CAPACITY as f32);
    level.insert("Target".into(), Tag::Float(target.to_bits()));
    level.insert("Speed".into(), Tag::Float(0.25f32.to_bits()));
    Some(tanks)
}

pub(super) fn set_raw(entity: &mut ModBlockEntity, tanks: Tag) -> bool {
    if read(entity).is_none() {
        return false;
    }
    entity.nbt.insert("Tanks".into(), tanks);
    true
}

const SYNC_RATE: i64 = 8;

#[derive(Default)]
struct FluidSync {
    owner: Option<ModRevision>,
    last_sent: Option<i64>,
    pending: bool,
}

#[derive(Default)]
struct SyncWorld {
    entries: BTreeMap<Pos, FluidSync>,
    pruned: Option<i64>,
}

thread_local! {
    static FLUID_SYNC: RefCell<SessionCache<SyncWorld>> = RefCell::new(SessionCache::default());
}

fn bind_sync(sync: &mut FluidSync, owner: ModRevision, now: i64) {
    if sync
        .owner
        .is_none_or(|old| !old.is_same_create_entity(owner))
        || sync.last_sent.is_some_and(|old| old > now)
    {
        *sync = FluidSync::default();
    }
    sync.owner = Some(owner);
}

/// SmartFluidTankBehaviour.sendDataLazily: the first change is visible now;
/// later changes share one update after its eight-world-tick cooldown.
/// Comparators still observe each actual change, independently of packets.
pub(super) fn notify_fluid(game: &mut Game<'_>, pos: Pos) {
    let Some(owner) = entity(game, pos).map(ModBlockEntity::revision) else {
        return;
    };
    let now = game.world.game_time;
    let send = FLUID_SYNC.with(|cache| {
        let mut cache = cache.borrow_mut();
        let sync = cache
            .dimension(std::ptr::from_ref(&game.world) as usize, game.dim.id())
            .entries
            .entry(pos)
            .or_default();
        bind_sync(sync, owner, now);
        if sync
            .last_sent
            .is_none_or(|last| now.saturating_sub(last) >= SYNC_RATE)
        {
            sync.last_sent = Some(now);
            sync.pending = false;
            true
        } else {
            sync.pending = true;
            false
        }
    });
    if send {
        game.native_block_entity_sync(pos);
    }
    game.native_update_output_neighbors(game.dim.id(), pos);
}

/// Source phase transitions publish immediately, including the exact fluid
/// amount at conversion, and supersede any pending lazy fluid update.
fn notify_immediate(game: &mut Game<'_>, pos: Pos) {
    let Some(owner) = entity(game, pos).map(ModBlockEntity::revision) else {
        return;
    };
    let now = game.world.game_time;
    FLUID_SYNC.with(|cache| {
        let mut cache = cache.borrow_mut();
        let sync = cache
            .dimension(std::ptr::from_ref(&game.world) as usize, game.dim.id())
            .entries
            .entry(pos)
            .or_default();
        bind_sync(sync, owner, now);
        sync.last_sent = Some(now);
        sync.pending = false;
    });
    game.native_block_entity_sync(pos);
    game.native_update_output_neighbors(game.dim.id(), pos);
}

/// The supplied entity is temporarily out of the world during its own tick.
/// Flush the last queued amount even when its pump stopped changing storage.
fn flush_fluid(game: &mut Game<'_>, pos: Pos, owner: ModRevision) {
    let now = game.world.game_time;
    let send = FLUID_SYNC.with(|cache| {
        let mut cache = cache.borrow_mut();
        let world = cache.dimension(std::ptr::from_ref(&game.world) as usize, game.dim.id());
        if now % 20 == 0 && world.pruned != Some(now) {
            world
                .entries
                .retain(|at, _| game.native_block(*at) == BLOCK);
            world.pruned = Some(now);
        }
        let Some(sync) = world.entries.get_mut(&pos) else {
            return false;
        };
        bind_sync(sync, owner, now);
        if sync.pending
            && sync
                .last_sent
                .is_none_or(|last| now.saturating_sub(last) >= SYNC_RATE)
        {
            sync.pending = false;
            sync.last_sent = Some(now);
            true
        } else {
            false
        }
    });
    if send {
        game.native_block_entity_sync(pos);
    }
}

fn commit(
    game: &mut Game<'_>,
    pos: Pos,
    fluid: Option<&Fluid<fluids::Key>>,
    immediate: bool,
) -> bool {
    let Some(tanks) = replacement(game, pos, fluid) else {
        return false;
    };
    let committed =
        game.native_with_mod_block_entity(pos, |entity| set_raw(entity, tanks)) == Some(true);
    if committed {
        if immediate {
            notify_immediate(game, pos);
        } else {
            notify_fluid(game, pos);
        }
    }
    committed
}

pub(super) fn view(
    game: &Game<'_>,
    pos: Pos,
    side: Option<Direction>,
) -> Option<Vec<NativeFluidTankView>> {
    let current = state(game, pos)?;
    Some(if side == Some(Direction::Down) {
        Vec::new()
    } else {
        vec![NativeFluidTankView {
            tank: 0,
            content: current.fluid.as_ref().map(native),
            capacity: CAPACITY as i32,
            can_fill: true,
            can_drain: true,
        }]
    })
}

pub(super) fn fill(
    game: &mut Game<'_>,
    pos: Pos,
    side: Option<Direction>,
    resource: &NativeFluidStack,
    action: NativeFluidAction,
) -> Option<i32> {
    let current = state(game, pos)?;
    if side == Some(Direction::Down)
        || resource.amount <= 0
        || !resource.components.is_empty()
        || !matches!(
            resource.fluid.as_str(),
            "minecraft:water" | "minecraft:lava"
        )
    {
        return Some(0);
    }
    if current
        .fluid
        .as_ref()
        .is_some_and(|f| f.key.id != resource.fluid)
    {
        return Some(0);
    }
    let before = current.fluid.as_ref().map_or(0, |f| f.amount);
    let accepted = (resource.amount as u32).min(CAPACITY - before);
    let fluid = Fluid {
        key: fluids::Key {
            id: resource.fluid.clone(),
            components: Compound::new(),
        },
        amount: before + accepted,
    };
    #[allow(unreachable_patterns)]
    match action {
        NativeFluidAction::Simulate => {}
        NativeFluidAction::Execute => {
            if accepted > 0 && !commit(game, pos, Some(&fluid), false) {
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
    side: Option<Direction>,
    request: &NativeFluidDrain,
    action: NativeFluidAction,
) -> Option<NativeFluidStack> {
    if side == Some(Direction::Down) {
        return None;
    }
    let mut fluid = state(game, pos)?.fluid?;
    let maximum = match request {
        NativeFluidDrain::Any(n) => *n,
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
    let amount = fluid.amount.min(maximum as u32);
    let mut result = native(&fluid);
    result.amount = amount as i32;
    #[allow(unreachable_patterns)]
    match action {
        NativeFluidAction::Simulate => {}
        NativeFluidAction::Execute => {
            fluid.amount -= amount;
            if !commit(game, pos, (fluid.amount > 0).then_some(&fluid), false) {
                return None;
            }
        }
        _ => return None,
    }
    Some(result)
}

pub(super) fn comparator(entity: &ModBlockEntity) -> i32 {
    read(entity)
        .and_then(|s| s.fluid)
        .map_or(0, |f| (14 * f.amount / CAPACITY) as i32 + 1)
}

#[derive(Clone)]
struct Recipe {
    input: Ingredient,
    fluid: String,
    amount: u32,
    output: ItemStack,
}
#[derive(Default)]
struct Recipes {
    source: Vec<(String, Arc<Value>)>,
    compiled: Vec<Arc<Recipe>>,
    warned: std::collections::BTreeSet<Pos>,
}
thread_local! { static RECIPES:RefCell<SessionCache<Recipes>>=RefCell::new(SessionCache::default()); }
thread_local! {
    // Java Math.random is independent of the level RNG. Sound pitch must not
    // perturb gameplay drops, weather or any recipe probability sequence.
    static SOUND_RANDOM:RefCell<LegacyRandom>=RefCell::new(LegacyRandom::new(
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0,|t|t.as_nanos() as i64)
    ));
}

pub(super) fn reset() {
    RECIPES.with(|cache| cache.borrow_mut().clear());
    FLUID_SYNC.with(|cache| cache.borrow_mut().clear());
    MOTION.with(|cache| cache.borrow_mut().clear());
}

fn compile(game: &Game<'_>, json: &Value) -> Option<Recipe> {
    if json.as_object()?.keys().any(|k| {
        !matches!(
            k.as_str(),
            "type" | "ingredients" | "results" | "processing_time"
        )
    }) {
        return None;
    }
    let inputs = json["ingredients"].as_array()?;
    let [item, fluid] = inputs.as_slice() else {
        return None;
    };
    // The data adapter puts the one item first. No unknown compound predicate
    // is weakened to a plain holder match.
    let input = match item {
        Value::String(_) => game.native_recipe_ingredient(item)?,
        Value::Object(map)
            if map.len() == 1 && (map.contains_key("item") || map.contains_key("tag")) =>
        {
            game.native_recipe_ingredient(&super::create_port::ingredient(item)?)?
        }
        _ => return None,
    };
    let object = fluid.as_object()?;
    if object.len() != 3 || fluid["type"] != "neoforge:single" {
        return None;
    }
    let fluid_id = fluid["fluid"].as_str()?;
    if !matches!(fluid_id, "minecraft:water" | "minecraft:lava") {
        return None;
    }
    let amount = u32::try_from(fluid["amount"].as_u64()?).ok()?;
    if amount == 0 || amount > CAPACITY {
        return None;
    }
    let [result] = json["results"].as_array()?.as_slice() else {
        return None;
    };
    if result
        .as_object()?
        .keys()
        .any(|k| !matches!(k.as_str(), "id" | "count" | "components" | "chance"))
        || result
            .get("chance")
            .is_some_and(|v| v.as_f64() != Some(1.0))
    {
        return None;
    }
    let output = game.native_recipe_result(result)?;
    if output.count == 0 || output.count > game.native_max_stack_size(&output.item)? {
        return None;
    }
    Some(Recipe {
        input,
        fluid: fluid_id.into(),
        amount,
        output,
    })
}

fn recipes(game: &Game<'_>) -> Vec<Arc<Recipe>> {
    let source = game.native_recipes_of_type("create:filling");
    RECIPES.with(|cache| {
        let mut cache = cache.borrow_mut();
        let cache = cache.dimension(std::ptr::from_ref(&game.world) as usize, game.dim.id());
        if cache.source.len() != source.len()
            || !cache
                .source
                .iter()
                .zip(source.iter())
                .all(|((id, json), r)| id == &r.id && Arc::ptr_eq(json, &r.json))
        {
            cache.compiled = source
                .iter()
                .filter_map(|r| {
                    let value = compile(game, &r.json);
                    if value.is_none() {
                        log::warn!("Create Spout: unsupported filling recipe {}", r.id);
                    }
                    value.map(Arc::new)
                })
                .collect();
            cache.source = source
                .iter()
                .map(|r| (r.id.clone(), r.json.clone()))
                .collect();
        }
        cache.compiled.clone()
    })
}

fn lookup(
    game: &Game<'_>,
    input: &ItemStack,
    fluid: Option<&Fluid<fluids::Key>>,
) -> (bool, Option<(u32, ItemStack)>) {
    let recipes = recipes(game);
    let can_fill =
        input.item.as_ref() == "minecraft:bucket" || recipes.iter().any(|r| r.input.test(input));
    let matched = fluid.and_then(|fluid| {
        if let Some(recipe) = recipes
            .iter()
            .find(|r| r.input.test(input) && r.fluid == fluid.key.id)
        {
            return Some((recipe.amount, recipe.output.clone()));
        }
        if input.item.as_ref() != "minecraft:bucket" {
            return None;
        }
        let mut one = input.clone();
        one.count = 1;
        let (amount, output) = game.native_item_fluid_fill(&one, &fluid.key.id, CAPACITY as i32)?;
        (amount == CAPACITY as i32).then_some((CAPACITY, output))
    });
    (can_fill, matched)
}

pub(super) fn process(
    game: &mut Game<'_>,
    pos: Pos,
    item: &mut Cargo,
    held: bool,
) -> ProcessingUpdate<ItemStack> {
    let Some(mut current) = state(game, pos) else {
        RECIPES.with(|cache| {
            let mut cache=cache.borrow_mut();
            let cache=cache.dimension(std::ptr::from_ref(&game.world) as usize,game.dim.id());
            if cache.warned.insert(pos) { log::warn!("Create Spout: unsupported tank or phase at {:?}; preserving data and holding cargo",pos); }
        });
        return ProcessingResult::Hold.into();
    };
    let input = belt::game_stack(&item.stack);
    let (can_fill, matched) = lookup(game, &input, current.fluid.as_ref());
    let context = Context {
        can_item_be_filled: can_fill,
        fluid_present: current.fluid.is_some(),
        required_amount: matched.as_ref().map(|(n, _)| *n),
        available_fluid: current.fluid.as_ref().map_or(0, |f| f.amount),
    };
    let decision = if held {
        current.cycle.held(context)
    } else {
        spout::received(context)
    };
    match decision {
        Decision::Pass => ProcessingResult::Pass.into(),
        Decision::Hold => ProcessingResult::Hold.into(),
        Decision::Start => {
            game.native_with_mod_block_entity(pos, |entity| {
                entity
                    .nbt
                    .insert("ProcessingTicks".into(), Tag::Int(current.cycle.ticks()));
            });
            notify_immediate(game, pos);
            let pitch = SOUND_RANDOM.with(|r| 0.9 + 0.2 * (r.borrow_mut().next_double() as f32));
            game.with_level(|_, level, _| {
                level.native_play_sound(
                    "create:spout",
                    pos.map(|v| f64::from(v) + 0.5),
                    4,
                    0.75,
                    pitch,
                );
            });
            ProcessingResult::Hold.into()
        }
        Decision::Fill => {
            let (Some((amount, output)), Some(mut fluid)) = (matched, current.fluid) else {
                return ProcessingResult::Hold.into();
            };
            if fluid.amount < amount || input.count == 0 {
                return ProcessingResult::Hold.into();
            }
            fluid.amount -= amount;
            if !commit(game, pos, (fluid.amount > 0).then_some(&fluid), true) {
                return ProcessingResult::Hold.into();
            }
            item.clear_fan_processing();
            let _ = belt::angle(game, &input);
            let mut result = item.source_copy();
            result.stack = belt::core_stack(game, output);
            let held = if item.stack.count > 1 {
                let _ = belt::angle(game, &input);
                let mut remainder = item.source_copy();
                remainder.stack.count -= 1;
                Some(remainder)
            } else {
                None
            };
            ProcessingUpdate {
                decision: ProcessingResult::Hold,
                conversion: Some(TransportedResult::Convert {
                    outputs: vec![result],
                    held,
                }),
            }
        }
    }
}

pub(super) fn tick(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity) {
    let Some(mut current) = read(entity) else {
        return;
    };
    let phase_changed = current.cycle.tick();
    let before = current.level.value;
    current.level.tick();
    if phase_changed {
        entity
            .nbt
            .insert("ProcessingTicks".into(), Tag::Int(current.cycle.ticks()));
    }
    if let Some(Tag::List(segments)) = entity.nbt.get_mut(&"Tanks".into())
        && let [Tag::Compound(segment)] = segments.as_mut_slice()
        && let Some(Tag::Compound(level)) = segment.get_mut(&"Level".into())
    {
        level.insert("Value".into(), Tag::Float(current.level.value.to_bits()));
    }
    if phase_changed || before != current.level.value {
        game.world.modified.insert((pos[0] >> 4, pos[2] >> 4));
    }
    flush_fluid(game, pos, entity.revision());
}

#[derive(Default)]
struct Motion {
    /// Also identifies the last accepted contents revision. A new packet may
    /// repeat the same phase while still resetting the source client phase.
    owner: Option<ModRevision>,
    tick: Option<u64>,
    cycle: Cycle,
    level: FluidLevel,
    rendered_fluid: Option<Fluid<fluids::Key>>,
}

thread_local! {
    static MOTION: RefCell<SessionCache<BTreeMap<Pos, Motion>>> = RefCell::new(SessionCache::default());
}

/// Run from the actual client world-tick hook, not the render or wall clock.
/// Receipt revisions correct the phase; intervening ticks predict it locally.
pub(super) fn client_tick(game: &Game<'_>) {
    if !game.native_window() || game.remote.is_none() || !game.native_on("create") {
        return;
    }
    let rows = game
        .world
        .block_entities
        .iter()
        .filter_map(|(&(x, y, z), value)| {
            let BlockEntity::Mod(entity) = value else {
                return None;
            };
            let pos = [x, y, z];
            if game.native_block(pos) != BLOCK {
                return None;
            }
            Some((pos, entity.revision(), read(entity)?))
        })
        .collect::<Vec<_>>();
    let now = game.ticks;
    MOTION.with(|cache| {
        let mut cache = cache.borrow_mut();
        let cache = cache.dimension(std::ptr::from_ref(&game.world) as usize, game.dim.id());
        let live = rows.iter().map(|row| row.0).collect::<BTreeSet<_>>();
        cache.retain(|pos, _| live.contains(pos));
        for (pos, owner, current) in rows {
            let motion = cache.entry(pos).or_default();
            if motion
                .owner
                .is_none_or(|old| !old.is_same_create_entity(owner))
                || motion.tick.is_some_and(|old| old > now)
            {
                *motion = Motion {
                    owner: Some(owner),
                    tick: None,
                    cycle: current.cycle,
                    level: current.level,
                    rendered_fluid: current.fluid.clone(),
                };
            } else if motion.owner != Some(owner) {
                motion.owner = Some(owner);
                motion.cycle = current.cycle;
                if current.force_level {
                    motion.level.value = current.level.value;
                    motion.level.previous = current.level.value;
                }
                motion.level.target = current.level.target;
                motion.level.speed = current.level.speed;
                if current.fluid.is_some() {
                    motion.rendered_fluid = current.fluid.clone();
                }
            }
            if motion.tick != Some(now) {
                motion.cycle.tick();
                motion.level.tick();
                motion.tick = Some(now);
                if motion.level.value == 0.0 && motion.level.target == 0.0 {
                    motion.rendered_fluid = None;
                }
            }
        }
    });
}

pub(super) fn draw(
    game: &Game<'_>,
    pos: Pos,
    entity: &ModBlockEntity,
    draw: &mut super::native_block_entity_draw::BlockEntityDraw,
) {
    let Some(state) = read(entity) else {
        return;
    };
    let (ticks, level, rendered_fluid) = MOTION.with(|cache| {
        cache
            .borrow_mut()
            .dimension(std::ptr::from_ref(&game.world) as usize, game.dim.id())
            .get(&pos)
            .filter(|motion| {
                motion
                    .owner
                    .is_some_and(|old| old.is_same_create_entity(entity.revision()))
            })
            .map_or_else(
                || (state.cycle.ticks(), state.level.value, state.fluid.clone()),
                |motion| {
                    (
                        motion.cycle.ticks(),
                        motion.level.at(draw.partial),
                        motion.rendered_fluid.clone(),
                    )
                },
            )
    });
    let pt = ticks as f32 - draw.partial;
    let progress = (1.0 - (pt - 5.0) / 10.0).clamp(0.0, 1.0);
    let radius = if rendered_fluid.is_some() && ticks != -1 {
        (f64::from(2.0 * progress - 1.0).powi(2) - 1.0) as f32
    } else {
        0.0
    };
    let squeeze = if pt < 0.0 {
        0.0
    } else if pt < 2.0 {
        -pt / 2.0
    } else if pt < 10.0 {
        -1.0
    } else {
        radius
    };
    draw.push();
    for name in ["top", "middle", "bottom"] {
        game.native_draw_model(draw, &format!("create:block/spout/{name}"), 0xFFFFFF);
        draw.translate([0.0, -3.0 * squeeze / 32.0, 0.0]);
    }
    draw.pop();
    let Some(fluid) = rendered_fluid else {
        return;
    };
    let resource = native(&fluid);
    let options = |bottom| NativeFluidRenderOptions {
        argb: None,
        render_bottom: bottom,
        uv_shrink_ratio: 4.0 / 2048.0,
    };
    if level > 0.0 {
        let min = 2.5 / 16.0;
        let max = min + 11.0 / 16.0;
        game.native_draw_fluid_box(
            draw,
            &resource,
            [
                min,
                min,
                min,
                max,
                min + 11.0 / 16.0 * level.max(0.175),
                max,
            ],
            options(false),
        );
    }
    if ticks != -1 && radius < 0.0 {
        let width = -radius / 32.0;
        game.native_draw_fluid_box(
            draw,
            &resource,
            [
                0.5 - width,
                -1.2 + width,
                0.5 - width,
                0.5 + width,
                -width,
                0.5 + width,
            ],
            options(true),
        );
    }
}
