# Logistics slice: edits to apply to shared files

New files of this slice (all under `mixins/create/` unless noted): `package_runtime.rs`,
`packager_runtime.rs`, `gauge_runtime.rs`, the hook helpers `packager_entity.rs`,
`packager_ticks.rs`, `packager_tick.rs`, `packager_update_tag.rs`, `packager_draw.rs`,
`packager_container.rs`, `packager_right_click.rs`, `packager_destroy.rs`,
`packager_placed.rs`, `gauge_signal.rs`, `gauge_source.rs`, `fragments/logistics.toml`;
`tools/logistics_content.py`; `stage4/acceptance/Test-HomesteadPackager.ps1`.

All hook helpers are *additional* helpers on hooks that other injections already use
(the homestead bundle does the same for its pots), so no existing helper is edited.
Hook helper function names: `create_logistics_*`, `create_gauge_*`.

## 1. horizon-mixin.toml

Append the whole of `fragments/logistics.toml` at the end of `horizon-mixin.toml`
(3 `[[module]]` + 12 `[[injection]]`, ids prefixed `logistics-`, `gauge-`, `package-`,
`packager-`). No existing entry changes. The three module names are
`create_port_package`, `create_port_packager`, `create_port_gauge`.

The engine crate already declares `package`, `packager`, `stock_network`, `factory_gauge`,
`package_post` in `engine/src/lib.rs`; nothing to add there.

## 2. arm_runtime.rs (Packager point kind, minimal diff)

`PointKind::Packager` and `BlockProbe.packager` already exist in `engine/src/core/arm.rs`
(`can_create`: `b.packager || b.repackager`, 17 kinds). Two additive edits:

(a) In `fn probe(...)`, in the `match block.as_str()` after the `b if b == super::create_port_chute::BLOCK ...`
arm and before the `b if b == super::create_port_drill::SAW` arm, add:

```rust
        b if b == super::create_port_packager::BLOCK => probe.packager = true,
```

(b) In `impl ArmWorld<ItemStack> for World`, `fn insert`, replace the two lines

```rust
            // The port has no Packager: the arm treats such a point as full and empty.
            PointKind::Packager => 0,
```

with

```rust
            // `PackagerPoint.insert`: a package is unpacked into the inventory behind.
            PointKind::Packager => {
                super::create_port_packager::arm_insert(self.game, pos, &offered, simulate)
            }
```

Extraction needs no edit: the Packager is a mod container, so the generic path in
`extract` reads its output slot (the finished box) through `native_container_slots`.
Also update the header comment (`Supported point types ...`) and the sentence "Packager does
not exist in the port" in `docs/mechanical-arm.md`.

Wiring check: `use super::{...}` in arm_runtime.rs already reaches sibling modules through
`super::create_port_packager`, no new `use`.

## 3. tools/content.py

Imports (next to `from link_content import ...`):

```python
from logistics_content import logistics, logistics_lang
```

In `main()`, immediately after the line `foundation()` (the recipe clean-up and the
pickaxe tag are written inside `foundation()`, so the call must come after it):

```python
    foundation()
    logistics(write, model, CONTENT)
    logistics_lang(CONTENT)
```

This writes the packager and gauge blocks (JSON, blockstate, fallback models, loot tables), the
four `cardboard_package_*` items (max stack 1), two placeholder recipes, the
`create:logistics` creative tab and the pickaxe tag entry, and merges English and Russian
labels into the lang files. Note `foundation()` already wrote the lang files, so
`logistics_lang` has to run after it.

## 4. tools/catalog.py (optional)

Not required: the fallback models load without it. Add them only if the JAR importer should
replace the models with originals (the part names below are this port's own, so
originals will not match until the names are aligned with the JAR):

```python
from logistics_content import PACKAGER, GAUGE
BLOCKS.append(PACKAGER)
BLOCKS.append(GAUGE)
PARTS += [f'{PACKAGER}/block', f'{PACKAGER}/item', f'{GAUGE}/block', f'{GAUGE}/lit', f'{GAUGE}/item']
```

If you do this, run `tools/content.py` so `native/assets/src/catalog.rs` is regenerated
(`check_distribution.py` asserts it is current) and note that `block_catalog_checks.py`
then audits both blocks (default state, block entity types).

## 5. tools/fallback_textures.py

No lines needed: the models use the existing `brass_casing` and `pump_rim` materials and
vanilla textures only.

## 6. stage4 scripts

`Run-Acceptance.ps1`:
* `param(...)`: append `,[switch] $Packager` before the closing parenthesis.
* `$reportName=if ($Pot) {...}`: prepend `if ($Packager) {'run-packager-result.json'} elseif ` (so it becomes
  `$reportName=if ($Packager) {'run-packager-result.json'} elseif ($Pot) {...`).
* In the `$runs` chain, before `if ($Pot) {` add
  ```powershell
      if ($Packager) {
          $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadPackager.ps1'),'Packager','stage20-packager-'))
      } elseif ($Pot) {
  ```
  (replacing the original `if ($Pot) {` line).

`regress.ps1`: append `,@('packager',@{Packager=$true})` inside the `$sets=@(...)` list, after the `@('pot',@{Pot=$true})` entry.

`Build-Package.ps1`: append `'run-packager-result.json'` to the `$runs` list (after `'run-pot-result.json'`).

## 7. After the build

Run `Run-Acceptance.ps1 -Packager`. Expected output lines: six `Passed` checks. Open
questions the run answers (nothing was executed by the author): whether `setblock ... {Filter:...}` merges
onto the default gauge NBT, whether `data get block ... HeldBox` prints the key, and whether the
`facing` value on `setblock` survives the `place_block` hook (the hook only runs for placing the
item, not for `setblock`, so the script sets `facing` explicitly).
