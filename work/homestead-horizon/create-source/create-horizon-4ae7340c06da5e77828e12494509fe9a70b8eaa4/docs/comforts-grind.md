# Comforts, Peaceful Hunger, Grind Enchantments (stage 27, engine patches)

These three mods were skipped earlier because the engine gave them no way in (see `logic-b.md`, `logic-d.md`, the
triage in `stage4/research/logic-machines-triage.md`). Three small local engine patches (`stage4/engine-patches/20-,
21-, 22-*.ps1`) open the doors; the mods stand on them. All three were rebuilt from public descriptions only
(Comforts and Grind Enchantments are copyleft, Peaceful Hunger permissive; none of the sources was read).
**Nothing here was compiled or run.** The patch scripts were dry-run on copies of the pinned engine files (anchors
match, idempotent) and the new Rust files pass `rustfmt --check`.

## The engine patches

| Patch | What it changes | Why |
| --- | --- | --- |
| `20-sleep-host.ps1` | `try_sleep` calls `native_can_player_sleep`, `native_player_spawn_point_set` (cause "bed") and `native_player_bed_enter` for the acting player (guest or host) instead of only for a LAN guest. `is_bed` and the two "bed is gone" wake-up checks accept a block with a bed's state data (`part`, `occupied`, `facing`) as well as a name ending in `_bed`. | The sleep hooks were silent in a single-player world. A bed on the vanilla bed template with another name (a sleeping bag) was not a bed to the engine and woke the player at once. |
| `21-peaceful-hunger.ps1` | Adds `horizon_client::hunger::PEACEFUL_HUNGER` (default false = vanilla); while it is true it skips the Peaceful food and saturation refill AND makes the food tick see Easy instead of Peaceful (`TickWorld::difficulty`), because vanilla `FoodData.tick` only takes a food point away when the difficulty is not Peaceful. | The engine refilled the food bar in Peaceful like vanilla, and even without the refill the bar would never have gone down (found in the stage 22 acceptance run: food stayed 20 with a 255-level Hunger effect). |
| `22-grind-take.ps1` | The grindstone result's `onTake` in `menu.rs` asks `homestead_grind::plan` first: the item stays in slot 1, the book goes, the levels are charged, no experience. | Vanilla clears both inputs, which would lose the item. |

The hooks need no new catalogue entries: `can_player_sleep`, `player_spawn_point_set` and `player_bed_enter` exist
(`native-hooks.json`); the grindstone and hunger changes are plain patches plus an injection by `target` (the same way
Too Cheap! injects into `anvil_result`).

## Comforts

**Done.** `comforts_content.py` writes a content mod with 16 sleeping bags and 16 hammocks (`comforts:sleeping_bag_<colour>`,
`comforts:hammock_<colour>`): blocks on the vanilla bed template (two blocks, right click to sleep, the head drops the item), items that place them, a
creative tab, English and Russian names. The native part cancels the respawn-point change when the bed block is
one of these. The bed template makes them beds for the engine's class rules (`Class::AbstractBed`), the patch makes
them beds for the sleeping code.

**Differs from the original (from its public description; unverified against the real mod).**

- Recipes are placeholders (bag: three wool over a leather; hammock: a row of wool under two strings), not the original's.
- The look is vanilla wool on a slab (bag) or a carpet (hammock), not the original models. A bed template renders by
  its model: blockstates map every facing/part/occupied combination to one model per colour, so a bed is drawn as two
  identical flat blocks, not as a bag with a pillow.
- No daytime nap, no cold/heat or "tied hammock" rules, no rope, no night-sleep effects of the original.
- Shape and collision are the bed template's (9/16 high); the flat model is lower than its outline.
- Villagers do not use them (their bed checks still look for a name ending in `_bed`).

**Not verified.** That a player really lies down in a bag (`sleeping_position` for a non-vanilla shape), that the
head/foot placement works from the item, the model rendering. The acceptance test checks `occupied=true` after a
right click at night, the respawn point (if readable through the socket) and a vanilla bed as the control.

## Peaceful Hunger

**Done.** Switch in the engine, set from `config/homestead-hunger.txt` on the first server tick (`peaceful_hunger=false`
turns it off; the default is on, as in the pack). In Peaceful the food and saturation bars no longer climb back by
themselves, exhaustion takes saturation and then food away as in Easy, and the health regeneration stays. Starvation then follows Easy's rules (it stops at five hearts).

**Differs / unknown.** Whether the original also changes the damage of starvation in Peaceful (vanilla starvation does
not hurt in Peaceful) is not known; here it follows Easy (five hearts). **Not verified.** The test drains the bar with
`effect give @s minecraft:hunger 20 255`, then checks the bar did not refill over 200 ticks; this is timing-sensitive.

## Grind Enchantments

**Done.** An enchanted item (tool, armour, enchanted book) beside a plain book in a grindstone gives an enchanted
book with its non-curse enchantments. Mode `all` moves every one, mode `first` only the first (`config/homestead-grind.txt`:
`mode=first|all`, `cost=<levels>`, `enabled=false`). Taking the result leaves the item without what moved in the first
slot, uses the book, charges `cost` levels (default 1, not in creative) and awards no experience. It works the same in
the preview, the host's menu and a LAN guest's menu (the result is injected into the shared pure function).

**Differs from the original (from its public description).**

- There are no buttons; the mode comes from the config file, not from the screen.
- The cost is a flat level count, not per enchantment level, and cannot stop a player who has too few levels (the
  result slot cannot see the player): a player with fewer levels pays what they have. Stopping the take would need the
  grindstone slot's `mayPickup` to know the level.
- Curses stay on the item (vanilla grindstone behaviour); an item with only curses is not a candidate.
- The vanilla grindstone cases (two damaged items, one enchanted item alone) are untouched: the mod only answers an
  enchanted item together with a plain `minecraft:book`.

**Not verified.** The whole thing: the control socket has no menu-click command, so the acceptance test only opens the
grindstone screen. Manual check: put an enchanted sword in the top slot and a book in the bottom slot of a grindstone,
the result is an enchanted book; take it: the sword comes back (still in the top slot) without the enchantments, the
book is gone, one level is gone.

## Risks (what may not compile)

See `mixins/homestead/fragments/engine20.md`, section 4, for the list of things only a build can confirm.

## `homestead status`

A read-only operator command (module `homestead_status`, `status.rs`) printing the nick, game mode, difficulty, health, food, saturation, exhaustion, the movement state's `invulnerable` and `spectator` flags (they suppress food exhaustion), the server view of `invulnerable`/`may_fly`/`flying`, and the active effects with amplifier and ticks left. It exists so a play-test report about hunger or regeneration can be one pasted line.
