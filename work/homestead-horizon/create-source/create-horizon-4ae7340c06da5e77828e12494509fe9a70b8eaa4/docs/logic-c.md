# Logic mods, group C: Stack Refill, CraftingPad, Trade Cycling

Part of the `homestead` native bundle (`mixins/homestead`). None of the three mods is in
`pack/mods`, and no jar or source was available while this was written, so the rules below come
from what the mods are known to do, not from a read of their code. Every guess is listed.

| Mod | Status |
| --- | --- |
| Stack Refill | Ported (`stack_refill.rs`). Local player only, no server. |
| CraftingPad | Ported (`craftingpad.rs`) with a content mod (`tools/craftingpad_content.py`). Placeholder texture. |
| Trade Cycling | Not ported: no hook for it. See the end. |

## Stack Refill

When the stack in the selected hotbar slot or in the off hand is used up, the next stack of the
same item moves into that slot. Once per tick (`level_tick_pre`), for the window game's player.

* **Used up.** A tick sees the slot empty where the tick before saw a stack, the player is playing
  (no screen open), and the inventory (hotbar, main, off hand) holds less of that exact item
  than before. A stack that only moved keeps the total and is left alone: swapping hands, pressing
  another hotbar key (the check also needs the same selected slot) or moving items in a screen.
* **Thrown.** `item_dropped` is heard before the next tick and switches the refill off for that
  tick, so the drop key leaves an empty hand as in vanilla.
* **Source.** The first exactly equal stack (same item and components) in the main inventory
  (slots 9 to 35), then the rest of the hotbar, then the off hand (or the main hand for the
  off hand). A damageable item that broke (a tool) falls back to the first stack of the same
  item with other components, so a worn pickaxe is replaced by a fresh one. The whole stack moves.
* **Commands.** `item replace ... air` and `clear` empty the hand without a drop, so they refill as
  well when another stack exists.

Differences and guesses: the exact search order, the break fallback, the off hand being served,
and the creative mode behaviour (nothing is consumed there, so nothing happens) are assumptions.
There is no configuration, no sound and no hotbar-only rule. A remote server is not served
(its inventory is the server's), and LAN guests are not either.

## CraftingPad

An item (`craftingpad:craftingpad`, stack of one) that opens the 3x3 crafting menu on a right
click. The recipe is the original's: a crafting table directly above a stick.

The menu is the game's own crafting table screen, opened the way a click on a table opens it
(`craft_size = 3`, `craft_table`, `Screen::Inventory`): the same grid, recipes, recipe book,
shift-click and result. A table's menu is closed by `tick_craft_table` when the block is gone or
out of reach; the pad's menu is marked with a position no world can have (`craft_table =
Some([i32::MIN; 3])`) and a cancellable head on `tick_craft_table` skips the check for a
marked menu. Closing is a table's closing (grid back to the inventory, mark cleared).

* **Content.** `Build-CraftingPad.ps1` writes `craftingpad-1.0.12-horizon.1.hmod` to
  `stage4/extra-mods`: the item (a stick template with `max_stack_size` 1), the recipe, a
  creative tab, English and Russian names, and a generated 16x16 placeholder texture.
  The original artwork was not available. `Build-DataMods.ps1` removes every non-Farmer's
  Delight `*-horizon.*.hmod` from `extra-mods`, so it must keep this one (see the fragment).
* **Used in the air.** A click on a block with its own interaction (a chest, a table) does that
  block's own thing first; any other click opens the pad.
* **Not done.** Use on a server; the original's exact close rule (the menu stays open here
  until it is closed); the original texture and model.

## Trade Cycling: not ported

The mod adds a button to the villager trading screen that re-rolls a villager's offers while it
has not been traded with. The mod API has no part of that:

* the merchant hooks only listen (`trade_with_villager`, `merchant_offer_traded`,
  `player_trade`, `trade_select`) and are server-side (LAN guests); none opens, extends or
  replaces the trading screen of the window player,
* there is no call that re-generates a villager's offer list or reads its `uses`/`xp`,
* a button on the vanilla merchant screen would need a screen hook for the built-in screen
  (`screen` hooks are for mod screens).

A command that re-rolls the nearest villager would not be the mod. It needs engine hooks
(merchant screen widgets and a villager offer reroll) first.

## Files

`mixins/homestead/`: `stack_refill.rs`, `stack_refill_tick.rs`, `stack_refill_drop.rs`,
`craftingpad.rs`, `craftingpad_use.rs`, `craftingpad_table.rs`, `fragments/logicC.toml` and
`logicC.md`. `tools/craftingpad_content.py`. `stage4/Build-CraftingPad.ps1`,
`stage4/acceptance/Test-HomesteadLogicC.ps1`.

## Unverified

Nothing here was compiled or run. The first build may show: `max_damage`'s type on the item
info, `Screen::Playing` and `Screen::Inventory` paths, the visibility of `craft_table`,
`craft_size` and `capture` from the injected helpers, and whether the `tick_craft_table` target
path resolves (`horizon_client::game::storage::Game<'_>::tick_craft_table`). The acceptance test
drives refills with `interact` and `break` in survival mode and the pad with `use-item`;
it never drags items in the crafting grid.


## Статус на stage19: выключено

Проверка в игре показала: хуки `level_tick_pre` и `use_item` вызываются на серверном потоке, где у `Game` нет локального игрока, инвентаря и экрана (`player` пуст, `selected` = 0). Поэтому Stack Refill не видит руку, а CraftingPad не может открыть экран. Модули и инъекции убраны из `horizon-mixin.toml`, файлы остались для будущей работы с клиентским хуком или доступом к инвентарю серверного игрока.
