# Баги игровой проверки, часть 2: дыры под плюшами, лавина достижений, лодки модов

Диагностика только чтением: код, сборки и игра не запускались. Все пути относительные. `HS` = `work/homestead-horizon`, `PH` = `work/horizon-source/horizon-<rev>` (исходники клиента Horizon), `CS` = `HS/create-source/create-horizon-<rev>`.
Цифры получены скриптами по `HS/pack/mods/<мод>/carry` (45 «данных» модов). Оценки модели «полный куб или нет» приблизительные (см. «Ограничения»).

## Сводка и порядок

| # | Баг | Причина (одной строкой) | Масштаб | Ценность | Усилие | Порядок |
| --- | --- | --- | --- | --- | --- | --- |
| B | 18 достижений мгновенно (и ещё около 24 такого же типа), по 1000 опыта каждое | перенос из 1.20.1 не переименовывает `structure`/`biome` в `structures`/`biomes` внутри `location`-предиката; клиент 26.3 игнорирует неизвестный ключ, предикат становится пустым и всегда истинным | 42 файла в 3 модах | 5 | S | 1 |
| A | дыра под плюшами (и подобными блоками) | блоки без «оракула» получают шаблон по свойству `facing` = `minecraft:carved_pumpkin` (полный непрозрачный куб), а модель маленькая: соседние грани скрыты | около 1970 блоков в 17 модах (47 плюшей) | 4 | S-M | 2 |
| C | лодки модов не ставятся | предмет объявлен как `minecraft:stick`, а установка лодок в клиенте принимает только предметы `minecraft:<дерево>_boat`; своих сущностей лодок у модов нет | 32 предмета (16 лодок + 16 лодок с сундуком), все в Regions Unexplored | 2 | S (спрятать) / M (ставить ванильную) / L (настоящие) | 3 |

---

## A. Дыра под плюшами

### Что видно
Под плюшем (скриншоты) виден небесно-голубой/чёрный прямоугольник вместо верхней грани земли под ним, а рамка выделения вокруг плюша это полный куб.

### Причина (доказана чтением)
1. **Объявление блока.** `HS/pack/mods/perfectplushies/carry/data/perfectplushies/horizon/block/aye_aye_plushie.json` содержит только `{"template": "minecraft:carved_pumpkin"}`. Нет ни `properties`, ни `shapes`, ни `occludes`. То же у всех 47 плюшей (у `daniel_plushie` шаблон `stone`).
2. **Откуда такой шаблон.** `carry-report.json` каждого мода говорит `"oracle": false` (проверено по всем 20 модам с блоками: «модов с оракулом нет»). Без оракула `horizon-port-pack` берёт запасной путь: `PH/crates/horizon-port-pack/src/carry.rs:153-175` (таблица `TEMPLATES`) и функция `template(...)` выбирают шаблон по набору свойств блока из blockstate. Плюш имеет единственное свойство `facing`, значит `("facing", "minecraft:carved_pumpkin")` (carry.rs:174). Блок пишется как `{"template": t}` (carry.rs:1456).
3. **Что даёт шаблон.** Вырезанная тыква это полный непрозрачный куб: загораживает соседние грани, имеет полную рамку выделения и полную коллизию. Модель плюша маленькая (элементы по 5-12 пикселей, `render_type: cutout`, файл `assets/perfectplushies/models/block/aye_aye_plushie.json`).
4. **Документация движка говорит об этом прямо.** `PH/docs/mods.md:153-157`: `occludes: false` «makes a block occlude nothing whatever its template does... a thin panel or a block drawn by its block entity on a full-cube template doesn't leave a hole in the floor under it». То есть нужное свойство в движке есть, его просто не выставляют.
5. **Почему не выставляется.** Единственное место, где `occludes: false` попадает в декларацию, это путь оракула: `PH/crates/horizon-port-pack/src/oracle.rs:441-444` (`if s.can_occlude == Some(false) ... properties.insert("occludes", false)`), и он требует фактов из `jar` мода. Без оракула такого шага нет.
6. Формат свойства: `"properties": {"occludes": false}` (пример `PH/crates/horizon-mods/src/content.rs:2139`, тест `PH/crates/horizon-client/src/mod_content_tests.rs:808`).

### Масштаб: блоки с неполной моделью на шаблоне-полном кубе (шаблоны stone, carved_pumpkin, furnace, redstone_lamp, oak_log)
Всего блоков в carry 9289, из них без оракула (все) 9289. Затронуто (неполная модель + полный шаблон + нет `occludes:false`) около **1970**:

| Мод | Блоков | Затронуто | Примеры |
| --- | --- | --- | --- |
| luphieclutteredmod | 509 | 330 | birdhouse, bicycle, flower_shelf, table |
| chipped | 6936 | 681 | acacia_torch, ковры (carpet), прочие тонкие |
| regions_unexplored | 805 | 334 | саженцы, цветы (alpha_dandelion, alpha_rose), ветки (acacia_branch) |
| mcwholidays | 308 | 205 | bat_*, дверные коврики, настенный декор |
| mcwpaths | 258 | 154 | плоские дорожки (paving, path) |
| dustydecorations | 79 | 68 | стулья, полки, якорь |
| justoutdoorstuffs | 50 | 48 | скамья, купальня для птиц, фламинго |
| perfectplushies | 47 | 47 | все плюши |
| froglightsreimagined | 48 | 32 | потолочные и напольные лампы (шаблон redstone_lamp) |
| candlelight | 71 | 20 | полки, плита (furnace), знамёна |
| bakery | 19 | 13 | breadbox, cake_stand, блоки кексов |
| alcocraftplus | 14 | 14 | бочки, кеги |
| betterendrod | 8 | 8 | стержни |
| fairylights | 21 | 7 | гирлянды, крепёж |
| suppsquared | 26 | 4 | таблички-пластины, полка |
| blahaj | 3 | 3 | акулы |
| beautify | 2 | 2 | botanist_workbench, rope |
| dyedend, ends_delight, natures_delight | 64 + 2 + 19 | 0 | (после уточнения шаблонов) |
| **Итого** | 9289 | **1970** | |

Ещё около 1190 блоков с неполной моделью стоят на «частичных» шаблонах (плита, лестница, люк, стена, кнопка, ограда): дыры под ними не ожидаются (форма этих шаблонов не полный куб), но форма выделения и коллизия могут не совпадать с моделью. Это отдельный косметический вопрос, не этот баг.

### Исправление
Добавить шаг, который для каждого блока с неполной моделью на шаблоне-полном кубе дописывает в декларацию `properties.occludes = false`, и по желанию `shapes`/`collision_shapes` по ограничивающим коробкам элементов модели (чтобы рамка и коллизия не были полным кубом).
- **Новый файл:** `CS/tools/block_looks.py`: по аргументам как у `creative_tab.py` (`--out <dir> <mod dir>`), читает `data/<ns>/horizon/block/*.json`, blockstate и модель (разбор родителей в пространстве мода, `elements`), решает полный/не полный, пишет оверлей с изменённой декларацией (слияние с уже существующим `properties`).
- **Подключение:** `HS/stage4/Build-DataMods.ps1` строки 34-38 (после вызова `tools/creative_tab.py`): вызвать второй тул в тот же `$tabs`-каталог (или во второй) и добавить результат в словарь `$over`. Механизм оверлея уже есть: файлы оверлея заменяют одноимённые из `carry`.
- **Не трогать:** `pack/mods/*/carry` (они пересоздаются командой `carry`), Rust и движок.
- **Дополнительно (желательно, как постоянная защита):** вынести правило в сам `horizon-port-pack` (запасной путь без оракула выставляет `occludes:false`, когда модель блока не полный куб). Это апстрим, не наш код.

### Как проверять автоматически
- **Оффлайн:** скрипт-проверка: для всех архивов `*-horizon.*.hmod` распаковать декларации блоков, пересчитать полный/не полный и убедиться, что для всех блоков с неполной моделью на шаблоне-полном кубе стоит `occludes:false` (ожидаем 0 нарушений). Можно добавить в `Test-HomesteadDataMods.ps1`/`-DataMods`.
- **В игре:** ctl не показывает отрисовку, поэтому проверка грани: `setblock` плюша на камень и скриншот сверху (ручная проверка), либо проверка свойства через команду, если она есть (`data get block` не подтверждена). Помечать «не проверено в игре».

### Ограничения оценки
Разбор моделей учитывает только родителей из пространства мода; родители `minecraft:` распознаются по списку куб-моделей. Для blockstate берётся первая модель (variants/multipart). Блоки с `parent` из других модов или без blockstate попадают в «полные». Точность оценки около ±10 %.

---

## B. Лавина достижений

### Что видно
В чате подряд «has made the advancement» для 20 достижений, уровень опыта 53 на панели: 1000 опыта за каждое (`rewards.experience: 1000` в файлах Repurposed Structures).

### Какие моды и какие достижения
Все 20 из списка пользователя принадлежат двум модам:
- **repurposed_structures (18):** `ancient_cities` (Civilization from the Beyond), `bastion_underground` (Bastions, Evil, and Bones...), `city_nether` (Blazing Cities), `city_overworld` (Skyward Cities), `fortress_jungle` (Decaying Glory), `igloos` (Having a Hobbit Time!), `mansions` (Lavish Livings), `mineshafts` (Toiling in the Mines), `monuments` (Tributes and Sacrifices), `outposts` (Territory Control), `pyramids` (Raider of the Tombs), `ruined_portal_end` (The Useless Portal), `ruins` (Remains of the Past), `shipwrecks` (Interdimensional Pirates), `strongholds` (Following the Eyes), `temples` (Unanswered Prayers), `villages` (Sprawling Civilizations), `witch_huts` (Hexes and Curses). Файлы: `pack/mods/repurposed_structures/carry/data/repurposed_structures/advancement/*.json`.
- **regions_unexplored (2 из списка + 2 такого же типа):** `regions_explored` (Regions Explored), `spelunker` (Spelunker), а также `pioneer` и `eternal_expedition`.
- «To beer or not to beer» (alcocraftplus) легитимно: триггер `minecraft:inventory_changed` (предмет в инвентаре).

### Причина (доказана чтением)
1. **Формат 1.20.1.** В перенесённых файлах условие места записано так: `"location": {"structure": "repurposed_structures:ancient_city_ocean"}` (у Regions Unexplored `"biome": "regions_unexplored:alpha_grove"`).
2. **Клиент 26.3** читает во `LocationPredicate` ключи `biomes` и `structures` (множественное число): `PH/crates/horizon-client/src/advancements.rs:938-965` (`location(...)`: `v["biomes"]`, `v["structures"]`). Единственное число никто не читает.
3. **Результат:** `location` получает объект без распознанных ограничений и доходит до конца функции `true`. Триггер `minecraft:location` срабатывает при первом же проверяемом тике, поэтому все критерии, которые держатся только на таком предикате, выполняются сразу. У ancient_cities три критерия (ocean, nether, end) в `requirements` одним списком каждый, и их тоже «выполняет» один и тот же пустой предикат.
4. **Перенос эту проблему уже частично чинит**, но не это: `PH/crates/horizon-port-pack/src/carry.rs:420-441` (`upgrade_advancement`) переименовывает только `enter_block.block` в `blocks` (20 достижений у Regions Unexplored). Поэтому в `carry-report.json` у repurposed_structures `advancements_upgraded = 0`, у regions_unexplored `20`.
5. **Не вина:** триггеры `minecraft:tick` (корневые достижения) и неизвестные триггеры (`yungsapi:structure_location` у Better Dungeons, 6 файлов) мгновенных выдач не дают (неизвестный триггер просто не срабатывает; в игре это не проверялось).

### Масштаб
Файлов с одиночными `biome`/`structure` в `location`-предикатах: **repurposed_structures 18, regions_unexplored 4, incendium 20 (8 structure + 12 biome)** = 42. Все `announce_to_chat: true`. Incendium (Незер) пользователь, вероятно, ещё не получил, потому что в Незере не был, но предикат у них такой же пустой, и они выдадутся при первом тике, как только условия измерения совпадут (если они есть).

### Исправление
Переименовать внутри любого объекта `location` (и в `predicate` условий `location_check`): `biome` → `biomes`, `structure` → `structures` (значение строка/список/тег сохраняется: `id_matches` понимает строку, `#тег` и список). Применить ко всем файлам `advancement/` и `advancements/` перенесённых модов.
- **Новый файл:** `CS/tools/advancement_fix.py` (тот же интерфейс `--out <dir> <mod dir>`).
- **Подключение:** `HS/stage4/Build-DataMods.ps1`, там же, где `creative_tab.py` (строки 34-38), результат в `$over`. Оверлей затем заменяет одноимённые файлы.
- **Побочно проверить** (grep по `carry/data/*/loot_table`, `predicate`, `recipe`): те же ключи в `location_check` у таблиц добычи и предикатов. Если есть, расширить тул на эти каталоги.
- **Апстрим (правильное место):** расширить `upgrade_advancement` в `horizon-port-pack` (carry.rs:420) ровно этой заменой; добавить пункт в `PH/docs/modpacks.md` рядом с описанием `enter_block`.
- **Как защитить мир, который уже в этом состоянии:** старый тестовый мир хранит выданные достижения в `advancements/<uuid>.json`; новая сборка их не отзовёт. Нужен новый мир или команда `advancement revoke @s everything` (если есть в ctl, не проверялось).

### Как проверять автоматически
- **Оффлайн:** скрипт: ни один файл `advancement(s)/*.json` в архивах `.hmod` не содержит одиночных `structure`/`biome` внутри `location` (0 нарушений).
- **В игре:** после входа в свежий мир подождать 60 секунд и посчитать строки «has made the advancement» и «has completed the challenge» в логе клиента. Ожидаем не больше нескольких (корни, предметы в инвентаре). Свежий мир обязателен: тестовый мир `HomesteadAutomationAcceptance` содержит старые выдачи. Добавить в `-DataMods`.

---

## C. Лодки модов

### Что это
- Предметов-лодок: **32** (16 лодок + 16 лодок с сундуком), **все в regions_unexplored**; других модов с лодками/плотами в carry нет (проверено по `horizon/item/*boat*`, `*raft*`).
- Объявление предмета: `pack/mods/regions_unexplored/carry/data/regions_unexplored/horizon/item/baobab_boat.json` = `{"template": "minecraft:stick"}` (палка-заглушка, без поведения «ставить лодку»; тот же вид у `*_chest_boat`). Иконка и модель предмета есть (`assets/.../items/eucalyptus_boat.json`, `models/item/eucalyptus_boat.json`, слой `item/generated`).
- Рецептов: 33 файла `data/regions_unexplored/recipe/*boat*.json` (лодка из досок «# #/###», лодка с сундуком из сундука и лодки) и рецепт-достижения `advancement/recipes/transportation/*boat*.json`. То есть предметы можно скрафтить в выживании, но поставить нельзя.

### Почему не ставятся
Размещение лодок в клиенте жёстко завязано на предметы vanilla:
- `PH/crates/horizon-client/src/game/boats.rs:393-408`: `any_boat_wood(item)` начинается с `item.strip_prefix("minecraft:")?` и принимает только `minecraft:<дерево>_boat`, `_chest_boat`, `_raft`. Для `regions_unexplored:eucalyptus_boat` это сразу `None`.
- `boats.rs:1457-1465`: `boat_to_place` дополнительно требует, чтобы модель дерева была в `boat_models` (модели сущностей `minecraft:boat/<дерево>#main`, только ванильные породы).
- Сущность лодки имеет тип `minecraft:<дерево>_boat` (`boats.rs:380-387`); сущностей `regions_unexplored:*_boat` и их моделей нет. Слоя сущностей для модов в Horizon нет (`PH/docs/native-mods.md`, «deep» моды).
- Значит чистого data-only пути нет. Нужно либо скрыть, либо подменить на ванильное, либо делать настоящий слой сущностей.

### Варианты
| Вариант | Что делаем | Усилие | Результат | Риск |
| --- | --- | --- | --- | --- |
| 1. Спрятать | убрать 32 предмета из креатива и удалить рецепты и рецепт-достижения лодок | S (около 1-2 часов) | лодок Regions Unexplored нет, ничего не «сломано» | теряется часть контента; ванильные лодки остаются |
| 2. Ванильные лодки | мелкий патч движка: `any_boat_wood` и `boat_wood` принимают `<ns>:<порода>_boat`, порода берётся из карты псевдонимов (например по названию дерева на ближайшую ванильную породу или `oak`); предметам добавить `max_stack_size: 1` | M (патч + карта + тест) | лодка ставится и плывёт, выглядит как ванильная (не с текстурами мода) | патч привязан к якорю в `boats.rs`; вид не оригинальный; сохранённая лодка получает тип ванильной |
| 3. Настоящие | сущности-лодки модов с их текстурами `textures/entity/boat/*.png` и поддержкой в модели лодки | L | оригинальный вид | нужен слой сущностей/реестр пород с пространствами имён; движок для этого не готов |

### Рекомендация
Сначала вариант 1 (дёшево, безопасно), вариант 2 по желанию позже. Не делать вариант 3 без слоя сущностей.

### Файлы для варианта 1 (спрятать)
- Список на удаление из архива: `HS/stage4/data-mods-skip.json` (формат: `{"<id мода>": ["относительный/путь/в/архиве", ...]}`, сейчас `{}`), механизм `$skip[$id]` в `Build-DataMods.ps1:34`:
  - `data/regions_unexplored/horizon/item/*_boat.json` и `*_chest_boat.json` (32),
  - `data/regions_unexplored/recipe/*boat*.json` (33),
  - `data/regions_unexplored/advancement/recipes/transportation/*boat*.json`,
  - `assets/regions_unexplored/items/*boat.json`, `models/item/*boat.json` (для чистоты; не обязательно),
  - упоминания в тегах (`data/minecraft/tags/item/boats.json`, `chest_boats.json`, если есть в `carry` или в `outside`: проверить, чтобы тег не ссылался на несуществующие предметы).
- Автогенерация списка лучше ручного файла: добавить в `Build-DataMods.ps1` шаг, который собирает пути по шаблону `*_boat.json`/`*_chest_boat.json` для мода `regions_unexplored` и добавляет в `$drop`. Тогда вкладка креатива (`tools/creative_tab.py`) не увидит этих предметов.
- Тест: в `-DataMods` проверить, что в архиве regions_unexplored нет файлов `*boat*` в `horizon/item` и `recipe`.

### Файлы для варианта 2 (если решим делать)
- Новый патч: `HS/stage4/engine-patches/23-modded-boats.ps1` (по образцу `20-sleep-host.ps1`): якоря в `PH/crates/horizon-client/src/game/boats.rs` у функций `any_boat_wood` и `boat_wood` (около строк 393-408).
- Карта псевдонимов: файл конфигурации в `config/` (по образцу `homestead-grind.txt`) или таблица в модуле `homestead` бандла.
- Декларации предметов: `"max_stack_size": 1` в `horizon/item/*boat.json` (через оверлей).
- Тест: `give` лодки и `use` на воде, затем `entities` и проверка, что появилась сущность (если ctl умеет).

---

## Что передать агенту-исправителю (в порядке работы)
1. **B (достижения):** `CS/tools/advancement_fix.py` (новый), правка `HS/stage4/Build-DataMods.ps1` (подключение рядом с `creative_tab.py`), проверка в `HS/stage4/acceptance/Test-HomesteadDataMods.ps1` (или как называется набор `-DataMods`): оффлайн скан архивов + счётчик достижений в логе после входа в свежий мир.
2. **A (дыры):** `CS/tools/block_looks.py` (новый), правка `Build-DataMods.ps1` (тот же блок), оффлайн скан архивов в том же наборе.
3. **C (лодки):** автогенерация `$drop` в `Build-DataMods.ps1` (или `HS/stage4/data-mods-skip.json`); тест в том же наборе.
Все три пересобирают только архивы `.hmod` (скрипт `Build-DataMods.ps1`), движок и Rust не трогаются, пересборка клиента не нужна. После правки нужно перепаковать 45 архивов и прогнать `-DataMods`.

## Не установлено
- Другие места с одиночными `structure`/`biome` (`location_check` в таблицах добычи, предикаты `data/*/predicate`): не проверялись.
- Ведут ли себя неизвестные триггеры (`yungsapi:structure_location`) в 26.3 безвредно: не проверялось.
- Есть ли в ctl команда отзыва достижений и чтения свойств блока.
- Оценка «полная модель» (±10 %): точный список блоков даст сам `block_looks.py`.
- Не проверялось, какие из достижений Incendium уже выданы в мире пользователя.
