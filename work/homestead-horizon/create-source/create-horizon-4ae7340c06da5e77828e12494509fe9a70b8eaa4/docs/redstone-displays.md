# Redstone logistics and displays (pure logic)

Pinned original: Create 6.0.10, `Creators-of-Create/Create` revision
`ac0c444d9828da3453ae8cc65338e8de063286fb` (MIT). Behaviour was ported, not
pasted. Java was read through summarising fetches of the raw files, so every
detail marked "unverified" below was not confirmed against the literal source.

All five cores are declared in `engine/src/lib.rs`. The Display Link and Display
Board are wired into the game by the adapters listed under "Display Link and Display
Board in the game"; the Nixie Tube, switches, observers, latches and the Linked
Controller are described by their own slices.

## Modules

| Rust module | Contents |
| --- | --- |
| `core/redstone_link.rs` | Frequency, per-level `LinkNetwork`, transmitter/receiver/manual members, range rule, `hasAnyLoadedPower`, Redstone Link block signal rules, Linked Controller server handler and client packet cadence |
| `core/redstone_switches.rs` | Stockpile Switch, Smart Observer, Redstone Contact, Analog Lever, Pulse Repeater / Extender / Timer, Toggle Latch, Powered Latch |
| `core/nixie.rs` | Nixie Tube text/redstone model, row text distribution, row directions, dye block names |
| `core/display_board.rs` | Flap display sections, layouts, board size rules, spin animation (`JavaRandom` for determinism), line colour/glow |
| `core/display_link.rs` | Display Link state machine, line reservation, targets (sign, nixie, single line, board), sources (time of day, item count, fluid sum, kinetic speed/stress, redstone power, percent/progress bar, scoreboard/value list), label and flap layout rules |

## Java to Rust mapping

| Java (`com.simibubi.create...`) | Rust |
| --- | --- |
| `content.redstone.link.RedstoneLinkNetworkHandler` (`Frequency`, `addToNetwork`, `removeFromNetwork`, `updateNetworkOf`, `withinRange`, `hasAnyLoadedPower`, `globalPowerVersion`) | `Frequency`, `LinkNetwork::{add_to_network, remove_from_network, update_network_of, set_frequency}`, `within_range`, `has_any_loaded_power`, `power_version` |
| `LinkBehaviour` (`newPosition`, `isListening`, `setReceivedStrength`, `setFrequency`) | `Linkable`, `LinkMode`, `LinkNetwork::set_received` (gated by `new_position`) |
| `RedstoneLinkBlock` (`getPower`, `updateTransmittedSignal`, `getSignal`, `getDirectSignal`, `isSignalSource`, `toggleMode`) | `transmitter_power`, `link_block_signal`, `link_block_direct_signal`, `link_block_is_signal_source`, `toggle_mode` |
| `RedstoneLinkBlockEntity.tick` receiver branch | `receiver_tick` |
| `LinkedControllerServerHandler` (`TIMEOUT` 30, `ManualFrequencyEntry`) | `ControllerServer`, `LinkMode::Manual`, `CONTROLLER_TIMEOUT` |
| `LinkedControllerClientHandler` (`PACKET_RATE` 5) | `ControllerClient::tick`, `InputPacket` |
| `LinkedControllerItem.toFrequency` (slots 2n, 2n+1) | `controller_frequency` |
| `ThresholdSwitchBlockEntity.updateCurrentLevel`, `ThresholdSwitchBlock.getSignal` | `ThresholdSwitch`, `storage_levels` |
| `SmartObserverBlockEntity.tick/activate`, `SmartObserverBlock.getSignal/tick/onFunnelTransfer` | `SmartObserver`, `Observation` |
| `RedstoneContactBlock` | `contact_*` functions |
| `AnalogLeverBlockEntity`, `AnalogLeverBlock` | `AnalogLever` |
| `BrassDiodeBlockEntity` + `PulseRepeater/Extender/TimerBlockEntity` + `BrassDiodeBlock` | `BrassDiode`, `DiodeKind` |
| `ToggleLatchBlock`, `PoweredLatchBlock`, vanilla `DiodeBlock.tick` | `ToggleLatch`, `PoweredLatch`, `diode_tick` |
| `NixieTubeBlockEntity`, `NixieTubeBlock` (walk, `getPower`, `withColor`) | `NixieTube`, `apply_text_to_row`, `tube_power`, `left_direction`, `tube_block_name` |
| `trains.display.FlapDisplaySection/Layout/BlockEntity` | `FlapSection`, `FlapLayout`, `FlapDisplay`, `controller_size`, `max_char_count` |
| `DisplayLinkBlock`, `DisplayLinkBlockEntity` | `DisplayLink`, `display_link_should_be_powered` |
| `api.behaviour.display.DisplaySource/DisplayTarget` (reserve, transfer rules) | `Reservations`, `with_label`, `load_single_line_layout` |
| `SignDisplayTarget`, `NixieTubeDisplayTarget`, `SingleLineDisplayTarget`, `DisplayBoardTarget` | `sign_accept_text`, `nixie_accept_line`, `single_line_accept`, `board_accept_flap_text`, `*_stats` |
| `TimeOfDayDisplaySource`, `ItemCountDisplaySource`, `FluidAmountDisplaySource`, `KineticSpeed/StressDisplaySource`, `RedstonePowerDisplaySource`, `PercentOrProgressBarDisplaySource`, `ScoreboardDisplaySource`, `ValueListDisplaySource` | `time_of_day`, `item_count`, `fluid_amount`, `kinetic_speed`, `stress_*`, `redstone_power_*`, `percent_text`/`progress_bar`, `scoreboard_entries`, `value_list_*` |

## Ported (pure rules, with unit tests: 100 tests)

- Link network: ordered frequency pairs, strict squared range check, max of
  in-range transmitters with the exact `< 15` guard, receivers written
  unconditionally on every update, dead member pruning, remove/rejoin
  behaviour, `power_version`, cross-level `has_any_loaded_power`.
- Link block: `getPower` (second probe skips the attached face), factory panel
  contribution, receiver output and direct output, `POWERED` sync, mode toggle.
- Linked Controller: pair per button, 30 tick lifetime, release at next tick,
  award condition, client new/release/keep-alive packet order and cooldown.
- Stockpile Switch: hysteresis (defaults 128 on, 64 off), clamped level,
  LEVEL 0..5, no-target reset path, inversion and delayed power.
- Smart Observer: per-source hold times (3, 6, 4), belt rule, inventory
  sustain, tank fall-through, output sides.
- Contact, Analog Lever (15 tick output delay, click pitch), the three pulse
  diodes, both latches.
- Nixie: two characters per tube, decimal digits from strength, custom text
  priority, row distribution and directions.
- Display Board: flap section refresh/spin/transitions, layouts, size scan,
  char capacity, colours, volumes.
- Display Link: refresh cadence, power edges, reservations, all targets and
  listed sources.

## Not ported / known gaps

- Everything that touches the world: block states, neighbour updates,
  scheduling, saving NBT, sync packets, sounds, particles, advancements,
  renderers, GUIs and goggle tooltips.
- Block placement/orientation rules (Smart Observer target picking,
  `canSurvive`, shapes) and the Redstone Link `FactoryPanelSupportBehaviour`.
- Linked Controller lectern handling, bind mode, controller menu, item cooldown.
- ComputerCraft/CC:Tweaked signals on Nixie Tubes and the Computer display source.
- Rich text: `DynamicComponent`, click-event sanitisation (`requiresComponentSanitization`).
  Text is plain `String` here.
- Display sources not ported: boiler, current floor, death counter, enchant
  power, entity name, factory gauge, fluid list, item list, item name,
  item throughput, accumulated item count, observed train name, package
  address, station summary, stopwatch, train status, Nixie source, lectern
  target. `FluidFormatter` output of the fluid source and train-related
  sources belong to other subsystems.
- Smart Observer target discovery and `invVersionTracker` (adapter supplies
  `still_waiting`).
- Flap character cycle lists live in the language file
  (`flap_display.cycles.*`); `split_cycle` parses them, content must supply text.
- Brass/Andesite tunnel and funnel redstone behaviour (not self-contained).
- NBT (de)serialisation of all of the above; field names are recorded in the
  analysis only (Threshold: `OnAboveAmount`, `OffBelowAmount`, `Inverted`, ...;
  note Java writes them as float and reads int).

## Decisions and discrepancies

- Link range: the task text mentions 128, but the pinned `CLogistics.linkRange`
  default is 256 (min 1). `DEFAULT_LINK_RANGE = 256`; the adapter passes the
  configured value to `LinkNetwork::new`.
- Range uses `Vec3i.closerThan` semantics: squared distance strictly below
  `range^2`, measured from the actor that triggered the update, not pairwise
  per receiver.
- The Stockpile Switch thresholds in 6.0.10 are absolute amounts (items or mB),
  not fractions. Level 0..5 drives the indicator only.
- Pulse Extender/Repeater default delay is 2 and the Timer 20 (only the timer
  overrides `defaultValue`); range 2..72000 ticks.
- A Display Link refresh fires on tick `passive_ticks + 1` (post-increment).
- Sources use pure data in, text out; adapters gather inventory/tank/kinetic
  data.

## Unverified details

- `refreshTicks++ < getPassiveRefreshTicks()` ordering (post-increment) was recalled, not read.
- `IntAttached.comparator()` was assumed to sort descending (scoreboard order).
- `FlapDisplaySection.tick` random call order and the neighbour-spin
  conditions follow a summarised description; with a different RNG order the
  animation differs (it is cosmetic, not gameplay).
- Board `controller_size` assumes each scan stops at the first non-matching
  block.
- `format_number` (`CreateLang.number`), `AM`/`PM`, `rpm`, `su`, `million`,
  `thousand` strings are placeholders for the language files / locale formatter.
- Non-natural dimension time (random) ordering relative to 12-hour conversion.
- Time of day, ValueList and Scoreboard behaviours assume the summaries given
  by the fetch tool were accurate.
- The compile/test pass was run on a scratch copy of the whole engine
  (371 tests, 100 of them new, all passing, no warnings in new files). This is
  an isolated algorithm oracle, not a cross-client comparison and not
  presentation parity.

## Display Link and Display Board in the game

Status: authored, not yet built or run (the lead builds and runs the acceptance script
`stage4/acceptance/Test-HomesteadDisplays.ps1`). Nothing below is verified in the game.

Files: `mixins/create/display_link_runtime.rs` (`create_port_display_link`),
`display_board_runtime.rs` (`create_port_display_board`), `display_source.rs`
(`create_port_display_source`), hook helpers `display_*.rs`, injections in
`mixins/create/fragments/displays.toml`, edits to shared files in
`mixins/create/fragments/displays.md`, content in `tools/display_content.py`.

Blocks: `create:display_link` (`facing` 6 ways, `powered`) and `create:display_board`
(`facing` horizontal, `up`, `down`). Both carry a ticking block entity of the same id.

What works as designed (pending the build):

- Link state machine from the core: refresh at tick `interval + 1`, pause while powered,
  immediate refresh when power is lost, `TargetOffset` range check (64 blocks).
- Board: controller/size scan every 10 ticks, `up`/`down` stacking states, text per line
  through `board_accept_flap_text` with line reservation, labelled single-line layouts,
  text drawn per flap from the controller, render bounds covering the whole board.
- Sources: time of day (24h/12h), game time (port addition: world tick count, not a Create
  source), redstone power (number/percent/bar), item count and item name of the adjacent
  container, fluid amount of an adjacent Fluid Tank, kinetic speed of a gauge,
  kinetic stress (bar/percent/stress/capacity/remaining). `auto` picks one from the block.
- Block entity data to look at with `data get block`: link `Source`, `Mode`, `Label`,
  `Interval`, `TargetOffset`, `TargetLine`, `LastText`, `Status`; board controller
  `Controller`, `XSize`, `YSize`, `Lines`, `Rows` (plain text per line).

Configuration without a GUI (Create has a configuration screen; none is ported):

- Target: right click a Display Board with the Display Link item (not sneaking), then place
  the link within 1200 ticks and 64 blocks; it takes the board as target. The selection
  is a session value and is not saved.
- Empty hand: next source. Sneaking, empty hand: next target line (0..15). Wrench: next mode.
  Sneaking with a wrench: next interval (20, 100, 200, 600 ticks). A named item (name tag):
  its name becomes the label. Every change refreshes the board on the next tick.
- Feedback goes to the action bar as plain English text (not translated, not Create's text).

Deliberate differences and gaps:

- The Display Board is not a kinetic block: it works without rotation
  (`speed_requirement_fulfilled` is always true), there is no flap spin animation and no
  clacking sound, text changes instantly. The core could drive both; that needs a kinetic
  node for the board in the kinetic graph (shared code) and is not done.
- Text is drawn on a fixed grid of 3.5 board pixels per flap with the game's proportional
  font, glyphs left aligned in their cell. Lines are two per block, 0.1 block below the
  line top. Glow ink and dye are read from the data but nothing sets them (no dye or
  glow ink interaction, no name tag per line, no clipboard).
- The Redstone Power source reads `best_neighbor_signal` of the source block plus its `power`
  property. Java reads `getDirectSignalTo` (strong power only) and the engine API gives no
  per-side or strong-only read, so weak power is included.
- The link's own `powered` state uses the strongest neighbour signal, including the
  attached face (Java ignores the attached face). To keep a redstone block, lever, torch,
  daylight detector or target block used as a source from pausing its own link, those five
  blocks on the attached face never count as power; any other signal on that face still does.
- Item sources read vanilla chest-like containers (`BlockEntity::Chest`) and mod containers
  (`ModBlockEntity.items`, e.g. an Item Vault). Double chests count one half, there is no
  item filter, no furnaces, barrels, shulkers or hoppers unless they report as one of those.
  The `fluid_amount` source reads `TankContent.Fluid.amount` of a Fluid Tank controller; that
  key layout is taken from the basin code and untested on a tank.
- Time of day works on any source block (Create only on a Cuckoo Clock, not ported).
- Other targets (signs, Nixie Tubes) are not wired; the cores for them exist. The Nixie source
  and target belong to the switches slice.
- Reservations of board lines are kept in a session cache and are lost on reload
  (a second link on the same line then simply overwrites it).
- Bar glyphs (U+2588, U+2592) depend on the game font having them.
- Shapes, models (box models of vanilla textures), recipes (placeholders) and names are
  not Create's; the original models are only used if the player's JAR import provides them.
- The Display Link item selection and the board draw use engine API `native_draw_text`
  (`@API:draw_text@`); confirm it exists at the pinned Horizon revision.

Verification plan: `Test-HomesteadDisplays.ps1` checks, on block entity data, board
joining and sizing, stacking, link-to-board transfer (redstone power 0 then 15), interval,
powered pause and refresh on power loss, source and label changes, a second link on
another line, empty-hand source cycling and link removal. The drawn text is not compared;
a visual check of the board is still required.

## Integration

### `lib.rs` mod lines

Mirror the existing `#[path]` style (modules refer to each other as
`crate::redstone_link`, `crate::display_board`, `crate::nixie`):

```rust
#[path = "core/redstone_link.rs"]
pub mod redstone_link;
#[path = "core/redstone_switches.rs"]
pub mod redstone_switches;
#[path = "core/nixie.rs"]
pub mod nixie;
#[path = "core/display_board.rs"]
pub mod display_board;
#[path = "core/display_link.rs"]
pub mod display_link;
```

Order does not matter. Another subsystem defining a top-level `Direction`
should be reconciled with `redstone_link::Direction` (vanilla order).

### Adapter hooks

- Level load/unload: create/drop one `LinkNetwork` per level; also one
  `ControllerServer` per level, `tick(&mut net)` once per server tick.
- Redstone Link block entity: on first tick/initialise `create` + `add_to_network`;
  on unload/remove `remove_from_network`; on frequency change
  `set_frequency`; apply returned `(LinkId, strength)` writes to the receiver
  block entities (set `receivedSignal`, mark `receivedSignalChanged`).
  Neighbour probes feed `transmitter_power`; result -> `transmit`.
  Chunk unload sets `alive = false`.
- Linked Controller packets: map button index through `controller_frequency`
  and call `ControllerServer::receive_pressed` with the player position; client
  runs `ControllerClient::tick` each tick with the pressed button set.
- Stockpile Switch: lazy tick every 10 ticks gathers `SlotSample`/`TankSample`
  (or a `ThresholdSwitchObservable`) and calls `update_current_level`; schedule
  the block tick at +2 for `update_power_after_delay`; apply
  `level_property` to the `LEVEL` state.
- Smart Observer: build an `Observation` each tick from the target; apply
  `powered_changed` (set `POWERED`, update neighbours) and `schedule_off`
  (block tick +1 calls `block_tick`); funnels call `on_funnel_transfer`.
- Diodes/latches: block entity tick calls `BrassDiode::tick`; latches call
  `ToggleLatch::tick`/`PoweredLatch::tick` from scheduled block ticks
  (delay 1; priority rules are vanilla `DiodeBlock`).
- Nixie: on neighbour change (1 tick scheduled) compute `tube_power` and call
  `update_displayed_redstone_value`; row walking and dye block replacement stay
  in the block code; Display Link target calls `nixie_accept_line`.
- Display Link: server tick calls `DisplayLink::tick`; act on `LinkAction`
  by gathering source data and calling the matching source function, then the
  target function. Neighbour changes use `display_link_should_be_powered` and
  `set_powered`. Board writes also need `FlapDisplay::tick_sections` on the
  client with the level random (`FlapRandom`), the sound volumes from
  `flap_volumes`, and `controller_size` from block scanning.

### Block entities and states

- Blocks: `redstone_link` (FACING, POWERED, RECEIVER), `linked_controller`
  item, `lectern_controller`, `stockpile_switch` (FACING, TARGET, LEVEL 0..5),
  `smart_observer` (POWERED), `redstone_contact`/`elevator_contact` (POWERED,
  FACING), `analog_lever`, `pulse_repeater`, `pulse_extender`, `pulse_timer`
  (POWERING, POWERED, INVERTED), `powered_toggle_latch`, `powered_latch`
  (POWERED, POWERING, POWERED_SIDE), 16 nixie tubes (FACE, FACING, WATERLOGGED),
  `display_link` (FACING, POWERED), `display_board` (HORIZONTAL_FACING, UP, DOWN).
- Block entities: link (Transmitter, Receive, ReceivedChanged, Transmit,
  frequencies, LastKnownPosition), threshold switch, smart observer (TurnOff),
  analog lever (State, ChangeTimer), brass diode (State, scroll value),
  nixie (CustomText, CustomTextIndex, RedstoneStrength), display link
  (TargetOffset, TargetLine, Source with Id, config), flap display
  (Controller, XSize, YSize, CustomLine*, GlowingLine*, Dye*, Display*).

### Content and recipes needed

Items/blocks above plus filter slots, scroll-value boxes, wrench toggling,
dye/name-tag/clipboard interactions on nixie tubes and boards, display link
click-to-link item, lang strings (`display_source.*`, `flap_display.cycles.*`,
`generic.unit.*`, `generic.daytime.*`), flap/nixie/link models and textures
(private player JAR), and the vanilla Create recipes for each block. Recipe and
loot data are not part of this subsystem's Rust code.
