# Redstone Link

Modules: `mixins/create/engine/src/core/redstone_link.rs` (pure frequency network, block signal rules, unit
tested), the game adapter `mixins/create/link_runtime.rs` (module `create_port_link`) and the content generator
`tools/link_content.py`. Source: Create 6.0.10, package `content/redstone/link`.

Status: written against the staged engine API by reading signatures; **not compiled and not run in game**.

## Behaviour

- Block `create:redstone_link`, properties `facing` (the plate hugs the opposite neighbour, placed with the
  clicked face as `facing`), `powered`, `receiver` (default false = transmitter).
- Frequency = ordered pair of two item stacks (`FrequencyFirst`, `FrequencyLast`, count 1, `item_to_nbt`).
  Equality is item id plus the `minecraft:dyed_color` component (`Frequency::of`).
- Transmitter: every tick it reads `native_neighbor_signal` (strongest neighbour signal, 0..15), stores it in
  `Transmit`, mirrors it into `powered` (flag 2) and calls `LinkNetwork::transmit`. Every receiver of the same
  pair within range (`DEFAULT_LINK_RANGE` = 256 in 6.0.10, measured from the acting link) gets the maximum.
- Receiver: the network writes `Receive`; `flush` (after the block entities tick) sets `powered = Receive > 0`
  and calls `native_update_signal_neighbors`. The `block_signal` / `is_signal_source` hooks answer
  `link_block_signal`, `link_block_direct_signal` (front face only) and `link_block_is_signal_source`, so the
  output is graded 0..15 on every side, direct on the front.
- Network registry: thread-local `SessionCache<Dimension>` keyed by world pointer and dimension, one
  `LinkNetwork` per level. Not saved. After a chunk load each link registers on its first tick from its NBT;
  a sweep every 10 ticks removes entries whose chunk is not resident or whose block is gone (the equivalent
  of `LinkBehaviour.unload/remove`) and re-evaluates their receivers.
- Interaction: right click with an item sets the first, then the last frequency (alternating, flag
  `create_horizon:LinkNextSlot`); right click with an empty hand or the Wrench toggles transmitter / receiver
  (`toggle_mode`); sneak with an item places a block normally.
- `can_survive` / `update_shape`: the plate needs a non-replaceable neighbour behind it and disappears when
  that neighbour is replaced.

## Differences and unverified points

- Frequency slots: Create uses two slots on the plate chosen by the crosshair; the `right_click_block` hook has
  no hit position, so the port alternates. There is no way to clear a slot.
- Toggle: recalled as non-sneaking empty-hand `use` of `RedstoneLinkBlock` and the wrench; the engine comment
  says "wrench / sneak-use". The adapter toggles for an empty hand with or without sneaking and for the wrench.
- Transmitter input uses `getBestNeighborSignal`; Create probes each side with the actual direction and a
  second `UP` probe (`transmitter_power`). Factory panel input is not ported.
- NBT names `Transmit`, `Receive`, `FrequencyFirst`, `FrequencyLast` follow Create from memory; `ReceivedChanged`
  and `Transmitter` are not written.
- Receivers settle at the end of the tick in which the transmitter changed, not in their own tick.
- Range: engine default 256 (6.0.10 `logistics.linkRange`); no config.
- Shape (10x10x2 plate), hardness, sound, recipe (electron tube over brass sheet, 2 links) and loot are
  estimates; models are fallback boxes. Models named `redstone_link/{transmitter,transmitter_powered,receiver,
  receiver_powered,item}` are guesses at the original layout. The frequency items are not drawn on the plate.
- Not ported: Linked Controller, Lectern Controller, train schedule link condition, ponder, advancement,
  hooks into Display Link / contraption movement of links.

## Integration

See the integration checklist in the hand-off report (runtime.rs lists, `horizon-mixin.toml`, catalog,
placement state, right-click chain, language keys).
