# Engine-patch mods (stage 27 "engine20"): shared-file edits for the lead

Slice: three small local engine patches and the mods that stand on them: Comforts (sleeping bags, hammocks), Peaceful
Hunger, Grind Enchantments. Nothing here was built, run or committed. The patch scripts were dry-run on copies of
the real pinned engine files (anchors match, a second run changes nothing) and the new Rust files pass
`rustfmt --check`; no compiler ran.

New files (CS = `work/homestead-horizon/create-source/create-horizon-<rev>`, ST = `work/homestead-horizon/stage4`):

- ST `engine-patches/20-sleep-host.ps1`, `21-peaceful-hunger.ps1`, `22-grind-take.ps1` (picked up by `build.ps1`'s
  `foreach ... engine-patches -Filter *.ps1`, which runs after `horizon-mixins apply`)
- CS `mixins/homestead/comforts.rs`, `comforts_spawn.rs`, `peaceful_hunger.rs`, `peaceful_hunger_tick.rs`,
  `grind.rs`, `grind_result.rs`, `fragments/engine20.toml` (this slice: 3 modules, 3 injections)
- CS `tools/comforts_content.py`, `docs/comforts-grind.md`; ST `Build-Comforts.ps1`,
  ST `acceptance/Test-HomesteadEngine20.ps1`

## 1. `mixins/homestead/horizon-mixin.toml`

Append `fragments/engine20.toml` and bump the bundle version (now `0.1.0+homestead.10`; use `.11`). Ids and module
names do not clash with the bundle (checked). The first two modules are declared in `game.rs` like the others; the
third, `homestead_grind`, in `crates/horizon-client/src/stations.rs` (the library crate, where `menu.rs` and the
pure item logic live). Other bundle modules already use files other than game.rs (`save.rs`,
`sound_playback.rs`, `game/machines.rs`), so a different `file` is supported; if the tool refuses `stations.rs`,
check how the `sound_playback.rs` module is declared and mirror it.

Ordering: the patches are applied by `build.ps1` AFTER `horizon-mixins apply`. Patch 22 refuses to run unless the
module `homestead_grind` is already in `stations.rs` (it throws "needs the homestead bundle") and the menu.rs patch
calls `crate::stations::homestead_grind::plan`, so the bundle and patch 22 ship together; dropping the grind module
from the toml means deleting `22-grind-take.ps1` too. Patches 20 and 21 do not depend on the bundle on their own
(21 adds `hunger::PEACEFUL_HUNGER`, default false = vanilla; 20 changes only guards and `is_bed`).

## 2. Content mod

Run `ST/Build-Comforts.ps1` before `Prepare-Candidate.ps1` (like `Build-Graves.ps1`): it writes
`stage4/extra-mods/comforts-0.1.0-homestead.1.hmod` (32 blocks and items). `Start-Test.ps1` loads every `.hmod` from
`extra-mods`; `Build-Package.ps1` copies `extra-mods/*.hmod` into `mods/` already. Make sure `Build-DataMods.ps1`'s
cleanup does not delete it (its pattern is `*-horizon.*.hmod`; this name is safe). Without the archive the
sleeping-bag blocks do not exist and the Comforts scenarios fail on "Unknown block".

## 3. Acceptance wiring

`ST/Run-Acceptance.ps1`: add `,[switch] $Engine20` to `param(...)`; start the `$reportName=` chain with
`if ($Engine20) {'run-engine20-result.json'} elseif ` ; add a branch (same layout as `-LogicD`):

```powershell
} elseif ($Engine20) {
    $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadEngine20.ps1'),'Engine20','stage27-engine20-'))
```

`ST/regress.ps1`: append `,@('engine20',@{Engine20=$true})` to `$sets`. `ST/Build-Package.ps1`: add
`'run-engine20-result.json'` to `$runs` and a provenance line "Engine patches: sleep hooks for the single-player
host, bed by state data, Peaceful Hunger switch, grindstone take; Comforts, Peaceful Hunger, Grind Enchantments".

`ST/Start-Test.ps1`: it already removes `config/homestead-*.txt` before runs (stage 26 edit); keep that so a
`homestead-grind.txt` or `homestead-hunger.txt` left behind does not change other suites.

## 4. What may not compile or anchor (never built)

- **Patch 20** edits three calls in `try_sleep` (game.rs), `is_bed` (respawn_anchor.rs) and two wake-up checks
  (game.rs `tick_sleep`, lan_actions.rs). The anchors were checked against the pinned engine
  `4e5296d1800b3f89f2ae48dfd99d431801df4d46`; a different engine revision fails loudly ("anchor missing").
  `native_acting_id()` is `&mut self` and `acting_player_name()` is `&self`: both are used one after the other in
  statements, not inside one expression. In the spawn-point `if` chain they sit in a tuple expression
  `(Some(self.native_acting_id()), Some(self.acting_player_name().to_owned()))`: two sequential method calls on
  `self`, the first returns a plain u128, which compiles; if the borrow checker objects, hoist them into locals.
- **Hook dispatch for the host:** `dispatch_can_player_sleep` and the two others take `(player, name, ...)` and call
  mods; they do not look the player up in the LAN hub. The doc comments say the player's state "is swapped in" for a
  guest packet; for the host nothing is swapped, which is what the helpers want. Unverified in a game.
- **`is_bed` by state data:** `registry.state(state)` returns the state with a `properties` map (used the same way in
  `try_sleep` and `beds.rs`); `contains_key("part")` on a `HashMap<String,_>`/`BTreeMap<String,_>` compiles either way.
  Villagers, raids and the straw-bed code still use the `_bed` name suffix, so villagers do not claim a sleeping bag.
- **`sleeping_position(head)`** (`bed_geometry::slept_on`) decides where the player lies; it was not read for a bed
  whose shape is not vanilla's. If it returns None for a bag the player stays up (`try_sleep` returns): then the
  `occupied=true` scenario fails and the bag needs a shape that matches the bed template's.
- **Patch 22** needs `Menu::creative`, `Menu::levels`, `Menu::level_change`, `Menu::put` (all seen in menu.rs) and
  `env.enchantments: &Enchantments` (it is passed to `grindstone_xp_base` just above). `Plan.remaining` is moved into
  `put`.
- **`grind.rs`** uses `ItemStack::{count, item, has_any_enchantments, crafting_enchantments,
  set_crafting_enchantments, stored_enchantments, transmute, tidy, clone}` and `Enchantments::is_curse`, every one
  of them as `stations.rs` uses them. `transmute("minecraft:enchanted_book", 1)` assumes the count parameter infers
  from an integer literal (stations.rs passes `item.count`). That `set_crafting_enchantments` on an enchanted book
  writes the stored enchantments is inferred from `remove_non_curses`; if it does not, the result book carries none.
- **The injection into `stations::grindstone_result`** returns `ControlFlow<Option<ItemStack>>` like the
  `anvil_result` injection returns `ControlFlow<AnvilResult>`. The `target` string
  `horizon_client_lib::stations::grindstone_result` follows `anvil-limit-menu`.
- **`homestead_peaceful_hunger`** reaches `horizon_client::hunger::PEACEFUL_HUNGER` the way game.rs reaches
  `horizon_client::mobs`; `hunger` is `pub mod` in the library.
- **Guests (LAN):** the grindstone result and the take run in the shared `Menu`, so a guest gets the same behaviour
  (the level cost reaches them through `lan_stations.rs`'s `level_change`). The experience orbs are suppressed
  (`ground = Some(0)` still plays the grindstone sound).
