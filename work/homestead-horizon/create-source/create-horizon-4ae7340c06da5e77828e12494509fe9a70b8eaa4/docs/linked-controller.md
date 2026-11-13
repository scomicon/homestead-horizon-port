# Linked Controller

Create 6.0.10 `LinkedControllerItem`, `LinkedControllerClientHandler`, `LinkedControllerServerHandler`. The rules are in
`mixins/create/engine/src/core/redstone_link.rs` (`controller_frequency`, `ControllerServer`, `ControllerClient`, the
30-tick `CONTROLLER_TIMEOUT` and 5-tick `CONTROLLER_PACKET_RATE`, already unit-tested). The adapter is
`mixins/create/controller_runtime.rs`; the network it feeds is the Redstone Link network of `link_runtime.rs`.

## What works

- **Item** `create:linked_controller`, stack size 1. Its data lives in the `create:linked_controller_items` component:
  12 frequency items (`F0`..`F11`; button n owns slots 2n and 2n+1) and the binding cursor `Next`.
- **Active mode**: hold the controller and press **G** to toggle. While active **Z X C V B N** are buttons 0-5.
  The client runs `ControllerClient` (release packets, new-press packets, keep-alive every 5 ticks) and sends
  `create:controller_input` to the server; dropping or switching the item, or toggling off, releases everything.
- **Server**: `ControllerServer::receive_pressed` creates one `Manual` member per pressed pair in the level's
  `LinkNetwork`, at the player's position (256 block range as for links); a keep-alive refreshes the 30 tick timer,
  a release sets it to 0, `ControllerServer::tick` (each level tick, `level_tick_post`) drops expired members and
  re-evaluates the receivers. Receivers power exactly like for a transmitter (strength 15 while held).
- **Binding**: right-click a Redstone Link with the controller copies the link's two frequency items into the next
  button and advances the cursor (wraps after button 5); sneak + right-click clears the button bound last. Buttons with
  both slots empty send nothing.
- **Operator commands** (permission 2, the same server path without keys): `create controller give | status | clear |
  set <button> <first> <second> | bind <button> <x> <y> <z> | press <button> | release <button>`. Item words are
  `stone` (= `minecraft:stone`) or `namespace.path` (`create.brass_ingot`) because command words cannot hold a colon.
  `press` lasts the 30 tick timeout.

## Differences from Create (documented approximations)

- Key layout: Create uses the movement keys and suppresses player movement while active; this port cannot cancel
  movement from a native mod, so it uses Z X C V B N and G. Create's right-click-in-air to activate and the
  Lectern Controller are not ported (no air-use hook; no Lectern support).
- Frequencies are set by right-clicking a link or by command, not in the controller's own screen (Create's frequency
  ghost slots and button layout GUI, the activation HUD and the tooltip are missing).
- The model is a vanilla compass sprite; the recipe is a placeholder (lever / andesite casing / Redstone Link).
- Held inputs and the network are not saved; they belong to one level (leaving a dimension while a button is held
  leaves that member to expire only when that level ticks again).
- The sender is not checked for actually holding a controller on the server (the packet carries the frequencies);
  acceptable for the single-player / trusting-LAN use of this port, not for a hostile server.
- The `LINKED_CONTROLLER` advancement flag returned by `receive_pressed` is ignored.

## Unverified

Written without a build or game run. Unverified API assumptions: `tick` runs on the client window game only
(`native_window`), the input packet reaches `net_from_client` with the player's uuid, `native_server_players` knows the
sender, and `stack_in_mut` write-back of the held stack is synced to the player. The acceptance test
(`stage4/acceptance/Test-HomesteadController.ps1`) covers commands, timeout, release and right-click binding, not keys.