// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Schematicannon (SchematicannonBlock / SchematicannonBlockEntity / SchematicannonMenu /
// SchematicannonScreen / LaunchedItem) over the pure state machine in
// `create_engine::schematicannon`. This adapter gives that machine its world:
//   * the block entity (5 item slots, menu `create:schematicannon`, 9 data slots),
//   * `PrinterBackend`: the schematic file (`create_schematics` preflight, rotation and
//     mirror from the Schematic item) with the print cursor of `schematic_printer`,
//   * `ItemSource`: the six neighbouring containers, read once per tick into a snapshot
//     and written back with `native_extract_item`,
//   * landing of the projectiles (block placed through the same `destination` rules as
//     `create schematic paste`),
//   * the screen (own drawing, buttons through `screen_click` / `menu_button`).
// Not ported (see docs/schematicannon.md): creative crate, projectile rendering, belts,
// entities, block-entity-bearing blocks beyond what the paste adapter accepts, hopper
// automation of the cannon, the original GUI textures.
use super::{
    Game, create_port_schematic_data as data, create_port_schematic_runtime as paste,
};
use crate::{
    containers::{BlockEntity, ModBlockEntity, ModContainer},
    items::ItemStack,
};
use create_engine::{
    material_checklist::{ItemRequirement, MaterialChecklist, StackRequirement},
    schematic_printer::{Bounds, PrintCursor},
    schematic_tool::{Mirror, SchematicStackData},
    schematicannon::{
        self as cn, CannonConfig, CannonEvent, CannonInventory, ConfigOption, Env, FlyingItem,
        ItemSource, LaunchAction, PlaceDecision, PlaceQuery, PrinterBackend, ReplaceInfo,
        Schematicannon, State, Stack, Status, TargetWorld,
    },
};
use create_schematics::{
    BlockState as SchemState, Limits, Mirror as PlanMirror, Options as PlanOptions, Plan, PlannedBlock,
};
use horizon_core::geometry::Direction as D;
use horizon_world::{
    nbt::{Compound, JavaString, Tag},
    registry::BlockStateId,
};
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

pub(super) const BLOCK: &str = "create:schematicannon";
/// The menu type declared by `data/create/horizon/menu/schematicannon.json`.
const MENU: &str = "create:schematicannon";
const SLOTS: usize = cn::SLOT_COUNT;

type Pos = [i32; 3];

// -------------------------------------------------------------- NBT keys
const K_STATE: &str = "State";
const K_STATUS: &str = "Status";
const K_FUEL: &str = "RemainingFuel";
const K_PLACED: &str = "AmountPlaced";
const K_TO_PLACE: &str = "AmountToPlace";
const K_MODE: &str = "ReplaceMode";
const K_SKIP: &str = "SkipMissing";
const K_BLOCK_ENTITIES: &str = "ReplaceBlockEntities";
const K_PAPER: &str = "PaperProgress";
const K_STEPS: &str = "PrintSteps";
const K_MISSING: &str = "MissingItem";
const K_MISSING_COUNT: &str = "MissingCount";
const K_ERROR: &str = "Error";
const K_FILE: &str = "File";

const STATUSES: [Status; 18] = [
    Status::Idle,
    Status::SchematicInvalid,
    Status::SchematicNotPlaced,
    Status::SchematicErrored,
    Status::SchematicExpired,
    Status::TargetOutsideRange,
    Status::Ready,
    Status::NoGunpowder,
    Status::TargetNotLoaded,
    Status::Searching,
    Status::Skipping,
    Status::MissingBlock,
    Status::Placing,
    Status::Clearing,
    Status::Finished,
    Status::Running,
    Status::Paused,
    Status::Stopped,
];

/// `schematicannon.status.*` in English (Create's lang file as remembered; unverified).
fn status_text(index: i32) -> &'static str {
    match STATUSES.get(usize::try_from(index).unwrap_or(usize::MAX)) {
        Some(Status::Idle) => "Idle",
        Some(Status::SchematicInvalid) => "Invalid Schematic",
        Some(Status::SchematicNotPlaced) => "Schematic not Positioned",
        Some(Status::SchematicErrored) => "Schematic Errored",
        Some(Status::SchematicExpired) => "Schematic File Expired",
        Some(Status::TargetOutsideRange) => "Target too Far Away",
        Some(Status::Ready) => "Ready",
        Some(Status::NoGunpowder) => "Out of Gunpowder",
        Some(Status::TargetNotLoaded) => "Target not Loaded",
        Some(Status::Searching) => "Searching",
        Some(Status::Skipping) => "Skipping",
        Some(Status::MissingBlock) => "Missing Block",
        Some(Status::Placing) => "Placing",
        Some(Status::Clearing) => "Clearing",
        Some(Status::Finished) => "Finished",
        Some(Status::Running) => "Running",
        Some(Status::Paused) => "Paused",
        Some(Status::Stopped) => "Stopped",
        None => "?",
    }
}

fn simulates(game: &Game<'_>) -> bool {
    game.serving.is_some() || game.options.dedicated.is_some()
}

// ------------------------------------------------------------ hook entries

pub(super) fn entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if block == BLOCK && entity.is_none() {
        *entity = Some((block.into(), Compound::new()));
    }
}

pub(super) fn ticks(block: &str, ticks: &mut bool) {
    if block == BLOCK {
        *ticks = true;
    }
}

/// Five slots and the menu; no face, so no hopper reaches them (the original's item
/// handler is reachable from hoppers; not ported).
pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    if block == BLOCK && container.is_none() {
        *container = Some(ModContainer {
            slots: SLOTS,
            faces: std::array::from_fn(|_| Vec::new()),
            inputs: Vec::new(),
            outputs: Vec::new(),
            menu: Some(MENU.into()),
        });
    }
}

fn put(nbt: &mut Compound, key: &str, tag: Tag) -> bool {
    if nbt.get(&key.into()) == Some(&tag) {
        return false;
    }
    nbt.insert(key.into(), tag);
    true
}

fn int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => *v,
        Some(Tag::Byte(v)) => i32::from(*v),
        _ => 0,
    }
}

fn flag(nbt: &Compound, key: &str) -> bool {
    int(nbt, key) != 0
}

fn string(nbt: &Compound, key: &str) -> String {
    match nbt.get(&key.into()) {
        Some(Tag::String(s)) => s.to_string_lossy(),
        _ => String::new(),
    }
}

/// What clients get: the status text pieces the data slots cannot carry.
pub(super) fn update_tag(entity: &ModBlockEntity) -> Compound {
    let text = |key: &str| -> (JavaString, Tag) { (key.into(), Tag::String(string(&entity.nbt, key).as_str().into())) };
    Compound::from([
        text(K_MISSING),
        text(K_ERROR),
        text(K_FILE),
        (K_MISSING_COUNT.into(), Tag::Int(int(&entity.nbt, K_MISSING_COUNT))),
    ])
}

// ------------------------------------------------------------ the runtime

/// A projectile's payload: what lands at `pos` (None for an erase) and the second half of a
/// pair (door, bed) placed with it.
struct Pending {
    pos: Pos,
    block: Option<PlannedBlock>,
    partner: Option<(Pos, PlannedBlock)>,
}

struct Loaded {
    anchor: Pos,
    plan: Plan,
    index: HashMap<Pos, usize>,
    cursor: PrintCursor,
    steps: i32,
}

#[derive(Default)]
struct PrinterState {
    loaded: Option<Loaded>,
    error: Option<String>,
    /// Cursor advances to replay after a load from disk.
    replay: i32,
    pending: Vec<Pending>,
}

struct Runtime {
    cannon: Schematicannon,
    printer: PrinterState,
}

static RUNTIMES: LazyLock<Mutex<HashMap<(String, Pos), Runtime>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn runtimes() -> MutexGuard<'static, HashMap<(String, Pos), Runtime>> {
    RUNTIMES.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The runtime of the block entity at `key`. A block entity without `Init` is a new one (a
/// cannon placed over an old one, or set by `/setblock`, never fired the destroy hook): any
/// runtime left in the map for that position is stale and is dropped.
fn take(key: &(String, Pos), entity: &mut ModBlockEntity) -> Runtime {
    let stored = runtimes().remove(key);
    if entity.nbt.contains_key(&"Init".into()) {
        if let Some(rt) = stored {
            return rt;
        }
    } else {
        entity.nbt.insert("Init".into(), Tag::Byte(1));
        return Runtime { cannon: Schematicannon::new(key.1), printer: PrinterState::default() };
    }
    restore(&entity.nbt, key.1)
}

fn restore(nbt: &Compound, pos: Pos) -> Runtime {
    let mut cannon = Schematicannon::new(pos);
    cannon.state = match int(nbt, K_STATE) {
        1 => State::Paused,
        2 => State::Running,
        _ => State::Stopped,
    };
    let key = string(nbt, K_STATUS);
    cannon.status = STATUSES.into_iter().find(|s| s.key() == key).unwrap_or(Status::Idle);
    cannon.remaining_fuel = int(nbt, K_FUEL);
    cannon.blocks_placed = int(nbt, K_PLACED);
    cannon.blocks_to_place = int(nbt, K_TO_PLACE);
    cannon.replace_mode = int(nbt, K_MODE).clamp(0, 3) as u8;
    cannon.skip_missing = flag(nbt, K_SKIP);
    cannon.replace_block_entities = flag(nbt, K_BLOCK_ENTITIES);
    if let Some(Tag::Float(bits)) = nbt.get(&K_PAPER.into()) {
        cannon.book_printing_progress = f32::from_bits(*bits);
    }
    let missing = string(nbt, K_MISSING);
    if !missing.is_empty() {
        cannon.missing_item = Some(StackRequirement::consume(&missing, int(nbt, K_MISSING_COUNT).max(1)));
    }
    let printer = PrinterState { replay: int(nbt, K_STEPS), ..PrinterState::default() };
    Runtime { cannon, printer }
}

/// Writes the machine into the block entity (NBT + data slots). True when what clients see
/// in the update tag changed.
fn store(rt: &Runtime, entity: &mut ModBlockEntity) -> bool {
    let c = &rt.cannon;
    let nbt = &mut entity.nbt;
    let state = match c.state {
        State::Stopped => 0,
        State::Paused => 1,
        State::Running => 2,
    };
    put(nbt, K_STATE, Tag::Byte(state));
    put(nbt, K_STATUS, Tag::String(c.status.key().into()));
    put(nbt, K_FUEL, Tag::Int(c.remaining_fuel));
    put(nbt, K_PLACED, Tag::Int(c.blocks_placed));
    put(nbt, K_TO_PLACE, Tag::Int(c.blocks_to_place));
    put(nbt, K_MODE, Tag::Byte(c.replace_mode as i8));
    put(nbt, K_SKIP, Tag::Byte(i8::from(c.skip_missing)));
    put(nbt, K_BLOCK_ENTITIES, Tag::Byte(i8::from(c.replace_block_entities)));
    put(nbt, K_PAPER, Tag::Float(c.book_printing_progress.to_bits()));
    put(nbt, K_STEPS, Tag::Int(rt.printer.loaded.as_ref().map_or(0, |l| l.steps)));
    let (missing, count) = c
        .missing_item
        .as_ref()
        .map_or((String::new(), 0), |m| (m.item.clone(), m.count));
    let mut changed = put(nbt, K_MISSING, Tag::String(missing.as_str().into()));
    changed |= put(nbt, K_MISSING_COUNT, Tag::Int(count));
    changed |= put(nbt, K_ERROR, Tag::String(rt.printer.error.clone().unwrap_or_default().as_str().into()));
    let status = STATUSES.iter().position(|s| *s == c.status).unwrap_or(0) as i32;
    let split = |v: i32| (v.clamp(0, 0x3FFF_FFFF) & 0x7FFF, (v.clamp(0, 0x3FFF_FFFF) >> 15) & 0x7FFF);
    let (placed_lo, placed_hi) = split(c.blocks_placed);
    let (total_lo, total_hi) = split(c.blocks_to_place);
    let flags = i32::from(c.skip_missing) | (i32::from(c.replace_block_entities) << 1) | (i32::from(c.replace_mode) << 2);
    let fresh = vec![
        i32::from(state),
        status,
        flags,
        c.remaining_fuel.clamp(0, 0x7FFF),
        placed_lo,
        placed_hi,
        total_lo,
        total_hi,
        (c.book_printing_progress * 100.0).clamp(0.0, 100.0) as i32,
    ];
    if entity.data != fresh {
        entity.data = fresh;
    }
    changed
}

fn apply_button(rt: &mut Runtime, id: i32) -> bool {
    let (option, set) = match id {
        1 => (ConfigOption::Play, true),
        2 => (ConfigOption::Pause, true),
        3 => (ConfigOption::Stop, true),
        4 => (ConfigOption::DontReplace, true),
        5 => (ConfigOption::ReplaceSolid, true),
        6 => (ConfigOption::ReplaceAny, true),
        7 => (ConfigOption::ReplaceEmpty, true),
        8 => (ConfigOption::SkipMissing, !rt.cannon.skip_missing),
        9 => (ConfigOption::SkipBlockEntities, !rt.cannon.replace_block_entities),
        _ => return false,
    };
    rt.cannon.apply_option(option, set);
    true
}

/// `ConfigureSchematicannonPacket`: a button of the open menu (server).
pub(super) fn menu_button(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity, id: i32) {
    if entity.id != BLOCK || !simulates(game) {
        return;
    }
    let key = (game.dim.id().to_owned(), pos);
    let mut rt = take(&key, entity);
    if apply_button(&mut rt, id) {
        store(&rt, entity);
    }
    runtimes().insert(key, rt);
}

/// The operator command `create cannon control X Y Z ACTION`: the same buttons without the screen.
pub(super) fn control(game: &mut Game<'_>, pos: Pos, action: &str) -> Result<String, String> {
    let id = match action {
        "play" => 1,
        "pause" => 2,
        "stop" => 3,
        "replace_none" => 4,
        "replace_solid" => 5,
        "replace_any" => 6,
        "replace_empty" => 7,
        "skip_missing" => 8,
        "replace_entities" => 9,
        _ => return Err("action: play|pause|stop|replace_none|replace_solid|replace_any|replace_empty|skip_missing|replace_entities".into()),
    };
    let dim = game.dim.id().to_owned();
    let done = game
        .native_with_mod_block_entity(pos, |entity| {
            if entity.id != BLOCK {
                return None;
            }
            let mut rt = take(&(dim.clone(), pos), entity);
            apply_button(&mut rt, id);
            store(&rt, entity);
            let line = format!(
                "state {:?}, status {}, fuel {}, placed {}/{}",
                rt.cannon.state,
                rt.cannon.status.key(),
                rt.cannon.remaining_fuel,
                rt.cannon.blocks_placed,
                rt.cannon.blocks_to_place
            );
            runtimes().insert((dim.clone(), pos), rt);
            Some(line)
        })
        .flatten();
    done.ok_or_else(|| "no Schematicannon there".to_owned())
}

/// `status` of a cannon for the operator command and acceptance tooling.
pub(super) fn status_line(game: &mut Game<'_>, pos: Pos) -> Option<String> {
    game.native_with_mod_block_entity(pos, |entity| {
        (entity.id == BLOCK).then(|| {
            format!(
                "state {} status {} fuel {} placed {} to_place {} error '{}' missing '{}'",
                int(&entity.nbt, K_STATE),
                string(&entity.nbt, K_STATUS),
                int(&entity.nbt, K_FUEL),
                int(&entity.nbt, K_PLACED),
                int(&entity.nbt, K_TO_PLACE),
                string(&entity.nbt, K_ERROR),
                string(&entity.nbt, K_MISSING)
            )
        })
    })
    .flatten()
}

pub(super) fn destroyed(game: &mut Game<'_>, pos: Pos) {
    if game.native_block(pos) == BLOCK {
        runtimes().remove(&(game.dim.id().to_owned(), pos));
    }
}

// --------------------------------------------------------------- item source

const SIDES: [([i32; 3], D); 6] = [
    ([0, -1, 0], D::Up),
    ([0, 1, 0], D::Down),
    ([0, 0, -1], D::South),
    ([0, 0, 1], D::North),
    ([-1, 0, 0], D::East),
    ([1, 0, 0], D::West),
];

struct Handler {
    pos: Pos,
    face: D,
    slots: Vec<(usize, ItemStack)>,
}

/// The six neighbours' extractable stacks, read once. `taken` is replayed on the real
/// containers after the tick.
struct Snapshot {
    handlers: Vec<Handler>,
    taken: Vec<(Pos, D, usize, u32)>,
}

impl Snapshot {
    fn build(game: &Game<'_>, cannon: Pos) -> Self {
        let mut handlers = Vec::new();
        for (offset, face) in SIDES {
            let pos = [cannon[0] + offset[0], cannon[1] + offset[1], cannon[2] + offset[2]];
            if !game.world.loaded(pos[0], pos[2]) {
                continue;
            }
            let Some(slots) = game.native_container_slots(game.dim.id(), pos, face) else {
                continue;
            };
            let slots: Vec<(usize, ItemStack)> = slots
                .into_iter()
                .filter(|s| s.can_take)
                .filter_map(|s| s.stack.map(|stack| (s.slot, stack)))
                .filter(|(_, stack)| stack.count > 0)
                .collect();
            if !slots.is_empty() {
                handlers.push(Handler { pos, face, slots });
            }
        }
        Self { handlers, taken: Vec::new() }
    }
}

impl ItemSource for Snapshot {
    fn rescan(&mut self) {}
    fn has_creative_crate(&self) -> bool {
        false
    }
    fn handler_count(&self) -> usize {
        self.handlers.len()
    }
    fn extract(&mut self, handler: usize, req: &StackRequirement, max: i32, simulate: bool) -> i32 {
        let Some(h) = self.handlers.get_mut(handler) else {
            return 0;
        };
        let mut got = 0;
        for (slot, stack) in &mut h.slots {
            if got >= max {
                break;
            }
            if &*stack.item != req.item.as_str() || stack.count == 0 {
                continue;
            }
            let take = (max - got).min(stack.count as i32);
            got += take;
            if !simulate {
                stack.count -= take as u32;
                self.taken.push((h.pos, h.face, *slot, take as u32));
            }
        }
        got
    }
    fn damage_item(&mut self, _req: &StackRequirement, _simulate: bool) -> bool {
        // Damage-use requirements (shears and the like) are never produced by this adapter.
        false
    }
    fn take_gunpowder(&mut self) -> bool {
        let req = StackRequirement::consume(cn::GUNPOWDER, 1);
        (0..self.handlers.len()).any(|h| self.extract(h, &req, 1, false) == 1)
    }
    fn visit_stacks(&self, f: &mut dyn FnMut(&str, i32)) {
        for h in &self.handlers {
            for (_, stack) in &h.slots {
                f(&stack.item, stack.count as i32);
            }
        }
    }
}

struct WorldView<'a, 'g>(&'a Game<'g>);

impl TargetWorld for WorldView<'_, '_> {
    fn is_loaded(&self, pos: Pos) -> bool {
        self.0.world.loaded(pos[0], pos[2])
    }
}

// ------------------------------------------------------------- the printer

fn state_id(game: &Game<'_>, state: &SchemState) -> Option<BlockStateId> {
    let registry = game.pack.assets.registry;
    let default = registry.default_state(&state.name)?;
    let mut descriptor = registry.state(default)?.clone();
    for (key, value) in &state.properties {
        if !descriptor.properties.contains_key(key) {
            return None;
        }
        descriptor.properties.insert(key.clone(), value.clone());
    }
    registry.id(&descriptor)
}

fn is_air_name(name: &str) -> bool {
    matches!(name, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air")
}

/// Blocks the original prints after the solid ones (`BlockMovementChecks.isBrittle`, by name).
fn brittle(name: &str) -> bool {
    let short = name.rsplit(':').next().unwrap_or(name);
    short.ends_with("torch")
        || short.ends_with("_button")
        || short.ends_with("_pressure_plate")
        || short.ends_with("_carpet")
        || short.ends_with("rail")
        || matches!(short, "lever" | "repeater" | "comparator" | "redstone_wire" | "ladder")
}

fn prop<'a>(state: &'a SchemState, key: &str) -> Option<&'a str> {
    state.properties.get(key).map(String::as_str)
}

/// A second half placed by the first one's shot: the upper half of a door or tall plant, the
/// head of a bed.
fn is_second_half(state: &SchemState) -> bool {
    prop(state, "half") == Some("upper") && state.name != "minecraft:piston_head"
        || prop(state, "part") == Some("head") && state.name.ends_with("_bed")
}

fn bed_step(facing: &str) -> [i32; 3] {
    match facing {
        "north" => [0, 0, -1],
        "south" => [0, 0, 1],
        "west" => [-1, 0, 0],
        _ => [1, 0, 0],
    }
}

fn requirement(game: &Game<'_>, block: Option<&PlannedBlock>) -> ItemRequirement {
    let Some(block) = block else {
        return ItemRequirement::Empty;
    };
    let state = &block.state;
    if is_air_name(&state.name) || state.name == "minecraft:structure_void" {
        return ItemRequirement::Empty;
    }
    // Paid for by the first half (or by nothing: a piston head comes with its piston).
    if is_second_half(state) || state.name == "minecraft:piston_head" {
        return ItemRequirement::Empty;
    }
    if state.name.starts_with("create:belt")
        || matches!(
            state.name.as_str(),
            "minecraft:water" | "minecraft:lava" | "minecraft:fire" | "minecraft:soul_fire" | "minecraft:bubble_column"
        )
    {
        return ItemRequirement::Invalid;
    }
    let Some(id) = state_id(game, state) else {
        return ItemRequirement::Invalid;
    };
    let facts = game.res.facts().get(id);
    let item: &str = if facts.item.is_empty() { &state.name } else { &facts.item };
    if item == "minecraft:air" || game.res.items().get(item).is_none() {
        return ItemRequirement::Invalid;
    }
    let count = if prop(state, "type") == Some("double") && state.name.ends_with("_slab") {
        2
    } else {
        ["layers", "candles", "pickles", "eggs"]
            .into_iter()
            .find_map(|key| prop(state, key).and_then(|v| v.parse::<i32>().ok()))
            .unwrap_or(1)
    };
    ItemRequirement::Items(vec![StackRequirement::consume(item, count)])
}

/// `SchematicPrinter.shouldPlaceCurrent`'s world checks plus the facts `PlaceRules` needs.
fn decision(game: &Game<'_>, l: &Loaded, rel: Pos) -> PlaceDecision {
    let target = [l.anchor[0] + rel[0], l.anchor[1] + rel[1], l.anchor[2] + rel[2]];
    let (min_y, height) = game.native_world_height();
    if target[1] < min_y
        || target[1] >= min_y + height
        || !game.world.loaded(target[0], target[2])
        || !game.border.contains(f64::from(target[0]) + 0.5, f64::from(target[2]) + 0.5)
    {
        return PlaceDecision::Vetoed;
    }
    let block = l.index.get(&rel).map(|i| &l.plan.blocks[*i]);
    let air_name = "minecraft:air";
    let name = block.map_or(air_name, |b| b.state.name.as_str());
    let placing_air = is_air_name(name);
    let wanted = match block {
        Some(b) => state_id(game, &b.state),
        None => game.native_default_state(air_name),
    };
    let old = game.world.block(target[0], target[1], target[2]);
    if wanted == Some(old) || game.res.facts().get(old).hardness < 0.0 {
        return PlaceDecision::Vetoed;
    }
    let req = requirement(game, block);
    let state = block.map(|b| &b.state);
    let second = state.is_some_and(is_second_half) || name == "minecraft:piston_head";
    let facts_old = game.res.facts().get(old);
    PlaceDecision::Block(PlaceQuery {
        pos: target,
        placing_air,
        structure_void: name == "minecraft:structure_void",
        // Second halves are ignored (paid and placed with the first half), see `requirement`.
        requirement_empty: req.is_empty() && !second,
        requirement_invalid: req.is_invalid(),
        double_block_upper: state.is_some_and(|s| prop(s, "half") == Some("upper")),
        bed_head: state.is_some_and(|s| prop(s, "part") == Some("head") && s.name.ends_with("_bed")),
        piston_head: name == "minecraft:piston_head",
        belt_middle: false,
        to_replace: ReplaceInfo {
            has_block_entity: game.world.block_entities.contains_key(&(target[0], target[1], target[2])),
            redstone_conductor: facts_old.solid_render,
        },
        to_replace_other: None,
        is_normal_cube: wanted.is_some_and(|w| game.res.facts().get(w).solid_render) && !placing_air,
    })
}

struct Printer<'a, 'g> {
    game: &'a Game<'g>,
    st: &'a mut PrinterState,
}

fn build(game: &Game<'_>, d: &SchematicStackData) -> Result<Loaded, String> {
    let file = d.file.as_deref().ok_or("the Schematic has no file")?;
    let anchor = d.anchor.ok_or("the Schematic has no anchor")?;
    let bytes = paste::read_file(file)?;
    let schematic = create_schematics::read_gzip(&bytes, &Limits::default()).map_err(|e| e.to_string())?;
    let options = PlanOptions {
        include_air: true,
        quarter_turns: d.rotation.ordinal() as u8,
        mirror: match d.mirror {
            Mirror::None => PlanMirror::None,
            Mirror::LeftRight => PlanMirror::LeftRight,
            Mirror::FrontBack => PlanMirror::FrontBack,
        },
    };
    let plan = create_schematics::preflight(&schematic, &paste::Registry { game }, &options).map_err(|issues| {
        issues.iter().take(3).map(|i| i.message.clone()).collect::<Vec<_>>().join("; ")
    })?;
    let index = plan.blocks.iter().enumerate().map(|(i, b)| (b.relative_pos, i)).collect();
    let cursor = PrintCursor::new(transformed_bounds(&schematic.size, &options), Vec::new());
    Ok(Loaded { anchor, plan, index, cursor, steps: 0 })
}

/// `activeTemplate.getBoundingBox(settings, anchor)` relative to the anchor: the file's box after
/// the mirror and the quarter turns about the zero pivot (`SchematicItem.getSettings`), so it
/// is not `[0, size)` once turned: one clockwise quarter turn maps (x, z) to (-z, x).
fn transformed_bounds(size: &[i32; 3], options: &PlanOptions) -> Bounds {
    let far = [size[0] - 1, size[1] - 1, size[2] - 1];
    let map = |[x, y, z]: Pos| {
        let (x, z) = match options.mirror {
            PlanMirror::None => (x, z),
            PlanMirror::LeftRight => (x, -z),
            PlanMirror::FrontBack => (-x, z),
        };
        match options.quarter_turns {
            1 => [-z, y, x],
            2 => [-x, y, -z],
            3 => [z, y, -x],
            _ => [x, y, z],
        }
    };
    let (a, b) = (map([0; 3]), map(far));
    Bounds::new(
        [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])],
        [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])],
    )
}

impl PrinterBackend for Printer<'_, '_> {
    fn is_loaded(&self) -> bool {
        self.st.loaded.is_some()
    }
    fn is_errored(&self) -> bool {
        self.st.error.is_some()
    }
    fn load(&mut self, blueprint: &Stack) {
        let Some(d) = &blueprint.schematic else {
            return;
        };
        match build(self.game, d) {
            Ok(mut loaded) => {
                self.st.error = None;
                for _ in 0..std::mem::take(&mut self.st.replay) {
                    if !advance(&mut loaded) {
                        break;
                    }
                }
                self.st.loaded = Some(loaded);
            }
            Err(e) => {
                log::warn!("Create Schematicannon: {e}");
                self.st.error = Some(e);
                self.st.loaded = None;
            }
        }
    }
    fn is_world_empty(&self) -> bool {
        self.st.loaded.as_ref().is_none_or(|l| l.plan.blocks.is_empty())
    }
    fn anchor(&self) -> Option<Pos> {
        self.st.loaded.as_ref().map(|l| l.anchor)
    }
    fn reset_schematic(&mut self) {
        self.st.loaded = None;
        self.st.error = None;
        self.st.replay = 0;
    }
    fn advance_current_pos(&mut self) -> bool {
        self.st.loaded.as_mut().is_some_and(advance)
    }
    fn current_target(&self) -> Pos {
        self.st.loaded.as_ref().map_or([0; 3], |l| l.cursor.target(l.anchor))
    }
    fn current_requirement(&mut self) -> ItemRequirement {
        match &self.st.loaded {
            Some(l) => requirement(self.game, l.index.get(&l.cursor.current_pos).map(|i| &l.plan.blocks[*i])),
            None => ItemRequirement::Empty,
        }
    }
    fn current_place_decision(&mut self) -> PlaceDecision {
        match &self.st.loaded {
            Some(l) => decision(self.game, l, l.cursor.current_pos),
            None => PlaceDecision::Vetoed,
        }
    }
    fn current_action(&mut self) -> LaunchAction {
        let Some(l) = &self.st.loaded else {
            return LaunchAction::Block { pos: [0; 3], air: true };
        };
        let rel = l.cursor.current_pos;
        let pos = l.cursor.target(l.anchor);
        let block = l.index.get(&rel).map(|i| l.plan.blocks[*i].clone());
        let air = block.as_ref().is_none_or(|b| is_air_name(&b.state.name));
        let mut partner = None;
        if let Some(b) = block.as_ref().filter(|_| !air) {
            let step = if prop(&b.state, "half") == Some("lower") {
                Some([0, 1, 0])
            } else if prop(&b.state, "part") == Some("foot") && b.state.name.ends_with("_bed") {
                Some(bed_step(prop(&b.state, "facing").unwrap_or("north")))
            } else {
                None
            };
            if let Some(step) = step {
                let other = [rel[0] + step[0], rel[1] + step[1], rel[2] + step[2]];
                if let Some(second) = l
                    .index
                    .get(&other)
                    .map(|i| &l.plan.blocks[*i])
                    .filter(|s| s.state.name == b.state.name && is_second_half(&s.state))
                {
                    let at = [pos[0] + step[0], pos[1] + step[1], pos[2] + step[2]];
                    partner = Some((at, second.clone()));
                }
            }
        }
        self.st.pending.push(Pending { pos, block: block.filter(|_| !air), partner });
        LaunchAction::Block { pos, air }
    }
    fn mark_all_block_requirements(
        &mut self,
        checklist: &mut MaterialChecklist,
        should_place: &dyn Fn(&PlaceQuery) -> bool,
    ) -> i32 {
        let Some(l) = &self.st.loaded else {
            return 0;
        };
        let mut count = 0;
        for block in &l.plan.blocks {
            if is_air_name(&block.state.name) {
                continue;
            }
            let PlaceDecision::Block(q) = decision(self.game, l, block.relative_pos) else {
                continue;
            };
            if !should_place(&q) {
                continue;
            }
            let req = requirement(self.game, Some(block));
            if req.is_invalid() {
                continue;
            }
            checklist.require(&req);
            count += 1;
        }
        count
    }
    fn mark_all_entity_requirements(&mut self, _checklist: &mut MaterialChecklist) {}
    fn send_block_updates(&mut self) {}
}

fn advance(l: &mut Loaded) -> bool {
    let more = l.cursor.advance(&|rel| {
        l.index
            .get(&rel)
            .is_some_and(|i| brittle(&l.plan.blocks[*i].state.name))
    });
    if more {
        l.steps += 1;
    }
    more
}

impl PrinterState {
    fn take_pending(&mut self, pos: Pos) -> Option<Pending> {
        let at = self.pending.iter().position(|p| p.pos == pos)?;
        Some(self.pending.remove(at))
    }
}

// ----------------------------------------------------------------- landing

fn place_block(game: &mut Game<'_>, dim: &str, pos: Pos, block: &PlannedBlock) {
    let mut single = block.clone();
    single.relative_pos = [0, 0, 0];
    let plan = Plan { size: [1, 1, 1], blocks: vec![single], notes: Vec::new(), skipped_air: 0 };
    match paste::destination(game, &plan, pos) {
        Ok(writes) => {
            for w in writes {
                if w.old == w.state {
                    continue;
                }
                game.native_set_block(dim, w.pos, w.state, 18);
                if let Some(nbt) = &w.nbt {
                    game.load_block_entity_nbt(w.pos, nbt);
                    game.native_block_entity_sync(w.pos);
                }
            }
        }
        Err(e) => log::warn!("Create Schematicannon: a block was refused on landing at {pos:?}: {e}"),
    }
}

fn land(game: &mut Game<'_>, dim: &str, p: Pending) {
    match &p.block {
        None => {
            if let Some(air) = game.native_default_state("minecraft:air") {
                game.native_set_block(dim, p.pos, air, 18);
            }
        }
        Some(block) => {
            place_block(game, dim, p.pos, block);
            if let Some((at, second)) = &p.partner {
                place_block(game, dim, *at, second);
            }
        }
    }
}

// -------------------------------------------------------------------- tick

fn map_output(item: &str) -> &str {
    if item == cn::WRITTEN_BOOK { data::CLIPBOARD } else { item }
}

/// Server tick of the block entity (`SchematicannonBlockEntity.tick`).
pub(super) fn tick(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity) {
    if entity.id != BLOCK || !simulates(game) || !game.native_on("create") || entity.items.len() < SLOTS {
        return;
    }
    let dim = game.dim.id().to_owned();
    let key = (dim.clone(), pos);
    let mut rt = take(&key, entity);
    let centre = [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 1.1, f64::from(pos[2]) + 0.5];

    // Items the slot filters refuse (placed by a plain click) are thrown out.
    for slot in [cn::SLOT_BLUEPRINT, cn::SLOT_BOOK_IN, cn::SLOT_GUNPOWDER] {
        let refused = entity.items[slot]
            .as_ref()
            .is_some_and(|s| s.count > 0 && !CannonInventory::is_item_valid(slot, &s.item));
        if refused && let Some(stack) = entity.items[slot].take() {
            game.spawn_item(stack, centre, [0.0, 0.2, 0.0], 10);
        }
    }
    let before: Vec<Option<Stack>> = (0..SLOTS)
        .map(|i| entity.items[i].as_ref().filter(|s| s.count > 0).map(|s| data::cannon_stack(game, s)))
        .collect();
    for (i, stack) in before.iter().enumerate() {
        rt.cannon.inventory.set(i, stack.clone());
    }

    let (arrived, taken) = {
        let g: &Game<'_> = game;
        let mut printer = Printer { game: g, st: &mut rt.printer };
        let mut items = Snapshot::build(g, pos);
        let world = WorldView(g);
        let mut env = Env {
            printer: &mut printer,
            items: &mut items,
            world: &world,
            config: CannonConfig::default(),
            server: true,
        };
        let arrived = rt.cannon.tick(&mut env);
        (arrived, items.taken)
    };
    for (at, face, slot, count) in taken {
        game.native_extract_item(&dim, at, face, slot, count);
    }
    let flights: Vec<FlyingItem> = arrived;
    for flight in flights {
        if let Some(pending) = rt.printer.take_pending(flight.action.target()) {
            land(game, &dim, pending);
        }
    }
    let mut checklist_printed = false;
    for event in std::mem::take(&mut rt.cannon.events) {
        match event {
            CannonEvent::FiringSound => {
                game.native_play_sound_in_place("minecraft:block.dispenser.launch", centre, 2, 0.6, 1.0);
            }
            CannonEvent::FinishSound => {
                game.native_play_sound_in_place("minecraft:block.note_block.chime", centre, 2, 1.0, 1.0);
            }
            CannonEvent::ChecklistPrinted { .. } => checklist_printed = true,
        }
    }

    for i in 0..SLOTS {
        let now = rt.cannon.inventory.get(i).cloned();
        if now == before[i] {
            continue;
        }
        let existing = entity.items[i].take();
        entity.items[i] = now.map(|s| {
            let item = map_output(&s.item).to_owned();
            let mut out = match existing {
                Some(e) if *e.item == *item => e,
                _ => ItemStack::new(&item, 1),
            };
            out.count = s.count.max(0) as u32;
            out
        });
    }
    if checklist_printed && let Some(stack) = entity.items[cn::SLOT_BOOK_OUT].as_mut() {
        data::write_checklist(game, stack, &rt.cannon.checklist);
    }
    if let Some(file) = entity.items[cn::SLOT_BLUEPRINT]
        .as_ref()
        .and_then(|s| data::read(game, s))
        .and_then(|d| d.file)
    {
        put(&mut entity.nbt, K_FILE, Tag::String(file.as_str().into()));
    } else {
        put(&mut entity.nbt, K_FILE, Tag::String("".into()));
    }
    let changed = store(&rt, entity);
    if changed || std::mem::take(&mut rt.cannon.send_update) {
        game.native_block_entity_sync(pos);
    }
    runtimes().insert(key, rt);
}

// ------------------------------------------------------------- quick move

/// `SchematicannonMenu.quickMoveStack` for a shift-click from the player's inventory: the
/// schematic slot, the paper slot or the gunpowder slot (`CannonInventory::quick_move_target`).
pub(super) fn quick_move(
    game: &Game<'_>,
    entity: &mut ModBlockEntity,
    container_slots: usize,
    slot: usize,
    stack: &mut ItemStack,
    handled: &mut bool,
) {
    if entity.id != BLOCK || slot < container_slots || entity.items.len() < SLOTS {
        return;
    }
    *handled = true;
    let Some(target) = CannonInventory::quick_move_target(&stack.item) else {
        return;
    };
    let limit = game
        .native_max_stack_size(&stack.item)
        .unwrap_or(64)
        .min(cn::SLOT_LIMIT as u32);
    match &mut entity.items[target] {
        Some(held) if held.same_item_same_components(stack) => {
            let moved = stack.count.min(limit.saturating_sub(held.count));
            held.count += moved;
            stack.count -= moved;
        }
        Some(_) => {}
        empty => {
            let moved = stack.count.min(limit);
            let mut put_in = stack.clone();
            put_in.count = moved;
            stack.count -= moved;
            *empty = Some(put_in);
        }
    }
}

// ------------------------------------------------------------------ screen

const WIDTH: f32 = 220.0;
const HEIGHT: f32 = 246.0;
const INV_X: f32 = 22.0;
const INV_Y: f32 = 164.0;
const SLOT_POS: [[f32; 2]; SLOTS] = [[14.0, 22.0], [190.0, 22.0], [14.0, 50.0], [190.0, 50.0], [14.0, 78.0]];

/// id, x, y, width, height, label, tooltip.
const BUTTONS: [(i32, f32, f32, f32, f32, &str, &str); 9] = [
    (1, 40.0, 100.0, 40.0, 14.0, "Play", "Play"),
    (2, 84.0, 100.0, 40.0, 14.0, "Pause", "Pause"),
    (3, 128.0, 100.0, 40.0, 14.0, "Stop", "Stop and reset the print"),
    (4, 40.0, 118.0, 32.0, 14.0, "None", "Do not replace solid blocks"),
    (5, 76.0, 118.0, 32.0, 14.0, "Solid", "Replace solid with solid"),
    (6, 112.0, 118.0, 32.0, 14.0, "Any", "Replace solid with any block"),
    (7, 148.0, 118.0, 32.0, 14.0, "Empty", "Replace solid with empty blocks"),
    (8, 40.0, 136.0, 64.0, 14.0, "Skip missing", "Skip missing blocks instead of pausing"),
    (9, 108.0, 136.0, 72.0, 14.0, "Replace BEs", "Replace block entities"),
];

pub(super) fn screen(block: &str, screen: &mut Option<crate::mod_api::NativeScreen>) {
    if block != BLOCK || screen.is_some() {
        return;
    }
    *screen = Some(crate::mod_api::NativeScreen {
        width: WIDTH,
        height: HEIGHT,
        slots: SLOT_POS.iter().map(|p| Some(*p)).collect(),
        inventory_y: INV_Y,
        inventory_x: INV_X,
        own_labels: true,
        own_background: true,
    });
}

fn frame(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch, x: f32, y: f32) {
    game.native_fill(batch, [x - 1.0, y - 1.0, 18.0, 18.0], 0xFF37_3737);
    game.native_fill(batch, [x, y, 16.0, 16.0], 0xFF8B_8B8B);
}

pub(super) fn panel(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch, origin: [f32; 2], w: f32, h: f32) {
    let [x, y] = origin;
    game.native_fill(batch, [x, y, w, h], 0xFF55_5555);
    game.native_fill(batch, [x + 2.0, y + 2.0, w - 4.0, h - 4.0], 0xFFC6_C6C6);
    game.native_fill(batch, [x, y, w - 2.0, 2.0], 0xFFFF_FFFF);
    game.native_fill(batch, [x, y, 2.0, h - 2.0], 0xFFFF_FFFF);
}

pub(super) fn button(
    game: &Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    rect: [f32; 4],
    label: &str,
    active: bool,
) {
    let fill = if active { 0xFF3F_7F3F } else { 0xFF6B_6B6B };
    game.native_fill(batch, rect, 0xFF22_2222);
    game.native_fill(batch, [rect[0] + 1.0, rect[1] + 1.0, rect[2] - 2.0, rect[3] - 2.0], fill);
    let width = game.native_text_width(label);
    game.native_text(
        batch,
        label,
        [rect[0] + (rect[2] - width) / 2.0, rect[1] + (rect[3] - 8.0) / 2.0],
        0xFF_FF_FF,
        true,
    );
}

pub(super) fn screen_draw(
    game: &Game<'_>,
    block: &str,
    data: &[i32],
    batch: &mut horizon_client::hud::HudBatch,
    origin: [f32; 2],
) {
    if block != BLOCK {
        return;
    }
    let [x, y] = origin;
    let get = |i: usize| data.get(i).copied().unwrap_or(0);
    panel(game, batch, origin, WIDTH, HEIGHT);
    for [sx, sy] in SLOT_POS {
        frame(game, batch, x + sx, y + sy);
    }
    for row in 0..3 {
        for col in 0..9 {
            frame(game, batch, x + INV_X + 18.0 * col as f32, y + INV_Y + 18.0 * row as f32);
        }
    }
    for col in 0..9 {
        frame(game, batch, x + INV_X + 18.0 * col as f32, y + INV_Y + 58.0);
    }
    let (flags, mode) = (get(2), (get(2) >> 2) & 3);
    for (id, bx, by, bw, bh, label, _) in BUTTONS {
        let active = match id {
            1 => get(0) == 2,
            2 => get(0) == 1,
            3 => get(0) == 0,
            4..=7 => mode == id - 4,
            8 => flags & 1 != 0,
            _ => flags & 2 != 0,
        };
        button(game, batch, [x + bx, y + by, bw, bh], label, active);
    }
    let placed = get(4) | (get(5) << 15);
    let total = get(6) | (get(7) << 15);
    let entity_nbt = game
        .native_screen_pos()
        .and_then(|at| match game.world.block_entities.get(&(at[0], at[1], at[2])) {
            Some(BlockEntity::Mod(e)) => Some(e.nbt.clone()),
            _ => None,
        })
        .unwrap_or_default();
    let title = game.native_screen_title().unwrap_or_else(|| "Schematicannon".into());
    game.native_text(batch, &title, [x + 14.0, y + 6.0], 0x40_40_40, false);
    game.native_text(batch, status_text(get(1)), [x + 40.0, y + 24.0], 0x20_20_20, false);
    let missing = string(&entity_nbt, K_MISSING);
    let error = string(&entity_nbt, K_ERROR);
    let file = string(&entity_nbt, K_FILE);
    if !missing.is_empty() {
        let line = format!("{} x{}", missing, int(&entity_nbt, K_MISSING_COUNT));
        game.native_text(batch, &line, [x + 40.0, y + 76.0], 0x9F_20_20, false);
    } else if !error.is_empty() {
        game.native_text(batch, &error.chars().take(36).collect::<String>(), [x + 40.0, y + 76.0], 0x9F_20_20, false);
    } else if !file.is_empty() {
        game.native_text(batch, &file, [x + 40.0, y + 76.0], 0x40_40_40, false);
    }
    // Progress bar and counts.
    game.native_fill(batch, [x + 40.0, y + 38.0, 140.0, 8.0], 0xFF37_3737);
    if total > 0 {
        let width = (138.0 * placed as f32 / total as f32).clamp(0.0, 138.0);
        game.native_fill(batch, [x + 41.0, y + 39.0, width, 6.0], 0xFF4C_B04C);
    }
    game.native_text(batch, &format!("{placed} / {total} blocks"), [x + 40.0, y + 50.0], 0x40_40_40, false);
    game.native_text(batch, &format!("{} shots of fuel", get(3)), [x + 40.0, y + 62.0], 0x40_40_40, false);
    if get(8) > 0 {
        game.native_text(batch, &format!("printing checklist {}%", get(8)), [x + 40.0, y + 88.0], 0x40_40_40, false);
    }
    game.native_text(batch, "Inventory", [x + INV_X, y + INV_Y - 11.0], 0x40_40_40, false);
}

fn hit(at: [f32; 2]) -> Option<usize> {
    BUTTONS
        .iter()
        .position(|(_, x, y, w, h, _, _)| at[0] >= *x && at[0] < x + w && at[1] >= *y && at[1] < y + h)
}

pub(super) fn screen_click(block: &str, at: [f32; 2], button: &mut Option<i32>) {
    if block == BLOCK && button.is_none() && let Some(i) = hit(at) {
        *button = Some(BUTTONS[i].0);
    }
}

pub(super) fn screen_tooltip(block: &str, at: [f32; 2], lines: &mut Vec<String>) {
    if block == BLOCK && let Some(i) = hit(at) {
        lines.push(BUTTONS[i].6.into());
    }
}

/// Test hook for the acceptance script: the item in a cannon slot, if any.
#[allow(dead_code, reason = "read helper for acceptance tooling")]
pub(super) fn slot_item(game: &Game<'_>, pos: Pos, slot: usize) -> Option<ItemStack> {
    match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
        Some(BlockEntity::Mod(e)) if e.id == BLOCK => e.items.get(slot).cloned().flatten(),
        _ => None,
    }
}
