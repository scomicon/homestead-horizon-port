# Logic mods, group B

Panda's Falling Tree's is ported. Bridging Mod, Comforts and SimpleHats are not: each needs
something the engine's native hooks do not give a mod today. This page says what was done, what
was left out and what would unblock the rest. Nothing here has been compiled or run yet.

Sources: the pack's jars (`fallingtrees-fabric-mc1.20-0.13.2`, `BridgingMod-2.5.1`,
`comforts-fabric-6.4.0`, `simplehats-fabric-1.20.1-0.4.0a`), read as class names, strings and
language files; no code of any of them was used. The engine facts below come from
`native-hooks.json`, `docs/native-mods.md` and the game's sources.

## Panda's Falling Tree's (done, `mixins/homestead/falling_trees.rs`)

A player breaking a log with the `destroy_block` hook (`BlockEvent.BreakEvent`) fells the rest of
the tree: the logs joined to it through their 26 neighbours and never below the broken one, then
the leaves held up by those logs (flood through leaf blocks up to distance 7). The blocks break
through `native_machine_destroy_block` with the tool in hand, so loot tables, Fortune/Silk Touch
and `doTileDrops` apply as for a machine; the broken log itself is broken by the game afterwards.

Defaults used (the mod's configuration names are in its language file; the default values are
recalled, not read from the jar):

| Setting | Value |
| --- | --- |
| Tree type | generic: stems `#minecraft:logs`, leaves `#minecraft:leaves` |
| Persistent leaves | ignored (a log house with player-placed leaves is not a tree) |
| A tree needs leaves | yes; a pillar or wall of logs is not felled |
| Max logs | 500 (a bigger tree stands) |
| Crouch mining | sneaking breaks only the block (the original's default) |
| Tool required | no (any tool or the bare hand) |
| Tool damage / food exhaustion | multiplied by the logs felled: the extra logs cost `extra` durability and `0.005 * extra` exhaustion, none in creative |
| Creative | the tree falls, nothing drops |

Not ported:

- The falling tree **entity** and its fall/bounce animation, the impact sounds and the client
  settings (`fallAnimLength`, volumes, `invertCrouchMining`). The tree is gone at once. The engine
  has no hook to draw a block structure as an entity with an animation (`mob_layer` and
  `player_render_listened` only add layers to mobs and players).
- The dynamic mining speed that slows a thick trunk (no hook changes a player's digging speed).
- The vertical tree types (cactus, bamboo): their support rules already drop the whole column;
  only the animation is missing. Chorus and huge mushroom trees are left out too.
- The config files and `treeLifetimeLength`.

Known differences: a canopy shared with a neighbouring tree loses the shared leaves (the flood
does not know whose leaf it is); a single player is assumed (`game.held()` and `native_sneaking()`
read the local player, like the other homestead helpers); the first break by a sneaking LAN guest
is not told apart from the host's.

## Bridging Mod (not ported)

Client-only (`server: unsupported`). Its three mixins are `MinecraftClientMixin` (it replaces the
ray trace the use key places against, so a block can be placed along the edge of the one you stand
on without looking at its face), `OutlineRendererMixin` and `CrosshairRenderingMixin` (the ghost
outline and the crosshair change). The engine has no client hook that changes the pick result,
the block outline or the crosshair: the client hooks are `interaction_key`, `key`, `mouse_*`,
`render_hand`, `gui_layer_*` and `frame` (listen only). The server hooks `use_item` and
`use_item_on` see only what the vanilla pick already chose, and without the ghost the player cannot
tell where the block would go. A server-side "place ahead" on an air click would be a different
feature, not this mod, so it is not added.

Unblocked by: a client hook that lets a mod replace `Minecraft.hitResult` (or a pre-use hook with
the look ray), and `block_outline` / `crosshair` draw hooks.

## Comforts (not ported)

Sleeping bags and hammocks in 16 colours that work like beds without setting the spawn point,
with their own time rules. The blocks are easy to declare (a bed template gives `Class::AbstractBed`
use, `facing`, `part`, `occupied`, and `horizon-port-pack` style content), but the game's bed code
rules the behaviour out for a mod:

1. **The sleep hooks do not fire for the local player.** `try_sleep` (game.rs) calls
   `native_can_player_sleep` and `native_player_spawn_point_set` only inside
   `if let (Some(player), Some(name)) = (self.lan_acting_uuid, self.lan_acting_name.clone())`,
   which is a LAN guest's turn; the host (and a single-player game) has `lan_acting_uuid == None`
   (`acting_player_uuid` falls back to the host). `player_bed_enter` has the same guard. So a mod
   can neither let a bag be slept in by day, nor keep the spawn point, for the player it matters
   to most. Only `sleep_finished_time` is unconditional.
2. **A bed is a block whose name ends in `_bed`.** `is_bed`, `tick_sleep` (`bed_gone`) and
   `bed_wake_pose` test `name.ends_with("_bed")`: a block named `comforts:sleeping_bag_red` wakes
   the sleeper on the next tick and gets no stand-up pose. Comforts' ids would have to change to
   `*_bed`.
3. **The time rule is the dimension's** (`bed_rule.can_sleep`), not the block's, and the original's
   `hammockUse` / `sleepingBagUse` / wake-time offsets, `restrictSleeping`, `sleepingBagEffects`
   and break chance have no hook to hang on.
4. Hammocks need two Rope and Nail blocks to hang between (`block.comforts.hammock.no_rope`), the
   sleeping bag item auto-uses when placed and stacks, and both are drawn by block entity
   renderers over the bed entity model. None of that has a hook either.

Unblocked by: an engine patch (`stage4/engine-patches`) that (a) calls the three sleep hooks for
the host as well, with `lan_acting_uuid` replaced by `acting_player_uuid()`, and (b) asks the
bed's block for its `bed_rule` through a data-driven attribute instead of `ends_with("_bed")`. With
that, the bags and hammocks are a content mod (`pack/mods/comforts/carry` style: bed-template
blocks with wool-textured low boxes, recipes from the original, the `minecraft:beds` tag) plus a
~150 line native module.

## SimpleHats (not ported)

Hats are `accessories` slot items (the Accessories mod's `hat` slot, `data/accessories/tags/
items/hat.json`), drawn by `HatLayer` on the player and by a `HatDisplay` entity, and their
definitions are read at runtime from config JSON (`HatJson`), not from data. The engine has no
accessory slots (no hook adds an equipment slot or a slot to the player inventory screen), no mod
entity type with a custom renderer, and no layer on the player model that draws an item model
(`player_render_listened` only listens). A hat worn as a helmet would be a different mechanic and
would still need the layer.

Unblocked by: a data-driven player model layer (item model on a named bone), and an accessory or
extra equipment slot in the player inventory.

## Tests

`stage4/acceptance/Test-HomesteadLogicB.ps1` (`-LogicB`) places trees with `fill`, breaks the base
with the control socket's `break` in survival and checks the blocks, the drops, the axe damage and
the creative case. Sneak mining cannot be driven (the socket has no sneak key) and is not tested.
The axe-damage check reads `data get entity @s SelectedItem`; if the game prints the damage
component differently the expectation needs adjusting on the first run.
