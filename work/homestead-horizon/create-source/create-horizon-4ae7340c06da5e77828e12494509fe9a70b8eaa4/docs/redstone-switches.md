# Redstone switches and Nixie Tube

Modules: `mixins/create/engine/src/core/redstone_switches.rs` and `nixie.rs` (pure rules, unit tested, unchanged),
the game adapter `mixins/create/switch_runtime.rs` (module `create_port_switch`, hook helpers `switch_*.rs`) and
the content generator `tools/switch_content.py`. Source: Create 6.0.10.

Status: written against the staged engine API by reading signatures; **not compiled and not run in game**. The
wiring is in `mixins/create/fragments/switches.toml`, the shared-file edits are in `fragments/switches.md`.

## Blocks

| Block | Behaviour ported | Block state |
| --- | --- | --- |
| `create:pulse_repeater` | `BrassDiode` PulseRepeater: input on `facing`, delayed pulse, default delay 2 | `facing, inverted, powered, powering` |
| `create:pulse_extender` | PulseExtender: output held while powered and `max_state` ticks after | same |
| `create:pulse_timer` | PulseTimer: self-running pulse every 20 ticks, held off by input | same |
| `create:powered_toggle_latch` | rising edge of the input flips the output, empty-hand use flips it | `facing, powered, powering` |
| `create:powered_latch` | back input sets, either side resets, manual use refused while inputs disagree | `facing, powered, powered_side, powering` |
| `create:analog_lever` | 0..15, use +1, sneak-use -1, output `power` on all sides, direct to the attached block | `facing, power` |
| `create:redstone_contact` | powered while the block in front is a contact facing back; 15 on all sides except the back | `facing, powered` |
| `create:nixie_tube` | shows the strength it reads as two characters, drawn on the display face | `mount` |

Pulse Repeater / Extender / Timer / Latch are placed with `facing` = opposite of the player's look (the input points at
the player, output away, as a vanilla repeater). Right click (empty hand or Wrench) toggles `inverted` on the three
diodes and flips the stored output of the latches. Lever and contact take the clicked face as `facing`; the Nixie
tube takes `mount` = `floor_`, `ceiling_` or `wall_` plus a direction from the clicked face and the player.

## How it runs

- Every switch except the lever has a **ticking block entity** (`create:<name>`) with the logical state in NBT
  (`State`, `MaxState`, `Powered`, `Powering`, `Inverted`, `PoweredSide`, `Again`, `InputDelay`, `RedstoneStrength`).
  Each tick it samples its inputs, runs the core state machine (`BrassDiode.tick`, `ToggleLatch.tick`,
  `PoweredLatch.tick`, `contact_next_powered`, `tube_power`) and, when something changed, queues the block for `flush`.
- `flush` (the `level_tick_post` hook, after all block entities ticked) mirrors NBT into the block state and calls
  `native_update_signal_neighbors` when an output changed. The same pattern as the Redstone Link receiver, so an output
  appears one tick after the block entity decided it.
- `block_signal` / `is_signal_source` answer from **block state only**; they never read the block entity.
- The lever has no block entity: its strength is the `power` property.
- The Nixie tube sends `RedstoneStrength` in the update tag; `block_entity_submit` draws the two characters
  with `native_draw_text` under a pose that repeats the blockstate rotation of the `mount`.
- Clicks play `minecraft:block.lever.click` with the core's pitch.

## Redstone Link additions

Sneaking with an empty hand on a link clears both frequency slots (`link_runtime.rs`, function `reset_frequency`;
`FrequencyFirst` / `FrequencyLast` are removed and the link re-keys in the network). Empty-hand use without
sneaking still toggles transmitter / receiver. This is a port addition: Create clears a ghost slot in its own UI.

## Differences and unverified points

- **Directional input is approximated.** The engine exposes only the strongest signal over the six neighbours
  (`native_neighbor_signal`). Redstone blocks, redstone wire and these switches are read per side from block state. Any
  further signal (lever, torch, button, plate, comparator, observer, powered conductor) is attributed to every input
  side whose neighbour is not classified, so a lever next to a pulse repeater can also trigger it, and a powered latch
  cannot tell a back signal from a side signal when both come from unclassified sources.
- Diode block delay (the 2 tick scheduled `DiodeBlock.tick` that copies the input into POWERED) is emulated with a
  counter (`InputDelay`) instead of scheduled block ticks; the latches react one tick after the input changed.
- Output is applied in `flush`, one tick after the block entity decided it.
- The pulse delay (`MaxState`) cannot be edited: Create's scroll-value UI is not ported. Defaults: repeater 2,
  extender 2, timer 20.
- The Analog Lever updates its neighbours at once; Create delays the neighbour update by 15 ticks
  (`AnalogLever.tick` / `last_change` are unused). The lever click pitch follows the core.
- Redstone Contact is not driven by contraption contacts (moving contacts on contraptions).
- Nixie tube: orange only. No dye variants (`nixie.rs::tube_block_name` is unused), no name-tag text, no rows of
  tubes sharing a text (`apply_text_to_row`), no display link target. Only the `floor`, `ceiling` and `wall` mounts exist
  (no `wall_reversed`). The power probe feeds the same directional input into both parameters of `tube_power`.
- Shapes, hardness, recipes (estimated: see `_RECIPES` in `switch_content.py`), sounds, materials and the
  16 pixel models are ours; every model uses fallback tiles (`andesite_casing`, `brass_casing`, `copper_casing`,
  `chain_indicator`, `chain_indicator_powered`, `steel_axle`). The glyph draw orientation (blockstate x/y mapped to
  `rotate_about`) was derived from `steam_runtime.rs`, not seen on screen.
- `can_survive`: diodes and latches need a non-replaceable block below, the lever a non-replaceable block behind it.
  Create requires a rigid support.

## Not done

- **Linked Controller.** The core (`redstone_link.rs`: `ControllerServer`, `ControllerClient`, 12 frequency slots)
  exists, and held keys are visible to a mod (`game.keys`, the motor port uses `net_from_client`), but the item
  needs a frequency UI (menu with ghost slots), per-item frequency storage, a client key handler, a packet and a
  server handler. Not started.
- Stockpile Switch, Smart Observer, Pulse repeater scroll value UI, Lectern Controller, Display Link / Nixie target.
- Nixie dye / name tag behaviour (see above).

## Acceptance

`stage4/acceptance/Test-HomesteadSwitches.ps1` (`Run-Acceptance.ps1 -Switches`). It cannot sneak, so the lever
decrement, the link frequency reset and the Nixie glyph are manual checks.
