# Logic mods group D (SpawnFilter, Universal Graves): shared-file edits for the lead

New files (paths relative to the create-source checkout unless marked ST = `work/homestead-horizon/stage4`):

- `mixins/homestead/spawnfilter.rs` (module `homestead_spawnfilter`), `spawnfilter_respawn.rs`, `spawnfilter_login.rs`
- `mixins/homestead/graves.rs` (module `homestead_graves`), `graves_entity.rs`, `graves_container.rs`,
  `graves_loot.rs`, `graves_right_click.rs`, `graves_destroy.rs`
- `mixins/homestead/fragments/logicD.toml` (2 modules, 7 injections), this file
- `tools/graves_content.py`, `docs/logic-d.md`
- ST `Build-Graves.ps1`, ST `acceptance/Test-HomesteadLogicD.ps1`

Nothing was built, run or committed. Grind Enchantments is documented and skipped (`docs/logic-d.md`).

## 1. `mixins/homestead/horizon-mixin.toml`

Append `fragments/logicD.toml` and bump `version` (the file shows `0.1.0+homestead.8` after the crops/feasts slice;
use the next one). Ids are unique (`spawnfilter`, `graves`, `spawnfilter-*`, `graves-*`). Hooks used:
`player_respawned`, `player_logged_in`, `block_entity`, `block_entity_container`, `loot_drops`, `right_click_block`,
`destroy_block`; all but the first two and `loot_drops` already carry other injections.

## 2. Content mod

Run `ST/Build-Graves.ps1` (needs the same `create-source-path.txt` and python as `Build-FarmersDelight.ps1`); it
writes `stage4/extra-mods/universal_graves-3.0.3-homestead.1.hmod` (outside `Build-DataMods.ps1`'s
`*-horizon.*.hmod` cleanup). Call it from the build chain before `Prepare-Candidate` or whenever the `.hmod` files are
gathered, and make `Build-Package.ps1` copy the archive into the package's `mods/` and `native-mods.json`-style
digest list like the other `.hmod` files (check how the data mods are listed and mirror it). Without the archive the
grave block does not exist: the death hook then finds no state for `universal_graves:grave` and leaves the drops alone.

## 3. Acceptance wiring

`ST/Run-Acceptance.ps1`:

- `param(...)`: add `,[switch] $LogicD`.
- `$reportName=` chain: start with `if ($LogicD) {'run-logicd-result.json'} elseif ` (the same shape as the other switches).
- add a branch (same layout as `-Feasts`):
  ```powershell
  } elseif ($LogicD) {
      $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadLogicD.ps1'),'LogicD','stage25-logicd-'))
  ```

`ST/regress.ps1`: append `,@('logicd',@{LogicD=$true})` to `$sets`.

`ST/Build-Package.ps1`: add `'run-logicd-result.json'` to `$runs` and a provenance line
"Logic mods: Universal Graves (grave block on death, 900 s protection), SpawnFilter (blocked spawn moved)".

## 4. Risks (never compiled)

- `crate::containers::ModContainer { slots, faces, inputs, outputs, menu }` and `ModBlockEntity.items/nbt/id`: as `cooking_pot.rs`.
- `horizon_client::mod_api::NativeLootDrops` is named as `game.rs` names it; the helper in `graves_loot.rs` spells it the
  same way. If the helper's crate path differs, use `crate::mod_api::NativeLootDrops` (the injected code lives in `game.rs`).
- `game.native_server_players()`, `native_teleport_player`, `native_surface_heights`, `native_biome`,
  `native_server_block_in_tag`, `native_player_persistent_data`, `native_set_player_persistent_data` are used as their
  doc comments in `native_*.rs` describe; `NativeHeightmap` comes from `super::native_world_edit` (a sibling private module).
- `game.world.game_time` is read as an integer (`display_source.rs` casts it to `f64`).
- A block tag name may need the `#` or not for `native_server_block_in_tag`; `block_tags.get(tag)` in
  `respawn_anchor.rs` uses names without `#`, which is what `spawnfilter.rs` passes.
- The hook binding values (`context`, `drops` for `loot_drops`; `player` for the two player hooks) are the locals of
  the hooked functions (`native_loot_drops(&mut self, context, drops)`, `native_player_respawned(&mut self, player, name)`,
  `native_player_logged_in(&mut self, player, name)`).
- `loot_drops` also runs for block and mob drops: the helper returns at once unless the entity is `minecraft:player`
  and `used` is false and `table` is empty.
- The barrel template's `facing`/`open` properties are not set by `native_set_block` (default state): harmless.
- `Test-HomesteadLogicD.ps1`: `item replace block ... container.40` on a 54-slot mod container, `data merge block` of the
  owner longs (mod block entity NBT merge), and the control socket's `container` reply having a `slots` array are
  assumed as `Test-HomesteadPot.ps1` uses them. The test cannot cover the death path or SpawnFilter.
