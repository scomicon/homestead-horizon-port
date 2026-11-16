# Client HUD and screens (group H): Detail Armor Bar, Better Mount HUD, Traveler's Titles, Welcome Screen

Four client-side mods for the `homestead` bundle, rebuilt from their public descriptions. No code,
text or assets of the originals were read or copied (Better Mount HUD, Traveler's Titles are
copyleft, Welcome Screen is all rights reserved, Detail Armor Bar is permissive; the jars are not
in this tree). They run **only on the window's client**: every one starts with `native_window()` or
sits on a client hook (`gui_layer_pre/post`, `health_hud`, `screen_render_post`,
`screen_mouse_button_pressed_pre`, `screen_key_pressed_pre`) or on the client tick
(`native_client_tick_post`, which the game fires 20 times a second in the window only). None of them
uses a server-thread hook (`level_tick_pre`, `use_item`, `native_title` …), the mistake that made
Stack Refill and CraftingPad do nothing in single player.

Source: `mixins/homestead/armor_bar.rs`, `mount_hud.rs`, `travel_titles.rs`, `welcome_screen.rs` and
their helpers; the injections are in `mixins/homestead/fragments/clientH.toml` (4 modules, 9
injections, ids `clienth-*`). Acceptance: `stage4/acceptance/Test-HomesteadClientH.ps1` (`-ClientH`).

## Done

### Detail Armor Bar

Vanilla draws at most ten armor icons (20 points). After the `minecraft:player_health` layer this
adds, on top of the armor row:

- **extra rows by tint**: each further 20 points repaints the ten icons gold, then cyan, then violet
  (34 points: a full vanilla row and seven gold icons);
- **a readout** to the left of the row: `24 T4 (62%)`: armor points, toughness when there is some,
  and the share of a reference hit of 8 damage that armor and toughness take off, worked out as
  vanilla's `CombatRules.getDamageAfterAbsorb` does.

The row's height follows vanilla's layout (`HealthHud::layout`): rows of hearts stack from
`height - 39`, each `10 - (rows - 2)` high (at least 3), counting absorption.

### Better Mount HUD

While riding a living mount in survival or adventure: vanilla's food row is kept (vanilla drops it
for the mount's hearts), the mount's hearts are drawn one row above the food, and a readout sits to
the right of them: `12/30  9.7 b/s  jump 60%` (the mount's health, its top speed from the
movement-speed attribute at 43.17 blocks per second per 1.0, and the jump charge while a jump is
charging). Done by hiding vanilla's `minecraft:vehicle_health` layer and answering `health_hud`
with Horizon's own layout (called with the vehicle count at 0) plus the mount's hearts. In creative
mode vanilla's hearts stay (the health rows are not drawn there).

### Traveler's Titles

A banner (the name scaled 2.5 times, gold, fading in over half a second, held for 2.5 seconds,
fading out over one) when the player walks into another biome, and the dimension's name with the
biome as a subtitle when they change dimension. A new biome has to hold for a second before it
counts, banners keep two seconds apart, and the first reading after joining announces the biome.
Names come from the language files (`biome.<namespace>.<path>`) with the id in words as the
fallback (`minecraft:dark_forest` is `Dark Forest`). Painted after the `minecraft:title` layer, so F1
hides it like a vanilla title. Each banner also logs `Homestead titles: <title> / <subtitle>`
(that line is what the acceptance test looks for).

### Welcome Screen

A mod screen (`native_mod_screen_open`) with a title, four lines and a footer, shown once per
install: five seconds after the player has started playing, when the marker
`horizon-mods/.horizon-mod-data/homestead/welcome-shown` does not exist; the marker is written when
it opens. The world keeps running under it; Escape closes it, and so do a left click and the Enter
or Space key (those only raise a flag, the next client tick closes the screen, because the screen
hooks are read-only). The text can be replaced by the lines of
`horizon-mods/.horizon-mod-data/homestead/welcome.txt` (first line is the title, at most ten
lines of 64 characters). Logs `Homestead welcome screen opened`.

## Differs from the originals

- All behaviour is a reconstruction from descriptions, not from the mods: look, wording, colours,
  timings (banner 80 ticks, settle 20, quiet 40), the 8-damage reference hit and the 43.17
  blocks-per-second factor are this port's own choices.
- **Detail Armor Bar:** it does not replace the armor row, it paints over it, and it works with
  the attribute values the client has (`minecraft:armor`, `armor_toughness`, `max_health`); it
  places the readout using the player's absorption when the player object is readable, so a modded
  layout of the health rows would misplace it.
- **Better Mount HUD:** the jump bar and the experience bar stay as vanilla draws them (this port
  does not move or combine them); there is no live speed (the readout shows the mount's top speed, the
  attribute), and the mount's air row is not shifted (bubbles can overlap the second row of a very
  large mount's hearts while swimming).
- **Traveler's Titles:** no per-biome configuration, no "only the first time" memory, no
  translations beyond what the game's language files have; the banner stays up while a screen is
  open (the HUD pass draws under screens).
- **Welcome Screen:** one fixed layout, English default text, no buttons or images, no
  configuration screen; the lines come from the optional text file.

## Not verified

Nothing here has been compiled with the real client or run in the game.

- **Compiled only against stubs.** The four modules and their 11 unit tests were built and run in a
  scratch crate with stand-in types for `Game`, `HudBatch`, `GuiDraw`, `HealthInputs`, `Text`
  and `NativeScreenView` (the pure logic and the borrow structure pass; the real signatures do not
  get checked until the first build). Likely first-build fixes: the field names `creative`, `mobs`,
  `mount`, `player` on `Game` (private fields, reachable because the modules sit under `game`),
  `player.health.absorption()` (needs `&mut`? the code uses `as_mut()`), `NativePlayer`'s field
  types, `Text::literal`'s parameter type, and whether the injection tool accepts `target = …` for
  `native_client_tick_post` (it is not a named hook; `native_trample` is injected the same way).
- **Health HUD ownership.** Better Mount HUD answers `health_hud`, which replaces every row; if a
  later mod also answers it, the first answer wins (the helper returns when `draws` is already
  set).
- **The look of every element** (positions, tints, text scaling through `GuiDraw::Push` with
  `PoseOp::Scale`, readability on small GUI scales) is unseen. The acceptance test saves two
  screenshots (`stage26-clienth-armor.png`, `stage26-clienth-mount.png`) for a human to look at.
- **Verified by the acceptance test only when the game cooperates:** the welcome screen opening,
  closing on Escape and writing its marker, and the Nether banner line in the log. The
  `execute in …` hop, the `attribute` command, riding a horse and `horizon-ctl logs` are assumed to
  exist; where they do not, the test records a note instead of passing or failing.
- **Not driven:** the click, Enter and Space closing of the welcome screen; Better Mount HUD's
  jump charge; mount hearts beyond ten (second row); absorption hearts for the armor readout.
- The welcome screen would pop up in every automated suite; `Start-Test.ps1` therefore has to write
  the marker (see `fragments/clientH.md`). Package users see it once, as intended.

## Left out on purpose

Chat Heads, Inventory HUD+, AppleSkin and others of the 57 client mods need hooks the client does
not have yet (see `stage4/research/client-mods.md`, section 6); this group only takes what the
current client hooks allow.
