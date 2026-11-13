# Stock network slice: edits to apply to shared files

New files of this slice (all under `mixins/create/` unless noted): `stock_link_runtime.rs`,
`stock_ticker_runtime.rs`, `frogport_runtime.rs`, the hook helpers `stock_entity.rs`,
`stock_ticks.rs`, `stock_tick.rs`, `stock_update_tag.rs`, `stock_draw.rs`,
`stock_container.rs`, `stock_right_click.rs`, `stock_destroy.rs`, `stock_placed.rs`,
`stock_register.rs`, `stock_command.rs`, `fragments/stock.toml`; `docs/stock-network.md`
(under the create-source root); `tools/stock_content.py`;
`stage4/acceptance/Test-HomesteadStock.ps1`.

All hook helpers are *additional* helpers on hooks that other injections already use, so no
existing helper is edited. Helper function names are `create_stock_*`. The slice needs no
change to any existing runtime file: the Packager, the package item and the engine cores
are used as they are (`pk::step`, `pk::BLOCK`, `pk::arm_insert` and the `package::*`
conversions are already `pub(super)`).

## 1. horizon-mixin.toml

Append the whole of `fragments/stock.toml` at the end of `horizon-mixin.toml` (3 `[[module]]`
+ 11 `[[injection]]`, ids prefixed `stock-`). No existing entry changes. Module names:
`create_port_stock_link`, `create_port_stock_ticker`, `create_port_frogport`. Module order
does not matter (they reach each other through `super::`). Bump the create bundle version
(it was `0.10.0+homestead.1`).

Hook ids used again: `block_entity`, `block_entity_ticks`, `block_entity_tick`,
`block_entity_update_tag`, `block_entity_submit`, `block_entity_container`,
`right_click_block`, `destroy_block`, `place_block`, `register_commands`, `perform_command`.
The command helpers set `*ran = true` only for `create stock ...`, so they do not meet
`create cannon ...` or `create schematic ...`.

## 2. tools/content.py

Imports (next to `from logistics_content import ...`):

```python
from stock_content import stock, stock_lang
```

In `main()`, immediately after the `logistics(...)` / `logistics_lang(CONTENT)` lines:

```python
    stock(write, model, CONTENT)
    stock_lang(CONTENT)
```

It writes four blocks (JSON, blockstate, fallback models, loot tables), four placeholder
recipes, the `create:stock` creative tab and the pickaxe tag entries, and merges English and
Russian labels into the lang files (`stock_lang` must run after `foundation()` wrote them).

## 3. tools/catalog.py (optional)

Not required: the fallback models load without it. Add them only if the JAR importer should
replace the models with originals:

```python
from stock_content import BLOCKS as STOCK_BLOCKS
BLOCKS += list(STOCK_BLOCKS)
PARTS += [f'{name}/block' for name in STOCK_BLOCKS] + [f'{name}/item' for name in STOCK_BLOCKS]
```

(then regenerate `native/assets/src/catalog.rs` through `tools/content.py`; the part names
are this port's own, so originals will not match until they are aligned with the JAR).

## 4. tools/fallback_textures.py

No lines needed: the models use the existing `brass_casing`, `pump_rim` and `chain_indicator`
materials only.

## 5. stage4 scripts

`Run-Acceptance.ps1`:
* `param(...)`: append `,[switch] $Stock` before the closing parenthesis.
* `$reportName=if (...)`: prepend `if ($Stock) {'run-stock-result.json'} elseif ` to the chain.
* In the `$runs` chain, before the existing first `if (...) {` branch add
  ```powershell
      if ($Stock) {
          $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadStock.ps1'),'Stock','stage23-stock-'))
      } elseif (<the previous first condition>) {
  ```
  (the same shape as the `-Packager` edit of the logistics slice).

`regress.ps1`: append `,@('stock',@{Stock=$true})` inside the `$sets=@(...)` list (after the
`packager` entry; 25 suites).

`Build-Package.ps1`: append `'run-stock-result.json'` to the `$runs` list.

## 6. Docs

* `docs/logistics.md`: in the section that says there is no Stock Link, request mode or Package
  Post, point to `docs/stock-network.md` (the Packager's `linked` is still always false).
* `docs/roadmap.md` (and `stage4/research/roadmap.md`): stock network row.

## 7. After the build

`Run-Acceptance.ps1 -Stock`. Expected output: eleven `Passed` lines (twelve when the HeldBox text shows the address, else one `note:`)
(the HeldBox address check). The run settles the open points listed at the end of
`docs/stock-network.md`. The Frogport pair test is the one most likely to need a fix, since
it chains the Packager's output slot, the Frogport's pull, the virtual network and the
push into a chest.
