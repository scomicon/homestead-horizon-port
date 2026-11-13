# Shared-file edits for the redstone switches slice

Apply these by hand (lead). Nothing here is applied by the slice itself. `fragments/switches.toml` is appended
to `horizon-mixin.toml` as a whole (no edit inside existing entries).

Already applied by the slice (additive): `link_runtime.rs` (`reset_frequency` + one match arm
`None if sneaking => reset_frequency(game, pos)`).

## 1. `mixins/create/horizon-mixin.toml`

Append the full contents of `mixins/create/fragments/switches.toml` after the `link-flush` injection
(or at the end of the file). It adds one `[[module]]` (`create_port_switch`, `switch_runtime.rs`) and 12
`[[injection]]` entries, all additional HEAD helpers on existing hooks: `block_entity`, `block_entity_ticks`,
`block_entity_tick`, `block_entity_update_tag`, `block_entity_submit`, `right_click_block`,
`state_for_placement`, `block_signal`, `is_signal_source`, `can_survive`, `update_shape`, `level_tick_post`.
Every helper acts only on `create:pulse_*`, `create:powered_*`, `create:analog_lever`, `create:redstone_contact`,
`create:nixie_tube` and leaves the shared answers untouched. The right-click helper returns early when `*canceled`
is already set.

If the mixin tool rejects two injections on the same hook (the file already has two or more on
`can_survive`, `update_shape`, `level_tick_post`, `interaction_key`, so it should accept them; it is untested for
`block_entity*`, `state_for_placement`, `right_click_block`), fall back to the explicit chain below instead.

## 2. Fallback chain (only if parallel injections on a single-injection hook are refused)

Replace the toml entries `switch-entity`, `switch-ticks`, `switch-tick`, `switch-update-tag`, `switch-draw`,
`switch-right-click` and `switch-placement` by these single-line additions and drop their `[[injection]]` blocks:

- `mixins/create/entity.rs`, end of `create_on_entity`: `crate::game::create_port_switch::entity(block, entity);`
- `mixins/create/ticks.rs`, end of `create_on_ticks`: `crate::game::create_port_switch::ticks(block, ticks);`
- `mixins/create/tick.rs`, before the existing call: `crate::game::create_port_switch::tick(game, pos, entity);`
- `mixins/create/update_tag.rs`, after `create_port_chassis::update_tag(entity, tag);`:
  `crate::game::create_port_switch::update_tag(entity, tag);`
- `mixins/create/draw.rs`, after the existing call:
  `crate::game::create_port_switch::draw(game, pos, block, state, entity, draw);`
- `mixins/create/right_click.rs`, after the `create_port_link::right_click` block:
  ```rust
      if crate::game::create_port_switch::right_click(game, pos) {
          game.swing_arm();
          *canceled = true;
          return;
      }
  ```
- `mixins/create/placement_state.rs` (the `create_placement_state` helper), before the existing call:
  `crate::game::create_port_switch::placement(game, state, context, answer);`

(`placement_runtime::state` only writes `answer` for its own blocks, so calling the switch placement first or last
is equivalent.)

## 3. `tools/content.py`

1. After `from link_content import BLOCK as LINK, redstone_link, redstone_link_recipes` (line ~30):
   ```python
   from switch_content import LANG_EN as SWITCH_EN, LANG_RU as SWITCH_RU, switches, switch_recipes
   ```
2. After `redstone_link(write, model)` (line ~974): `switches(write, model)`
3. After `redstone_link_recipes(write)` (line ~1012): `switch_recipes(write)`
4. After `extra_blocks[LINK] = 'Redstone Link'` (line ~1084): `extra_blocks.update(SWITCH_EN)`
5. After `lang['block.create.' + LINK] = 'Звено красного камня'` (same indentation, line ~1233):
   `lang.update({'block.create.' + k: v for k, v in SWITCH_RU.items()})`

## 4. `tools/catalog.py`

1. After `from link_content import BLOCK as LINK, PARTS as LINK_PARTS`:
   `from switch_content import BLOCKS as SWITCH_BLOCKS, PARTS as SWITCH_PARTS`
2. After `BLOCKS.append(WHISTLE)`: `BLOCKS.extend(SWITCH_BLOCKS)`
3. After `PARTS += [f'{LINK}/{part}' for part in LINK_PARTS]`:
   `PARTS += [f'{block}/{part}' for block, parts in SWITCH_PARTS.items() for part in parts]`

## 5. `tools/fallback_textures.py`, `FAMILIES` (after `'redstone_link': 'andesite_casing',`)

```python
    'pulse_repeater': 'brass_casing',
    'pulse_extender': 'andesite_casing',
    'pulse_timer': 'copper_casing',
    'powered_latch': 'andesite_casing',
    'powered_toggle_latch': 'brass_casing',
    'analog_lever': 'andesite_casing',
    'redstone_contact': 'andesite_casing',
    'nixie_tube': 'copper_casing',
```
(Only used by tools iterating `PARTS`; the models themselves already reference `create:block/fallback/*` tiles.)

## 6. Acceptance

`stage4/Run-Acceptance.ps1`:
- line 5, `param(...)`: add `,[switch] $Switches` before the closing parenthesis.
- line 20, `$reportName=`: start the chain with
  `if ($Switches) {'run-switches-result.json'} elseif ($Pot) ...` (turn the existing leading `if ($Pot)` into `elseif`).
- before `} elseif ($Link) {` (line ~65) insert:
  ```powershell
      } elseif ($Switches) {
          $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadSwitches.ps1'),'Switches','stage20-switches-'))
  ```

`stage4/regress.ps1`, in `$sets=@(...)` after `@('link',@{Link=$true})` add `,@('switches',@{Switches=$true})`.
(`regress2.ps1` has no per-feature sets that need changing.)

## 7. Docs

Optional: in `docs/redstone-link.md` replace "There is no way to clear a slot." by "Sneaking with an empty hand clears
both slots (see redstone-switches.md)." and add `redstone-switches.md` to the README/doc index.
