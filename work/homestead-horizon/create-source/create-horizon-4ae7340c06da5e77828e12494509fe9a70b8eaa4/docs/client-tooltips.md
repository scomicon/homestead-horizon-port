# Client tooltips (group T, stage 20)

Three client-side mods of the Homestead pack, rebuilt as one native module
(`mixins/homestead/client_tooltips.rs`, module `homestead_client_tooltips`): EnchantmentDescriptions,
More Enchantment Info and Tooltip Overhaul. Nothing here has been compiled or run yet; the module
passes `rustfmt --check` (syntax) and the acceptance script parses.

Sources: the three mods' public descriptions only (their names, what they show, their usual
options). Several are copyleft or all rights reserved, so no code and no asset of any of them was
read or used. Every text below is our own wording; every number (the 38-character wrap, the colours)
is our choice. The engine facts come from `native-hooks.json`, `crates/horizon-client/src/game/*`
(`native_hooks.rs`, `stations.rs`, `inventory_input.rs`) and `crates/horizon-items/src/enchant.rs`.

## Why this is safe on the client (the lesson of stage 19)

Stack Refill and CraftingPad did nothing because their hooks (`level_tick_pre`, `use_item`) run on the
server thread, where the game has no local player and no screen. The hooks used here are different:

- `tooltip` is called by `Game::stack_tooltip_lines` once the vanilla lines of an item tooltip are
  built, `tooltip_color` by `Game::tooltip_box` when the frame is drawn. Both belong to the window's
  `Game`; they run only when a tooltip is being built for a screen.
- The commands (`register_commands`, `perform_command`) run on the game that runs the command line,
  which for the control socket's `command` is the same window game. The test aid
  `homestead tooltip show` calls `Game::stack_tooltip`, the very function the screen uses, so what it
  prints is what the hook produced.

## Done

### Enchantment descriptions (EnchantmentDescriptions)

Under every enchantment line of an item tooltip the module adds the enchantment's description, wrapped
at 38 characters and indented two spaces, in grey. The text comes from the language key
`enchantment.<namespace>.<path>.desc` (so `enchantment.minecraft.sharpness.desc`); when the language
has no such key, a built-in table gives a short English or Russian text for the 42 vanilla
enchantments. A modded enchantment without a key gets no description line. The pack's
"Enchantment Descriptions Compat" resource pack, or a content mod, can therefore supply texts for
other enchantments through ordinary language files.

### More enchantment info (More Enchantment Info)

Three more grey-blue lines under the enchantment: `Max level: V`, `Applies to: Swords, Axes` (from the
enchantment's `primary` item set, else its `supported` set; the last segment of a
`#minecraft:enchantable/...` tag is turned into a name) and `Incompatible with: ...` (its
`exclusive` set). A group without a known name is shown as its tag's last segment with the first
letter capitalised.

### Rarity frame (Tooltip Overhaul)

The tooltip box's frame takes the item's rarity colour: yellow for uncommon, aqua for rare, magenta
for epic, a darker shade of it at the bottom edge, and a faint tint of the same colour in the
otherwise near-black background. Common items, and tooltips that belong to no item, keep vanilla's
purple frame.

### When the lines show

Enchanted books always show the description and the info (the place where you read them); every other
item shows them only while Shift is held, so a normal tooltip stays short. Items with no enchantment
are untouched.

### Switches and the test aid: `homestead tooltip ...` (operator, level 2)

| Command | Does |
| --- | --- |
| `homestead tooltip status` | the three switches, the forced-Shift flag and how many times each hook ran |
| `homestead tooltip show` | the tooltip lines of the held item, `|`-separated (the lines the client draws) |
| `homestead tooltip color` | the three frame colours the hook gives the held item (`background`, `top`, `bottom`, hex ARGB) |
| `homestead tooltip descriptions on\|off` | description lines |
| `homestead tooltip info on\|off` | max level, applies to, incompatible |
| `homestead tooltip borders on\|off` | rarity frame |
| `homestead tooltip shift on\|off` | behave as if Shift were held (the control socket has no modifier keys) |

The switches are not saved: they are all on after every start. A saved configuration (the originals
have config files) is not built.

## Differs from the originals

- The defaults of the original mods (whether a key is needed, how wide a line is, which items show
  what) were not read; the Shift rule and the 38-character wrap are ours.
- No config file or config screen; the switches reset at start. No per-item or per-mod blacklist.
- Tooltip Overhaul in the original also restyles the tooltip (background styles, item-name colours,
  a drop shadow and other options); only the rarity frame is rebuilt here.
- More Enchantment Info in the original may show different or more lines; ours is the three above.
- The "Applies to" names come from a small table of vanilla item groups; the Russian table is
  our translation, not any pack's.
- Enchantment books and gear only. Potions, spawn eggs and other tooltips are untouched.

## Not verified (never run)

- Compilation. Types, paths and field names were taken from the Horizon sources and each was seen in
  use in the client crate, but nothing was compiled. `clientT.md` lists the risks.
- How the tooltip looks on screen: spacing, line widths, the frame colours against the rarity
  colours of the name line, the tint in the background.
- The language lookup of `.desc` keys, and the Russian texts and group names (the check
  `language.code` may not exist in the Horizon language files; then English is shown).
- A real Shift key (`homestead tooltip shift on` stands in for it in the script).
- That the hook is called for every place a stack tooltip shows (inventory, hotbar, creative tabs,
  chat hover, recipe book): the code path `stack_tooltip_lines -> native_tooltip` covers them in the
  sources, but only the control socket's path was exercised by the script.

## What is not built

- A saved config and an in-game screen for the switches.
- Descriptions for enchantments of other mods beyond what their language files supply.
- Tooltip Overhaul's other styles (background styles, name colours, shadows, fading).
- Potion and effect descriptions, and other tooltip additions of the same mods.
