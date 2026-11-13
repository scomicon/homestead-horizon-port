# Farmer's Delight feasts, pies, drinks and Comfort: shared-file edits for the lead

New files (paths relative to the create-source checkout):

- `mixins/homestead/feast.rs` (module `homestead_feast`), `feast_right_click.rs` (hook helper)
- `mixins/homestead/comfort.rs` (module `homestead_comfort`), `comfort_tick.rs` (hook helper)
- `mixins/homestead/fragments/feasts.toml` (2 modules + 2 injections)
- `fd/feast.py` (content: blocks, items, recipes, effect, drink components, language)
- `docs/farmersdelight-feasts.md`
- `stage4/acceptance/Test-HomesteadFeasts.ps1` (15 scenarios)

Nothing was built, run or committed. No change to any existing Rust module is needed.

## 1. `mixins/homestead/horizon-mixin.toml`

Append the whole of `fragments/feasts.toml` at the end. Bump the version from `0.1.0+homestead.7`
to `0.1.0+homestead.8`. The injection `feast-right-click` is a second injection on
`right_click_block` (like `controller-bind` in the create bundle); it sits after `right-click`,
which only acts on stoves, skillets, pots, boards and harvestable crops, so none of them conflict
with feasts, pies or the cake. If a vanilla cake eats before the hook runs, move the injection
before `id = "right-click"` or add the call at the top of `right_click.rs`:

```rust
    if !*canceled && crate::game::homestead_feast::right_click(game, pos) {
        game.swing_arm();
        *canceled = true;
        return;
    }
```
(then drop `feast_right_click.rs` and the injection).

## 2. `fd/overlay.py`

After `cooking_pot.patch(out)` (line 225), same indentation:

```python
    spec = importlib.util.spec_from_file_location('fd_feast', Path(__file__).with_name('feast.py'))
    feast = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(feast)
    feast.patch(out)
```

`feast.patch` needs the item loop to have run first (it edits `milk_bottle`, `hot_cocoa`,
`melon_juice`) and the lang file to exist; both are true at that position.

## 3. Acceptance wiring

`stage4/Run-Acceptance.ps1`:

- `param(...)`: append `,[switch] $Feasts`.
- `$reportName=` chain: start with `if ($Feasts) {'run-feasts-result.json'} elseif (...)`.
- `$runs` chain: add
  ```powershell
  } elseif ($Feasts) {
      $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadFeasts.ps1'),'Feasts','stage24-feasts-'))
  ```
  next to the other FD branches (same shape as `-Skillet`).

`stage4/regress.ps1`: append `,@('feasts',@{Feasts=$true})` to `$sets`.
`stage4/Build-Package.ps1`: add `'run-feasts-result.json'` to `$runs`, a provenance line
"Farmer's Delight: feasts, pies, drinks, Comfort (stage 24)" and the next package name.

## 4. Docs

`stage4/research/roadmap.md`: a row for the slice ("Farmer's Delight, slice 5: feasts, pies, drinks,
Comfort; acceptance `-Feasts`"). `docs/farmersdelight-stage8.md`: a pointer to
`docs/farmersdelight-feasts.md` (the "not done: feasts" line).

## 5. Risks (never compiled)

- `NativeEffect` is reached as `super::native_player_state::NativeEffect`; if that module is
  private to a deeper path, import it from where `native_add_effect` is declared.
- `held.is_none_or(..)` needs Rust 1.82+ (the toolchain is 1.99). `game.native_action_bar`,
  `Text::translate`, `native_set_block_property(pos, &str, &str)` and `native_set_block` are used
  as `skillet.rs` and `harvest.rs` use them.
- `player.health.food.needs_food()` / `.eat()` and `player.health.set_health(f32, bool)` exist
  (`horizon-world/src/food.rs`, `horizon-client/src/health.rs`); the effect `duration` field is
  assumed to be an integer widening to `i64`.
- The recipe of the stuffed pumpkin is an estimate (the 1.20 file could not be fetched). The
  recipe tags (`forge:milk`, `forge:crops/onion`, ...) are replaced by items.
- `minecraft:remove_effects` takes an array of effect ids in `on_consume_effects`; the loader
  accepts the type (`horizon-items/src/items.rs:108`). A wrong shape fails the content mod load,
  which `-ReloadOnly`/`-FarmersDelight` shows at once.
- The Comfort test reads the effect from `state`; if `state` carries no effects, replace the check
  by `effect clear` returning a success line.
