// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

// Stock Ticker (`StockTickerBlock`): the ordering terminal of a stock network. Create
// opens a request screen on it; this port has no such screen, so the ticker is driven by
// operator commands instead. The block itself (frequency items, status on an empty-hand
// click) is handled by `stock_link_runtime.rs`; this file holds the `create stock ...`
// commands and the glue that turns their words into network operations.
//
//   create stock list    X Y Z                       the network's stock (ticker or link)
//   create stock status  X Y Z                       network size / port status
//   create stock order   X Y Z ITEM COUNT [ADDRESS]  order items; boxes leave the Packagers
//   create stock address X Y Z ADDRESS               address filter of a Frogport / Postbox
//
// Words: an item without a namespace is `minecraft:`; `create.brass_ingot` stands for
// `create:brass_ingot` because command words cannot hold a colon. In addresses `+` means
// `*` (glob), `all` is `*`, `none` makes a port stop accepting packages and `clear`
// empties its filter.

use super::{
    Game, create_port_frogport as frogport, create_port_stock_link as link,
};
use crate::mod_api::{NativeCommand, NativeServerPlayer};

const SYNTAXES: &[&str] = &[
    "create stock list <x:integer> <y:integer> <z:integer>",
    "create stock status <x:integer> <y:integer> <z:integer>",
    "create stock order <x:integer> <y:integer> <z:integer> <item:word> <count:integer>",
    "create stock order <x:integer> <y:integer> <z:integer> <item:word> <count:integer> <address:word>",
    "create stock address <x:integer> <y:integer> <z:integer> <address:word>",
];

const MAX_COUNT: u32 = 1_000_000;

pub(super) fn register(game: &Game<'_>, commands: &mut Vec<NativeCommand>) {
    if game.native_on("create") {
        for syntax in SYNTAXES {
            commands.push(NativeCommand::op(*syntax, 2));
        }
    }
}

fn say(game: &mut Game<'_>, message: &str) {
    game.native_feedback(horizon_client::text::Text::literal(message));
}

fn coords(words: &[&str]) -> Result<[i32; 3], String> {
    if words.len() < 3 {
        return Err("three coordinates are required".into());
    }
    let mut out = [0; 3];
    for (slot, word) in out.iter_mut().zip(words) {
        *slot = word.parse().map_err(|_| "coordinates must be integers".to_owned())?;
    }
    Ok(out)
}

/// `cobblestone` -> `minecraft:cobblestone`, `create.brass_ingot` -> `create:brass_ingot`.
fn item_id(word: &str) -> String {
    if word.contains(':') {
        word.to_owned()
    } else if let Some((namespace, name)) = word.split_once('.') {
        format!("{namespace}:{name}")
    } else {
        format!("minecraft:{word}")
    }
}

/// Address word of an order: `+` is `*`, `all` is `*`, `none` / `clear` are blank.
fn order_address(word: &str) -> String {
    match word {
        "none" | "clear" => String::new(),
        "all" => "*".to_owned(),
        other => other.replace('+', "*"),
    }
}

fn run(game: &mut Game<'_>, words: &[&str]) -> Result<String, String> {
    match words {
        ["list", rest @ ..] if rest.len() == 3 => {
            let at = coords(rest)?;
            let freq = link::freq_at(game, at).ok_or("no Stock Link or Stock Ticker there")?;
            Ok(link::describe(game, freq))
        }
        ["status", rest @ ..] if rest.len() == 3 => {
            let at = coords(rest)?;
            if let Some(freq) = link::freq_at(game, at) {
                let text = link::describe(game, freq);
                return Ok(text.lines().next().unwrap_or_default().to_owned());
            }
            frogport::status_line(game, at).ok_or_else(|| "no Stock Link, Stock Ticker, Frogport or Postbox there".to_owned())
        }
        ["order", rest @ ..] if rest.len() == 5 || rest.len() == 6 => {
            let at = coords(rest)?;
            let freq = link::freq_at(game, at).ok_or("no Stock Link or Stock Ticker there")?;
            let count: u32 = rest[4].parse().map_err(|_| "count must be an integer".to_owned())?;
            if count == 0 || count > MAX_COUNT {
                return Err(format!("count must be 1 to {MAX_COUNT}"));
            }
            let address = rest.get(5).map(|word| order_address(word)).unwrap_or_default();
            link::order(game, freq, &item_id(rest[3]), count, &address)
        }
        ["address", rest @ ..] if rest.len() == 4 => {
            let at = coords(rest)?;
            frogport::set_address(game, at, rest[3])
        }
        _ => Err("usage: create stock list|status X Y Z, order X Y Z ITEM COUNT [ADDRESS], address X Y Z ADDRESS".into()),
    }
}

pub(super) fn command(game: &mut Game<'_>, player: &NativeServerPlayer, line: &str, ran: &mut bool) {
    let words = line.split_whitespace().collect::<Vec<_>>();
    if words.get(..2) != Some(&["create", "stock"][..]) || !game.native_on("create") {
        return;
    }
    *ran = true;
    if player.permission < 2 {
        say(game, "create stock commands need operator permission 2");
        return;
    }
    let message = match run(game, &words[2..]) {
        Ok(ok) => ok,
        Err(e) => format!("create stock: {e}"),
    };
    say(game, &message);
}
