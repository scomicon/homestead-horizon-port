# Client mods group H (stage 20 slice): shared-file edits for the lead

New files (paths relative to the create-source checkout `CS`, and to `work/homestead-horizon/stage4` = `ST`):

- `mixins/homestead/armor_bar.rs`, `mount_hud.rs`, `travel_titles.rs`, `welcome_screen.rs` (modules
  `homestead_armor_bar`, `homestead_mount_hud`, `homestead_travel_titles`, `homestead_welcome`)
- helpers `armor_bar_layer.rs`, `mount_hud_hide.rs`, `mount_hud_health.rs`, `travel_titles_tick.rs`,
  `travel_titles_layer.rs`, `welcome_tick.rs`, `welcome_draw.rs`, `welcome_click.rs`, `welcome_key.rs`
- `mixins/homestead/fragments/clientH.toml` (4 `[[module]]`, 9 `[[injection]]`, ids prefixed `clienth-`)
- `docs/client-hud.md`
- `ST/acceptance/Test-HomesteadClientH.ps1` (prefix `stage26-clienth-`, windowed)

Nothing was built, run or committed. The fragment parses, its ids do not clash with the bundle or
the other fragments, every injection's hook name and version matches `native-hooks.json`, and every
helper/source file it names exists. The four modules and their 11 unit tests compile and pass in a
scratch crate with stub types (see docs/client-hud.md, "Not verified"). Another fork authors the
tooltip group (`client_tooltips.rs`, `ctip_*.rs`); none of this slice's names collide with it.

## 1. `mixins/homestead/horizon-mixin.toml`

Append `fragments/clientH.toml` at the end. Bump `version` to the next one
(`0.1.0+homestead.10`, or the lead's choice if the tooltip fragment bumps it too: one bump for
both is enough).

Hooks used, all client side: `gui_layer_pre`, `gui_layer_post` (twice), `health_hud`,
`screen_render_post`, `screen_mouse_button_pressed_pre`, `screen_key_pressed_pre`, and the method
`native_client_tick_post` (named by `target`, twice; it is called from `game.rs` around line
16862 and has no catalog entry).

## 2. `ST/Run-Acceptance.ps1`

- `param(...)`: append `,[switch] $ClientH` before the closing parenthesis.
- Line 16 (`HOMESTEAD_WINDOWED`): change the condition to `if ($Motor -or $ClientDefaults -or $ClientH)`,
  the HUD is only drawn in a real window.
- `$reportName=`: put `if ($ClientH) {'run-clienth-result.json'} elseif ...` at the front of the chain.
- In the `$runs` branch chain add, in the shape of the neighbouring `-ClientDefaults` branch:
  ```powershell
  } elseif ($ClientH) {
      $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadClientH.ps1'),'ClientH','stage26-clienth-'))
  ```

## 3. `ST/regress.ps1`

Append `,@('clienth',@{ClientH=$true})` inside `$sets=@(...)` (after the last entry).

## 4. `ST/Build-Package.ps1`

Add `'run-clienth-result.json'` to the `$runs` receipt list and a provenance line such as
"Client HUD: Detail Armor Bar, Better Mount HUD, Traveler's Titles, Welcome Screen".

## 5. `ST/Start-Test.ps1` (needed, otherwise every suite meets the welcome screen)

Every other suite must start with the welcome marker present. After the block that applies the
client defaults (around line 64), add:

```powershell
# Welcome Screen (clientH): other suites must never see it; -ClientH deletes the marker itself.
$welcomeDir=Join-Path $game 'horizon-mods/.horizon-mod-data/homestead'
New-Item -ItemType Directory -Force -Path $welcomeDir | Out-Null
[IO.File]::WriteAllText((Join-Path $welcomeDir 'welcome-shown'),'1')
```

`Test-HomesteadClientH.ps1` removes the file to provoke the screen and writes it back in its
`finally`. Packages and `Start-Homestead.ps1` need no edit: a player is meant to see the screen once.

## 6. Docs

Optional: roadmap rows for the four mods (client slice K2) in `ST/research/roadmap.md`, and the
status column of the four mods in the table of `ST/research/client-mods.md` (API-NOW -> done,
unverified).

## Risks for the first build (all unverified)

- The helpers read private fields of `Game` (`creative`, `mobs`, `mount`, `player`); this works
  only because the modules are compiled inside `game.rs`, as `mounts.rs`'s own code does. If one is
  not visible, add a `pub(super)` accessor in the engine or drop that feature.
- `p.health.absorption()` may need `&mut`; the code uses `player.as_mut()`.
- `Text::literal(&String)` and `GuiDraw::Text`'s field set follow `hud_layout.rs` at the pinned
  revision; a change there breaks the banner and the readouts at compile time, not at run time.
- Whether `target = "...::native_client_tick_post"` is accepted for an injection without a
  catalog hook. The fallback is to use the `frame` hook (client, once a frame) for the two tick
  helpers and count frames instead of ticks (the banner timings are in ticks: scale by the frame
  time).
- `native_mod_screen_open` refuses while a loading overlay, a dialog or another screen is up; the
  tick then retries every five seconds (the marker is written only when it opened).
- The acceptance test assumes `horizon-ctl` commands `logs --lines N`, `entities R TYPE`,
  `summon`, `interact-entity`, `screenshot`, and server commands `execute in … run tp`,
  `attribute`, `data merge entity`, `kill @e[...]`; each missing one turns its scenario into a note.
