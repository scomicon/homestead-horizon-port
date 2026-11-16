# Оставшиеся моды уровней logic, machines и none: разбор и порядок переноса

Только чтение: код, сборки и игра не запускались. Источники: `pack/coverage.json` (статус, лицензия, уровень), `pack/inventory.json` (перепись файлов: блоки, рецепты, зависимости, стороны), `pack/modrinth.json` (описания), список хуков `engine/horizon-<ревизия>/native-hooks.json` (432 хука: 270 server, 60 client, 10 both, 92 без пометки), хелперы `crates/horizon-client/src/game/*.rs` (1096 функций `native_*`), наши миксины `create-source/*/mixins/homestead/` и документ `docs/logic-c.md` (почему отключены Stack Refill и CraftingPad).
Пути ниже относительные: `ST` = `work/homestead-horizon/stage4`, `CS` = `work/homestead-horizon/create-source/create-horizon-<ревизия>`.

## 0. Что считать «безопасным хуком»

В документации Horizon (`engine/docs/api/README.md`) сказано, что одиночная игра, серверный поток, LAN и выделенный сервер исполняют один и тот же код игры, то есть серверные хуки срабатывают и в одиночной игре. Но урок Stack Refill такой: хук `level_tick_pre` и `use_item` исполняются там, где у `Game` нет «локального игрока окна» и экрана. Поэтому логику, которой нужен именно локальный игрок (инвентарь окна, экран), на них строить нельзя. Что сделано на практике:

- **Доказано нашими приёмками (есть игрок события, мир, блок, предмет в руке):** `destroy_block`, `right_click_block`, `loot_drops`, `interact_entity`, `random_tick`, `block_entity*` (тики, контейнеры, отправка), `state_for_placement`, `update_shape`, `level_tick_post`, `player_logged_in`, `player_respawned`, `perform_command`, `register_commands`.
- **Только по списку хуков, в наших мирах не проверялись:** `player_tick`, `block_step_on`, `living_incoming_damage*`, `living_death`, `player_totem`, `prepare_grindstone`, `prepare_item_enchant`, `player_fish`, `entity_tick_pre`, `food_level_changed`, `player_chat`, `living_fall`, `entity_breed`. Любой срез на них начинается с маленького пробного теста, что хук срабатывает в одиночной игре и даёт игрока.
- **Клиентские (только окно):** `screen*`, `gui_layer_*`, `health_hud`, `tooltip*`, `native_tick` + `native_window()`; они доказаны волной stage20.

Это нигде не подтверждено глубже, чем по коду и нашим тестам: везде, где написано «по списку», это гипотеза.

Обозначения вердиктов: **DO-NOW** (named hooks) сделать сейчас; **DATA** только данные (рецепты, лут, теги, структуры, оболочки блоков из `carry`); **HOOK** нужен хук или патч движка; **ENTITY** нужен слой сущностей; **SKIP** не стоит делать; **BUILT-IN** уже есть в Horizon, ваниле 26.3 или у нас. Усилие S/M/L/XL, ценность 1-5 для сборки Homestead (уютная ферма с Create и Farmer's Delight, одиночная игра).

Лицензия: **ARR** (All Rights Reserved), **GPL** (copyleft), **MIT/perm** (permissive), **custom/restricted**. Для ARR и GPL поведение воссоздаётся только по публичным описаниям, чужой код и ресурсы не копируются; визуальные ресурсы идут из вашего собственного JAR через `carry`, как у `Build-DataMods.ps1`.

## 1. Уже сделано или отключено (в списке `coverage.json` они всё ещё `not-yet`)

| Мод | Статус |
| --- | --- |
| Right Click Harvest, Secure Crops, Too Cheap!, Preferred Gamerules, Crops Love Rain, Better Days | сделаны (stage7) |
| Double Doors, Shulker Drops Two, Invisible Frames, Accelerated Decay, No Animal Tempt Delay, Falling Trees (Panda's Falling Tree's), SpawnFilter, Universal Graves | сделаны (stage19) |
| Mob Explosion Griefing Gamerule | сделан в составе Preferred Gamerules (stage7) |
| Create: Oxidized | сделан (`mods/create_oxidized-*.hmod`, 30 рецептов) |
| Farmer's Delight (две записи) | сделан, см. roadmap |
| Stack Refill, Crafting Pad | отключены: хуки не подходят (раздел 0) |

## 2. Таблица: logic (44 записи, из них 8 уже учтены выше)

| Мод | Лиц. | Что делает (по описанию) | Вердикт | Хуки / причина | Усилие | Ценность |
| --- | --- | --- | --- | --- | --- | --- |
| [Let's Do Addon] Compat | copyleft | совместимость модов Let's Do, только сервер, зависит от architectury | SKIP | библиотека совместимости, нечего переносить | S | 1 |
| Armor Statues | copyleft | экран позы и рук для подставок для брони | HOOK | экран на клик по сущности: `interact_entity` + экран мода; позы через `native_entity_merge_nbt`; зависит от PuzzlesLib | M-L | 2 |
| BEBPM | perm | книжные полки вокруг стола зачарований по расширенной схеме | DO-NOW | `prepare_item_enchant` (+`native_prepare_item_enchant`) для пересчёта силы; хук только по списку, нужен проб | M | 3 |
| BetterThanMending | copyleft | описание в Modrinth пустое, назначение по названию неясно | не установлено | нужен JAR/описание | ? | ? |
| Block Runner | copyleft | быстрее бег по тропинкам и дорожным блокам | DO-NOW | `block_step_on` + `native_set_attributes`/`native_attribute` (скорость), либо `player_tick` | S-M | 3 |
| Bridging Mod | perm | установка блоков «из-за угла» и мост по бедроку | HOOK | клиентская подмена цели луча (`native_view_hit`/цель прицела); хука нет (см. stage20 отчёт) | M | 2 |
| Colored Water | ARR | цветная вода, 64 блока, 96 рецептов | ENTITY/HOOK | свои жидкости: в API нет регистрации жидкостей | L | 2 |
| Comforts | copyleft | спальные мешки и гамаки (16 цветов), без смены точки возрождения | HOOK | хуки сна (`can_player_sleep`, `player_bed_enter`) срабатывают только у гостя LAN: патч движка вызывать их и для хозяина; блоки/предметы данные | M | 3 |
| Convenient Name Tags | perm | ПКМ по бирке в воздухе открывает ввод имени | HOOK | нужен экран ввода (client) и ПКМ в воздухе; `use_item` не подходит (серверный поток); `native_set_name_tag` есть | M | 3 |
| Crafting Pad | perm | переносной верстак | отключён | см. раздел 1 | | |
| Create: Oxidized | perm | рецепты окисления меди | сделан | | | |
| Easy Magic | copyleft | стол зачарований: предметы остаются после закрытия, перебросы | HOOK/DO-NOW | `native_menu_give_back` (не возвращать предметы при закрытии) + `player_container_close`; хуков экрана стола нет; нужен проб | M-L | 3 |
| Emojiful | perm | эмодзи в чате (текстуры, 2 рецепта) | HOOK | подмена текста чата на значки: клиентский хук рендера строки чата (`native_client_chat_received`, `native_chat_*` частично) | M | 2 |
| Farmer's Knives | perm | дополнительные ножи Farmer's Delight (60 предметов, 60 рецептов) | DATA | оболочки предметов и рецепты через `carry`; ножи FD уже режут на доске | S-M | 3 |
| FTB Essentials | ARR | серверные команды (`/home` и т.д.) | SKIP | серверная утилита; команды можно сделать через `register_commands`, но ценность мала | S | 1 |
| FTB Teams | ARR | команды игроков | SKIP | одиночная игра | | 1 |
| FTB XMod Compat | ARR | мост между модами FTB | SKIP | | | 1 |
| Grass Seeds | ARR | семена превращают землю в траву | DO-NOW | `right_click_block` (доказан) + `native_set_block`/превращение + расход предмета | S | 3 |
| Grind Enchantments | copyleft | снять чары на книгу или перенести через точило | HOOK | есть `prepare_grindstone`, но забор результата очищает оба входа внутри `menu.rs`; нужен хук «результат взят» (патч движка, малый) | M | 3 |
| Inventory Totem | ARR | тотем бессмертия работает из любого слота инвентаря | DO-NOW | `living_incoming_damage_v3`/`living_death` + `player_totem`; нужен проб, что можно отменить смерть и применить эффект тотема вручную | M | 4 |
| Jade Addons | ARR | дополнения к Jade | SKIP | Jade не перенесён | | 1 |
| Log Begone | copyleft | скрывает лишний шум в логах | BUILT-IN | у Horizon свой журнал | | 1 |
| Minecraft Accelerated | perm | 28 блоков, 18 рецептов, описание пустое | не установлено | назначение неясно | ? | ? |
| Mixin Conflict Helper | copyleft | понятные ошибки конфликта миксинов | SKIP | Java-миксинов в Horizon нет | | 1 |
| Mob Explosion Griefing Gamerule | perm | | сделан | | | |
| Mouse Wheelie | perm | прокрутка предметов, сортировка, пополнение | BUILT-IN (часть) | Mouse Tweaks wheel уже встроен в клиент; сортировка/пополнение отдельно | S | 2 |
| Neruina | perm | не даёт тику-ошибке ломать мир | SKIP | у Rust другая модель ошибок | | 1 |
| Nether's Delight Refabricated | copyleft | дополнение FD: Нижний мир (15 блоков, 32 рецепта, 34 предмета) | DATA+DO-NOW | оболочки и рецепты через `carry`; плита-печь и особые блоки: часть поведения нужно портировать (по образцу нашей печи) | M-L | 3 |
| Not Enough Crashes | perm | возврат на заголовок после краха | SKIP | | | 1 |
| Obsidian Equipment Reworked | perm | обсидиановые инструменты и броня (15 предметов, 32 рецепта) | DATA | оболочки предметов + `modify_default_components`; нужна проверка материалов брони/инструментов в `content.py` | M | 2 |
| Panda's Falling Tree's | copyleft | | сделан (Falling Trees) | | | |
| Respect My Trims | copyleft | золотая отделка брони успокаивает пиглинов | DO-NOW | `entity_tick_pre` (сброс цели пиглина `native_entity_target`) или хук цели; проб | S-M | 2 |
| Show Me What You Got | perm | показывать свои предметы в чате | HOOK | `player_chat` (server) + компонент текста с подсказкой предмета: нужен вызов вставки предмета в сообщение | M | 2 |
| Show Me Your Skin! | perm | скрыть или настроить отрисовку брони | HOOK | клиентская отрисовка слоя брони; хука нет | M | 2 |
| SimpleHats | perm | шляпы (351 модель, 13 рецептов), слот Accessories | HOOK/ENTITY | слот аксессуаров и слой модели игрока в API нет | L | 2 |
| Smarter Farmers | custom | жители лучше пересаживают | ENTITY | поведение жителей (мозг) | L | 2 |
| Sooty Chimneys | perm | дымоходы с частицами (14 блоков) | DATA+HOOK | оболочки блоков из данных; частицы дыма нужен клиентский спавн частиц | M | 2 |
| SparseStructures | perm | реже или чаще все структуры (конфиг) | DATA | перезаписать `structure_set` (`spacing`/`separation`) пакетом данных; только новые миры, чужие структуры (по пакетам модов) требуют своих наборов | S | 3 |
| StackDeobfuscator | copyleft | читаемые имена в трассировках | SKIP | | | 1 |
| Things | perm | безделушки и утилиты, нужны trinkets | HOOK/ENTITY | слот аксессуаров | L | 2 |
| Tokimi's Toolshed | perm | многоблочные инструменты (молотки и т.п.), 62 предмета, 79 рецептов | DO-NOW | `destroy_block` + `native_machine_destroy_block` (как Falling Trees) для области 3x3 | M | 3 |
| Trade Cycling | ARR | перегенерировать предложения жителя без ломания рабочего блока | HOOK | нет вызова пересоздания предложений и кнопки на экране торговли (патч движка + клиентский хук) | M | 3 |
| YUNG's Bridges | copyleft | мосты-структуры | DATA? | перепись не нашла блоков/рецептов (0); структуры нужно проверить в JAR | ? | 3 |

## 3. Таблица: machines (27 записей)

| Мод | Лиц. | Что делает | Вердикт | Хуки / причина | Усилие | Ценность |
| --- | --- | --- | --- | --- | --- | --- |
| [Let's Do] Farm & Charm | custom | кухня, посуда, декор фермы (65 блоков, 157 рецептов, 112 предметов), зависит от architectury | DO-NOW (большой) | блок-сущности и меню у нас доказаны (FD, Create); много блоков | L-XL | 4 |
| Camerapture | perm | фотоаппарат, картины в рамках | HOOK | снимок экрана и текстура в рамке: клиентский захват | L | 2 |
| Carry On | copyleft | переносить блок-сущности и мобов | DO-NOW | `right_click_block` + копирование NBT (`native_entity_nbt`, `native_mod_block_entity`) + `place_block`; отрисовка переносимого блока: нужен клиентский хук | M-L | 4 |
| Create Contraption Terminals | perm | терминалы Tom's на контрапциях | SKIP | контрапций (поезда/каркасы) у нас нет | | 1 |
| Create Deco | custom | индустриальный декор (397 блоков, 1729 рецептов) | DATA | оболочки по нашему генератору декора; лицензия «custom» не проверена | M | 3 |
| Create Slice & Dice | perm | нарезка Create (4 блока) | DO-NOW | нужен станок-спутник Create (развёртыватель/прессы уже есть) | M | 3 |
| Decorative Lamps | ARR | 233 лампы, диммер редстоуном | DATA+DO-NOW | оболочки + состояние яркости (`block_signal`, `state_for_placement`); текстуры из вашего JAR | M | 2 |
| Elytra Slot | copyleft | слот для элитр рядом с нагрудником | HOOK | слот аксессуаров (Trinkets): нет | L | 3 |
| Explorer's Compass / Nature's Compass | restricted | компас поиска структур и биомов | SKIP/HOOK | лицензия «restricted»; плюс нужен API поиска структур | L | 3 |
| FTB Quests | ARR | квесты | SKIP | | XL | 1 |
| Handcrafted | custom | мебель (267 блоков), сидения | DATA+ENTITY | оболочки данных; сидение требует сущности «стул» | M / L | 3 |
| Immersive Furniture | copyleft | редактор мебели в игре | SKIP | | XL | 1 |
| Just Hammers | copyleft | молотки 3x3, 5x5... | DO-NOW | общий движок «область при ломании» с Tokimi (`destroy_block`) | M | 3 |
| labels | ARR | ярлыки на хранилищах | HOOK | рендер и сущность | M | 2 |
| Lootr | perm | лут персонально для каждого игрока | SKIP | смысл только в многопользовательской игре | M | 1 |
| NiftyCarts | perm | повозки (84 предмета) | ENTITY | сущности с пассажирами | L | 3 |
| Polymorphic Tom | copyleft | совместимость Polymorph и Tom's | SKIP | | | 1 |
| Simple Copper Pipes | perm | медные трубы с предметами (16 блоков) | DO-NOW | `block_entity*` + передача предметов (у нас есть движок хоппера/воронки) | M-L | 3 |
| Sophisticated Storage | copyleft | улучшаемые сундуки и бочки (48 блоков) | DO-NOW (большой) | меню мода, блок-сущности; зависит от sophisticatedcore (нет в списке) | XL | 4 |
| Soundscape | perm | музыкальные диски окружения | DATA+HOOK | предметы и рецепты данными; звуки из вашего пака, проигрыватель | M | 2 |
| Tom's Simple Storage | perm | сеть хранилищ с терминалом | DO-NOW (большой) | блок-сущности и экран терминала (экран мода доказан в Create/FD) | L | 4 |
| Trash Cans | ARR | мусорные баки: предметы, жидкости, энергия | DO-NOW | `block_entity_container` (приём), как у Item Drain | S-M | 3 |
| Umbrellas | perm | зонты (11 блоков, 43 предмета) | DO-NOW | предметы; замедленное падение через `living_fall` (both) или атрибут | M | 2 |
| Universal Sawmill | custom | пила по дереву для любых рецептов | DO-NOW | блок с меню по образцу камнереза; рецепты по тегам | M | 3 |
| Farmer's Delight | perm | | сделан | | | |

## 4. Таблица: none (19 записей)

| Мод | Лиц. | Что делает | Вердикт | Причина | Усилие | Ценность |
| --- | --- | --- | --- | --- | --- | --- |
| [Let's Do] API | copyleft | библиотека | BUILT-IN/SKIP | заменяется API Horizon | | 1 |
| [Let's Do] Brewery | custom | напитки и бражные бочки (59 блоков, 55 рецептов) | DO-NOW (большой) | зависит от Farm & Charm | L | 3 |
| Configurable, Configured | perm/copyleft | конфигурационные библиотеки | SKIP | нет конфигов Java | | 1 |
| CraftTweaker | perm | скриптовая правка рецептов | DATA | эффект сводится к пакету данных | | 1 |
| Data Anchor | copyleft | библиотека данных | SKIP | | | 1 |
| Default Options | ARR | стартовые настройки | BUILT-IN | у нас `Apply-ClientDefaults.ps1` | | 2 |
| Expanded Delight | perm | дополнение FD (17 блоков, 72 рецепта, 55 предметов) | DATA | рецепты для нашего котла FD и предметы-оболочки через `carry`; ближе всего к готовому | S-M | 4 |
| FastBoot | ARR | быстрый старт | BUILT-IN | | | 1 |
| Go Fish | perm | новая рыба и рыбалка (12 блоков, 37 рецептов, 78 предметов) | DATA | таблицы добычи рыбалки и предметы; сами хуки `player_fish` нужны только для особенностей | M | 3 |
| Harvestable Flowers | perm | кость на цветок даёт цветок | DO-NOW | `right_click_block` + `native_is_valid_bonemeal_target`/`native_perform_bonemeal` | S | 2 |
| MixinTrace | perm | отладка | SKIP | | | 1 |
| Mo Glass | copyleft | варианты стекла (36 блоков, 72 рецепта) | DATA | оболочки блоков и рецепты | S | 2 |
| Peaceful Hunger | perm | голод в мирном режиме | HOOK | в мирном режиме движок обнуляет голод: нужен малый патч или `food_level_changed` + самостоятельный расход | S-M | 2 |
| Perfect Homestead Plushies | ARR | 53 плюшевых блока сборки | DATA | оболочки, текстуры из вашего JAR | S | 2 |
| Quests Additions | ARR | дополнения FTB Quests | SKIP | | | 1 |
| Spud's Shops | perm | предметы лавок (43 блока, 209 рецептов, 211 предметов) | DATA | оболочки и рецепты | M | 2 |
| Tameable Ribbits | perm | приручаемые лягушки Ribbits | ENTITY | слой сущностей | L | 2 |
| Wireless Redstone | copyleft | передатчики и приёмники по частоте | DO-NOW | наша сеть Redstone Link (`link_runtime.rs`, ядро `redstone_link.rs`) можно использовать с другой схемой частот | M | 3 |

## 5. Только данные (можно отдать без Rust)

Эффект этих модов сводится к рецептам, лут-таблицам, структурам, тегам или оболочкам блоков из `carry`; пакуются так же, как 45 «данных» модов (`Build-DataMods.ps1`):

Expanded Delight, Farmer's Knives, Mo Glass, Perfect Homestead Plushies, Spud's Shops, Go Fish (добыча), Obsidian Equipment Reworked (после проверки материалов), Create Deco (после проверки лицензии), Nether's Delight (первая половина), SparseStructures, CraftTweaker (правка рецептов пакетом данных).

## 6. Рекомендуемый порядок: 15 шагов по ценности и усилию

| № | Что | Почему | Усилие | Ценность |
| --- | --- | --- | --- | --- |
| 1 | Expanded Delight | 72 рецепта и 55 предметов для готового нашего котла FD | S-M | 4 |
| 2 | Inventory Totem | заметный выигрыш в выживании | M | 4 |
| 3 | Grass Seeds | маленький, доказанные хуки | S | 3 |
| 4 | SparseStructures | один пакет данных, только новые миры | S | 3 |
| 5 | Trash Cans | по образцу Item Drain | S-M | 3 |
| 6 | Just Hammers + Tokimi's Toolshed (один модуль области) | два мода одним кодом, шаблон из Falling Trees | M | 3 |
| 7 | Block Runner | маленький | S-M | 3 |
| 8 | Farmer's Knives | оболочки и рецепты | S-M | 3 |
| 9 | Go Fish | добыча данными | M | 3 |
| 10 | Wireless Redstone | переиспользуем сеть Link | M | 3 |
| 11 | Harvestable Flowers | маленький | S | 2 |
| 12 | BEBPM | нужен проб `prepare_item_enchant` | M | 3 |
| 13 | Universal Sawmill | станок с меню | M | 3 |
| 14 | Carry On | ценный, но без отрисовки переносимого блока | M-L | 4 |
| 15 | Nether's Delight (данные) | вторая половина после Expanded Delight | M-L | 3 |

### Три пакета независимых модов для авторов

- **Пакет 1: данные и Farmer's Delight (без Rust или с минимальным):** Expanded Delight, Farmer's Knives, Nether's Delight (данные), Go Fish, Mo Glass, Perfect Homestead Plushies, Spud's Shops, SparseStructures.
- **Пакет 2: логика игрока:** Grass Seeds, Harvestable Flowers, Inventory Totem, Block Runner, BEBPM, Respect My Trims.
- **Пакет 3: блоки и станки:** Trash Cans, Just Hammers + Tokimi's Toolshed, Wireless Redstone, Universal Sawmill, Simple Copper Pipes, Carry On.

В каждом пакете авторство идёт параллельно, интеграция и сборка одним агентом (как раньше). Перед каждым хуком из «только по списку» агент пишет проб: тест, что хук срабатывает в одиночной игре и даёт игрока.

## 7. Патчи нашего движка (по малости)

Мы можем править закреплённый движок через `ST/engine-patches/*.ps1`.

1. **Малые:**
   - Хуки сна (`can_player_sleep`, `player_bed_enter`, `player_spawn_point_set`) вызывать и для хозяина игры, а не только для гостя LAN. Это разблокирует Comforts. Малый, если условие «гость» локально.
   - Мирный режим: не обнулять голод (Peaceful Hunger). Малый.
   - Хук «результат точила взят» (Grind Enchantments). Малый или средний: правка в `menu.rs`.
2. **Средние:**
   - Вызов пересоздания предложений жителя и клиентский хук для кнопки на экране торговли (Trade Cycling).
   - Клиентская подмена цели луча и обводки (Bridging Mod).
   - Вставка предмета в сообщение чата и подмена текста строки чата для эмодзи (Show Me What You Got, Emojiful).
   - API поиска структур и биомов (компасы; лицензия мешает).
3. **Крупные:**
   - Слот аксессуаров и слой модели игрока (Things, SimpleHats, Elytra Slot, Show Me Your Skin).
   - Регистрация жидкостей (Colored Water).
   - Слой сущностей и мозга жителей (повозки, лягушки, стулья, умные фермеры).

## 8. Что не установлено

- Назначение **BetterThanMending**, **Minecraft Accelerated** (описаний нет, исходников нет).
- Содержимое **YUNG's Bridges**: перепись показывает 0 блоков и 0 рецептов, так что структуры в JAR, но их формат не проверен.
- Лицензии «custom» и «restricted» (**Create Deco**, **Farm & Charm**, **Handcrafted**, **Sawmill**, **Smarter Farmers**, **Brewery**, **Explorer's/Nature's Compass**) прочитаны только по метке в `coverage.json`, файлы лицензий не открывались.
- Срабатывание хуков из «списка без доказательства» (раздел 0) в одиночной игре: `player_tick`, `block_step_on`, `living_incoming_damage*`, `living_death`, `player_totem`, `prepare_item_enchant`, `prepare_grindstone`, `player_fish`, `entity_tick_pre`, `food_level_changed`.
- Поведение многих модов описано по краткому тексту Modrinth, исходников модов в `pack/mods` нет. Точные правила воссоздаются по описаниям и догадкам, как у предыдущих срезов.
- В `coverage.json` две записи `Farmer's Delight` (версии 1.4.3 и 2.4.1 из сборки); переносится более новая, как записано в roadmap.
