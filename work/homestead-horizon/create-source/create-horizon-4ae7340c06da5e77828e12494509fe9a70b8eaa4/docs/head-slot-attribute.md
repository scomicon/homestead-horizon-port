# Убираем «When on Head: -100% Waypoint Transmit Range» у декора с шаблоном `minecraft:carved_pumpkin`

Сборка и игра при подготовке документа не запускались; выводы сделаны по исходникам Horizon (`work/horizon-source/horizon-*`), ресурсам `resources-26.3`, инструментам `CS/tools/` и перенесённым модам `work/homestead-horizon/pack/mods` и `pack-batch1/mods`.

## 1. Причина

1. **Выбор шаблона без оракула.** В `crates/horizon-port-pack/src/carry.rs:155-201` (`TEMPLATES`, строка 174) блоки с единственным свойством состояния `facing` привязаны к `("facing", "minecraft:carved_pumpkin")`.
   - В базовых 45 модах (`work/homestead-horizon/pack/mods/*/carry`) таких блоков **1 165** в 16 модах: `luphieclutteredmod` (373), `chipped` (333), `mcwholidays` (194), `dustydecorations` (75), `justoutdoorstuffs` (49), `perfectplushies` (33), `regions_unexplored` (30), `mcwpaths` (24), `candlelight` (17), `alcocraftplus` (14), `betterendrod` (8), `bakery` (6), `suppsquared` (4), `blahaj` (3), `beautify` (1), `fairylights` (1). Во всех 1 165 файлах `item` по умолчанию `true` (`item: false` и `components` нигде не заданы).
   - В `pack-batch1/mods` таких блоков **4**: `expandeddelight` (`juicer`, `mortar_and_pestle`) и `nethersdelight` (`hoglin_trophy`, `propelplant_wall_torch`).
   - В Farmer's Delight: `cutting_board` (`fd/overlay.py:212`) и `half_tatami_mat` (`fd/stage23.py:126`).
   - В `batch2`: декоративные блоки с `facing` (включая `mcaccel`, `createdeco`, `sootychimneys`).
2. **Наследование компонентов предмета.** При компиляции контент-мода (`crates/horizon-client/src/mod_content.rs:1185-1198`, `1397-1446`) для каждого блока с `item: true` его предмет копирует запись `items.json["minecraft:carved_pumpkin"]`, а затем применяет патч `components` из JSON блока (`patch_components`).
3. **Что приносит `minecraft:carved_pumpkin` в `resources-26.3/items.json`:**
   - `"minecraft:attribute_modifiers"`:
     `[{"type": "minecraft:waypoint_transmit_range", "amount": -1.0, "display": {"type": "hidden"}, "id": "minecraft:waypoint_transmit_range_hide", "operation": "add_multiplied_total", "slot": "head"}]`
   - `"minecraft:equippable"`:
     `{"camera_overlay": "minecraft:misc/pumpkinblur", "slot": "head", "swappable": false}`
   - `"minecraft:compostable"`:
     `{"layers": "minecraft:compostable/medium"}`
4. **Почему строка видна в подсказке.** При чтении `items.json` (`crates/horizon-items/src/items.rs:984-997`) Horizon извлекает только `type`, `id`, `amount`, `operation`, `slot` и **игнорирует поле `"display": {"type": "hidden"}`**. Затем `item_tooltip::attribute_lines` (`crates/horizon-client/src/item_tooltip.rs:146-194`, вызов в `crates/horizon-client/src/game/stations.rs:2469` и `item_tooltip.rs:1022`) выводит группу `"head"` (`item.modifiers.head` → `"When on Head:"`) и строку `-100% Waypoint Transmit Range`.
5. **Побочные эффекты помимо подсказки.** Из-за `minecraft:equippable` (`equip_slot == Some("head")`) любой такой декор можно вручную положить в слот шлема (`crates/horizon-client/src/menu.rs:1442-1455`), надеть раздатчиком (`crates/horizon-client/src/game/dispenser.rs:682`), получить экранный оверлей `misc/pumpkinblur` (`render_tick.rs:342-354`), а из-за `minecraft:compostable` он принимается компостером.

## 2. Сравнение вариантов

### Вариант 1 (РЕКОМЕНДУЕТСЯ): Удалять компоненты в JSON блока через `"components"`

`BlockFile` (`crates/horizon-mods/src/content.rs:311`) и `Block` (`content.rs:344`) поддерживают поле `"components"` у блоков с `item != false` (`content.rs:1276-1279`). `patch_components` (`mod_content.rs:801-818`) поддерживает удаление любого известного ванильного компонента префиксом `!` со значением `{}`:

```json
"components": {
  "!minecraft:attribute_modifiers": {},
  "!minecraft:equippable": {},
  "!minecraft:compostable": {}
}
```

- Все три типа (`minecraft:attribute_modifiers`, `minecraft:equippable`, `minecraft:compostable`) есть у `minecraft:carved_pumpkin` в `items.json`, поэтому проходят `validate_components` и `patch_components`.
- В `crates/horizon-items/src/items.rs:981-997` и `crates/horizon-client/src/item_render.rs:763-768` (`Components = BTreeMap<String, Value>`) отсутствие этих ключей штатно даёт `modifiers = []`, `equip_slot = None`, `equip_camera_overlay = None`, `compostable = None`.
- **Что сохраняется без изменений:**
  - шаблон `minecraft:carved_pumpkin` и все правила `block_looks.py`;
  - горизонтальное размещение `facing` (`Class::CarvedPumpkin` в `crates/horizon-world/src/level/place.rs:366`);
  - физика и звук шаблона (`hardness: 1.0`, `requires_tool: false`, `emission: 0`, `can_occlude: true` для кубов или `false` от `block_looks.py`, `sound: wood`);
  - отсутствие меню по ПКМ (`interact.rs` не открывает GUI для `CarvedPumpkin`);
  - големы не спавнятся: `crates/horizon-client/src/game/golems.rs:16-20` проверяет имя блока `"minecraft:carved_pumpkin" | "minecraft:jack_o_lantern"`, а не класс.
- **Ограничение в `content.rs:1276`:** если у блока `"item": false`, поле `"components"` запрещено (`"item is false, so the block has no item to take components"`). Генератор обязан проверять `decl.get("item") is not False`.
- **Не требует патча движка и пересборки клиента (`cargo`)**: всё решается в `.hmod`.

### Вариант 2 (НЕ РЕКОМЕНДУЕТСЯ): Замена шаблона `minecraft:carved_pumpkin` на другой ванильный блок с `facing`

По `resources-26.3/facts.json`, `gameplay-facts.json`, `block-behaviours.json` и `items.json` проверены **все** ванильные блоки с единственным свойством `facing` (`east/north/south/west`):

| Кандидат | `items.json` | Что меняет в поведении/физике/рендере | Вердикт |
| --- | --- | --- | --- |
| `minecraft:white_glazed_terracotta` (и остальные цвета) | Нет `attribute_modifiers`, `equippable`, `compostable` | `GlazedTerracottaBlock`: `push = "PUSH"` (липкий поршень не тянет), **`requires_tool = true`**, **`hardness = 1.4`**, **`sound = stone`**. У 1 165 перенесённых блоков `properties` пусты — вся мебель, плюшки и т.д. начнут требовать кирку для дропа и звучать как камень | Нельзя без массового задания `properties` |
| `minecraft:jack_o_lantern` | Нет `attribute_modifiers`, `equippable`, `compostable` | **`emission = 15`** (все 1 165 блоков станут источниками света 15, если не прописать всем `properties.light: 0`), `push = "POPPED"` | Нельзя без `properties.light: 0` |
| `minecraft:loom` | Нет `equippable`/`attribute_modifiers`, но есть `minecraft:cooking_fuel` | `Class::Loom`: по ПКМ **открывает меню ткацкого станка** (`crates/horizon-world/src/level/interact.rs:446-455`) | Ломает декор |
| `minecraft:stonecutter`, `minecraft:furnace`, `minecraft:lectern`, `minecraft:anvil` | Нет `equippable` | Открывают GUI по ПКМ (`interact.rs:430-455`), либо падают (`AnvilBlock`), либо имеют `block_entity` / доп. свойства (`lit`, `has_book`, `powered`) / неполную коллизию | Не подходят |
| `minecraft:wall_torch`, `minecraft:wall_banner`, `minecraft:attached_*_stem` | — | Требуют опорную стену (`survive`), пустая коллизия, свет 14 или `block_entity` | Не подходят |

В ванильном 26.3 **нет** ни одного блока с единственным свойством `facing`, который одновременно не требовал бы инструмента, не светил на 15, не открывал GUI и не менял поведение при клике.

### Вариант 3 (не требуется как основной): Патч движка (`engine-patches`)

Потребовался бы только если нужно исправить саму ванильную `minecraft:carved_pumpkin` (игнорирование `"display": {"type": "hidden"}` в `crates/horizon-items/src/items.rs:984-997`) или добавить в `horizon-mods` новый синтетический шаблон без предметов/атрибутов. Но:
- исправление одного только `display.type == "hidden"` **не снимет** `minecraft:equippable` и `minecraft:compostable` с 1 165 декоративных блоков (плюшки и украшения останутся надеваемыми на голову с тыквенным оверлеем);
- Вариант 1 закрывает проблему целиком без пересборки `horizon-client.exe`.

*(Если позже понадобится исправить подсказку и у самой ванильной `minecraft:carved_pumpkin`, достаточно в отдельном engine-patch пропускать модификаторы с `m["display"]["type"].as_str() == Some("hidden")` в `crates/horizon-items/src/items.rs:984-997`.)*

## 3. Замечания по реализации Варианта 1

1. **`block_looks.py` сейчас пропускает неизменённые кубы и `unknown`-модели.**
   В `tools/block_looks.py:402-405` файл в `--out` записывается только при `want_collision or want_occ`, а в `check()` (`443-444`) модели со статусом `look.status != 'ok'` пропускаются через `continue`.
   Поэтому патч `components` для `template == 'minecraft:carved_pumpkin'` и `decl.get('item') is not False` должен срабатывать **независимо от геометрии модели** (в том числе для полных кубов и `unknown`-моделей) как в `process()`, так и в `check()`.
2. **Сохранение существующих `components`.** При слиянии в `process()` добавлять `!minecraft:attribute_modifiers`, `!minecraft:equippable`, `!minecraft:compostable` только если в `decl['components']` нет ни `minecraft:X`, ни `!minecraft:X`.
3. **Настоящие резные тыквы в декоративных модах** (`chipped`: 52 блока, `mcwholidays`: 20 блоков `carved_*_pumpkin`, `mcaccel`: ~28 тыкв/фонарей в batch2). С предложенной правкой они тоже перестанут надеваться на голову и показывать подсказку про Waypoint. Если для каких-то тыкв захочется оставить надевание на голову без подсказки, достаточно в их overlay явно задать `components` или сделать исключение в правиле для `*carved_pumpkin*` (сняв только `!minecraft:attribute_modifiers`).
4. **Охват сборочных скриптов:**
   - `ST/Build-DataMods.ps1` (45 модов) и `ST/Build-DataMods-batch2.ps1` (batch2) **уже вызывают** `tools/block_looks.py --out $tabs ...`.
   - `ST/Build-Batch1.ps1` **не вызывает** `block_looks.py` (в `pack-batch1` 4 таких блока: `expandeddelight:juicer`, `expandeddelight:mortar_and_pestle`, `nethersdelight:hoglin_trophy`, `nethersdelight:propelplant_wall_torch`) — нужно добавить вызов `block_looks.py`.
   - Farmer's Delight (`fd/overlay.py:211-215` — `cutting_board`, `fd/stage23.py:125-132` — `half_tatami_mat`) — прописать `"components"` прямо в объявлениях этих двух блоков.

## 4. Точный список правок (для интегратора)

Все точные фрагменты кода и порядок проверки вынесены в `CS/mixins/homestead/fragments/head-slot.md`:

1. `CS/tools/block_looks.py` — добавить `HEAD_STRIP_KEYS`, `missing_head_strips(decl)`, счётчик `head_stripped`, запись `"components"` в `process()` (для всех `carved_pumpkin` с `item != False`, включая кубы и `unknown`) и проверку `'head_slot'` в `check()` до `if look.status != 'ok': continue`.
2. `CS/tools/test_block_looks.py` — обновить ожидаемые нарушения в `Check.test_unfixed_mod_fails` (`['head_slot', 'occludes', 'oversized']`) и `Check.test_archive_is_read_like_a_directory` (`len == 3`), добавить тесты на полный куб, `unknown`-модель, `item: false`, сохранение пользовательских `components` и идемпотентность.
3. `CS/fd/overlay.py` (`cutting_board`) и `CS/fd/stage23.py` (`half_tatami_mat`) — добавить `"components": {"!minecraft:attribute_modifiers": {}, "!minecraft:equippable": {}, "!minecraft:compostable": {}}`.
4. `ST/Build-Batch1.ps1` — добавить вызов `block_looks.py --out $tabs $overlay` после `creative_tab.py` (как в `Build-DataMods-batch2.ps1`).
5. Перепаковать архивы: `Build-DataMods.ps1`, `Build-Batch1.ps1`, `Build-DataMods-batch2.ps1`, `Build-FarmersDelight.ps1`, прогнать `unittest test_block_looks` и приёмку `-Fixes2` (`Test-HomesteadFixes2.ps1`, где `block_looks.py --check` проверит все 45 базовых архивов).
