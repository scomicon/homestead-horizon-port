// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

// Linked Controller (LinkedControllerItem and the client / server handlers).
// The rules (frequency pairs, held inputs with a 30 tick timeout, the 5 tick
// keep-alive cadence) live in `create_engine::redstone_link`
// (`ControllerClient`, `ControllerServer`); this file adapts them to the game.
//
// * Item data: the `create:linked_controller_items` component holds the 12
//   frequency items (`F0`..`F11`, button n owns slots 2n and 2n+1) and the
//   binding cursor (`Next`, the button the next bind fills).
// * Client: `tick` reads the keys while the controller is the held item and
//   active, runs `ControllerClient` and sends one packet per diff to the server
//   (`create:controller_input`).
// * Server: `receive` turns packets into `receive_pressed` calls on the level's
//   `LinkNetwork` (owned by link_runtime.rs, reached through
//   `create_port_link::controller_net`); `flush` runs `ControllerServer::tick`.
// * Operator command fallback (`create controller ...`) drives the same
//   server path without keys, for tests and for players without the keys.
//
// Differences from Create (see docs/linked-controller.md): Create uses the
// movement keys and cancels movement while active; this port cannot cancel
// movement, so the buttons are Z X C V B N and G toggles active mode. Binding by
// the controller screen is replaced by right-clicking a Redstone Link (copies its
// two frequency items into the next button) and by `create controller bind/set`.

use super::{Game, create_port_link as link_rt};
use crate::items::ItemStack;
use create_engine::redstone_link::{
    CONTROLLER_BUTTONS, CONTROLLER_SLOTS, ControllerClient, ControllerServer, Frequency, FrequencyPair,
};
use horizon_items::item_nbt::{item_from_nbt, item_to_nbt};
use horizon_world::nbt::Tag;
use std::{cell::RefCell, collections::BTreeMap};
use winit::keyboard::KeyCode;

pub(super) const ITEM: &str = "create:linked_controller";
const LINK_BLOCK: &str = "create:redstone_link";
pub(super) const COMPONENT: &str = "create:linked_controller_items";
const CHANNEL: &str = "create:controller_input";
const NEXT: &str = "Next";
const KEYS: [&str; CONTROLLER_SLOTS] =
    ["F0", "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11"];
const BUTTON_KEYS: [KeyCode; CONTROLLER_BUTTONS] =
    [KeyCode::KeyZ, KeyCode::KeyX, KeyCode::KeyC, KeyCode::KeyV, KeyCode::KeyB, KeyCode::KeyN];
const TOGGLE_KEY: KeyCode = KeyCode::KeyG;
const SYNTAXES: &[&str] = &[
    "create controller give",
    "create controller status",
    "create controller clear",
    "create controller set <button:integer> <first:word> <second:word>",
    "create controller bind <button:integer> <x:integer> <y:integer> <z:integer>",
    "create controller press <button:integer>",
    "create controller release <button:integer>",
];

type Pos = [i32; 3];

pub(super) fn component_types(types: &mut Vec<String>) {
    types.push(COMPONENT.into());
}

fn simulates(game: &Game<'_>) -> bool {
    game.serving.is_some() || game.options.dedicated.is_some()
}

fn session(game: &Game<'_>) -> usize {
    std::ptr::from_ref(&game.world) as usize
}

// ------------------------------------------------------------------ item data

/// One frequency item: identity plus the dye colour (what `Frequency.of` uses).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Slot {
    item: String,
    dye: Option<i32>,
}

type Pair = (Option<Slot>, Option<Slot>);

struct Data {
    items: Vec<Option<ItemStack>>,
    next: i32,
}

fn slot_of(stack: &ItemStack) -> Slot {
    Slot { item: stack.item.to_string(), dye: stack.data().and_then(|data| data.dyed_color) }
}

fn frequency(slot: &Option<Slot>) -> Frequency {
    slot.as_ref().map_or_else(Frequency::empty, |s| Frequency::of(&s.item, s.dye))
}

fn frequency_pair(pair: &Pair) -> FrequencyPair {
    (frequency(&pair.0), frequency(&pair.1))
}

fn read(game: &Game<'_>, stack: &ItemStack) -> Data {
    let mut data = Data { items: vec![None; CONTROLLER_SLOTS], next: 0 };
    if let Ok(Some(tag)) = game.native_component(stack, COMPONENT) {
        for (index, key) in KEYS.iter().enumerate() {
            data.items[index] = tag.get(key).and_then(item_from_nbt);
        }
        if let Some(Tag::Int(next)) = tag.get(NEXT) {
            data.next = next.rem_euclid(CONTROLLER_BUTTONS as i32);
        }
    }
    data
}

fn write(game: &Game<'_>, stack: &mut ItemStack, data: &Data) -> bool {
    let mut entries: Vec<(&str, Tag)> = KEYS
        .iter()
        .zip(&data.items)
        .filter_map(|(key, item)| item.as_ref().map(|item| (*key, item_to_nbt(item))))
        .collect();
    entries.push((NEXT, Tag::Int(data.next)));
    game.native_set_component(stack, COMPONENT, Tag::compound(entries)).is_ok()
}

/// The two frequency slots of `button`; None when both are empty.
fn pair_of(data: &Data, button: usize) -> Option<Pair> {
    let slot = |index: usize| data.items.get(index).and_then(Option::as_ref).map(slot_of);
    let pair = (slot(button * 2), slot(button * 2 + 1));
    (pair.0.is_some() || pair.1.is_some()).then_some(pair)
}

fn is_controller(stack: &ItemStack) -> bool {
    &*stack.item == ITEM
}

fn held_controller(game: &Game<'_>) -> Option<ItemStack> {
    game.stack_in(game.selected).filter(|stack| stack.count > 0 && is_controller(stack)).cloned()
}

// ----------------------------------------------------------------- wire format

fn put_slot(out: &mut Vec<u8>, slot: &Option<Slot>) {
    match slot {
        None => out.push(0),
        Some(slot) => {
            let bytes = slot.item.as_bytes();
            out.push(1);
            out.push(bytes.len().min(255) as u8);
            out.extend_from_slice(&bytes[..bytes.len().min(255)]);
            match slot.dye {
                Some(color) => {
                    out.push(1);
                    out.extend_from_slice(&color.to_be_bytes());
                }
                None => out.push(0),
            }
        }
    }
}

fn encode(pressed: bool, pairs: &[Pair]) -> Vec<u8> {
    let mut out = vec![u8::from(pressed), pairs.len() as u8];
    for pair in pairs {
        put_slot(&mut out, &pair.0);
        put_slot(&mut out, &pair.1);
    }
    out
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn take(&mut self, count: usize) -> Option<&[u8]> {
        if self.0.len() < count {
            return None;
        }
        let (head, rest) = self.0.split_at(count);
        self.0 = rest;
        Some(head)
    }

    fn byte(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }

    fn slot(&mut self) -> Option<Option<Slot>> {
        if self.byte()? == 0 {
            return Some(None);
        }
        let len = usize::from(self.byte()?);
        let item = String::from_utf8(self.take(len)?.to_vec()).ok()?;
        let dye = match self.byte()? {
            0 => None,
            _ => Some(i32::from_be_bytes(self.take(4)?.try_into().ok()?)),
        };
        Some(Some(Slot { item, dye }))
    }
}

fn decode(data: &[u8]) -> Option<(bool, Vec<FrequencyPair>)> {
    let mut reader = Reader(data);
    let pressed = reader.byte()? != 0;
    let count = usize::from(reader.byte()?);
    if count > CONTROLLER_BUTTONS {
        return None;
    }
    let mut pairs = Vec::with_capacity(count);
    for _ in 0..count {
        let first = reader.slot()?;
        let last = reader.slot()?;
        pairs.push(frequency_pair(&(first, last)));
    }
    reader.0.is_empty().then_some((pressed, pairs))
}

// ---------------------------------------------------------------------- state

#[derive(Default)]
struct Client {
    handler: ControllerClient,
    active: bool,
    toggle_down: bool,
    /// Pairs of the controller seen last, so a release can still be sent after the
    /// controller left the hand.
    pairs: Vec<Option<Pair>>,
}

thread_local! {
    static CLIENT: RefCell<Client> = RefCell::new(Client::default());
    static SERVERS: RefCell<BTreeMap<(usize, String), ControllerServer>> = RefCell::new(BTreeMap::new());
}

fn say(game: &mut Game<'_>, message: &str) {
    game.native_feedback(horizon_client::text::Text::literal(message));
}

// --------------------------------------------------------------------- client

/// `tick` hook (client window): active toggle, key diffing, packets.
pub(super) fn tick(game: &mut Game<'_>) {
    if !game.native_on("create") || !game.native_window() {
        return;
    }
    let held = held_controller(game);
    let data = held.as_ref().map(|stack| read(game, stack));
    let toggled = game.keys.contains(&TOGGLE_KEY);
    let (packets, pairs, edge, active) = CLIENT.with_borrow_mut(|client| {
        let edge = toggled && !client.toggle_down;
        client.toggle_down = toggled;
        if held.is_none() {
            client.active = false;
        } else if edge {
            client.active = !client.active;
        }
        if let Some(data) = &data {
            client.pairs = (0..CONTROLLER_BUTTONS).map(|button| pair_of(data, button)).collect();
        }
        let pressed: Vec<usize> = if client.active {
            (0..CONTROLLER_BUTTONS).filter(|button| game.keys.contains(&BUTTON_KEYS[*button])).collect()
        } else {
            Vec::new()
        };
        (client.handler.tick(pressed), client.pairs.clone(), edge && held.is_some(), client.active)
    });
    if edge {
        say(game, if active { "Linked Controller: active (Z X C V B N, G to leave)" } else { "Linked Controller: inactive" });
    }
    for packet in packets {
        let chosen: Vec<Pair> =
            packet.buttons.iter().filter_map(|button| pairs.get(*button).cloned().flatten()).collect();
        if chosen.is_empty() {
            continue;
        }
        let _ = game.native_net_send_to_server(CHANNEL, &encode(packet.pressed, &chosen));
    }
}

// --------------------------------------------------------------------- server

fn press(game: &mut Game<'_>, dimension: &str, player: u128, pos: Pos, pairs: &[FrequencyPair], pressed: bool) {
    let key = (session(game), dimension.to_owned());
    link_rt::controller_net(game, dimension, |net| {
        SERVERS.with_borrow_mut(|servers| {
            servers.entry(key).or_default().receive_pressed(net, pos, player, pairs, pressed);
        });
        ((), true)
    });
}

/// `net_from_client` hook: `LinkedControllerInputPacket`.
pub(super) fn receive(game: &mut Game<'_>, player: u128, channel: &str, data: &[u8]) {
    if channel != CHANNEL || !game.native_on("create") || !simulates(game) {
        return;
    }
    let Some((pressed, pairs)) = decode(data) else { return };
    let Some(who) = game.native_server_players().into_iter().find(|p| p.uuid == player) else { return };
    let pos = [who.position[0].floor() as i32, who.position[1].floor() as i32, who.position[2].floor() as i32];
    press(game, &who.dimension, player, pos, &pairs, pressed);
}

/// `level_tick_post` hook: `LinkedControllerServerHandler.tick`.
pub(super) fn flush(game: &mut Game<'_>, dimension: &str) {
    if !simulates(game) || !game.native_on("create") {
        return;
    }
    let key = (session(game), dimension.to_owned());
    if !SERVERS.with_borrow(|servers| servers.contains_key(&key)) {
        return;
    }
    link_rt::controller_net(game, dimension, |net| {
        let writes = SERVERS.with_borrow_mut(|servers| {
            servers.get_mut(&key).map(|server| server.tick(net)).unwrap_or_default()
        });
        ((), !writes.is_empty())
    });
}

// ---------------------------------------------------------------- right click

fn link_frequency(game: &mut Game<'_>, pos: Pos) -> Option<(Option<ItemStack>, Option<ItemStack>)> {
    game.native_with_mod_block_entity(pos, |entity| {
        (entity.id == LINK_BLOCK).then(|| {
            let slot = |key: &str| entity.nbt.get(&key.into()).and_then(item_from_nbt);
            (slot("FrequencyFirst"), slot("FrequencyLast"))
        })
    })
    .flatten()
}

/// Right click on a Redstone Link with the controller: copies the link's two
/// frequency items into the next button and advances the cursor; sneaking clears
/// the button bound last. Must run before the link's own right click.
pub(super) fn right_click(game: &mut Game<'_>, pos: Pos) -> bool {
    if !simulates(game)
        || !game.native_on("create")
        || game.held_slot() != game.selected
        || !game.may_build()
        || game.native_block(pos) != LINK_BLOCK
    {
        return false;
    }
    let Some(mut stack) = held_controller(game) else { return false };
    let sneaking = game.keys.contains(&KeyCode::ShiftLeft);
    let mut data = read(game, &stack);
    let buttons = CONTROLLER_BUTTONS as i32;
    if sneaking {
        data.next = (data.next + buttons - 1) % buttons;
        let first = data.next as usize * 2;
        data.items[first] = None;
        data.items[first + 1] = None;
    } else {
        let Some((first, last)) = link_frequency(game, pos) else { return false };
        let base = data.next as usize * 2;
        data.items[base] = first;
        data.items[base + 1] = last;
        data.next = (data.next + 1) % buttons;
    }
    if !write(game, &mut stack, &data) {
        return false;
    }
    let selected = game.selected;
    *game.stack_in_mut(selected) = Some(stack);
    true
}

// ------------------------------------------------------------------- commands

pub(super) fn register(game: &Game<'_>, commands: &mut Vec<crate::mod_api::NativeCommand>) {
    if game.native_on("create") {
        for syntax in SYNTAXES {
            commands.push(crate::mod_api::NativeCommand::op(*syntax, 2));
        }
    }
}

/// `stone` -> `minecraft:stone`, `create.brass_ingot` -> `create:brass_ingot`
/// (command words cannot hold a colon).
fn item_id(word: &str) -> Result<String, String> {
    let id = match word.split_once('.') {
        Some((namespace, path)) => format!("{namespace}:{path}"),
        None => format!("minecraft:{word}"),
    };
    let valid = id.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_:-/".contains(&c));
    if valid && id != "minecraft:air" { Ok(id) } else { Err(format!("not an item id: {word}")) }
}

fn button_arg(word: &str) -> Result<usize, String> {
    word.parse::<usize>()
        .ok()
        .filter(|button| *button < CONTROLLER_BUTTONS)
        .ok_or_else(|| "button must be 0 to 5".to_owned())
}

fn held_or_err(game: &Game<'_>) -> Result<ItemStack, String> {
    held_controller(game).ok_or_else(|| "hold a Linked Controller in the selected hotbar slot".to_owned())
}

fn store(game: &mut Game<'_>, mut stack: ItemStack, data: &Data) -> Result<(), String> {
    if !write(game, &mut stack, data) {
        return Err("component write failed".into());
    }
    let selected = game.selected;
    *game.stack_in_mut(selected) = Some(stack);
    Ok(())
}

fn run(game: &mut Game<'_>, player: &crate::mod_api::NativeServerPlayer, words: &[&str]) -> Result<String, String> {
    let pos = [
        player.position[0].floor() as i32,
        player.position[1].floor() as i32,
        player.position[2].floor() as i32,
    ];
    match words {
        ["give"] => {
            let stack = ItemStack::new(ITEM, 1);
            if let Some(rest) = game.native_give_stack(player.uuid, stack) {
                game.spawn_item(rest, player.position, [0.0, 0.2, 0.0], 10);
            }
            Ok("gave a Linked Controller".into())
        }
        ["status"] => {
            let stack = held_or_err(game)?;
            let data = read(game, &stack);
            let held = SERVERS.with_borrow(|servers| {
                servers
                    .get(&(session(game), player.dimension.clone()))
                    .map_or(0, |server| server.held_count(player.uuid))
            });
            let mut text = format!("next={} held={held}", data.next);
            for button in 0..CONTROLLER_BUTTONS {
                let name = |index: usize| {
                    data.items[index].as_ref().map_or_else(|| "-".to_owned(), |item| item.item.to_string())
                };
                text.push_str(&format!(" | {button}: {} {}", name(button * 2), name(button * 2 + 1)));
            }
            Ok(text)
        }
        ["clear"] => {
            let stack = held_or_err(game)?;
            store(game, stack, &Data { items: vec![None; CONTROLLER_SLOTS], next: 0 })?;
            Ok("cleared all buttons".into())
        }
        ["set", button, first, second] => {
            let button = button_arg(button)?;
            let (first, second) = (item_id(first)?, item_id(second)?);
            let stack = held_or_err(game)?;
            let mut data = read(game, &stack);
            data.items[button * 2] = Some(ItemStack::new(first.as_str(), 1));
            data.items[button * 2 + 1] = Some(ItemStack::new(second.as_str(), 1));
            store(game, stack, &data)?;
            Ok(format!("button {button} = {first} + {second}"))
        }
        ["bind", button, x, y, z] => {
            let button = button_arg(button)?;
            let target = [
                x.parse::<i32>().map_err(|_| "coordinates must be integers")?,
                y.parse::<i32>().map_err(|_| "coordinates must be integers")?,
                z.parse::<i32>().map_err(|_| "coordinates must be integers")?,
            ];
            let (first, last) = link_frequency(game, target).ok_or("no Redstone Link there")?;
            let stack = held_or_err(game)?;
            let mut data = read(game, &stack);
            data.items[button * 2] = first;
            data.items[button * 2 + 1] = last;
            store(game, stack, &data)?;
            Ok(format!("button {button} bound to the link at {target:?}"))
        }
        ["press", button] | ["release", button] => {
            let pressed = words[0] == "press";
            let button = button_arg(button)?;
            let stack = held_or_err(game)?;
            let pair = pair_of(&read(game, &stack), button).ok_or("that button has no frequency")?;
            press(game, &player.dimension, player.uuid, pos, &[frequency_pair(&pair)], pressed);
            Ok(format!("button {button} {}", if pressed { "pressed (30 tick timeout)" } else { "released" }))
        }
        _ => Err("usage: give | status | clear | set | bind | press | release".into()),
    }
}

pub(super) fn command(
    game: &mut Game<'_>,
    player: &crate::mod_api::NativeServerPlayer,
    line: &str,
    ran: &mut bool,
) {
    let words = line.split_whitespace().collect::<Vec<_>>();
    if words.get(..2) != Some(&["create", "controller"][..]) || !game.native_on("create") {
        return;
    }
    *ran = true;
    if player.permission < 2 {
        say(game, "create controller commands need operator permission 2");
        return;
    }
    let message = match run(game, player, &words[2..]) {
        Ok(ok) => ok,
        Err(e) => format!("create controller: {e}"),
    };
    say(game, &message);
}
