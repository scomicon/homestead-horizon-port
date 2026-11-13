# Homestead stage 6: small logic mods (`homestead` bundle)

A second native bundle, `mixins/homestead`, holds behaviour from small
permissively licensed Homestead mods. It shares the pinned Horizon revision and
the same build as the Create bundle.

## Right Click Harvest (JamCoreModding, MIT, 4.6)

The settings in this file come from the pack's own configs in the CurseForge
instance, not from each mod's defaults.

Module `harvest.rs`, hooked into `right_click_block`. The pack's
`rightclickharvest.json5`: no hoe required, radius harvesting on, hunger level
NORMAL, no experience cost.

- A mature wheat, carrot, potato, beetroot, nether wart or cocoa plant is reset to
  age 0 and its block loot is dropped minus one replant item (`dropStacks`).
- Hunger NORMAL: nothing is harvested at food level 0 (outside creative), and each completed harvest adds 0.008 exhaustion.
- Sneaking, and spectators, are ignored. Immature plants are left alone.
- With a hoe, radius harvesting follows the `rightclickharvest` tier tags: low
  (copper, iron) the four orthogonal neighbours, mid (gold, diamond) the 3x3,
  high (netherite) twelve neighbours within Manhattan distance two. Cocoa, sugar
  cane and cactus are excluded from radius harvesting. The hoe loses one durability.
- Sugar cane and cactus: a click breaks the segment above the base and keeps the
  base, unless the held item is the same block and the clicked face is up.

Verified in game (`stage4/Run-Acceptance.ps1 -Harvest`, PowerShell 7, disposable
world): bare-handed harvest with wheat drop and replant, immature wheat
untouched, iron hoe cross, diamond hoe 3x3, netherite hoe diamond of radius two,
sugar cane column.

### Known differences

- Crops are the vanilla list above; modded crops with an `age` property (Farmer's
  Delight and others) are not harvested yet.
- Tier tags list vanilla plus the hoes of mods present in the pack; other mods'
  hoes get no radius.
- Hoe wear is applied through the engine's ordinary damage path, but its exact
  amount was not asserted in the acceptance run. Break/place events for claim mods,
  harvest statistics, the planting sound and the config screen are not ported.
- The experience option is not ported (it is off in the pack).

## Secure Crops (zekk051)

The pack's world rules `secureFarmlandAndCrops` and `secureFarmlandIfEmpty` are
both true (set by Preferred Gamerules), so no trampling ever happens. The
`trample` event is cancelled for everything: players, mobs and falling entities;
fall damage is still taken. Verified: a player landing on a crop and on empty
farmland leaves both in place. The mod's own gamerules are not registered, so the
behaviour cannot be switched off in game; the third rule (`secureFarmlandBreakCrops`)
is off in the pack and is not modelled.

## Too Cheap! (MIT, 1.4)

The pack's `too-cheap.json` sets `maxLevelCost` to 2147483647 and leaves every
cost-rewriting flag off (vanilla repair, rename and enchantment costs, 12% anvil
break chance). The only change is that `anvil_result` is answered as if the
player were creative, which is the sole thing the 40-level limit depends on.
Verified: two swords with repair cost 63 give an anvil result at 128 levels.

Known difference: the anvil screen still draws "Too Expensive!" in red at 40
levels or more, because that text is drawn inside a function the mixin tool
cannot edit (it contains closures). The result can be taken regardless.

## Preferred Gamerules (Estecka, MIT) and Mob Explosion Griefing Gamerule

The pack's `preferred-gamerules.properties` lists `doFireTick=false` and
`mobExplosionDropDecay=false` (plus rules of mods not in this port). Both map to
vanilla rules this version already has: `fire_spread_radius_around_player` 0 and
`mob_explosion_drop_decay` false, so the Mob Explosion Griefing Gamerule mod
needs no code. The Create New World screen cannot be hooked here (the mixin tool
refuses the functions that build it), so the two values are applied to a world
whose game time is 0, and only to a rule still at its default. This is **not
verified in game**: new-world creation was not exercised.

## Method notes

- The mixin tool refuses `MODIFY_ARG`, `MODIFY_CONSTANT`, `RETURN` on `Self`
  and `AFTER_ASSIGN` inside functions that contain closures or `?`; a cancellable
  `HEAD` is accepted there. A trait impl method could not be named as a target.
- `trample` is a plain function, not a named hook; target it by path.
- The acceptance world persists between runs. A player killed in one run stays
  dead; delete `runtime/game/saves/HomesteadAutomationAcceptance` to re-extract.
  Note this also removes the Vault restart fixture (rerun `-Vault` before
  `-Vault -ReloadOnly`).
- `Game::native_block_drops` only lets mods adjust drops; it does not roll the loot
table. Use `Game::block_loot_at` and `Game::pop_resource` instead. A test must
freeze random ticks (`gamerule random_tick_speed 0`) so farmland is not removed,
and re-lay farmland after the player stands on it.
