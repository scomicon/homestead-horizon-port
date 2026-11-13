// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

// Stock Link (`PackagerLinkBlock` / `LogisticallyLinkedBehaviour`) and the stock
// network built from Stock Links: summaries, ordering, packager requests. The rules
// live in `create_engine::stock_network` (summary, request splitting, promises) and
// `create_engine::packager` (request packing); this file adapts them to the game.
//
// Block `create:stock_link`: state `facing` points away from the Packager the link is
// attached to (`step(pos, facing.opposite())` is the Packager, as for the Factory
// Gauge). The Packager's own `facing` selects the inventory behind it, whose items are
// what the link offers to the network.
//
// Network identity: Create keys a network by a UUID chosen when the first link is
// placed. This port keys it by two frequency items, exactly like the Redstone Link
// (`FrequencyFirst` / `FrequencyLast`, item identity plus dye colour); a link or ticker
// with no frequency items belongs to the "blank" network. The same two keys are on the
// Stock Ticker, whose block id is `TICKER` below (its operator commands live in
// `stock_ticker_runtime.rs`).
//
// Both blocks are non-ticking block entities, so they stay in the world map and the
// network is found by scanning it. The Packager is a ticking entity: it is only out of
// the map while it ticks, never while a command or a click runs.
//
// Differences from Create (docs/stock-network.md): no UUID networks or lock, no
// request promise bookkeeping across orders, a linked Packager still reacts to
// redstone (its `linked` flag is not written), the packages are made at once for the
// whole order and queued on the Packagers.

use super::{Game, create_port_package as package, create_port_packager as pk};
use crate::{
    containers::{BlockEntity, ModBlockEntity},
    items::ItemStack,
};
use create_engine::{
    package::{self as cp, BigStack, ItemHandler, OrderWithCrafts, SimpleInventory, XorShift},
    packager::{self as engine_pk, PackagerState, PackagingRequest, RequestType},
    stock_network::{self as net, FreqId, InventorySummary, StockLink},
};
use horizon_client::text::Text;
use horizon_core::geometry::Direction as Face;
use horizon_items::item_nbt::{item_from_nbt, item_to_nbt};
use horizon_world::nbt::{Compound, Tag};

pub(super) const BLOCK: &str = "create:stock_link";
pub(super) const TICKER: &str = "create:stock_ticker";
type Pos = [i32; 3];

const FIRST: &str = "FrequencyFirst";
const LAST: &str = "FrequencyLast";
/// Which frequency slot the next item click fills (same simplification as the Redstone Link).
const NEXT_SLOT: &str = "NextSlot";
const SYNC_TICK: &str = "create_horizon:PackagerSync";

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

// ----------------------------------------------------------------- block entity

pub(super) fn entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if (block == BLOCK || block == TICKER) && entity.is_none() {
        *entity = Some((block.into(), Compound::new()));
    }
}

// ------------------------------------------------------------------ frequency

fn fnv(text: &str) -> u128 {
    let mut hash: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    for byte in text.bytes() {
        hash ^= u128::from(byte);
        hash = hash.wrapping_mul(0x0000_0000_0100_0000_0000_0000_0000_013B);
    }
    hash
}

fn slot_key(nbt: &Compound, key: &str) -> String {
    get(nbt, key)
        .and_then(item_from_nbt)
        .map_or_else(String::new, |s| format!("{}|{:?}", s.item, s.data().and_then(|d| d.dyed_color)))
}

fn freq_of(nbt: &Compound) -> FreqId {
    fnv(&format!("{}/{}", slot_key(nbt, FIRST), slot_key(nbt, LAST)))
}

/// The network a Stock Link or Stock Ticker at `pos` belongs to.
pub(super) fn freq_at(game: &Game<'_>, pos: Pos) -> Option<FreqId> {
    match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
        Some(BlockEntity::Mod(e)) if e.id == BLOCK || e.id == TICKER => Some(freq_of(&e.nbt)),
        _ => None,
    }
}

/// Short network name shown to players.
pub(super) fn freq_name(freq: FreqId) -> String {
    format!("{:08x}", freq as u32)
}

// ------------------------------------------------------------------ inventory

/// A copy of the target inventory's slots (same shape as the Packager's own).
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

/// Runs `f` on a snapshot of the inventory at `target` read through `face`, then applies
/// the extractions it made to the real inventory. `f` gets `None` without an inventory.
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

// ------------------------------------------------------- Packager state (NBT)

// The Packager keeps its state in its block entity NBT (`packager_runtime.rs`). These
// two functions read and write exactly that layout, so a request queued here is handed
// out by the Packager's own tick.

fn pk_load(reg: &mut package::Registry, nbt: &Compound, max: &dyn Fn(&str) -> u32) -> PackagerState {
    let core_box = |key: &str, reg: &mut package::Registry| {
        get(nbt, key).and_then(item_from_nbt).map(|stack| package::to_core(reg, &stack, max))
    };
    let mut st = PackagerState {
        held_box: core_box("HeldBox", reg),
        previously_unwrapped: core_box("PrevUnwrapped", reg),
        animation_ticks: int(nbt, "AnimTicks"),
        animation_inward: int(nbt, "AnimInward") != 0,
        button_cooldown: int(nbt, "Cooldown"),
        redstone_powered: int(nbt, "RedstonePowered") != 0,
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

fn pk_save(reg: &package::Registry, st: &PackagerState, nbt: &mut Compound) {
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

fn queue_len(game: &Game<'_>, packager: Pos) -> usize {
    match game.world.block_entities.get(&(packager[0], packager[1], packager[2])) {
        Some(BlockEntity::Mod(e)) => match get(&e.nbt, "Queue") {
            Some(Tag::List(list)) => list.len(),
            _ => 0,
        },
        _ => 0,
    }
}

// --------------------------------------------------------------- network scan

fn inventory_id(pos: Pos) -> u64 {
    (pos[0] as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (pos[1] as i64 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ (pos[2] as i64 as u64).wrapping_mul(0x1656_67B1_9E37_79F9)
}

fn entities_of(game: &Game<'_>, id: &str) -> Vec<Pos> {
    let mut found: Vec<Pos> = game
        .world
        .block_entities
        .iter()
        .filter_map(|(key, entity)| match entity {
            BlockEntity::Mod(e) if e.id == id => Some([key.0, key.1, key.2]),
            _ => None,
        })
        .collect();
    found.sort_unstable();
    found
}

/// The links of one network, with what each one offers.
pub(super) struct Scan {
    pub links: Vec<StockLink>,
    pub link_pos: Vec<Pos>,
    /// Packager position and its `facing`, indexed by `PackagerId`.
    pub packagers: Vec<(Pos, Face)>,
    /// Boxes already queued on each Packager (for `isTooBusyFor`).
    pub busy: Vec<usize>,
}

pub(super) fn scan(game: &mut Game<'_>, freq: FreqId, reg: &mut package::Registry) -> Scan {
    let dim = game.dim.id().to_owned();
    let mut out = Scan { links: Vec::new(), link_pos: Vec::new(), packagers: Vec::new(), busy: Vec::new() };
    for pos in entities_of(game, BLOCK) {
        if freq_at(game, pos) != Some(freq) {
            continue;
        }
        let power = game.native_neighbor_signal(&dim, pos).clamp(0, 15) as u8;
        let mut link = StockLink {
            id: out.links.len() as u32,
            freq,
            packager: None,
            inventory: None,
            power,
            loaded: true,
            available: InventorySummary::new(),
        };
        let facing = game.native_block_property(pos, "facing").as_deref().and_then(face_named);
        if let Some(facing) = facing {
            let at = pk::step(pos, facing.opposite());
            let pk_facing = (game.native_block(at) == pk::BLOCK)
                .then(|| game.native_block_property(at, "facing").as_deref().and_then(face_named))
                .flatten();
            if let Some(pk_facing) = pk_facing {
                let id = match out.packagers.iter().position(|(p, _)| *p == at) {
                    Some(i) => i,
                    None => {
                        out.packagers.push((at, pk_facing));
                        out.busy.push(queue_len(game, at));
                        out.packagers.len() - 1
                    }
                };
                link.packager = Some(id as u32);
                let inventory = pk::step(at, pk_facing.opposite());
                let slots = game.native_container_slots(&dim, inventory, pk_facing).filter(|s| !s.is_empty());
                if let Some(slots) = slots {
                    link.inventory = Some(inventory_id(inventory));
                    let max = max_of(game);
                    for slot in slots.iter().filter(|s| s.can_take) {
                        if let Some(stack) = slot.stack.as_ref().filter(|s| s.count > 0) {
                            let counted = package::to_core(reg, stack, &max);
                            link.available.add(&counted, stack.count);
                        }
                    }
                }
            }
        }
        out.links.push(link);
        out.link_pos.push(pos);
    }
    out
}

/// Plain-text summary of a network: sizes and the biggest stocks.
pub(super) fn describe(game: &mut Game<'_>, freq: FreqId) -> String {
    let mut reg = package::Registry::default();
    let scan = scan(game, freq, &mut reg);
    let summary = net::summary_of_network(&scan.links, freq);
    let stocks = summary.stacks_by_count();
    let mut text = format!(
        "network {}: {} link(s), {} packager(s), {} item type(s), {} item(s)",
        freq_name(freq),
        scan.links.len(),
        scan.packagers.len(),
        stocks.len(),
        summary.total_count()
    );
    for entry in stocks.iter().take(12) {
        text.push_str(&format!("\n  {} x{}", entry.stack.key.id, entry.count));
    }
    text
}

// ---------------------------------------------------------------- ordering

/// Hands `requests` to the Packager at `at` (`performPackageRequests`): the boxes are
/// packed from the inventory behind it and queued on the Packager, whose tick hands them out.
fn run_packager(
    game: &mut Game<'_>,
    at: Pos,
    facing: Face,
    requests: Vec<PackagingRequest>,
    reg: &mut package::Registry,
    rng: &mut XorShift,
) -> usize {
    let nbt = match game.world.block_entities.get(&(at[0], at[1], at[2])) {
        Some(BlockEntity::Mod(e)) if e.id == pk::BLOCK => e.nbt.clone(),
        _ => return 0,
    };
    let mut st = {
        let max = max_of(game);
        pk_load(reg, &nbt, &max)
    };
    let inventory = pk::step(at, facing.opposite());
    let sent = with_target(game, inventory, facing, reg, |target| {
        target.map(|t| st.perform_requests(t, requests, &mut *rng).len())
    })
    .unwrap_or(0);
    let now = game.world.game_time as i32;
    let reg_ref: &package::Registry = reg;
    game.native_with_mod_block_entity(at, |e| {
        pk_save(reg_ref, &st, &mut e.nbt);
        e.nbt.insert(SYNC_TICK.into(), Tag::Int(now));
    });
    game.native_block_entity_sync(at);
    sent
}

/// Orders `count` of `item` from the network of `freq` (`LogisticsManager.broadcastPackageRequest`
/// with `RequestType.PLAYER`). The packages carry `address`.
pub(super) fn order(game: &mut Game<'_>, freq: FreqId, item: &str, count: u32, address: &str) -> Result<String, String> {
    if game.native_max_stack_size(item).is_none() {
        return Err(format!("unknown item {item}"));
    }
    let mut reg = package::Registry::default();
    let scan = scan(game, freq, &mut reg);
    if scan.links.is_empty() {
        return Err(format!("no Stock Link on network {}", freq_name(freq)));
    }
    let summary = net::summary_of_network(&scan.links, freq);
    // The stack the network holds under this id (with its components), else a plain one.
    let held = summary.stacks_by_count().into_iter().find(|e| e.stack.key.id == item).map(|e| e.stack);
    let template = match held {
        Some(stack) => stack,
        None => {
            let max = max_of(game);
            package::to_core(&mut reg, &ItemStack::new(item, 1), &max)
        }
    };
    let order = OrderWithCrafts::simple(vec![BigStack::new(template.copy_with_count(1), count)]);
    let mut rng = XorShift((game.world.game_time as u64) ^ (freq as u64) ^ 0x5851_F42D_4C95_7F2D);
    let busy = scan.busy.clone();
    let grouped = net::broadcast_package_request(
        &scan.links,
        freq,
        RequestType::Player,
        &order,
        None,
        address,
        &|packager, ty| busy.get(packager as usize).is_some_and(|n| *n >= engine_pk::busy_threshold(ty)),
        &mut rng,
    )
    .ok_or_else(|| "refused: empty order or a Packager has too many boxes queued".to_owned())?;
    if grouped.is_empty() {
        return Ok(format!("accepted, but network {} has no {item}", freq_name(freq)));
    }
    let (mut requested, mut boxes) = (0u32, 0usize);
    let packagers = grouped.len();
    for (id, requests) in grouped {
        requested += requests.iter().map(|r| r.count).sum::<u32>();
        let Some((at, facing)) = scan.packagers.get(id as usize).copied() else { continue };
        boxes += run_packager(game, at, facing, requests, &mut reg, &mut rng);
    }
    Ok(format!(
        "ordered {requested} of {item} from {packagers} Packager(s): {boxes} box(es) made, address \"{address}\""
    ))
}

// ------------------------------------------------------------- interaction

fn status(game: &mut Game<'_>, text: String) {
    let player = game.native_acting_id();
    game.native_action_bar(player, Text::literal(&text));
}

fn label(block: &str) -> &'static str {
    if block == TICKER { "Stock Ticker" } else { "Stock Link" }
}

/// Right click on a Stock Link or Stock Ticker.
/// * Item in hand (not sneaking): sets the first, then the last frequency item.
/// * Empty hand: shows the network (action bar). Sneaking: clears both frequencies.
pub(super) fn right_click(game: &mut Game<'_>, pos: Pos) -> bool {
    if !simulates(game) || !game.native_on("create") || game.held_slot() != game.selected || !game.may_build() {
        return false;
    }
    let block = game.native_block(pos);
    if block != BLOCK && block != TICKER {
        return false;
    }
    let sneaking = game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft);
    let held = game.held().filter(|s| s.count > 0).cloned();
    let line = match held {
        Some(_) if sneaking => return false,
        Some(mut stack) => {
            stack.count = 1;
            let wrote = game
                .native_with_mod_block_entity(pos, |e| {
                    let last = int(&e.nbt, NEXT_SLOT) != 0;
                    e.nbt.insert(if last { LAST } else { FIRST }.into(), item_to_nbt(&stack));
                    e.nbt.insert(NEXT_SLOT.into(), Tag::Byte(i8::from(!last)));
                })
                .is_some();
            if !wrote {
                return false;
            }
            let freq = freq_at(game, pos).map_or_else(String::new, freq_name);
            format!("{}: frequency item set, network {freq}", label(&block))
        }
        None if sneaking => {
            let wrote = game
                .native_with_mod_block_entity(pos, |e| {
                    e.nbt.remove(&FIRST.into());
                    e.nbt.remove(&LAST.into());
                    e.nbt.insert(NEXT_SLOT.into(), Tag::Byte(0));
                })
                .is_some();
            if !wrote {
                return false;
            }
            let freq = freq_at(game, pos).map_or_else(String::new, freq_name);
            format!("{}: frequencies cleared, network {freq}", label(&block))
        }
        None => {
            let Some(freq) = freq_at(game, pos) else { return false };
            let mut reg = package::Registry::default();
            let scan = scan(game, freq, &mut reg);
            let summary = net::summary_of_network(&scan.links, freq);
            format!(
                "{}: network {}, {} link(s), {} item type(s), {} item(s)",
                label(&block),
                freq_name(freq),
                scan.links.len(),
                summary.stacks_by_count().len(),
                summary.total_count()
            )
        }
    };
    status(game, line);
    game.native_block_entity_sync(pos);
    true
}

/// A placed Stock Link attaches to an adjacent Packager (away from it).
pub(super) fn placed(game: &mut Game<'_>, pos: Pos) {
    if !simulates(game) || game.native_block(pos) != BLOCK {
        return;
    }
    for dir in [Face::Up, Face::Down, Face::North, Face::South, Face::West, Face::East] {
        if game.native_block(pk::step(pos, dir)) == pk::BLOCK {
            game.native_set_block_property(pos, "facing", face_name(dir.opposite()));
            game.native_block_entity_sync(pos);
            return;
        }
    }
}

/// Entity read helper for tooling (does not need `Game`).
#[allow(dead_code, reason = "read helper for acceptance tooling")]
pub(super) fn frequency_items(entity: &ModBlockEntity) -> (bool, bool) {
    (get(&entity.nbt, FIRST).is_some(), get(&entity.nbt, LAST).is_some())
}
