# Logic mods group D: SpawnFilter, Universal Graves, Grind Enchantments

Native Homestead bundle code (`mixins/homestead/`). Sources: the three jars and configs of the player's own
Homestead instance (`config/spawnfilter.toml`, `config/universal-graves/config.json`,
`config/grindenchantments.json`) and the Modrinth descriptions in `pack/modrinth.json`. The jars are
Fabric mods (SpawnFilter CC0, Universal Graves LGPLv3, Grind Enchantments LGPL-3.0-or-later); the Java code
itself was not decompiled, only its constant pools and the configs were read. Nothing here was built or run.

## SpawnFilter 1.1.0: ported (module `homestead_spawnfilter`)

What the mod does: a player must not spawn on a blocked block or in a blocked biome; a blocked spawn is moved
to the nearest safe spot, and if none is found a stone platform is built under them.

The pack's config, as it ships (copied into constants of `spawnfilter.rs`; the file is not read at run time):

| Key | Value |
| --- | --- |
| `blocked_blocks` | water, lava, magma block |
| `blocked_tags` | `#minecraft:fire`, `#minecraft:wool`, `#c:fluids`, `#minecraft:leaves` |
| `blocked_biomes` | none |
| `blocked_biome_tags` | `#minecraft:is_ocean`, `#minecraft:is_deep_ocean`, `#minecraft:is_river` |
| `max_search_radius` | 1024 |
| `build_safety_platform` | true, block `minecraft:stone` |

Hooks: `player_respawned` (after death) and `player_logged_in`. A login is only looked at for a player without the
persistent-data marker `homestead_spawnfilter`, and only while the world is younger than 12000 ticks, so a
brand-new world's first spawn is covered and an old world's players are never moved because of this module. The
check reads the block under and in the player's feet and the biome there; a blocked one starts a ring search
(every 8 blocks, nearest first, over the **loaded** columns: `native_surface_heights` gives `None` for a column that
is not loaded) for a column with solid, unblocked ground, two free blocks above and an unblocked biome, and
`native_teleport_player`s the player there. With nothing found, the 3x3 blocks under their feet become stone where
they are air or blocked.

Differences: the original changes where vanilla places the player; the port moves them after they are placed
(one frame at the first position may show). The biome tags are spelled out (the engine has no biome tag query).
`#c:fluids` only matches where the server's block tag table has it. The search is bounded by what is loaded, so
a spawn far from every loaded column in a blocked area may get the platform instead of a safe spot. A first join
into a world older than 12000 ticks is not checked. The control socket cannot respawn the player, so none of this is
covered by the acceptance script. **Manual check:** `/kill`, respawn at a world spawn in the ocean (or set the
spawn with `/setworldspawn` on a wool block), and see where the player appears.

## Universal Graves 3.0.3: ported in part (module `homestead_graves`, content mod `universal_graves`)

The pack's config: protection of 900 s for non-owners, no self destruction, no expiry drop, free unlock, death
compass given, click opens the GUI, shift-click quick pickup, experience stored (percent points, 100 %), no
grave limit, `replace_any_block` true, max distance 16, teleport cost "creative".

Ported:

* On death (no keepInventory), `loot_drops` for the dying player offers the whole inventory (main slots, armour,
  off hand, mod additions). It goes into a **grave block** `universal_graves:grave` placed at the nearest air, plant or
  fluid block within 16 blocks of the death position (and within the world's height). The block's entity keeps up to
  54 stacks (a vanilla 9x6 menu), the owner (two longs) and the world time of creation.
* For 900 s (18000 ticks, world time) only the owner and operators (permission level 2 or more) may open or
  break it (`right_click_block` and `destroy_block` hooks); after that anyone may. It never destroys itself.
* Breaking it drops its items (the engine drops a mod container's items); the block itself drops nothing and has no
  item.
* If no spot is found, the block cannot be set, or no block entity appears, the grave is withdrawn and the drops
  fall as in vanilla.

Not ported: experience storage, the death compass, the `/graves` commands and the GUI (list of graves, remote
teleport, fetch, remote break, remote unlock, unlocking cost), the hologram and the player-head or corpse models
(`config/universal-graves/example_models`), the shift-click quick pickup, the grave limit, blocked areas, worlds and
damage types, restoring the replaced block (the grave replaces plants and fluids only, never solid blocks, where
the original's `replace_any_block` replaces anything), grave generation on the ground, the real-time switch, the
attacker bypass. The look is a placeholder cube (polished blackstone bricks).

Build: `stage4/Build-Graves.ps1` runs `tools/graves_content.py` and zips `universal_graves-3.0.3-homestead.1.hmod`
into `stage4/extra-mods` (the name is outside `Build-DataMods.ps1`'s cleanup pattern). The package build needs to
copy it into `mods/` like the other `.hmod` files.

Unverified: that `native_set_block` creates the block entity at once (a failure withdraws the grave); that the
generic `minecraft:generic_9x6` menu works as a mod container's menu (the engine's `mod_menu` test lists
`generic_9x3`); that `native_acting_player` is the player who clicked or broke; the death path has no acceptance
test (the control socket cannot respawn). **Manual check:** die with items, find the grave, open it as another
player within 15 minutes (refused) and as the owner (opens).

## Grind Enchantments 3.1.4: not ported

What the mod does (from `config/grindenchantments.json` and the class list of the jar): the Grindstone gets two
new operations on top of vanilla's.

* **Disenchant to book** (`disenchant_to_book`, on): an enchanted item in one slot and a plain **book** in the
  other give an **enchanted book** with all the item's enchantments (curses too, `allow_removing_curses` on). The
  enchanted item is kept without its enchantments (`consume_enchanted_item` off), the book is used up. It costs
  levels: `transform(count_min_power, factor 0.3, offset 8.0)`.
* **Move enchantments** (`move_enchantments`, on): the **first** enchantment of the item in one slot moves to the item
  or book in the other (if it can take it). The cost is
  `transform(first_enchantment(count_levels(normal 3.0, treasure 8.0)), factor 0.5, offset 0.5)` levels.
* A client option shows the level cost on the grindstone screen (`show_enchantment_cost`); creative players pay
  nothing.

Why it is skipped: the grindstone is a client-side `menu.rs` menu with no mod hook. What can be injected by
function target is `stations::grindstone_accepts` (the slot filter: it would have to accept a book) and
`stations::grindstone_result` (the result stack). What cannot be reached without engine changes:

* the **take** of the result (`menu.rs`, the `Role::GrindResult` branch of the click code) clears both inputs and
  awards XP inline; the port needs the enchanted item to stay (stripped) and the book to go, or the moved
  enchantment to leave one slot and land in the other,
* the **level cost**: the result slot would have to refuse a take when the player has too few levels and to charge
  them (the anvil's flow has `cost`, the grindstone's has none),
* the screen's cost line.

Doing only the filter and the result would show a result whose take then destroys the item: wrong. The honest
route is an upstream pull request: a `grindstone` station hook with `accepts`, `result`, `take(inputs) -> remaining
inputs` and `cost` contracts (like the anvil's `anvil_result` and `AnvilResult::answer_update`), with a regression
test. When that exists the port is small: `DisenchantOperation` and `MoveOperation` are 200 lines of Java each and
the two cost functions above are plain arithmetic (`count_min_power`: sum of the enchantments' minimum cost at
their level; `count_levels`: levels times the normal or treasure factor; `first_enchantment`: the first one only;
`transform`: `factor * x + offset`, rounded).

## Acceptance

`Test-HomesteadLogicD.ps1` (`-LogicD`, prefix `stage25-logicd-`): the grave places, has 54 slots, keeps items, an
ownerless grave breaks and drops once, an operator breaks a protected grave and an expired one. Needs the
`universal_graves` mod in `extra-mods` (`Build-Graves.ps1`). SpawnFilter, the death path and Grind Enchantments have
no automated check.
