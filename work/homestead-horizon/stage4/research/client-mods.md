# Клиентские моды Homestead: что делать с интерфейсом, HUD, звуком, ресурспаками и шейдерами

Исследование только на чтение (код не менялся, игра не запускалась). Источник фактов: исходники клиента Horizon
(`work/horizon-source/horizon-<rev>/`, ниже пути относительно этой папки), `native-hooks.json`, `docs/*.md`, а также
`work/homestead-horizon/pack/coverage.json` и папки `resourcepacks/` и `shaderpacks/` установки Homestead.
Что подтверждено файлом, а что нет, помечено. Усилия: **S** до дня, **M** 2-4 дня, **L** больше недели.

## 0. Главные выводы

1. **Пометка «нужны клиентские интерфейсы» в `coverage.json` устарела.** Клиент Horizon уже даёт нативным модам
   богатый клиентский API: 60 хуков с `side: client` и 10 с `side: both` из 432 (`native-hooks.json`), плюс 177
   помощников. В документации прямо названы примеры: миникарта (в духе Xaero), боевая музыка, Ponder (`docs/native-mods.md`,
   разделы «What a client HUD reads», «Client world snapshots», «Scenes in screens»).
2. **Часть модов уже встроена в клиент** или есть как официальные нативные моды Horizon: Mouse Tweaks, динамический
   свет, 3D-слои скина, Mod Menu, зум, миникарта, просмотр рецептов (аналог JEI), CIT и связные текстуры.
   Их достаточно включить и настроить по умолчанию.
3. **Ресурспаки Java в Horizon грузятся** (`docs/resource-packs.md`: 32 строки «works», 5 «partly», 7 «no»).
   Пять ресурспаков сборки загрузятся, но они на формате 1.20 и частично мимо 26.3.
4. **Шейдеры Iris/OptiFine сейчас не работают.** Прежний загрузчик Horizon удалён и пишется заново
   (`roadmap/roadmap.toml`, пункт `shader-packs`, статус `next`; `horizon-port-pack` помечает `shaderpacks/` как
   `unsupported`). Все 7 шейдерпаков сборки пока выбывают, а лицензии у большинства запрещающие.
5. Хуки клиента работают на потоке окна. Серверные хуки (в том числе наши `level_tick_pre`, `use_item`) на нём не
   работают: поэтому Stack Refill и CraftingPad не заработали. Список безопасных хуков в разделе 2.

## 1. Что уже есть в клиенте Horizon

| Область | Что есть | Доказательство |
| --- | --- | --- |
| Загрузка ресурспаков | папки и zip в `resourcepacks/`, `pack.mcmeta` (`min_format`/`max_format`, оверлеи, фильтры), выбор на вкладке паков, слияние `lang`, `sounds.json`, шрифтов, атласов | `docs/resource-packs.md` («How it works», таблица), `packs.rs`, `game/packs.rs` (`reload_packs`) |
| Текстуры, модели, GUI | любой размер текстур, анимация (без `interpolate`), blockstate/модели/items, GUI-спрайты (`textures/gui/sprites/`, `nine_slice`), шрифты bitmap/space/unihex; частично `stretch_inner`/`tile` | `docs/resource-packs.md` |
| CIT / CTM / эмиссия | CIT Resewn-формат (`optifine/cit`, `citresewn/cit`) частично, связные текстуры (Continuity/OptiFine) частично, `_e.png` эмиссия, случайные текстуры мобов (ETF), кастомные модели мобов `.jem` (OptiFine/EMF) частично | `docs/resource-packs.md`, крейт `crates/horizon-optifine`, `game/custom_sky.rs` |
| Небо | OptiFine custom sky (`optifine/sky`), облака, звёзды, цвета травы/листвы | `game/custom_sky.rs`, `docs/resource-packs.md` |
| Zoom, FPS, Keystrokes, Minimap, Recipes | официальные нативные моды в репозитории `horizon-rust/horizon-mods` (`native/zoom`, `native/minimap`, `native/recipes`, ...); сам клиент даёт ядро (отрисовка карты, счётчики) | `docs/mods.md` («Horizon's official mods»), `docs/extras.md` |
| Extras (по умолчанию ВЫКЛЮЧЕНЫ) | fullbright, **динамический свет**, **Mouse Tweaks-подобное управление и сортировка**, плавный чат, страница музыки, свой ник-плейт, скрытие частиц ломания, авто-переподключение | `docs/extras.md`, `horizon-client-render/src/extras.rs`, `game/tweaks.rs`, `game/dynamic_lights.rs` |
| 3D-слои скина | опция `threeDSkinLayers` | `game/skin_models.rs` |
| Список модов | Options > Mods, `native_mods`, `native_settings` | `docs/mods.md` («Playing with mods»), `native_hooks.rs` |
| Видеонастройки | страница заменяет Sodium: графика, шейдеры, HDR, FSR и др. | `docs/graphics.md` |
| Чат | 100 строк истории (как ваниль), история команд 50, плавный чат (extra) | `horizon-client-render/src/chat.rs` (заголовок файла) |
| Тосты | `native_toast_add(kind, id, cancel)` отменяет тосты (рецепты, подсказки, достижения по id) | `docs/native-mods.md` («Toasts a mod hides») |
| Скриншоты | `native_take_screenshot()` как F2, возвращает путь файла | `docs/native-mods.md` («Screenshots a client mod takes») |
| Шейдеры Horizon | собственный формат `.hshader` (WGSL: стадии sky, material, weather, post), пример `golden_hour`, горячая перезагрузка | `docs/shaders.md`, `crates/horizon-shader-api` |

Чего нет или не работает (по таблице `docs/resource-packs.md`): `interpolate` у анимации, TTF-шрифты, шрифты из текстовых
компонентов, паковые GUI-флаги `stretch_inner`/`tile`, трим на надетой броне, титры, locator bar, пост-эффекты и core-шейдеры
(кроме текста GUI и обводки блока).

## 2. Клиентский API для нативных модов

Подтверждено по `native-hooks.json` (поле `side`), `docs/native-mods.md` и `crates/horizon-client/src/game/native_hooks.rs`.

**Счёт хуков:** `client` 60, `both` 10, `server` 270, поле `side` пустое у 92 (в основном старые: классифицированы ниже по докам).

**Хуки и помощники, работающие на клиенте (поток окна):**
- Кадр и отрисовка: `frame`, `world_drawn`, `hand_drawn`, `render_hand`/`render_hand_pose`, `gui_layer_pre/post`, `gui_render_post`
  (в нём `gui_hidden`), `block_highlight`, `render_fog`, `compute_fov`, `compute_fov_modifier`, `camera_pose`, `camera_distance`,
  `compute_fog_color`, `item_decorations`, `item_bar`, `use_animation`, `render_name_tag`, `render_item_in_frame`.
- Экраны: `screen_opening`, `screen_init_post`, `screen_render_post`, `screen_closing`, `screen_key_pressed_pre`,
  `screen_char_typed_pre`, `screen_mouse_*_pre` (отмена), `screen_mouse_scrolled_pre`, `menu_background`; экраны мода
  `screen`, `screen_draw`, `screen_click`, `screen_tooltip`, `mod_screen`/`mod_screen_open`.
- HUD: `hud` (боксы по углам, `native_fill`/`native_text`), `health_hud` (свои ряды здоровья/брони/еды/воздуха),
  `native_hud`, `native_hud_mode`, `native_local_attribute`, `client_state` (`screen`, `debug_screen`, `gui_hidden`, `world`, `sleeping_pos`),
  `native_player`, `native_camera`, `native_clock`, `native_biome`.
- Ввод: `key` (`native_key(name, pressed)`), `input_key`, `mouse_button_pre/post` (отмена), `mouse_scrolling` (отмена), `wheel`,
  `interaction_key` (отмена), `movement_input`, `mod_key`, `setting`.
- Звук: `native_play_local_sound`, `native_muffle_sounds` (глушение всего звука), `music`, `music_layers`,
  **`native_sound_play`** (`sound_playback.rs`, API `sound_play`: перехват каждого запуска звука, можно менять громкость и
  высоту или отменить).
- Мир на клиенте: `client_world_snapshots` (`native_client_chunks`, `native_client_biomes`), `client_entities`,
  `client_heads`, `client_profiles`, `native_raycast`/`native_view_hit`, `native_world_hold`.
- Тултипы: `tooltip` (добавить/изменить строки), `tooltip_component` + `_draw`, `tooltip_color`, `gui_tooltip`.
- Сеть и чат: `client_chat_received` (отмена), `native_send_chat`, `native_open_chat`, `native_chat_input`, `native_send_packet`,
  `native_net_send_to_server`/`net_from_server` (канал `horizon:mod_net` между серверной и клиентской половинами мода),
  `client_packet`, `client_level_load/unload`, `client_start`, `client_stopping`, `client_resources_reloaded`.
- Прочее: `native_client_tick_pre/post`, `native_shader_params`, `native_biome_colors`, `native_toast_add`,
  `native_take_screenshot`, `native_draw_scene`, `native_mod_image` (RGBA в атлас HUD, до `MOD_IMAGE_SIZE`),
  `native_client_attack`, `native_client_hurt`, `mob_submit`, `player_submit`, `mob_layer`.

**Работают и на клиенте, и на сервере:** `tick` (`native_tick`: «once a tick, in every Game»: окно и серверный поток; различать
через `native_window()`), `modify_default_components`, `critical_hit`, `living_*`, `register_configs`.

**Серверные: нельзя использовать для клиентских функций** (нет локального игрока, экрана, инвентаря): `level_tick_pre`,
`use_item`, `use_item_on`, `destroy_block`, `place_block`, `right_click_block`, `block_entity*`, `native_food_level_changed`,
`native_title` (отправка титла игроку; на клиенте возвращает false), `native_send_particles`, `native_attack_entity`, все
`native_menu_*`, `native_acting_player`. Клиентскую логику строить на `native_client_tick_pre/post`, `native_key`, `native_hud`,
`gui_render_post`, а к серверной половине обращаться по `native_net_*`.

**Чего не хватает** (проверено поиском по `native-hooks.json` и `native_hooks.rs`): чтение еды/сытости локального игрока на
клиенте, данные достижения для тоста, чтение клиентского инвентаря, помощник создания клиентских частиц, запись в
буфер обмена, хук на отрисовку строки чата, позы модели игрока, переопределение прицеливания, добавление слотов в контейнер
ванильного экрана, виджеты заглавного экрана. Подробно в разделе 5.

## 3. Таблица по 57 клиентским модам

Вердикты: **BUILT-IN** клиент уже умеет, **PACK** ресурс/данные без кода, **API-NOW** переносимо текущими клиентскими хуками,
**NEEDS-HOOK** нужен хук движка, **NOT-POSSIBLE** сейчас невозможно, **N/A** к Horizon не относится. Лицензия: `perm`
permissive, `copyleft`, `custom`, `ARR` all-rights-reserved, `restr` restricted (из `coverage.json`). Для ARR и restricted
переносить можно только поведение, переписав с нуля.

| Мод | Лиц. | Вердикт | Обоснование | Усилие |
| --- | --- | --- | --- | --- |
| 3d-Skin-Layers | custom | BUILT-IN | опция `threeDSkinLayers` (`skin_models.rs`); включить по умолчанию | S |
| Advancement Plaques | restr | NEEDS-HOOK | `native_toast_add` даёт только вид и id и умеет отменять, данных достижения (название, иконка) нет | M |
| AdvancementInfo | perm | NEEDS-HOOK | нужны виджеты экрана достижений; хуков на этот экран не нашёл (не проверено до конца) | M |
| AmbientSounds | copyleft | API-NOW | `native_play_local_sound`, `native_biome`, `client_state`, `native_clock` + звуки в контент-моде; лицензия звуков не проверена | M |
| AppleSkin | perm | NEEDS-HOOK | еда/сытость/истощение локального игрока на клиенте не читаются (`native_local_attribute` только атрибуты); рисование есть (`health_hud`) | S после хука |
| Better Climbing | perm | НЕ ПРОВЕРЕНО | что именно делает, из доступных данных не установил; вероятно правка движения (`movement_input`, клиентский) | ? |
| Better Mount HUD | copyleft | API-NOW | `gui_layer_pre/post`, `health_hud`, `client_entities` для транспорта; данные здоровья скакуна проверить | M |
| Celestial Configuration | perm | PACK | небо OptiFine (`custom_sky.rs`) встроено; формат мода (JSON) может не совпасть, нужен конвертер | M |
| Chat Heads | copyleft | NEEDS-HOOK | нет хука отрисовки строки чата и отправителя; головы дают `client_heads`/`client_profiles` | M |
| CIT Resewn | perm | BUILT-IN (частично) | `horizon-optifine`, `item_render/cit.rs`; ограничения в таблице `resource-packs.md` | S |
| Continuity | copyleft | BUILT-IN (частично) | связные текстуры: все методы кроме `name=`; `random` ложится иначе | S |
| Detail Armor Bar | perm | API-NOW | `health_hud` + `native_local_attribute` (броня) | S |
| Drippy Loading Screen | custom | NOT-POSSIBLE | ждёт FancyMenu; экран загрузки в движке (`game/loading_screen.rs`), из мода только `menu_background` | L |
| Eating Animation | perm | NEEDS-HOOK | есть `use_animation` и `render_hand_pose` (первое лицо), позы третьего лица нет | M |
| EnchantmentDescriptions | copyleft | API-NOW | `native_tooltip` добавляет строки; тексты описаний через `lang` (pack «Compat» их только дополняет) | S-M |
| Euphoria Patches | copyleft | NOT-POSSIBLE | надстройка к Complementary (формат Iris), загрузчика нет | - |
| Extreme sound muffler | copyleft | API-NOW | `native_sound_play` (громкость/отмена по событию) + экран/клавиши мода; `native_muffle_sounds` глушит всё сразу | M |
| FabricSkyBoxes | perm | NEEDS-HOOK | встроен только OptiFine-формат неба, свой JSON мода не читается | M |
| FabricSkyBoxes Interop | perm | NEEDS-HOOK | зависит от предыдущего | S после |
| Fadeless | perm | НЕ ПРОВЕРЕНО | назначение не установлено | ? |
| FancyMenu | custom | NOT-POSSIBLE | движок разметки экранов; есть только `screen_*` хуки и `menu_background` | L |
| GeckoLibIrisCompat | perm | N/A | заплатка между GeckoLib и Iris, которых в Horizon нет | - |
| GLFW X11 Fix | perm | N/A | баг окна X11 на Linux | - |
| Indium | perm | N/A | дополнение к Sodium/Fabric Rendering API; у Horizon свой рендер | - |
| Inventory HUD + | ARR | NEEDS-HOOK | чтения клиентского инвентаря в API не нашёл (`native_player_stacks`, `native_carried_items` серверные); переписывать с нуля по поведению | M |
| Jade | restr | API-NOW (частично) | `native_view_hit`/`native_raycast`, `native_block`, `entity_name`, `native_hud`; данные модов (энергия, прогресс) по каждому моду отдельно | L |
| Jump Over Fences | perm | НЕ ПРОВЕРЕНО | правка коллизии; помощник `block_collision` есть, клиентского предсказания не проверял | ? |
| Just Enough Breeding | perm | зависит от JEI | плагин для JEI; нужен плагин-API панели (`native_panel_recipes`) | M |
| Just Enough Items | perm | BUILT-IN | официальный мод «Recipes» (`native/recipes`) и API `native_panel_items`/`native_panel_recipes` | S (поставить) |
| Just Enough Professions | perm | зависит от JEI | как Breeding | M |
| LambDynamicLights | custom | BUILT-IN | Extras `dynamicLights` (по умолчанию выкл.) | S |
| Mod Menu | perm | BUILT-IN | Options > Mods (`docs/mods.md`) | - |
| Modern World Creation | custom | НЕ ПРОВЕРЕНО | вероятно закрыт созданием мира Horizon (`game/world_entry.rs`), не проверял | ? |
| More Chat History | perm | NEEDS-HOOK | история 100 строк зашита в `chat.rs`; нужна настройка движка | S |
| More Enchantment Info | perm | API-NOW | `native_tooltip` | S |
| Mouse Tweaks | perm | BUILT-IN | Extras `mouseTweaks`, `mouseTweaksWheel` (`game/tweaks.rs`) | S |
| No Chat Restrictions | custom | N/A | снимает ограничение чата аккаунта Microsoft; при офлайн-запуске ограничений нет (по `docs/microsoft-account.md`, `chat-filter.md` не сверял) | - |
| No Resource Pack Warnings | perm | PACK | предупреждение о несовместимом формате (`test_pack_screen_parity`): задать `min_format`/`max_format` в наших копиях паков | S |
| NotEnoughAnimations | custom | NEEDS-HOOK | нет правки позы модели игрока (`player_submit` только рисует) | L |
| Particle Rain | perm | NEEDS-HOOK | нет клиентского помощника частиц (есть серверный `native_send_particles`) | M |
| Polymorph | copyleft | NEEDS-HOOK | выбор рецептов в верстаке: хуки экрана крафта и серверный выбор | L |
| Presence Footsteps | perm | API-NOW (частично) | `native_sound_play` заменяет/отменяет шаги по событию; карта «блок -> звук» и звуки поставляет мод; позицию шага проверить | L |
| Radiant Gear | copyleft | PACK (частично) | эмиссия `_e.png` встроена для брони, не для предметов; что именно рисует мод, не установлено | M |
| Reese's Sodium Options | perm | N/A | видеонастройки заменены страницей Horizon (`docs/graphics.md`) | - |
| Screenshot Viewer | perm | NEEDS-HOOK | загрузка PNG из файла в экран: есть `native_mod_image` (ограниченный RGBA) и нет перечня файлов | M |
| ScreenshotToClipboard | perm | NEEDS-HOOK | `native_take_screenshot` даёт путь, записи в буфер обмена нет | S |
| SignCopy | perm | NEEDS-HOOK | хуков на экран редактирования таблички нет | M |
| Smooth Scroll | copyleft | NEEDS-HOOK | сглаживания прокрутки списков в API нет; ценность низкая | M |
| SwingThrough | copyleft | NEEDS-HOOK | `interaction_key` (отмена) и `native_raycast` есть, подмены цели удара нет | M |
| Tiny Item Animations | perm | NEEDS-HOOK | `render_hand_pose` для первого лица есть, остальное нет | M |
| Tooltip Overhaul | copyleft | API-NOW | `tooltip_color` (рамка), `tooltip_component`, `gui_tooltip` | M |
| TrashSlot | ARR | NEEDS-HOOK | слот поверх контейнерного экрана ванили не добавить; удаление через серверную половину (`native_net_*`); переписывать по поведению | M |
| Traveler's Titles | copyleft | API-NOW | `native_hud`/`gui_render_post` + `native_biome` + `client_state` | S-M |
| Visuality | perm | NEEDS-HOOK | клиентские частицы | M |
| Welcome Screen | ARR | API-NOW | `screen_opening`/`client_level_load` + экран мода (`mod_screen`) | S |
| YOSBR | copyleft | PACK | положить стартовые `options.txt`/конфиги установщиком | S |
| Zoomify | copyleft | BUILT-IN | официальный Zoom (`native_zoom`, `compute_fov`) | S |

Итог по 57: BUILT-IN 8 (включая частичные CIT и Continuity), PACK 5, API-NOW 14 (включая частичные), NEEDS-HOOK 20,
NOT-POSSIBLE 4, N/A 6, НЕ ПРОВЕРЕНО 5 (по несколько модов зависимы от JEI).

## 4. Ресурспаки и шейдерпаки установки Homestead

Файлы лежат в папке установки (`resourcepacks/`, `shaderpacks/`). В `pack/resourcepacks.json` пять паков помечены
`available`, а `shaderpacks/` `unsupported: shader-pack conversion is pending`.

### Ресурспаки

| Пак | Формат | Состав | Загрузится в Horizon | Лицензия |
| --- | --- | --- | --- | --- |
| Homestead GUI (папка, 203 файла) | `pack_format` 15 (1.20) | `lang`, `sounds`, `textures` для 12 пространств: `appleskin`, `detailab`, `trashslot`, `supplementaries`, `toms_storage`, `sophisticatedbackpacks`, `sophisticatedcore`, `trinkets`, `trinketlantern`, `backpack_allthemodium_upgrade`, `homestead`, `minecraft` | загрузится, но почти всё про моды, которых у нас нет; часть для ванили 1.20 (пути GUI в 26.3 другие: `textures/gui/sprites/`) | в паке нет файла лицензии; автор не указан; считать «все права защищены» |
| Enchantment Descriptions Compat 1.4 | 15 | только `lang` для `apotheosis` | бесполезен: нет ни мода EnchantmentDescriptions, ни Apotheosis | нет файла лицензии |
| Fancy Beds v4.2 | 15, `min_format` 15, `max_format` 999, оверлей `26-2` для формата 86+ | 158 файлов, кровати | вероятнее всего загрузится: паку заявлена поддержка 26.x | автор указан в описании («Agente_511»), лицензии нет |
| Nobushleaves | 55 | 77 файлов, листва; в `readme.txt` требует быть над Stay True | загрузится (порядок паков поддержан) | лицензии нет, автор «HaiMcFly» |
| Stay True 1.20 | 15 | 3984 файла, текстуры ванили в стиле Stay True | загрузится с предупреждением о формате; переименованные в 26.x текстуры останутся ванильными | лицензии нет, автор «HaiMcFly» |

Выводы: загрузка работает, но нужно проверять паки на 26.3 в игре (я их в игре не запускал). Все пять паков без явной
лицензии: в публичный репозиторий и в публичные релизы их не класть; для локального использования только копировать
в `resourcepacks/` экземпляра и включать в `options.txt`. Выбор и порядок паков хранит `options.txt` (не проверял, как именно
это делает наш `Start-Homestead`).

### Шейдерпаки

В установке 7 паков в формате Iris/OptiFine (`shaders/…`): BSL v8.4, Complementary Reimagined r5.9.3 (и готовые профили
с Euphoria Patches 1.10.5), Complementary Unbound r5.9.3, MakeUp UltraFast 9.4b, Nostalgia v5.1, PotatoShaders v1.0d,
superDuperVanilla.

- **Состояние загрузчика:** у Horizon прежний загрузчик Iris/OptiFine удалён, новый строится по «clean-room» спецификации
  (`docs/cleanroom/shader-packs-spec.md`, крейт `crates/horizon-shaders`: чтение пака, `.properties`, препроцессор, перевод GLSL
  в naga). Дорожная карта: `shader-packs` статус `next` («вернутся, когда лучшие паки будут рисоваться правильно»).
  `docs/mod-browser.md`: паки формата `shaders/` с Modrinth/CurseForge не предлагаются. В `docs/graphics.md` страница
  «Shader Packs» упоминает и такие паки, но противоречия с дорожной картой не проверял: **считать не работающими до проверки в игре**.
  Спецификация называет BSL, Complementary и Sildur тестовыми паками.
- **Собственный формат:** `.hshader` на WGSL (стадии sky/material/weather/post), пример `golden_hour`. Можно написать
  собственный «домашний» пак; готовые паки из Homestead переносить нельзя (см. лицензии).
- **Лицензии паков** (по файлам в zip): Complementary: «Complementary License Agreement 1.7» (запрещает редистрибуцию
  вне нормального использования); Nostalgia и PotatoShaders: «All Rights Reserved» (RRe36); superDuperVanilla: «FlameRender (C)
  Studios License 1.6» (Eldeston, All Rights Reserved unless stated); MakeUp UltraFast: файл `Credits`, лицензии в архиве нет;
  BSL: лицензии в архиве не нашёл. Переписывать эти пакеты на `.hshader` без явного разрешения нельзя.

## 5. План: первые 10 шагов по ценности и усилиям

1. **Включить встроенное по умолчанию** (S): динамический свет, Mouse Tweaks, 3D-слои скина через стартовые файлы
   (`horizon-extras.txt`, `options.txt`) в установщике. Закрывает LambDynamicLights, Mouse Tweaks, 3d-Skin-Layers, YOSBR.
2. **Официальные нативные моды Horizon** (S-M): собрать и поставить Zoom, Minimap, Recipes (замена JEI), FPS/Keystrokes из
   `horizon-rust/horizon-mods`. Закрывает Zoomify, JEI и в основном Xaero-подобные нужды. Проверить лицензию репозитория.
3. **Ресурспаки** (S-M): проверить в игре Fancy Beds, Stay True, Nobushleaves на 26.3, поправить `pack.mcmeta` (`min_format`/
   `max_format`, закрывает No Resource Pack Warnings), прописать в `options.txt` порядок. Homestead GUI пока не включать.
4. **Тултипы** (M): один нативный мод на `native_tooltip`/`tooltip_component`: EnchantmentDescriptions, More Enchantment Info,
   лёгкий Tooltip Overhaul. Тексты описаний чар писать самим (в паке Compat только дополнения).
5. **HUD-набор** (M): Detail Armor Bar (`health_hud` + `native_local_attribute`), Better Mount HUD (`gui_layer_*`),
   Traveler's Titles (`native_hud` + `native_biome`), Welcome Screen (`mod_screen`).
6. **Звук** (M): Extreme Sound Muffler на `native_sound_play`; заготовка Presence Footsteps и AmbientSounds
   (`native_play_local_sound`) после проверки, что `native_sound_play` отдаёт позицию и событие шага.
7. **Jade-lite** (M-L): панель по прицелу (`native_view_hit`, `native_hud`), сначала только ванильные блоки и мобы
   и блоки Create из нашего порта. Переписать по поведению (лицензия restricted).
8. **Дорожная работа вверх по течению**: завести запросы на хуки (список ниже), особенно локальная еда для AppleSkin
   (после хука S) и буфер обмена для ScreenshotToClipboard (S).
9. **Конвертер неба/небесных тел** (M): FabricSkyBoxes и Celestial Configuration (JSON) в формат `optifine/sky`, который
   Horizon уже читает; сначала посмотреть, какой формат реально использует Homestead (не проверял).
10. **Собственный `.hshader`-пак** (M-L): «домашний» вид (цветокоррекция, небо, мягкий пост) на уже работающем нативном API,
    вместо ожидания Iris-загрузчика; готовые Complementary/BSL не использовать (лицензии).

## 6. Хуки и возможности, которые стоит запросить у Horizon

1. Клиентское чтение еды, сытости и истощения локального игрока (`native_local_food`): AppleSkin.
2. Данные достижения в `native_toast_add` (название, описание, иконка) и хуки экрана достижений: Advancement Plaques, AdvancementInfo.
3. Чтение клиентского инвентаря (hotbar и основной): Inventory HUD+, TrashSlot, Jade (предметы).
4. Клиентский помощник частиц (`native_spawn_particle_local`): Particle Rain, Visuality.
5. Запись текста и изображений в буфер обмена: ScreenshotToClipboard, SignCopy.
6. Хук строки чата (отправитель + место в строке) и настройка длины истории чата: Chat Heads, More Chat History.
7. Правка позы модели игрока и мобов (`native_model_pose`): NotEnoughAnimations, Tiny Item Animations, Eating Animation в третьем лице.
8. Хук цели прицеливания (подмена `HitResult`) и отправка атаки/использования из клиента: SwingThrough.
9. Добавление слотов и виджетов на ванильные контейнерные экраны: TrashSlot, Polymorph.
10. Виджеты заглавного экрана и экрана загрузки: FancyMenu, Drippy Loading Screen.
11. Экран редактирования таблички (хуки открытия/текста): SignCopy.
12. Загрузчик шейдеров Iris (идёт): сам Euphoria/Complementary.
13. Ресурспаки: `interpolate` анимации, `stretch_inner`/`tile` GUI, TTF, текстовые шрифты (`docs/resource-packs.md`, «Gaps»).

## 7. Что не установлено и что проверить

- В игре ничего не запускалось: все выводы по коду и документам. Работоспособность `native_sound_play`, `custom_sky`,
  связных текстур и оверлеев паков для Homestead нужно проверить реально.
- Назначение «Better Climbing», «Fadeless», «Jump Over Fences», «Modern World Creation» из данных не выяснил; нужен
  взгляд на их исходники или описания (нет JAR-файлов этих модов в проверяемом месте).
- Лицензии ресурспаков: файлов нет, значит они должны считаться «все права защищены». У BSL и MakeUp UltraFast явной лицензии
  в архиве не нашёл.
- Противоречие в документах Horizon о том, работают ли пакеты `shaders/` формата (graphics.md против roadmap): проверить
  в игре на любом из простых паков.
- 92 хука с пустым `side` в `native-hooks.json` классифицированы мной по комментариям в `native_hooks.rs`;
  может понадобиться уточнение по каждому при использовании.
- Репозиторий `horizon-rust/horizon-mods` (официальные моды) в этом дереве нет: наличие и лицензию Zoom/Minimap/Recipes
  проверить отдельно.

## 8. Сделано (волна 1: значения по умолчанию, официальные моды, ресурспаки)

Проверено в настоящей игре (набор приёмки `-ClientDefaults`, 5 проверок, окно игры, один клиент) или только прочитано в коде, помечено.

### 8.1 Встроенное по умолчанию (шаг 1 плана)

- **Где хранятся настройки.** `horizon-extras.txt` рядом с `options.txt` в папке экземпляра, строки `ключ:значение`
  (`docs/extras.md`, `horizon-client-render/src/extras.rs`). `options.txt` не трогаем. Ключи, которые мы включаем
  (по коду `extras.rs`): `dynamicLights:true` (LambDynamicLights), `mouseTweaks:true` и `mouseTweaksWheel:true`
  (Mouse Tweaks), `threeDSkinLayers:true` (3d-Skin-Layers). Mod Menu это Options > Mods, включать нечего. CIT и
  связные текстуры встроены и включаются самим ресурспаком. `zoom`, `fps`, `minimap` ушли в официальные нативные
  моды (п. 8.2), в `horizon-extras.txt` их включать бессмысленно.
- **Как включаем.** `stage4/package/Apply-ClientDefaults.ps1` (только ASCII, читается Windows PowerShell 5), функция
  `Set-HomesteadClientDefaults`. Её вызывает `Start-Homestead.ps1` после создания папки экземпляра (ключ `-NoClientDefaults`
  отключает). Правила: ключ, который уже есть в файле, не меняется (даже `false`: игра сама переписывает весь файл,
  когда игрок что-то меняет, поэтому существующий ключ считается выбором игрока); недостающие ключи дописываются;
  файл `.homestead-client-defaults` в папке экземпляра отмечает, что значения применены, и удалённый игроком ключ
  второй раз не добавляется. `Build-Package.ps1` кладёт скрипт в корень пакета.
- **Проверено в игре:** файл создаётся с 4 ключами; игра читает его: на странице Extras написано
  «Mouse Tweaks controls: On» (`options extras` + `ui`, окно игры). **Только по коду:** `dynamicLights` и `threeDSkinLayers`
  как ключи (их страницы настроек в списке видимых виджетов не нашлись без прокрутки, проверка на странице не сделана).
  Правила слияния проверены на временной папке (значение игрока `false` сохраняется, повторный запуск ничего не делает).

### 8.2 Официальные нативные моды Zoom, Minimap, Recipes (шаг 2 плана)

- **Что это.** Нативные моды Horizon (`horizon-mixins`, Rust): исходники в `native/<имя>/` репозитория
  `horizon-rust/horizon-mods`, их собирает в клиент лаунчер Horizon («Mods» страница). Это не `.hmod`-файлы, которые можно
  положить в папку: они вплавляются в клиент при сборке.
- **Почему не кладём в пакет.** (1) У репозитория `horizon-mods` на его странице не указана лицензия и файла `LICENSE`
  в корне нет, то есть условий распространения мы не знаем (страница реестра описывает лицензии принимаемых модов, не свою).
  (2) Наш клиент собирается нашим скриптом из исходников Horizon плюс наши миксины; чтобы добавить Zoom/Minimap/Recipes, их
  надо подключить как ещё три бандла в `horizon-mixins apply` и пересобрать клиент (Rust-сборка, в этой волне не делалась).
  Поэтому `Install-Homestead.ps1` не получил шага установки: runtime-установка невозможна, лицензия не подтверждена.
- **Что дальше.** Выяснить лицензию (запрос авторам или файл лицензии в `native/<имя>/`), затем добавить бандлы в сборку.
  До этого JEI, Zoomify и Xaero-подобная карта в нашей сборке недоступны.

### 8.3 Ресурспаки (шаг 3 плана)

- **Как включить пак.** Положить папку или `.zip` в `resourcepacks/` экземпляра (для пакета `private/instance/resourcepacks`)
  и либо выбрать его на экране Resource Packs (Options > Resource Packs, как в ванили), либо вписать в `options.txt`
  строку `resourcePacks:["vanilla","file/<имя папки или zip>"]` (нижний пак первым; идентификатор всегда `file/` и имя файла).
  Команда управления: `horizon-ctl options-set resourcePacks '["vanilla","file/<имя>"]'` (применяется сразу).
- **Проверено в настоящей игре** (приёмка `-ClientDefaults`, пак, который генерирует тест: языковой файл + одна текстура
  камня): пак загружается (текст «Set own game mode to HS CREATIVE …» вместо «Creative Mode»), текстура камня
  рисуется красной (скриншот), и при пустом списке всё возвращается. Перезагрузка с 48 модами занимает от 20 секунд до
  примерно двух минут; в тесте ждём до 8 минут. Имя пака в тесте уникально для каждого запуска: Horizon переиспользует
  собранный оверлей для паков с тем же именем, и правка файлов под прежним именем иногда не подхватывалась (замечено в игре).
- **Smoke-проверка паков сборки Homestead** (локально, пять паков из экземпляра пользователя временно копировались в
  папку тестового экземпляра `stage4/runtime`, которая не в git, и удалены после проверки; в репозиторий и пакет не попали).
  Порядок включения: Fancy Beds, Enchantment Descriptions Compat, Stay True 1.20, Nobushleaves, Homestead GUI
  (Nobushleaves выше Stay True, как просит его readme). Результат по логу и по собранному оверлею
  (`.horizon-cache/resourcepacks/<хэш>`), одна перезагрузка около 20 секунд:

  | Пак | Результат |
  | --- | --- |
  | Fancy Beds v4.2 | загрузился: 32 из 32 файлов `assets` в оверлее |
  | Enchantment Descriptions Compat 1.4 | загрузился: в оверлее `assets/apotheosis/lang/en_us.json`, но мода Apotheosis нет, пользы нет |
  | Stay True 1.20 | загрузился: 3854 файла, в выборке 385 из 400 совпали, 15 перекрыты паками выше |
  | Nobushleaves | загрузился: 57 файлов, 46 совпали; 11 файлов `blockstates/*_leaves.json` в оверлее другого размера (причина не выяснена: возможно, их переписывает движок или перекрывает мод; визуально не проверялось) |
  | Homestead GUI | загрузился: 187 из 187 файлов (почти всё про моды, которых в нашей сборке нет) |

  В логе нет ни одного предупреждения про эти паки, `incompatibleResourcePacks` остался `[]`. Предупреждения «missing block
  model» и «missing texture» в логе относятся к модам-оболочкам (candlelight, supplementaries и другие) и появляются и без паков.
  **Не проверено:** как выглядят паки на экране, Stay True на кораблях и блоках 26.3 (переименованные текстуры), шейдеры.
- **Лицензии паков** без изменений (раздел 4): в паки не положены, не копируются в пакет и репозиторий.
- `No Resource Pack Warnings`: предупреждение о формате в этих запусках не возникало (`incompatibleResourcePacks:[]`), задавать
  `min_format`/`max_format` в наших копиях паков не пришлось.

### 8.4 Приёмка и что должен сделать пользователь

- Набор `-ClientDefaults` в `Run-Acceptance.ps1` и `regress.ps1` (файл `acceptance/Test-HomesteadClientDefaults.ps1`,
  квитанция `run-clientdefaults-result.json`, префикс `stage32-clientdefaults-`); `Build-Package.ps1` требует его квитанцию.
  Набор запускает игру в окне (как `-Motor`). `Start-Test.ps1` в обычных наборах удаляет `horizon-extras.txt`, чтобы остальные
  наборы шли на ванильных значениях.
- **Пользователю:** для готового пакета ничего, значения применяются при первом запуске. Чтобы включить ресурспак, нужен
  блок «Ресурспаки» из `research/install-client-section.md` (его надо перенести в `INSTALL.md` публичного репозитория;
  в этой волне публичный репозиторий не менялся). Игроку, у которого `horizon-extras.txt` уже есть, значения не меняются.

## 9. Готово в stage20 (подсказки, HUD, звук)

| Мод | Состояние |
| --- | --- |
| EnchantmentDescriptions, More Enchantment Info, Tooltip Overhaul (лёгкий) | сделано (-ClientT, 7 сценариев); вид на экране не проверялся |
| Detail Armor Bar | сделано, скриншот просмотрен (подпись 30 (80%) частично закрыта чатом); -ClientH |
| Traveler's Titles, Welcome Screen | сделано, проверено по логу и состоянию экрана; вид баннера и экрана не проверялся |
| Better Mount HUD | написан, **не проверен** (тест не может посадить игрока на лошадь) |
| Extreme Sound Muffler, Presence Footsteps, AmbientSounds | движок и конфиги сделаны; **перехват звука не подтверждён**: в тестовой среде хук 
ative_sound_play не вызывается ни для playsound, ни для ambient (hook_seen=0), слышимость и клавиши F9/F10 не проверены |

Урок по хуку звука: 
ative_sound_play лежит в нижнем крейте horizon_client_lib, который не видит game; хук работает через мост homestead_sound_bridge (модуль pub в sound_playback.rs), колбэк ставится при первом клиентском тике. Цель инъекции: horizon_client_lib::sound_playback::native_sound_play.
Что делать дальше: проверить в обычной игре с включённым звуком (hook_seen в config/homestead-sound-status.txt), если и там 0, искать, почему playback_parameters не доходит до хука (звуки без ресурсов не стартуют).

## Каталог нативных модов playhorizon.dev (разбор 2026-11-15)

Источник: https://playhorizon.dev/mods?kind=native-mod (на странице около 140 нативных модов; лицензии на общем списке не указаны, смотрятся на странице каждого мода).

**Ключевой факт.** Нативный мод (`.hnmod`) это обычный бандл `horizon-mixin.toml` + Rust-модули, то есть тот же формат, что у наших `create` и `homestead`. Лаунчер собирает его в отдельный экземпляр «Native mods»; нам он ложится в `mixins/<id>/` и накладывается тем же `horizon-mixins apply`. Отдельной установки в игру не нужно, но нужна пересборка клиента и приёмка на общем бинарнике. Мод заявляет нужную версию хуков в `compatibility.json` (`"hooks": N`): Recipes и Minimap v1-v2, Jade v411+. В нашем движке 432 хука; совместимость сверять по `compatibility.json` каждого мода при интеграции (не проверено).

| Мод | Автор | Лицензия | Хуки | Решение |
| --- | --- | --- | --- | --- |
| Recipes 2.0.0 (замена JEI) | Horizon | MIT | v2 | **брать первым**: R/U рецепты и применения, кнопка «+» заполняет сетку; неизвестно, видит ли предметы из `.hmod` (проверить в игре) |
| Jade 26.3.8 | Snownee, Joeypc6 | CC BY-NC-SA 4.0 (некоммерческая, share-alike) | v411+ | **брать**: только локально, в публичный репозиторий и релизы не класть без отдельного решения; сторонние Java-плагины не работают, данные Create надо подключать через его провайдеры (отдельный срез) |
| Minimap 2.0.0 (замена Xaero) | Horizon | MIT | v1 | **брать** |
| Mod Menu (port) / Mod Menu (Horizon) | DeportedToHell / tariksabanuz | проверить | проверить | брать один, после проверки лицензии |
| AppleSkin | sillysoufi / CayMac | проверить | проверить | брать один: показывает сытость и насыщение, важно для проверки голода |
| Horizon Voice Chat | Nirokx | MIT | c9d474d | **не брать**: нужен сервер с Simple Voice Chat, в одиночной игре не работает; нужна системная libopus |
| Distant Horizons, Baritone, WorldEdit, X To Xray, SeedCrackerX | разные | проверить | разные | не брать: тяжёлые или читерские |
| TreeFall, Chop!, Invisible Frames, Right-Click Crops | разные | проверить | проверить | не нужны: у нас уже есть свои (Falling Trees, Invisible Frames, Right Click Harvest) |

Отдельно: **Create for Horizon** (Delete228, 0.7.0-alpha.2) и **Applied Energistics 2** на сайте есть; сравнить с нашим портом Create и решить, не дублируем ли чужую работу. Шаги интеграции: (1) скачать `.hnmod`, проверить SHA-256 и прочитать исходник (моды не изолированы); (2) положить бандл в `mixins/`, проверить `compatibility.json`; (3) одна пересборка клиента сразу на несколько модов; (4) приёмка `-ClientN`; (5) проверка в игре на предметах Create и FD.