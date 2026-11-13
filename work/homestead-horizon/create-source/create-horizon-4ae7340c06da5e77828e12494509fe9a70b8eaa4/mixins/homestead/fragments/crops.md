# Farmer's Delight crops (stage 16 slice): shared-file edits for the lead

New files (paths relative to the create-source checkout `CS`, and to `work/homestead-horizon/stage4` = `ST`):

- `fd/crops.py` (content: blocks, planting items, tags, wild rice loot; runs inside the overlay)
- `mixins/homestead/crops.rs` (module `homestead_crops`), helpers `crops_tick.rs`, `crops_use.rs`
- `mixins/homestead/fragments/crops.toml` (1 module + 2 injections)
- `docs/farmersdelight-crops.md`
- `ST/acceptance/Test-HomesteadCrops.ps1` (15 scenarios, prefix `stage24-crops-`)

Nothing was built, run or committed.

## 1. `fd/overlay.py` (call the new patch)

In `main()`, right after the `cooking_pot.patch(out)` line and before `effects = out / ...`:

```python
    spec = importlib.util.spec_from_file_location('fd_crops', Path(__file__).with_name('crops.py'))
    crops = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(crops)
    crops.patch(out)
```

It must run after the item loop (it merges `place_block` into the rewritten item JSON) and after the carry.
Also add `'farmersdelight:rice'` to nothing else: the overlay's `FOODS` already has no rice grain.

## 2. `ST/Build-FarmersDelight.ps1`

Line 12: `$version = 'horizon.7'` becomes `'horizon.8'`. The archive name `farmersdelight-2.4.1-horizon.8.hmod`
is found by glob elsewhere (no other script spells the old name). Rebuild it with this script before `build.ps1`
(it needs the player's resources and the Horizon source tree; the old `.hmod` in `extra-mods` is deleted by the script).

## 3. `mixins/homestead/horizon-mixin.toml`

- Append `fragments/crops.toml` at the end (1 `[[module]]`, 2 `[[injection]]`, ids `crops`, `crops-tick`, `crops-use`).
- Bump `version = "0.1.0+homestead.7"` to `"0.1.0+homestead.8"`.

## 4. `mixins/homestead/harvest.rs` (Right Click Harvest knows the new crops)

In `fn crop(...)`, add before `_ => return None,`:

```rust
        "farmersdelight:cabbages" => (7, "farmersdelight:cabbage_seeds"),
        "farmersdelight:onions" => (7, "farmersdelight:onion"),
        "farmersdelight:tomatoes" => (3, "farmersdelight:tomato_seeds"),
        // The panicles drop whole (no replant item to take back): the id is a block, never a drop.
        "farmersdelight:rice_panicles" => (3, "farmersdelight:rice_panicles"),
```

## 5. `mixins/homestead/rain.rs` (Crops Love Rain for them)

In `fn max_age(...)`, add before `_ => None,`:

```rust
        "farmersdelight:cabbages" | "farmersdelight:onions" => Some(7),
        "farmersdelight:budding_tomatoes" | "farmersdelight:tomatoes" | "farmersdelight:rice_panicles" => Some(3),
```

(Optional. The rain tests do not cover them.)

## 6. Acceptance wiring

`ST/Run-Acceptance.ps1`:
- line 5, `param(...)`: append `,[switch] $Crops` before the closing parenthesis.
- line 20, `$reportName=`: start the chain with `if ($Crops) {'run-crops-result.json'} elseif ($Controller) ...`
  (the existing leading `if ($Controller)` becomes `elseif`).
- in the `$runs` chain, before `} elseif ($Stock) {` (line ~55) insert
  ```powershell
  } elseif ($Crops) {
      $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadCrops.ps1'),'Crops','stage24-crops-'))
  ```

`ST/regress.ps1`, line 3, in `$sets=@(...)`: append `,@('crops',@{Crops=$true})` after the `stock` entry
(28 suites when stage15's 27 are in).

`ST/Build-Package.ps1`: add `'run-crops-result.json'` to the `$runs` list (line 20), a provenance line
`'Farmer''s Delight: crops, rich soil, organic compost, rice (stage 24)'` next to the stage 22 line (line ~128), and set the
default package name to `homestead-horizon-stage16`. The package must carry the rebuilt FD `.hmod`
(`horizon.8`); `Prepare-Candidate.ps1` picks up `extra-mods\*.hmod`.

## 7. Docs

Optional: a roadmap row "Farmer's Delight: посевы (капуста, лук, помидоры, рис), богатая почва, компост;
приёмка `-Crops`" and a pointer from `docs/farmersdelight-*.md`.

## 8. Risks (never compiled or run)

- `data/minecraft/tags/block/supports_crops.json` and `mineable/shovel.json` are written by the overlay; the content
  loader allows mods to add to vanilla tags, but whether `supports_crops` (read by the survival rules at startup) merges
  mod entries is not verified. If crops are refused on rich soil farmland, that merge is the first suspect (fallback:
  put the check in a native `can_survive` hook).
- Block templates: `minecraft:seagrass` for rice (placement needs water at the clicked cell, as the vanilla item does)
  and `minecraft:dirt` with declared `moisture` / `composting` for the soil blocks. `random_ticks: true` on a dirt
  template is assumed to fire `native_random_tick`.
- `native_level_random`, `block_light`, `sky_light`, `sky_darken`, `facts.is_air`, `behaviours.set_block` and
  `registry.default_state` are used as `rain.rs` uses them; `info.name.to_string()` assumes a `Display` name.
- `place_block` on the seed items: `ItemFile.place_block` accepts "this namespace's block"; the onion keeps its food
  components, so an onion held against empty air is still eaten.
- Growth rates (rice 1 in 5, rich soil 1 in 3, compost 1 in 3 / 1 in 8) are estimates, not read from the Java.
- Carried blockstates are matched by property: `budding_tomatoes` has an `age=4` variant that is never used; wild rice
  has an `upper` half that is never placed; wild crops and the compost model have no state beyond those.
- The `rich soil speeds up growth` scenario is statistical (10 plots each at random tick speed 100, 300 ticks); it can
  fail on a bad draw. Rerun once before treating it as a real failure.
