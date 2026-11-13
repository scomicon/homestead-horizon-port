// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Pure Schematicannon logic after SchematicannonBlockEntity, SchematicannonInventory,
// SchematicannonMenu, ConfigureSchematicannonPacket and LaunchedItem (Create 6.0.10):
// state machine, status messages, fuel, cooldown, placement rules (replace modes),
// missing/skipped item handling, checklist paper printer, projectile flight time.
// The loaded schematic (blocks, requirements, world queries) is reached through
// `PrinterBackend`, neighbouring containers through `ItemSource` and chunk loading
// through `TargetWorld`; the adapter owns sounds, particles and the actual placement
// of arrived projectiles.
use crate::material_checklist::{ItemRequirement, MaterialChecklist, StackRequirement, UseKind};
use crate::schematic_tool::{EMPTY_SCHEMATIC_ITEM, SCHEMATIC_ITEM, SchematicStackData};

pub type Pos = [i32; 3];

pub const GUNPOWDER: &str = "minecraft:gunpowder";
pub const CLIPBOARD: &str = "create:clipboard";
pub const BOOK: &str = "minecraft:book";
pub const WRITTEN_BOOK: &str = "minecraft:written_book";

/// Ticks between inventory rescans while the cannon is not stopped.
pub const NEIGHBOUR_CHECKING: i32 = 100;
/// Maximum distance between cannon and schematic anchor.
pub const MAX_ANCHOR_DISTANCE: f64 = 256.0;
/// Per-tick limit of skipped targets handled in one tick.
pub const MAX_SKIPS_PER_TICK: i32 = 1000;
/// Paper printer progress per tick (a page takes about 20 ticks).
pub const PAPER_PROGRESS_STEP: f32 = 0.05;
/// A target closer than this to the cannon is never placed.
pub const MIN_TARGET_DISTANCE: f64 = 2.0;
pub const SLOT_COUNT: usize = 5;
pub const SLOT_BLUEPRINT: usize = 0;
pub const SLOT_BLUEPRINT_OUT: usize = 1;
pub const SLOT_BOOK_IN: usize = 2;
pub const SLOT_BOOK_OUT: usize = 3;
pub const SLOT_GUNPOWDER: usize = 4;
/// `ItemStackHandler` default slot limit.
pub const SLOT_LIMIT: i32 = 64;

/// Server config (`CSchematics.schematicannon`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CannonConfig {
    /// `schematicannonDelay`: ticks between shots (default 10, min 1).
    pub delay: i32,
    /// `schematicannonShotsPerGunpowder` (default 400, min 1).
    pub shots_per_gunpowder: i32,
}

impl Default for CannonConfig {
    fn default() -> Self {
        Self { delay: 10, shots_per_gunpowder: 400 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Stopped,
    Paused,
    Running,
}

/// `statusMsg` values; the language key is `schematicannon.status.<key>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Idle,
    SchematicInvalid,
    SchematicNotPlaced,
    SchematicErrored,
    SchematicExpired,
    TargetOutsideRange,
    Ready,
    NoGunpowder,
    TargetNotLoaded,
    Searching,
    Skipping,
    MissingBlock,
    Placing,
    Clearing,
    Finished,
    /// Set by the PLAY button packet.
    Running,
    /// Set by the PAUSE button packet.
    Paused,
    /// Set by the STOP button packet.
    Stopped,
}

impl Status {
    pub fn key(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::SchematicInvalid => "schematicInvalid",
            Self::SchematicNotPlaced => "schematicNotPlaced",
            Self::SchematicErrored => "schematicErrored",
            Self::SchematicExpired => "schematicExpired",
            Self::TargetOutsideRange => "targetOutsideRange",
            Self::Ready => "ready",
            Self::NoGunpowder => "noGunpowder",
            Self::TargetNotLoaded => "targetNotLoaded",
            Self::Searching => "searching",
            Self::Skipping => "skipping",
            Self::MissingBlock => "missingBlock",
            Self::Placing => "placing",
            Self::Clearing => "clearing",
            Self::Finished => "finished",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Stopped => "stopped",
        }
    }
    pub fn lang_key(self) -> String {
        format!("schematicannon.status.{}", self.key())
    }
}

/// `ConfigureSchematicannonPacket.Option` in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigOption {
    DontReplace,
    ReplaceSolid,
    ReplaceAny,
    ReplaceEmpty,
    SkipMissing,
    SkipBlockEntities,
    Play,
    Pause,
    Stop,
}

pub const REPLACE_NONE: u8 = 0;
pub const REPLACE_SOLID: u8 = 1;
pub const REPLACE_ANY: u8 = 2;
pub const REPLACE_EMPTY: u8 = 3;

/// An item stack as far as the cannon cares (id, count, schematic components).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stack {
    pub item: String,
    pub count: i32,
    pub schematic: Option<SchematicStackData>,
}

impl Stack {
    pub fn new(item: &str, count: i32) -> Self {
        Self { item: item.to_owned(), count, schematic: None }
    }
    pub fn schematic(data: SchematicStackData) -> Self {
        Self { item: SCHEMATIC_ITEM.to_owned(), count: 1, schematic: Some(data) }
    }
}

/// `SchematicannonInventory` (5 slots).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CannonInventory {
    slots: [Option<Stack>; SLOT_COUNT],
}

impl CannonInventory {
    /// `isItemValid`: schematic / none / clipboard, book, written book / none / gunpowder.
    pub fn is_item_valid(slot: usize, item: &str) -> bool {
        match slot {
            SLOT_BLUEPRINT => item == SCHEMATIC_ITEM,
            SLOT_BOOK_IN => item == CLIPBOARD || item == BOOK || item == WRITTEN_BOOK,
            SLOT_GUNPOWDER => item == GUNPOWDER,
            _ => false,
        }
    }

    /// `SchematicannonMenu.quickMoveStack` from a player slot: slot 0, then 2, then 4.
    pub fn quick_move_target(item: &str) -> Option<usize> {
        [SLOT_BLUEPRINT, SLOT_BOOK_IN, SLOT_GUNPOWDER].into_iter().find(|s| Self::is_item_valid(*s, item))
    }

    pub fn get(&self, slot: usize) -> Option<&Stack> {
        self.slots.get(slot).and_then(|s| s.as_ref())
    }
    pub fn is_empty(&self, slot: usize) -> bool {
        self.get(slot).is_none_or(|s| s.count <= 0)
    }
    pub fn count(&self, slot: usize) -> i32 {
        self.get(slot).map_or(0, |s| s.count)
    }
    pub fn set(&mut self, slot: usize, stack: Option<Stack>) {
        self.slots[slot] = stack.filter(|s| s.count > 0);
    }
    /// Player insertion honouring the slot filter and the 64 item limit.
    /// Returns the number of items that did not fit.
    pub fn insert(&mut self, slot: usize, stack: Stack) -> i32 {
        if !Self::is_item_valid(slot, &stack.item) {
            return stack.count;
        }
        match &mut self.slots[slot] {
            None => {
                let n = stack.count.min(SLOT_LIMIT);
                let left = stack.count - n;
                self.slots[slot] = Some(Stack { count: n, ..stack });
                left
            }
            Some(cur) if cur.item == stack.item && cur.schematic == stack.schematic => {
                let n = stack.count.min(SLOT_LIMIT - cur.count).max(0);
                cur.count += n;
                stack.count - n
            }
            Some(_) => stack.count,
        }
    }
    pub fn shrink(&mut self, slot: usize, n: i32) {
        if let Some(s) = self.slots[slot].as_mut() {
            s.count -= n;
            if s.count <= 0 {
                self.slots[slot] = None;
            }
        }
    }
}

/// Facts about one target needed by `shouldPlace`/`shouldIgnoreBlockState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplaceInfo {
    pub has_block_entity: bool,
    pub redstone_conductor: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaceQuery {
    /// World position of the target.
    pub pos: Pos,
    /// Schematic state is air (an "erase" target).
    pub placing_air: bool,
    pub structure_void: bool,
    /// Requirement flags used by `shouldIgnoreBlockState`.
    pub requirement_empty: bool,
    pub requirement_invalid: bool,
    pub double_block_upper: bool,
    pub bed_head: bool,
    pub piston_head: bool,
    pub belt_middle: bool,
    pub to_replace: ReplaceInfo,
    /// Second block that would be replaced (other bed half / upper door half).
    pub to_replace_other: Option<ReplaceInfo>,
    /// Schematic state is a redstone conductor ("normal cube").
    pub is_normal_cube: bool,
}

/// `shouldIgnoreBlockState`: blocks that never need an own shot.
pub fn should_ignore_block_state(q: &PlaceQuery) -> bool {
    if q.structure_void {
        return true;
    }
    if q.requirement_empty || q.requirement_invalid {
        return false;
    }
    q.double_block_upper || q.bed_head || q.piston_head || q.belt_middle
}

/// Cannon settings that decide `shouldPlace`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaceRules {
    pub cannon_pos: Pos,
    pub replace_mode: u8,
    pub replace_block_entities: bool,
}

impl PlaceRules {
    pub fn should_place(&self, q: &PlaceQuery) -> bool {
        let d: i64 = (0..3).map(|i| {
            let v = (q.pos[i] - self.cannon_pos[i]) as i64;
            v * v
        }).sum();
        if (d as f64) < MIN_TARGET_DISTANCE * MIN_TARGET_DISTANCE {
            return false;
        }
        if !self.replace_block_entities
            && (q.to_replace.has_block_entity || q.to_replace_other.is_some_and(|o| o.has_block_entity))
        {
            return false;
        }
        if should_ignore_block_state(q) {
            return false;
        }
        let air = q.placing_air;
        let none_conductor = !q.to_replace.redstone_conductor && !q.to_replace_other.is_some_and(|o| o.redstone_conductor);
        match self.replace_mode {
            REPLACE_EMPTY => true,
            REPLACE_ANY if !air => true,
            REPLACE_SOLID if (q.is_normal_cube || none_conductor) && !air => true,
            REPLACE_NONE if none_conductor && !air => true,
            _ => false,
        }
    }
}

/// `SchematicPrinter.shouldPlaceCurrent` outcome before the cannon predicate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlaceDecision {
    /// Unloaded chunk, outside the world border, target already equal, or unbreakable.
    Vetoed,
    /// Entities are always placed.
    Entity,
    Block(PlaceQuery),
}

/// What `handleCurrentTarget` hands to the launcher.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LaunchAction {
    Block { pos: Pos, air: bool },
    /// A belt run; non-final segments were turned into shafts before launching.
    Belt { pos: Pos, length: i32 },
    Entity { pos: Pos },
}

impl LaunchAction {
    pub fn target(&self) -> Pos {
        match self {
            Self::Block { pos, .. } | Self::Belt { pos, .. } | Self::Entity { pos } => *pos,
        }
    }
}

/// The loaded schematic (`SchematicPrinter`) seen by the cannon.
pub trait PrinterBackend {
    fn is_loaded(&self) -> bool;
    fn is_errored(&self) -> bool;
    /// `loadSchematic(blueprint, level, true)`.
    fn load(&mut self, blueprint: &Stack);
    fn is_world_empty(&self) -> bool;
    fn anchor(&self) -> Option<Pos>;
    fn reset_schematic(&mut self);
    /// Advance the print cursor; false when everything was printed.
    fn advance_current_pos(&mut self) -> bool;
    fn current_target(&self) -> Pos;
    fn current_requirement(&mut self) -> ItemRequirement;
    fn current_place_decision(&mut self) -> PlaceDecision;
    /// `handleCurrentTarget`: describes the block, belt or entity to launch.
    fn current_action(&mut self) -> LaunchAction;
    /// `markAllBlockRequirements`; returns the number of requirements added.
    fn mark_all_block_requirements(&mut self, checklist: &mut MaterialChecklist, should_place: &dyn Fn(&PlaceQuery) -> bool) -> i32;
    fn mark_all_entity_requirements(&mut self, checklist: &mut MaterialChecklist);
    /// `sendBlockUpdates` (water shell around the schematic).
    fn send_block_updates(&mut self);
}

/// Neighbouring item handlers (`attachedInventories`) and the creative crate.
pub trait ItemSource {
    /// `findInventories`: rebuild the list from the six neighbours.
    fn rescan(&mut self);
    fn has_creative_crate(&self) -> bool;
    fn handler_count(&self) -> usize;
    /// Extract up to `max` matching items from one handler; returns the amount.
    fn extract(&mut self, handler: usize, req: &StackRequirement, max: i32, simulate: bool) -> i32;
    /// Find a damageable matching item, apply one point of damage (and reinsert it
    /// unless broken) unless `simulate`. Returns whether one was found.
    fn damage_item(&mut self, req: &StackRequirement, simulate: bool) -> bool;
    /// Really extract one gunpowder from the first handler that has one.
    fn take_gunpowder(&mut self) -> bool;
    /// Visit every extractable stack (`item`, `count`) for the checklist.
    fn visit_stacks(&self, f: &mut dyn FnMut(&str, i32));
}

pub trait TargetWorld {
    fn is_loaded(&self, pos: Pos) -> bool;
}

/// Everything the cannon talks to during a tick.
pub struct Env<'a> {
    pub printer: &'a mut dyn PrinterBackend,
    pub items: &'a mut dyn ItemSource,
    pub world: &'a dyn TargetWorld,
    pub config: CannonConfig,
    pub server: bool,
}

/// `LaunchedItem.ticksForDistance`: `max(10, (int) (distSqr^(1/4) * 4))`.
pub fn ticks_for_distance(start: Pos, target: Pos) -> i32 {
    let d: i64 = (0..3).map(|i| {
        let v = (target[i] - start[i]) as i64;
        v * v
    }).sum();
    let t = (d as f64).sqrt().sqrt() * 4.0;
    (10.0f64.max(t)) as i32
}

/// A projectile in flight (`LaunchedItem`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlyingItem {
    pub action: LaunchAction,
    pub icon: Option<String>,
    pub total_ticks: i32,
    pub ticks_remaining: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CannonEvent {
    /// `SCHEMATICANNON_LAUNCH_BLOCK` played at the cannon.
    FiringSound,
    /// `SCHEMATICANNON_FINISH`.
    FinishSound,
    /// A checklist was printed into slot 3; `clipboard` says whether the input was a clipboard.
    ChecklistPrinted { clipboard: bool },
}

#[derive(Clone, Debug)]
pub struct Schematicannon {
    pub pos: Pos,
    pub inventory: CannonInventory,
    pub checklist: MaterialChecklist,
    pub flying: Vec<FlyingItem>,
    pub events: Vec<CannonEvent>,
    pub state: State,
    pub status: Status,
    pub missing_item: Option<StackRequirement>,
    pub position_not_loaded: bool,
    pub has_creative_crate: bool,
    pub send_update: bool,
    pub dont_update_checklist: bool,
    pub neighbour_check_cooldown: i32,
    printer_cooldown: i32,
    block_skipped: bool,
    pub remaining_fuel: i32,
    pub book_printing_progress: f32,
    pub schematic_progress: f32,
    pub blocks_placed: i32,
    pub blocks_to_place: i32,
    pub replace_mode: u8,
    pub skip_missing: bool,
    pub replace_block_entities: bool,
}

impl Schematicannon {
    pub fn new(pos: Pos) -> Self {
        Self {
            pos,
            inventory: CannonInventory::default(),
            checklist: MaterialChecklist::new(),
            flying: Vec::new(),
            events: Vec::new(),
            state: State::Stopped,
            status: Status::Idle,
            missing_item: None,
            position_not_loaded: false,
            has_creative_crate: false,
            send_update: false,
            dont_update_checklist: false,
            neighbour_check_cooldown: 0,
            printer_cooldown: 0,
            block_skipped: false,
            remaining_fuel: 0,
            book_printing_progress: 0.0,
            schematic_progress: 0.0,
            blocks_placed: 0,
            blocks_to_place: 0,
            replace_mode: REPLACE_ANY,
            skip_missing: false,
            replace_block_entities: false,
        }
    }

    pub fn rules(&self) -> PlaceRules {
        PlaceRules { cannon_pos: self.pos, replace_mode: self.replace_mode, replace_block_entities: self.replace_block_entities }
    }

    /// `ConfigureSchematicannonPacket` handler (menu guard is the caller's job).
    pub fn apply_option(&mut self, option: ConfigOption, set: bool) {
        match option {
            ConfigOption::DontReplace => self.replace_mode = REPLACE_NONE,
            ConfigOption::ReplaceSolid => self.replace_mode = REPLACE_SOLID,
            ConfigOption::ReplaceAny => self.replace_mode = REPLACE_ANY,
            ConfigOption::ReplaceEmpty => self.replace_mode = REPLACE_EMPTY,
            ConfigOption::SkipMissing => self.skip_missing = set,
            ConfigOption::SkipBlockEntities => self.replace_block_entities = set,
            ConfigOption::Play => {
                self.state = State::Running;
                self.status = Status::Running;
            }
            ConfigOption::Pause => {
                self.state = State::Paused;
                self.status = Status::Paused;
            }
            ConfigOption::Stop => {
                self.state = State::Stopped;
                self.status = Status::Stopped;
            }
        }
        self.send_update = true;
    }

    /// `getShotsPerGunpowder`: 0 with a creative crate.
    pub fn shots_per_gunpowder(&self, cfg: &CannonConfig) -> i32 {
        if self.has_creative_crate { 0 } else { cfg.shots_per_gunpowder }
    }

    fn find_inventories(&mut self, items: &mut dyn ItemSource) {
        items.rescan();
        self.has_creative_crate = items.has_creative_crate();
    }

    fn reset_printer(&mut self, printer: &mut dyn PrinterBackend) {
        printer.reset_schematic();
        self.missing_item = None;
        self.send_update = true;
        self.schematic_progress = 0.0;
        self.blocks_placed = 0;
        self.blocks_to_place = 0;
    }

    /// One game tick. Returns the projectiles that arrived (server only); the
    /// adapter places them (`LaunchedItem.place`).
    pub fn tick(&mut self, env: &mut Env) -> Vec<FlyingItem> {
        if self.state != State::Stopped {
            let due = self.neighbour_check_cooldown <= 0;
            self.neighbour_check_cooldown -= 1;
            if due {
                self.neighbour_check_cooldown = NEIGHBOUR_CHECKING;
                self.find_inventories(env.items);
            }
        }
        let arrived = self.tick_flying(env.server);
        if env.server {
            self.tick_paper_printer(env);
            self.refill_fuel_if_possible(env);
            let mut skips_left = MAX_SKIPS_PER_TICK;
            self.block_skipped = true;
            while self.block_skipped && skips_left > 0 {
                skips_left -= 1;
                self.tick_printer(env);
            }
            self.schematic_progress = if self.blocks_to_place == 0 {
                0.0
            } else {
                self.blocks_placed as f32 / self.blocks_to_place as f32
            };
        }
        arrived
    }

    /// `tickFlyingBlocks`: ticks remaining count down first, so a projectile lands
    /// `total_ticks + 1` ticks after launch. The client never places anything.
    pub fn tick_flying(&mut self, server: bool) -> Vec<FlyingItem> {
        let mut arrived = Vec::new();
        let mut keep = Vec::with_capacity(self.flying.len());
        for mut f in self.flying.drain(..) {
            if f.ticks_remaining > 0 {
                f.ticks_remaining -= 1;
                keep.push(f);
            } else if server {
                arrived.push(f);
            } else {
                keep.push(f);
            }
        }
        self.flying = keep;
        arrived
    }

    fn tick_printer(&mut self, env: &mut Env) {
        self.block_skipped = false;
        let blueprint = self.inventory.get(SLOT_BLUEPRINT).cloned();

        if blueprint.is_none() && self.status != Status::Idle && self.inventory.is_empty(SLOT_BLUEPRINT_OUT) {
            self.state = State::Stopped;
            self.status = Status::Idle;
            self.send_update = true;
            return;
        }

        if self.state == State::Stopped {
            if env.printer.is_loaded() {
                self.reset_printer(env.printer);
            }
            return;
        }

        if self.state == State::Paused && !self.position_not_loaded && self.missing_item.is_none() && self.remaining_fuel > 0 {
            return;
        }

        if !env.printer.is_loaded() {
            if let Some(bp) = blueprint {
                self.initialize_printer(&bp, env);
            }
            return;
        }

        if self.printer_cooldown > 0 {
            self.printer_cooldown -= 1;
            return;
        }

        if self.remaining_fuel <= 0 && !self.has_creative_crate {
            self.refill_fuel_if_possible(env);
            if self.remaining_fuel <= 0 {
                self.state = State::Paused;
                self.status = Status::NoGunpowder;
                self.send_update = true;
                return;
            }
        }

        if self.has_creative_crate {
            self.remaining_fuel = 0;
            if self.missing_item.is_some() {
                self.missing_item = None;
                self.state = State::Running;
            }
        }

        if self.missing_item.is_none() && !self.position_not_loaded {
            if !env.printer.advance_current_pos() {
                self.finished_printing(env);
                return;
            }
            self.send_update = true;
        }

        if !env.world.is_loaded(env.printer.current_target()) {
            self.position_not_loaded = true;
            self.status = Status::TargetNotLoaded;
            self.state = State::Paused;
            return;
        } else if self.position_not_loaded {
            self.position_not_loaded = false;
            self.state = State::Running;
        }

        let requirement = env.printer.current_requirement();
        let rules = self.rules();
        let place_ok = match env.printer.current_place_decision() {
            PlaceDecision::Vetoed => false,
            PlaceDecision::Entity => true,
            PlaceDecision::Block(q) => rules.should_place(&q),
        };
        if requirement.is_invalid() || !place_ok {
            self.send_update = self.status != Status::Searching;
            self.status = Status::Searching;
            self.block_skipped = true;
            return;
        }

        let required: Vec<StackRequirement> = requirement.items().to_vec();
        if !requirement.is_empty() {
            for r in &required {
                if !self.grab_items(r, true, env.items) {
                    if self.skip_missing {
                        self.status = Status::Skipping;
                        self.block_skipped = true;
                        if self.missing_item.is_some() {
                            self.missing_item = None;
                            self.state = State::Running;
                        }
                        return;
                    }
                    self.missing_item = Some(r.clone());
                    self.state = State::Paused;
                    self.status = Status::MissingBlock;
                    return;
                }
            }
            for r in &required {
                self.grab_items(r, false, env.items);
            }
        }

        self.state = State::Running;
        let icon = if requirement.is_empty() || required.is_empty() { None } else { Some(required[0].item.clone()) };
        let action = env.printer.current_action();
        self.launch(action, icon);

        self.printer_cooldown = env.config.delay;
        self.remaining_fuel -= 1;
        self.send_update = true;
        self.missing_item = None;
    }

    fn launch(&mut self, action: LaunchAction, icon: Option<String>) {
        match &action {
            LaunchAction::Block { air, .. } => {
                self.status = if *air { Status::Clearing } else { Status::Placing };
                if !*air {
                    self.blocks_placed += 1;
                }
            }
            LaunchAction::Belt { .. } => {
                self.status = Status::Placing;
                self.blocks_placed += 1;
            }
            LaunchAction::Entity { .. } => {
                self.status = Status::Placing;
                self.blocks_placed += 1;
            }
        }
        let ticks = ticks_for_distance(self.pos, action.target());
        self.flying.push(FlyingItem { action, icon, total_ticks: ticks, ticks_remaining: ticks });
        self.events.push(CannonEvent::FiringSound);
    }

    /// `grabItemsFromAttachedInventories`. The real pass asks every handler for
    /// the full amount (as the original does), so it can take more than needed
    /// when several handlers hold the item.
    fn grab_items(&mut self, req: &StackRequirement, simulate: bool, items: &mut dyn ItemSource) -> bool {
        if self.has_creative_crate {
            return true;
        }
        if req.usage == UseKind::Damage {
            return items.damage_item(req, simulate);
        }
        let mut success = false;
        let mut found = 0;
        for h in 0..items.handler_count() {
            found += items.extract(h, req, req.count, true);
            if found >= req.count {
                success = true;
                break;
            }
        }
        if !simulate && success {
            found = 0;
            for h in 0..items.handler_count() {
                found += items.extract(h, req, req.count, false);
                if found >= req.count {
                    break;
                }
            }
        }
        success
    }

    /// `refillFuelIfPossible`.
    pub fn refill_fuel_if_possible(&mut self, env: &mut Env) {
        if self.has_creative_crate {
            return;
        }
        let shots = self.shots_per_gunpowder(&env.config);
        if self.remaining_fuel > shots {
            self.remaining_fuel = shots;
            self.send_update = true;
            return;
        }
        if self.remaining_fuel > 0 {
            return;
        }
        if !self.inventory.is_empty(SLOT_GUNPOWDER) {
            self.inventory.shrink(SLOT_GUNPOWDER, 1);
        } else if !env.items.take_gunpowder() {
            return;
        }
        self.remaining_fuel += shots;
        if self.status == Status::NoGunpowder {
            if self.blocks_placed > 0 {
                self.state = State::Running;
            }
            self.status = Status::Ready;
        }
        self.send_update = true;
    }

    fn initialize_printer(&mut self, blueprint: &Stack, env: &mut Env) {
        let data = blueprint.schematic.as_ref();
        if data.is_none_or(|d| d.anchor.is_none()) {
            self.state = State::Stopped;
            self.status = Status::SchematicInvalid;
            self.send_update = true;
            return;
        }
        if !data.is_some_and(|d| d.deployed) {
            self.state = State::Stopped;
            self.status = Status::SchematicNotPlaced;
            self.send_update = true;
            return;
        }
        env.printer.load(blueprint);
        if env.printer.is_errored() {
            self.discard_blueprint(Status::SchematicErrored, env.printer);
            return;
        }
        if env.printer.is_world_empty() {
            self.discard_blueprint(Status::SchematicExpired, env.printer);
            return;
        }
        let anchor = env.printer.anchor().unwrap_or([0; 3]);
        let d: i64 = (0..3).map(|i| {
            let v = (anchor[i] - self.pos[i]) as i64;
            v * v
        }).sum();
        if (d as f64) >= MAX_ANCHOR_DISTANCE * MAX_ANCHOR_DISTANCE {
            self.state = State::Stopped;
            self.status = Status::TargetOutsideRange;
            env.printer.reset_schematic();
            self.send_update = true;
            return;
        }
        self.state = State::Paused;
        self.status = Status::Ready;
        self.update_checklist(env);
        self.send_update = true;
        self.blocks_to_place += self.blocks_placed;
    }

    fn discard_blueprint(&mut self, status: Status, printer: &mut dyn PrinterBackend) {
        self.state = State::Stopped;
        self.status = status;
        self.inventory.set(SLOT_BLUEPRINT, None);
        self.inventory.set(SLOT_BLUEPRINT_OUT, Some(Stack::new(EMPTY_SCHEMATIC_ITEM, 1)));
        printer.reset_schematic();
        self.send_update = true;
    }

    /// `finishedPrinting`.
    pub fn finished_printing(&mut self, env: &mut Env) {
        if self.replace_mode == REPLACE_EMPTY {
            env.printer.send_block_updates();
        }
        self.inventory.set(SLOT_BLUEPRINT, None);
        let n = self.inventory.count(SLOT_BLUEPRINT_OUT) + 1;
        self.inventory.set(SLOT_BLUEPRINT_OUT, Some(Stack::new(EMPTY_SCHEMATIC_ITEM, n)));
        self.state = State::Stopped;
        self.status = Status::Finished;
        self.reset_printer(env.printer);
        self.events.push(CannonEvent::FinishSound);
        self.send_update = true;
    }

    /// `updateChecklist`.
    pub fn update_checklist(&mut self, env: &mut Env) {
        self.checklist.clear_requirements();
        if env.printer.is_loaded() && !env.printer.is_errored() {
            self.blocks_to_place = self.blocks_placed;
            let rules = self.rules();
            self.blocks_to_place += env.printer.mark_all_block_requirements(&mut self.checklist, &|q| rules.should_place(q));
            env.printer.mark_all_entity_requirements(&mut self.checklist);
        }
        self.checklist.gathered.clear();
        self.find_inventories(env.items);
        let checklist = &mut self.checklist;
        env.items.visit_stacks(&mut |item, count| checklist.collect(item, count));
        self.send_update = true;
    }

    /// `tickPaperPrinter`: prints the material list into slot 3, one page per ~20 ticks.
    fn tick_paper_printer(&mut self, env: &mut Env) {
        let blueprint = self.inventory.get(SLOT_BLUEPRINT).cloned();
        let paper_empty = self.inventory.is_empty(SLOT_BOOK_IN);
        let output_full = self.inventory.count(SLOT_BOOK_OUT) == SLOT_LIMIT;

        if env.printer.is_errored() {
            return;
        }
        if !env.printer.is_loaded() {
            if let Some(bp) = blueprint {
                self.initialize_printer(&bp, env);
            }
            return;
        }
        if paper_empty || output_full {
            if self.book_printing_progress != 0.0 {
                self.send_update = true;
            }
            self.book_printing_progress = 0.0;
            self.dont_update_checklist = false;
            return;
        }
        if self.book_printing_progress >= 1.0 {
            self.book_printing_progress = 0.0;
            if !self.dont_update_checklist {
                self.update_checklist(env);
            }
            self.dont_update_checklist = true;
            let clipboard = self.inventory.get(SLOT_BOOK_IN).is_some_and(|s| s.item == CLIPBOARD);
            self.inventory.shrink(SLOT_BOOK_IN, 1);
            let item = if clipboard { CLIPBOARD } else { WRITTEN_BOOK };
            let n = self.inventory.count(SLOT_BOOK_OUT) + 1;
            self.inventory.set(SLOT_BOOK_OUT, Some(Stack::new(item, n)));
            self.events.push(CannonEvent::ChecklistPrinted { clipboard });
            self.send_update = true;
            return;
        }
        self.book_printing_progress += PAPER_PROGRESS_STEP;
        self.send_update = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schematic_printer::{Bounds, PrintCursor};
    use std::collections::{BTreeMap, BTreeSet, HashMap};

    #[derive(Default)]
    struct Mock {
        loaded: bool,
        errored: bool,
        empty: bool,
        anchor: Pos,
        size: Pos,
        cursor: Option<PrintCursor>,
        reqs: HashMap<Pos, ItemRequirement>,
        decisions: HashMap<Pos, PlaceDecision>,
        actions: HashMap<Pos, LaunchAction>,
        resets: i32,
        updates_sent: i32,
        loads: i32,
    }

    impl Mock {
        fn new(size: Pos, anchor: Pos) -> Self {
            Self { anchor, size, ..Default::default() }
        }
        fn rel(&self) -> Pos {
            self.cursor.as_ref().unwrap().current_pos
        }
        fn world(&self) -> Pos {
            let r = self.rel();
            [self.anchor[0] + r[0], self.anchor[1] + r[1], self.anchor[2] + r[2]]
        }
        fn query(pos: Pos) -> PlaceQuery {
            PlaceQuery {
                pos,
                placing_air: false,
                structure_void: false,
                requirement_empty: false,
                requirement_invalid: false,
                double_block_upper: false,
                bed_head: false,
                piston_head: false,
                belt_middle: false,
                to_replace: ReplaceInfo { has_block_entity: false, redstone_conductor: false },
                to_replace_other: None,
                is_normal_cube: false,
            }
        }
    }

    impl PrinterBackend for Mock {
        fn is_loaded(&self) -> bool {
            self.loaded
        }
        fn is_errored(&self) -> bool {
            self.errored
        }
        fn load(&mut self, _b: &Stack) {
            self.loads += 1;
            self.loaded = true;
            self.cursor = Some(PrintCursor::new(Bounds::from_size(self.size), vec![]));
        }
        fn is_world_empty(&self) -> bool {
            self.empty
        }
        fn anchor(&self) -> Option<Pos> {
            Some(self.anchor)
        }
        fn reset_schematic(&mut self) {
            self.loaded = false;
            self.cursor = None;
            self.resets += 1;
        }
        fn advance_current_pos(&mut self) -> bool {
            self.cursor.as_mut().unwrap().advance(&|_| false)
        }
        fn current_target(&self) -> Pos {
            self.world()
        }
        fn current_requirement(&mut self) -> ItemRequirement {
            self.reqs
                .get(&self.world())
                .cloned()
                .unwrap_or_else(|| ItemRequirement::Items(vec![StackRequirement::consume("minecraft:stone", 1)]))
        }
        fn current_place_decision(&mut self) -> PlaceDecision {
            let w = self.world();
            self.decisions.get(&w).cloned().unwrap_or_else(|| PlaceDecision::Block(Mock::query(w)))
        }
        fn current_action(&mut self) -> LaunchAction {
            let w = self.world();
            self.actions.get(&w).cloned().unwrap_or(LaunchAction::Block { pos: w, air: false })
        }
        fn mark_all_block_requirements(&mut self, c: &mut MaterialChecklist, sp: &dyn Fn(&PlaceQuery) -> bool) -> i32 {
            let mut n = 0;
            let b = Bounds::from_size(self.size);
            for y in 0..self.size[1] {
                for z in 0..self.size[2] {
                    for x in 0..self.size[0] {
                        assert!(b.contains([x, y, z]));
                        let w = [self.anchor[0] + x, self.anchor[1] + y, self.anchor[2] + z];
                        if !sp(&Mock::query(w)) {
                            continue;
                        }
                        let r = self.reqs.get(&w).cloned().unwrap_or_else(|| {
                            ItemRequirement::Items(vec![StackRequirement::consume("minecraft:stone", 1)])
                        });
                        if r.is_empty() || r.is_invalid() {
                            continue;
                        }
                        c.require(&r);
                        n += 1;
                    }
                }
            }
            n
        }
        fn mark_all_entity_requirements(&mut self, _c: &mut MaterialChecklist) {}
        fn send_block_updates(&mut self) {
            self.updates_sent += 1;
        }
    }

    #[derive(Default)]
    struct Items {
        handlers: Vec<BTreeMap<String, i32>>,
        creative: bool,
        damage_items: i32,
        damaged: i32,
        rescans: i32,
    }
    impl Items {
        fn with_stone(n: i32) -> Self {
            let mut m = BTreeMap::new();
            m.insert("minecraft:stone".to_string(), n);
            Self { handlers: vec![m], ..Default::default() }
        }
    }
    impl ItemSource for Items {
        fn rescan(&mut self) {
            self.rescans += 1;
        }
        fn has_creative_crate(&self) -> bool {
            self.creative
        }
        fn handler_count(&self) -> usize {
            self.handlers.len()
        }
        fn extract(&mut self, h: usize, req: &StackRequirement, max: i32, simulate: bool) -> i32 {
            let have = self.handlers[h].get(&req.item).copied().unwrap_or(0);
            let n = have.min(max);
            if !simulate && n > 0 {
                *self.handlers[h].get_mut(&req.item).unwrap() -= n;
            }
            n
        }
        fn damage_item(&mut self, _r: &StackRequirement, simulate: bool) -> bool {
            if self.damage_items > 0 {
                if !simulate {
                    self.damaged += 1;
                }
                true
            } else {
                false
            }
        }
        fn take_gunpowder(&mut self) -> bool {
            for h in self.handlers.iter_mut() {
                if let Some(n) = h.get_mut(GUNPOWDER) {
                    if *n > 0 {
                        *n -= 1;
                        return true;
                    }
                }
            }
            false
        }
        fn visit_stacks(&self, f: &mut dyn FnMut(&str, i32)) {
            for h in &self.handlers {
                for (k, v) in h {
                    f(k, *v);
                }
            }
        }
    }

    struct World {
        unloaded: BTreeSet<Pos>,
    }
    impl TargetWorld for World {
        fn is_loaded(&self, p: Pos) -> bool {
            !self.unloaded.contains(&p)
        }
    }

    fn deployed_blueprint() -> Stack {
        let mut d = SchematicStackData::create("p", "a.nbt", [2, 1, 1]);
        d.deployed = true;
        d.anchor = Some([10, 0, 0]);
        Stack::schematic(d)
    }

    fn tick(c: &mut Schematicannon, m: &mut Mock, i: &mut Items, w: &World) -> Vec<FlyingItem> {
        let mut env = Env { printer: m, items: i, world: w, config: CannonConfig::default(), server: true };
        c.tick(&mut env)
    }

    fn setup() -> (Schematicannon, Mock, Items, World) {
        let mut c = Schematicannon::new([0, 0, 0]);
        c.inventory.set(SLOT_BLUEPRINT, Some(deployed_blueprint()));
        c.inventory.set(SLOT_GUNPOWDER, Some(Stack::new(GUNPOWDER, 2)));
        (c, Mock::new([2, 1, 1], [10, 0, 0]), Items::with_stone(64), World { unloaded: BTreeSet::new() })
    }

    fn start(c: &mut Schematicannon) {
        c.apply_option(ConfigOption::Play, true);
    }

    #[test]
    fn inventory_slot_rules() {
        assert!(CannonInventory::is_item_valid(0, SCHEMATIC_ITEM));
        assert!(!CannonInventory::is_item_valid(0, GUNPOWDER));
        assert!(!CannonInventory::is_item_valid(1, EMPTY_SCHEMATIC_ITEM));
        assert!(CannonInventory::is_item_valid(2, CLIPBOARD));
        assert!(CannonInventory::is_item_valid(2, BOOK));
        assert!(CannonInventory::is_item_valid(2, WRITTEN_BOOK));
        assert!(!CannonInventory::is_item_valid(3, WRITTEN_BOOK));
        assert!(CannonInventory::is_item_valid(4, GUNPOWDER));
        assert!(!CannonInventory::is_item_valid(5, GUNPOWDER));
        assert_eq!(CannonInventory::quick_move_target(GUNPOWDER), Some(4));
        assert_eq!(CannonInventory::quick_move_target(BOOK), Some(2));
        assert_eq!(CannonInventory::quick_move_target(SCHEMATIC_ITEM), Some(0));
        assert_eq!(CannonInventory::quick_move_target("minecraft:stone"), None);
    }

    #[test]
    fn inventory_insert_respects_filter_and_limit() {
        let mut inv = CannonInventory::default();
        assert_eq!(inv.insert(4, Stack::new(GUNPOWDER, 50)), 0);
        assert_eq!(inv.insert(4, Stack::new(GUNPOWDER, 30)), 16);
        assert_eq!(inv.count(4), 64);
        assert_eq!(inv.insert(1, Stack::new(EMPTY_SCHEMATIC_ITEM, 1)), 1);
        inv.shrink(4, 64);
        assert!(inv.is_empty(4));
    }

    #[test]
    fn defaults_match_constructor() {
        let c = Schematicannon::new([1, 2, 3]);
        assert_eq!((c.state, c.status), (State::Stopped, Status::Idle));
        assert_eq!((c.replace_mode, c.skip_missing, c.replace_block_entities), (2, false, false));
        let cfg = CannonConfig::default();
        assert_eq!((cfg.delay, cfg.shots_per_gunpowder), (10, 400));
    }

    #[test]
    fn status_keys_and_lang() {
        assert_eq!(Status::TargetOutsideRange.key(), "targetOutsideRange");
        assert_eq!(Status::NoGunpowder.lang_key(), "schematicannon.status.noGunpowder");
        assert_eq!(Status::Running.key(), "running");
    }

    #[test]
    fn option_packet_semantics() {
        let mut c = Schematicannon::new([0; 3]);
        c.apply_option(ConfigOption::DontReplace, true);
        assert_eq!(c.replace_mode, 0);
        c.apply_option(ConfigOption::ReplaceSolid, false);
        assert_eq!(c.replace_mode, 1);
        c.apply_option(ConfigOption::ReplaceEmpty, false);
        assert_eq!(c.replace_mode, 3);
        c.apply_option(ConfigOption::ReplaceAny, false);
        assert_eq!(c.replace_mode, 2);
        c.apply_option(ConfigOption::SkipMissing, true);
        c.apply_option(ConfigOption::SkipBlockEntities, true);
        assert!(c.skip_missing && c.replace_block_entities);
        c.apply_option(ConfigOption::Play, false);
        assert_eq!((c.state, c.status), (State::Running, Status::Running));
        c.apply_option(ConfigOption::Pause, false);
        assert_eq!((c.state, c.status), (State::Paused, Status::Paused));
        c.apply_option(ConfigOption::Stop, false);
        assert_eq!((c.state, c.status), (State::Stopped, Status::Stopped));
        assert!(c.send_update);
    }

    #[test]
    fn flight_time_formula() {
        assert_eq!(ticks_for_distance([0; 3], [0; 3]), 10);
        assert_eq!(ticks_for_distance([0; 3], [10, 0, 0]), 12); // 100^(1/4)=3.162 -> 12.6 -> 12
        assert_eq!(ticks_for_distance([0; 3], [16, 0, 0]), 16); // 256^(1/4)=4 -> 16
        assert_eq!(ticks_for_distance([0; 3], [0, 0, 256]), 64); // 65536^(1/4)=16
        assert_eq!(ticks_for_distance([0; 3], [1, 1, 1]), 10);
        assert_eq!(ticks_for_distance([5, 5, 5], [5, 5, 5 + 81]), 36); // 81^2^(1/4)=sqrt(81)=9
    }

    #[test]
    fn flying_items_land_one_tick_after_countdown_and_only_on_server() {
        let mut c = Schematicannon::new([0; 3]);
        c.flying.push(FlyingItem { action: LaunchAction::Block { pos: [1, 1, 1], air: false }, icon: None, total_ticks: 2, ticks_remaining: 2 });
        assert!(c.tick_flying(true).is_empty());
        assert!(c.tick_flying(true).is_empty());
        assert_eq!(c.tick_flying(true).len(), 1);
        assert!(c.flying.is_empty());
        c.flying.push(FlyingItem { action: LaunchAction::Entity { pos: [0; 3] }, icon: None, total_ticks: 0, ticks_remaining: 0 });
        assert!(c.tick_flying(false).is_empty());
        assert_eq!(c.flying.len(), 1);
    }

    fn query() -> PlaceQuery {
        Mock::query([10, 0, 0])
    }
    fn rules(mode: u8, be: bool) -> PlaceRules {
        PlaceRules { cannon_pos: [0; 3], replace_mode: mode, replace_block_entities: be }
    }

    #[test]
    fn place_rules_distance() {
        let mut q = query();
        q.pos = [1, 1, 0]; // distSqr 2 < 4
        assert!(!rules(2, true).should_place(&q));
        q.pos = [2, 0, 0]; // distSqr 4, not closer
        assert!(rules(2, true).should_place(&q));
    }

    #[test]
    fn place_rules_modes() {
        let mut q = query();
        // air target
        q.placing_air = true;
        for m in 0..3 {
            assert!(!rules(m, true).should_place(&q), "mode {m}");
        }
        assert!(rules(3, true).should_place(&q));
        q.placing_air = false;
        // conductor in the world
        q.to_replace.redstone_conductor = true;
        assert!(!rules(0, true).should_place(&q));
        assert!(!rules(1, true).should_place(&q));
        assert!(rules(2, true).should_place(&q));
        // solid mode also replaces when the schematic block is itself a normal cube
        q.is_normal_cube = true;
        assert!(rules(1, true).should_place(&q));
        assert!(!rules(0, true).should_place(&q));
        // non conductor
        q.to_replace.redstone_conductor = false;
        q.is_normal_cube = false;
        assert!(rules(0, true).should_place(&q));
        assert!(rules(1, true).should_place(&q));
        // the second replaced block counts too
        q.to_replace_other = Some(ReplaceInfo { has_block_entity: false, redstone_conductor: true });
        assert!(!rules(0, true).should_place(&q));
        assert!(!rules(1, true).should_place(&q));
    }

    #[test]
    fn place_rules_block_entities() {
        let mut q = query();
        q.to_replace.has_block_entity = true;
        assert!(!rules(2, false).should_place(&q));
        assert!(rules(2, true).should_place(&q));
        q.to_replace.has_block_entity = false;
        q.to_replace_other = Some(ReplaceInfo { has_block_entity: true, redstone_conductor: false });
        assert!(!rules(3, false).should_place(&q));
    }

    #[test]
    fn ignored_blocks() {
        for f in [
            |q: &mut PlaceQuery| q.structure_void = true,
            |q: &mut PlaceQuery| q.double_block_upper = true,
            |q: &mut PlaceQuery| q.bed_head = true,
            |q: &mut PlaceQuery| q.piston_head = true,
            |q: &mut PlaceQuery| q.belt_middle = true,
        ] {
            let mut q = query();
            f(&mut q);
            assert!(should_ignore_block_state(&q));
            assert!(!rules(3, true).should_place(&q));
        }
        // empty / invalid requirements are never ignored (except structure void)
        let mut q = query();
        q.double_block_upper = true;
        q.requirement_empty = true;
        assert!(!should_ignore_block_state(&q));
        q.requirement_empty = false;
        q.requirement_invalid = true;
        assert!(!should_ignore_block_state(&q));
        q.structure_void = true;
        assert!(should_ignore_block_state(&q));
    }

    #[test]
    fn idle_cannon_without_blueprint() {
        let mut c = Schematicannon::new([0; 3]);
        c.status = Status::Ready;
        c.state = State::Running;
        let (mut m, mut i, w) = (Mock::new([1, 1, 1], [10, 0, 0]), Items::default(), World { unloaded: BTreeSet::new() });
        tick(&mut c, &mut m, &mut i, &w);
        assert_eq!((c.state, c.status), (State::Stopped, Status::Idle));
    }

    #[test]
    fn invalid_and_undeployed_blueprints() {
        let (mut c, mut m, mut i, w) = setup();
        start(&mut c);
        let mut bp = deployed_blueprint();
        bp.schematic.as_mut().unwrap().anchor = None;
        c.inventory.set(0, Some(bp));
        tick(&mut c, &mut m, &mut i, &w);
        assert_eq!((c.state, c.status), (State::Stopped, Status::SchematicInvalid));
        let mut bp = deployed_blueprint();
        bp.schematic.as_mut().unwrap().deployed = false;
        c.inventory.set(0, Some(bp));
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        assert_eq!(c.status, Status::SchematicNotPlaced);
        assert_eq!(m.loads, 0);
    }

    #[test]
    fn errored_and_expired_schematics_are_returned_as_empty_schematic() {
        for (errored, empty, status) in [(true, false, Status::SchematicErrored), (false, true, Status::SchematicExpired)] {
            let (mut c, mut m, mut i, w) = setup();
            m.errored = errored;
            m.empty = empty;
            start(&mut c);
            tick(&mut c, &mut m, &mut i, &w);
            assert_eq!((c.state, c.status), (State::Stopped, status));
            assert!(c.inventory.is_empty(0));
            assert_eq!(c.inventory.get(1).unwrap().item, EMPTY_SCHEMATIC_ITEM);
            assert_eq!(c.inventory.count(1), 1);
            assert!(!m.loaded);
        }
    }

    #[test]
    fn anchor_beyond_256_blocks_is_rejected() {
        let (mut c, _, mut i, w) = setup();
        let mut m = Mock::new([2, 1, 1], [256, 0, 0]);
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        assert_eq!((c.state, c.status), (State::Stopped, Status::TargetOutsideRange));
        assert!(!m.loaded);
        // 255 is fine
        let (mut c, mut m, mut i, w) = setup();
        m.anchor = [255, 0, 0];
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        assert_eq!(c.status, Status::Ready);
    }

    #[test]
    fn initialization_pauses_with_ready_and_counts_blocks() {
        let (mut c, mut m, mut i, w) = setup();
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        assert_eq!((c.state, c.status), (State::Paused, Status::Ready));
        assert_eq!(c.blocks_to_place, 2);
        assert_eq!(c.checklist.required["minecraft:stone"], 2);
        assert_eq!(c.checklist.gathered["minecraft:stone"], 64);
        assert!(i.rescans >= 1);
    }

    #[test]
    fn paused_with_fuel_waits_for_play() {
        let (mut c, mut m, mut i, w) = setup();
        c.remaining_fuel = 5;
        // initialise (Paused after init)
        c.state = State::Running;
        tick(&mut c, &mut m, &mut i, &w);
        assert_eq!(c.state, State::Paused);
        for _ in 0..30 {
            tick(&mut c, &mut m, &mut i, &w);
        }
        assert_eq!(c.blocks_placed, 0);
        assert!(c.flying.is_empty());
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        assert_eq!(c.blocks_placed, 1);
    }

    #[test]
    fn first_shot_consumes_gunpowder_item_and_sets_shots() {
        let (mut c, mut m, mut i, w) = setup();
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w); // init -> paused
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w); // first shot
        assert_eq!(c.inventory.count(4), 1);
        assert_eq!(c.remaining_fuel, 400 - 1);
        assert_eq!(c.blocks_placed, 1);
        assert_eq!(c.status, Status::Placing);
        assert_eq!(c.flying.len(), 1);
        assert_eq!(c.flying[0].action, LaunchAction::Block { pos: [10, 0, 0], air: false });
        assert_eq!(c.flying[0].icon.as_deref(), Some("minecraft:stone"));
        assert_eq!(c.flying[0].total_ticks, ticks_for_distance([0; 3], [10, 0, 0]));
        assert_eq!(c.events, vec![CannonEvent::FiringSound]);
        assert_eq!(i.handlers[0]["minecraft:stone"], 63);
    }

    #[test]
    fn cooldown_between_shots_is_config_delay() {
        let (mut c, mut m, mut i, w) = setup();
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        let mut placed_at = vec![0];
        for t in 1..=30 {
            tick(&mut c, &mut m, &mut i, &w);
            if c.blocks_placed as usize > placed_at.len() {
                placed_at.push(t);
            }
        }
        // 10 cooldown ticks then the 11th tick shoots (second block), then finished
        assert_eq!(placed_at, vec![0, 11]);
    }

    #[test]
    fn finishing_returns_empty_schematic_and_resets() {
        let (mut c, mut m, mut i, w) = setup();
        c.inventory.set(1, Some(Stack::new(EMPTY_SCHEMATIC_ITEM, 3)));
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        start(&mut c);
        for _ in 0..40 {
            tick(&mut c, &mut m, &mut i, &w);
        }
        assert_eq!((c.state, c.status), (State::Stopped, Status::Finished));
        assert!(c.inventory.is_empty(0));
        assert_eq!(c.inventory.count(1), 4);
        assert_eq!((c.blocks_placed, c.blocks_to_place), (0, 0));
        assert!(c.events.contains(&CannonEvent::FinishSound));
        assert_eq!(m.updates_sent, 0);
    }

    #[test]
    fn replace_empty_mode_sends_water_updates_when_finished() {
        let (mut c, mut m, mut i, w) = setup();
        c.apply_option(ConfigOption::ReplaceEmpty, true);
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        start(&mut c);
        for _ in 0..40 {
            tick(&mut c, &mut m, &mut i, &w);
        }
        assert_eq!(c.status, Status::Finished);
        assert_eq!(m.updates_sent, 1);
    }

    #[test]
    fn no_gunpowder_pauses_then_refills_from_external_inventory() {
        let (mut c, mut m, mut i, w) = setup();
        c.inventory.set(4, None);
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        assert_eq!((c.state, c.status), (State::Paused, Status::NoGunpowder));
        assert_eq!(c.blocks_placed, 0);
        i.handlers[0].insert(GUNPOWDER.into(), 1);
        tick(&mut c, &mut m, &mut i, &w);
        // refill happens in the fuel step of the tick (status back to ready), the
        // cannon stays paused until play is pressed (blocks_placed == 0)
        assert_eq!(c.status, Status::Ready);
        assert_eq!(c.remaining_fuel, 400);
        assert_eq!(i.handlers[0][GUNPOWDER], 0);
        assert_eq!(c.state, State::Paused);
    }

    #[test]
    fn refill_returns_to_running_after_blocks_were_placed() {
        let (mut c, mut m, mut i, w) = setup();
        let cfg = CannonConfig::default();
        c.status = Status::NoGunpowder;
        c.state = State::Paused;
        c.blocks_placed = 3;
        let mut env = Env { printer: &mut m, items: &mut i, world: &w, config: cfg, server: true };
        c.refill_fuel_if_possible(&mut env);
        assert_eq!((c.state, c.status), (State::Running, Status::Ready));
        assert_eq!(c.inventory.count(4), 1);
    }

    #[test]
    fn fuel_is_capped_to_config_and_zero_with_creative_crate() {
        let (mut c, mut m, mut i, w) = setup();
        let mut env = Env { printer: &mut m, items: &mut i, world: &w, config: CannonConfig { delay: 10, shots_per_gunpowder: 5 }, server: true };
        c.remaining_fuel = 99;
        c.refill_fuel_if_possible(&mut env);
        assert_eq!(c.remaining_fuel, 5);
        c.remaining_fuel = 2;
        c.refill_fuel_if_possible(&mut env); // still has fuel: nothing consumed
        assert_eq!(c.inventory.count(4), 2);
        c.has_creative_crate = true;
        assert_eq!(c.shots_per_gunpowder(&env.config), 0);
        c.remaining_fuel = 0;
        c.refill_fuel_if_possible(&mut env);
        assert_eq!(c.inventory.count(4), 2);
    }

    #[test]
    fn missing_item_pauses_then_resumes_when_supplied() {
        let (mut c, mut m, mut i, w) = setup();
        i.handlers[0].insert("minecraft:stone".into(), 0);
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        assert_eq!((c.state, c.status), (State::Paused, Status::MissingBlock));
        assert_eq!(c.missing_item.as_ref().unwrap().item, "minecraft:stone");
        // paused but missing item: keeps retrying every tick without advancing
        let target_before = m.world();
        tick(&mut c, &mut m, &mut i, &w);
        assert_eq!(m.world(), target_before);
        i.handlers[0].insert("minecraft:stone".into(), 5);
        tick(&mut c, &mut m, &mut i, &w);
        assert_eq!(c.blocks_placed, 1);
        assert!(c.missing_item.is_none());
        assert_eq!(c.state, State::Running);
    }

    #[test]
    fn skip_missing_skips_without_shooting_and_ends_finished() {
        let (mut c, mut m, mut i, w) = setup();
        i.handlers[0].insert("minecraft:stone".into(), 0);
        c.apply_option(ConfigOption::SkipMissing, true);
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        // all 2 targets skipped within the single tick via the 1000 skip loop
        assert_eq!((c.state, c.status), (State::Stopped, Status::Finished));
        assert_eq!(c.blocks_placed, 0);
        assert!(c.flying.is_empty());
    }

    #[test]
    fn skip_missing_clears_previous_missing_item() {
        let (mut c, mut m, mut i, w) = setup();
        i.handlers[0].insert("minecraft:stone".into(), 0);
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        assert!(c.missing_item.is_some());
        c.apply_option(ConfigOption::SkipMissing, true);
        tick(&mut c, &mut m, &mut i, &w);
        assert!(c.missing_item.is_none());
    }

    #[test]
    fn unplaceable_targets_are_searched_in_one_tick() {
        let (mut c, mut m, mut i, w) = setup();
        m.decisions.insert([10, 0, 0], PlaceDecision::Vetoed);
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        // first target skipped (searching), second placed in the same tick
        assert_eq!(c.blocks_placed, 1);
        assert_eq!(c.flying[0].action.target(), [11, 0, 0]);
    }

    #[test]
    fn invalid_requirement_is_skipped_and_status_searching() {
        let (mut c, mut m, mut i, w) = setup();
        m.reqs.insert([10, 0, 0], ItemRequirement::Invalid);
        m.reqs.insert([11, 0, 0], ItemRequirement::Invalid);
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        assert_eq!(c.status, Status::Finished);
        assert_eq!(c.blocks_placed, 0);
    }

    #[test]
    fn empty_requirement_places_without_consuming() {
        let (mut c, mut m, mut i, w) = setup();
        m.reqs.insert([10, 0, 0], ItemRequirement::Empty);
        m.actions.insert([10, 0, 0], LaunchAction::Block { pos: [10, 0, 0], air: true });
        i.handlers[0].insert("minecraft:stone".into(), 0);
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        // air target: status clearing, not counted as placed, no item needed
        assert_eq!(c.status, Status::Clearing);
        assert_eq!(c.blocks_placed, 0);
        assert!(c.flying[0].icon.is_none());
        assert_eq!(c.remaining_fuel, 399);
    }

    #[test]
    fn target_not_loaded_pauses_and_resumes() {
        let (mut c, mut m, mut i, mut w) = setup();
        w.unloaded.insert([10, 0, 0]);
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        assert_eq!((c.state, c.status), (State::Paused, Status::TargetNotLoaded));
        assert!(c.position_not_loaded);
        for _ in 0..5 {
            tick(&mut c, &mut m, &mut i, &w);
        }
        assert_eq!(c.blocks_placed, 0);
        w.unloaded.clear();
        // paused + positionNotLoaded still runs the printer
        tick(&mut c, &mut m, &mut i, &w);
        assert!(!c.position_not_loaded);
        assert_eq!(c.blocks_placed, 1);
        assert_eq!(c.state, State::Running);
    }

    #[test]
    fn creative_crate_needs_no_items_or_fuel() {
        let (mut c, mut m, mut i, w) = setup();
        c.inventory.set(4, None);
        i.handlers[0].clear();
        i.creative = true;
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        assert!(c.has_creative_crate);
        assert_eq!(c.blocks_placed, 1);
        assert_eq!(c.remaining_fuel, -1);
        for _ in 0..40 {
            tick(&mut c, &mut m, &mut i, &w);
        }
        assert_eq!(c.status, Status::Finished);
    }

    #[test]
    fn damage_requirements_use_damage_path() {
        let (mut c, mut m, mut i, w) = setup();
        m.reqs.insert([10, 0, 0], ItemRequirement::Items(vec![StackRequirement::damage("minecraft:shears", 1)]));
        i.damage_items = 1;
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        assert_eq!(i.damaged, 1);
        assert_eq!(c.blocks_placed, 1);
    }

    #[test]
    fn real_extraction_over_takes_when_several_handlers_hold_the_item() {
        // Quirk kept from the original: every handler is asked for the full amount.
        let mut c = Schematicannon::new([0; 3]);
        let mut i = Items { handlers: vec![BTreeMap::new(), BTreeMap::new()], ..Default::default() };
        i.handlers[0].insert("minecraft:stone".into(), 3);
        i.handlers[1].insert("minecraft:stone".into(), 5);
        let r = StackRequirement::consume("minecraft:stone", 5);
        assert!(c.grab_items(&r, true, &mut i));
        assert_eq!(i.handlers[0]["minecraft:stone"], 3);
        assert!(c.grab_items(&r, false, &mut i));
        assert_eq!(i.handlers[0]["minecraft:stone"], 0);
        assert_eq!(i.handlers[1]["minecraft:stone"], 0);
        let r9 = StackRequirement::consume("minecraft:stone", 9);
        assert!(!c.grab_items(&r9, true, &mut i));
    }

    #[test]
    fn skip_loop_is_bounded_per_tick() {
        let (mut c, _, mut i, w) = setup();
        let mut m = Mock::new([30, 30, 30], [10, 0, 0]); // 27000 cells, all vetoed
        for y in 0..30 {
            for z in 0..30 {
                for x in 0..30 {
                    m.decisions.insert([10 + x, y, z], PlaceDecision::Vetoed);
                }
            }
        }
        let mut bp = deployed_blueprint();
        bp.schematic.as_mut().unwrap().anchor = Some([10, 0, 0]);
        c.inventory.set(0, Some(bp));
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w); // init
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        assert_eq!(c.status, Status::Searching);
        assert_ne!(c.status, Status::Finished);
        assert_eq!(c.blocks_placed, 0);
    }

    #[test]
    fn stopped_cannon_resets_loaded_printer() {
        let (mut c, mut m, mut i, w) = setup();
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        assert!(m.loaded);
        c.blocks_placed = 4;
        c.apply_option(ConfigOption::Stop, false);
        tick(&mut c, &mut m, &mut i, &w);
        assert!(!m.loaded);
        assert_eq!((c.blocks_placed, c.blocks_to_place), (0, 0));
    }

    #[test]
    fn inventory_rescan_every_100_ticks_while_active() {
        let (mut c, mut m, mut i, w) = setup();
        start(&mut c);
        for _ in 0..250 {
            tick(&mut c, &mut m, &mut i, &w);
            c.apply_option(ConfigOption::Pause, false);
            c.state = State::Paused;
        }
        // rescans: at tick 1, 102, 203 plus the explicit ones in updateChecklist/init
        assert!(i.rescans >= 3);
        let mut c2 = Schematicannon::new([0; 3]);
        let mut i2 = Items::default();
        let mut m2 = Mock::new([1, 1, 1], [10, 0, 0]);
        for _ in 0..250 {
            tick(&mut c2, &mut m2, &mut i2, &w);
        }
        assert_eq!(i2.rescans, 0);
    }

    #[test]
    fn progress_is_placed_over_total() {
        let (mut c, mut m, mut i, w) = setup();
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        start(&mut c);
        tick(&mut c, &mut m, &mut i, &w);
        assert!((c.schematic_progress - 0.5).abs() < 1e-6);
    }

    fn paper_setup(item: &str) -> (Schematicannon, Mock, Items, World) {
        let (mut c, mut m, i, w) = setup();
        c.inventory.set(SLOT_BOOK_IN, Some(Stack::new(item, 2)));
        m.loaded = true;
        m.cursor = Some(PrintCursor::new(Bounds::from_size([2, 1, 1]), vec![]));
        (c, m, i, w)
    }

    #[test]
    fn paper_printer_takes_21_ticks_per_page() {
        // 20 additions of 0.05f32 do not reach 1.0 exactly in f32, as in Java floats.
        let mut p = 0.0f32;
        let mut n = 0;
        while p < 1.0 {
            p += PAPER_PROGRESS_STEP;
            n += 1;
        }
        let (mut c, mut m, mut i, w) = paper_setup(BOOK);
        c.state = State::Paused;
        c.remaining_fuel = 1;
        let mut done_at = None;
        for t in 1..=40 {
            tick(&mut c, &mut m, &mut i, &w);
            if c.inventory.count(SLOT_BOOK_OUT) > 0 {
                done_at = Some(t);
                break;
            }
        }
        assert_eq!(done_at, Some(n + 1));
        assert_eq!(c.inventory.get(SLOT_BOOK_OUT).unwrap().item, WRITTEN_BOOK);
        assert_eq!(c.inventory.count(SLOT_BOOK_IN), 1);
        assert_eq!(c.events, vec![CannonEvent::ChecklistPrinted { clipboard: false }]);
        assert!(c.dont_update_checklist);
        assert_eq!(c.book_printing_progress, 0.0);
    }

    #[test]
    fn paper_printer_clipboard_and_output_stacking() {
        let (mut c, mut m, mut i, w) = paper_setup(CLIPBOARD);
        c.state = State::Paused;
        c.remaining_fuel = 1;
        for _ in 0..60 {
            tick(&mut c, &mut m, &mut i, &w);
        }
        assert_eq!(c.inventory.get(SLOT_BOOK_OUT).unwrap().item, CLIPBOARD);
        assert_eq!(c.inventory.count(SLOT_BOOK_OUT), 2);
        assert!(c.inventory.is_empty(SLOT_BOOK_IN));
        // no paper left: progress and the checklist flag reset
        assert_eq!(c.book_printing_progress, 0.0);
        assert!(!c.dont_update_checklist);
    }

    #[test]
    fn paper_printer_stops_when_output_full_or_errored() {
        let (mut c, mut m, mut i, w) = paper_setup(BOOK);
        c.state = State::Paused;
        c.remaining_fuel = 1;
        c.inventory.set(SLOT_BOOK_OUT, Some(Stack::new(WRITTEN_BOOK, 64)));
        for _ in 0..30 {
            tick(&mut c, &mut m, &mut i, &w);
        }
        assert_eq!(c.book_printing_progress, 0.0);
        assert_eq!(c.inventory.count(SLOT_BOOK_IN), 2);
        let (mut c, mut m, mut i, w) = paper_setup(BOOK);
        c.state = State::Paused;
        c.remaining_fuel = 1;
        m.errored = true;
        for _ in 0..30 {
            tick(&mut c, &mut m, &mut i, &w);
        }
        assert_eq!(c.book_printing_progress, 0.0);
    }

    #[test]
    fn paper_printer_initialises_printer_when_blueprint_present() {
        let (mut c, mut m, mut i, w) = setup();
        c.inventory.set(SLOT_BOOK_IN, Some(Stack::new(BOOK, 1)));
        c.state = State::Paused;
        tick(&mut c, &mut m, &mut i, &w);
        assert!(m.loaded);
        assert_eq!(c.status, Status::Ready);
    }

    #[test]
    fn client_tick_only_flies() {
        let (mut c, mut m, mut i, w) = setup();
        start(&mut c);
        let mut env = Env { printer: &mut m, items: &mut i, world: &w, config: CannonConfig::default(), server: false };
        c.tick(&mut env);
        assert!(!m.loaded);
    }
}
