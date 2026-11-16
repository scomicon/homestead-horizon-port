# Client tooltips, group T (stage20): shared-file edits

New files (create-source root unless noted): `mixins/homestead/client_tooltips.rs` (module
`homestead_client_tooltips`), the hook helpers `mixins/homestead/ctip_tooltip.rs`,
`ctip_color.rs`, `ctip_register.rs`, `ctip_command.rs`, `mixins/homestead/fragments/clientT.toml`
(1 module, 4 injections), `docs/client-tooltips.md`, `stage4/acceptance/Test-HomesteadClientT.ps1`.
Nothing was built, run or committed. The `.rs` files pass `rustfmt --check` (syntax only); the
first compile will show any wrong type path or field name (see "Risks").

No content `.hmod`, lang file or `tools/*_content.py` is needed: the enchantment descriptions are a
built-in table (English and Russian) in the module and a language file (or a resource pack)
overrides them through `enchantment.<namespace>.<path>.desc`.

## 1. `mixins/homestead/horizon-mixin.toml`

Append the whole of `fragments/clientT.toml` at the end. Bump the version from
`0.1.0+homestead.9` to `0.1.0+homestead.10` (or whatever the next one is when this is applied).
The ids (`client-tooltips`, `ctip-tooltip`, `ctip-color`, `ctip-register`, `ctip-command`) are
checked against the current file: no clash. All four injections are additional `HEAD` helpers.
`register_commands` and `perform_command` already have injections (Create's `create ...`
commands); the helpers here only act on a line that starts with `homestead tooltip`.

## 2. `stage4/Run-Acceptance.ps1`

- `param(...)`: add `,[switch] $ClientT`.
- The `$reportName` chain: add `if ($ClientT) {'run-clientt-result.json'} elseif ...` at the start
  (the existing first `if` becomes `elseif`).
- In the `$runs` chain, next to the `-ClientDefaults` branch:
  ```powershell
  } elseif ($ClientT) {
  $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadClientT.ps1'),'ClientT','stage26-clientt-'))
  ```
- No window is needed: the script only reads the lines the game builds for an item tooltip and
  the frame colours (it needs no rendering). If a run shows the command is not answered in the
  default mode, set `$env:HOMESTEAD_WINDOWED='1'` for this switch as `-Motor` does.

## 3. `stage4/regress.ps1`

In `$sets` add `,@('clientt',@{ClientT=$true})` (one more suite).

## 4. `stage4/Build-Package.ps1`

Add `'run-clientt-result.json'` to `$runs`, a provenance line such as
`'Homestead client: tooltips - enchantment descriptions, enchantment info, rarity frames (stage 26)'`
and the README row (the README.ru.md block is built by hand in this repo), and bump the default
package name when packaging.

## 5. Docs

- `stage4/research/client-mods.md`: in the per-mod table change EnchantmentDescriptions,
  More Enchantment Info and Tooltip Overhaul from `API-NOW` to `done (stage20, unverified on screen)`.
- `stage4/research/roadmap.md`: one row for the stage20 client tooltips.

## Hooks used and why they are client side

| Hook | Called from | Thread | Used for |
| --- | --- | --- | --- |
| `tooltip` (`native_tooltip`) | `Game::stack_tooltip_lines`, the end of building an item tooltip's lines | the window's `Game` (drawing and measuring tooltips) | description and info lines |
| `tooltip_color` (`native_tooltip_color`) | `Game::tooltip_box` | the window's `Game` | frame colours |
| `register_commands`, `perform_command` | the operator command line (also the control socket's `command`) | the game running the command | `homestead tooltip ...` switches and the test aid |

`native-hooks.json` has no `side` entry for `tooltip` (it carries `side: client` for `tooltip_color`,
`tooltip_component`); the call site is the client's own tooltip builder, which only exists on the
window's `Game`, and no server-thread hook (`level_tick_*`, `use_item`, `native_food_*`) is used.

## Risks (never compiled)

- Paths and names taken from the Horizon sources, each seen in use elsewhere in the client crate:
  `horizon_client::{hud::rgba, items::ItemStack, text::Text, lang::get_or, item_tooltip::rarity}`,
  `Game::{enchantments, stack_tooltip, held, native_feedback, native_tooltip_color, has_shift_down}`,
  `Enchantments::{get, full_name, props}` and the public fields of `Enchantment`
  (`description_key`, `description`, `max_level`, `supported`, `primary`, `exclusive`).
  `pub(super)` methods declared in `game/*.rs` are visible to our module because the injected module
  is a child of `game`. If any visibility or path is wrong the compiler says so at the first build.
- The tooltip's enchantment lines are found by comparing a line's plain text with
  `Enchantments::full_name(id, level).0`. That is how the client builds them (`e.tooltip(...)`); a
  different build (another language layer or a custom colour code) just means no extra lines for
  that enchantment, never a crash.
- `language.code` (used to choose the Russian built-in texts) is a vanilla language key that the
  Horizon language files may not carry; without it the English text is used.
- `homestead tooltip` as the bare syntax is registered next to the longer ones.
- The command names: `NativeCommand::op` takes the same `<name:word>` argument syntax the create
  bundle's `create schematool ...` commands use.
