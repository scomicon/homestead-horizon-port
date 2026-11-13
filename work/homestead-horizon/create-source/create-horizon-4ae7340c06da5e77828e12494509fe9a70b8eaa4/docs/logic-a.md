# Logic mods, group A

Five small gameplay mods of the Homestead pack, ported as additive injections of the
`homestead` native bundle. None of them was ported from source: three (Double Doors, No Animal
Tempt Delay, Shulker Drops Two) are "all rights reserved" and their jars were not read; Accelerated
Decay is GPL-3.0 and was not read either; Invisible Frames is MIT. Behaviour comes from the public
descriptions and READMEs of the mods (Modrinth, GitHub, serilum.com), with the defaults the pack
ships. Anything not stated there is listed under "Differences" and is an approximation, not parity.

Files (all under `mixins/homestead/`): `doubledoors.rs`, `shulkerdrops.rs`, `invisibleframes.rs`,
`accdecay.rs`, helpers `doors_click.rs`, `shulker_drops.rs`, `frames_interact.rs`,
`decay_changed.rs`, `decay_tick.rs`, `tempt_calm.rs`, and `fragments/logicA.toml` (4 modules, 6
injections). Acceptance: `stage4/acceptance/Test-HomesteadLogicA.ps1` (12 scenarios, prefix
`stage25-logica-`).

## Double Doors 7.2

Clicking one half of a wooden double door opens or closes the other one too. A pair is two doors
of the same block, the same facing and opposite hinges, standing next to each other on the side
away from each hinge. The partner is set to the state the clicked door is about to take, on both
of its halves. Sneaking disables the feature (only the clicked door moves). Iron doors are not
opened by a hand, so they are left alone.

Hook: `right_click_block` HEAD, never cancels. It reads the state before the door's own use, so
the partner is written to the opposite of the clicked door's current `open`.

Differences from the original:
- Fence gates and trapdoors (config off by default), recursive opening (up to 10 blocks), buttons,
  pressure plates and villagers opening a pair are not ported.
- `doorBlockBlacklist` is not ported. Mod doors (`*_door` blocks that carry `half`, `hinge`,
  `facing`, `open`) pair the same way.
- The partner moves silently: the original plays its own sound; this port only plays the clicked
  door's sound.

## Shulker Drops Two 3.5

A shulker whose loot table rolled a shell (vanilla 50 %, looting raises it) drops two shells
instead. The mod's README says the amount is only altered when one would have dropped.

Hook: `loot_drops` HEAD, for the entity `minecraft:shulker`: a rolled `minecraft:shulker_shell`
stack below two is raised to two. Stacks above two are left alone.

Differences: the options ("always drop", amounts up to 64) are not ported, only the default two.

## Invisible Frames 1.1.0

Sneaking while right-clicking an item frame (or glow item frame) toggles the frame's invisibility.
The vanilla `Invisible` flag is written through `native_entity_merge_nbt`, which reloads item
frames. The click is taken (the engine swings the arm), so the frame's own rotate/insert does not
run on that click.

Hook: `interact_entity` HEAD (server side, once per hand).

Differences: client-only parts of the mod are not needed (the flag is vanilla). Any invisible
frame still shows its item, as in vanilla.

## Accelerated Decay 3.0.1

Once the last log supporting a set of leaves is gone, leaves that are ready to decay are removed
as soon as they can be, instead of waiting for random ticks. The mod delays its own leaf check so
the game can mark the leaves ready first; it needs no configuration.

Port: `block_changed` records a removed log (a block of `#minecraft:logs` replaced by a
non-log) in a bounded queue (128 entries). `level_tick_post` counts five ticks down, then sweeps
the cube of radius 7 around it: leaves (`*_leaves`) with `persistent=false` and `distance=7`
(vanilla's own "ready to decay" state) drop their loot and are removed. If anything non-persistent
was still standing in range or removed, another sweep is queued, up to six sweeps, because
removing leaves lets distances climb for the leaves behind them.

Differences: the original's timing values are not published, so five ticks and six sweeps are
this port's estimates. Logs are recognised by name suffix (`_log`, `_wood`, `_stem`, `_hyphae`),
leaves by `_leaves`, not by tags. Decay does not play the particles of random-tick decay.
Removals by explosions and pistons also queue a sweep (any log replaced by a non-log).

## No Animal Tempt Delay 1.2

Vanilla's `TemptGoal` sets a 100 tick calm-down when it stops (food put away, line of sight lost);
until it ends the animal ignores food. The mod removes that delay: an animal that sees its food is
always tempted.

Port: a narrow injection (`<Tempt as Goal>::can_use`, HEAD) clears the goal's private `calm_down`
before every check. This covers the animals that use the shared `Tempt` goal (cow, sheep, pig,
chicken, horse family, and the others that register it). Animals with their own temptation code
(camel, sniffer, armadillo, happy ghast, ocelot's `OcelotTempt` wrapper only through the shared
goal) keep their own behaviour.

Differences: the `can_scare` rule (a moving player scares some animals off) is vanilla's and is
left as it is.

## Unverified

Nothing here was compiled or run when this document was written. Assumptions to confirm on the
first build: the `interact_entity` hook fires for item frames, `Game::native_lifecycle.epoch()`
is reachable from the injected module, `Tag::Byte` and `Compound::default()/insert`, the helper
bindings for `loot_drops`, and that the trait-impl target ID `<Tempt as Goal>::can_use` resolves
(the `horizon_client_lib` crate holds `mobs`).
