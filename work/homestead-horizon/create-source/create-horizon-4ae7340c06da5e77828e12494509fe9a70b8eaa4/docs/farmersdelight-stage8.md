# Homestead stage 8: Farmer's Delight, slices 1 and 2

Farmer's Delight Refabricated 2.4.1 (MIT). The pack holds two jars with the id `farmersdelight` (1.4.3
and 2.4.1); Fabric loads the newer, so 2.4.1 is the port source. The result is a content mod
(`stage4/extra-mods/farmersdelight-2.4.1-horizon.N.hmod`, `side = both`) built by
`stage4/Build-FarmersDelight.ps1` from the player's own jar, plus a few lines of native code in the
`homestead` bundle. The jar and the generated mod are local and not committed.

## Slice 1: carried content

`horizon-port-pack carry farmersdelight` turns the jar's data and assets into a content mod:
109 blocks (17 left out because no vanilla template has their properties), 91 items, loot tables,
models, textures, lang, most recipes, tags. Blocks take the nearest vanilla template, so crates are
stone, the stove a furnace, the cutting board a carved pumpkin; they place and break, but a block
entity's behaviour is not there. Left out: `cooking_pot`, `rice`, `rice_roll_medley_block`,
`roast_chicken_block`, `honey_glazed_ham_block`, `apple_pie`, `chocolate_pie`, `full_tatami_mat`,
`organic_compost`, `rich_soil_farmland`, `rope` and the other feast blocks (see
`carry-report.json`, `blocks_unknown`).

## Slice 2: item components and Nourishment

`fd/overlay.py` rewrites the carried item shells (all `minecraft:stick`) with the data the Java
code sets:

- 76 foods from `FoodValues` (1.20 sources, vectorwing): nutrition, saturation (stored absolute in
  26.3, so `nutrition * modifier * 2`), always-eat, fast eating (0.8 s instead of 1.6 s), and effects.
- Drinks (bottles, bone broth) use the drink animation and sound.
- `use_remainder`: bowl for soups, stews, plated meals, feast portions and dog food; glass bottle for
  milk, cocoa, cider, melon juice and custard.
- Stack sizes (`max_stack_size` is a top-level key of the item declaration, not a component).
- Five knives on an iron sword template: attack damage 0.5 plus the tier bonus, attack speed -2,
  durability (flint 131, iron 250, gold 32, diamond 1561, netherite 2031), enchantability, repair
  item, a tool rule for `#farmersdelight:mineable/knife` and cobwebs, netherite fire-resistant.
- Mob effect `farmersdelight:nourishment` (beneficial, colour 0x8DC15E). Its tick is native
  (`mixins/homestead/nourishment.rs`, on `level_tick_pre`): while the effect lasts, exhaustion is reset
  every tick unless the player is healing from saturation (natural regeneration on, hurt, saturation
  above 0), as `NourishmentEffect`.

Verified in game (`stage4/Run-Acceptance.ps1 -FarmersDelight`): Hunger drains food without the effect;
cooked rice restores 6 and returns a bowl and gives Nourishment; cooked bacon is eaten with a 1.0 s
hold (fast); with Nourishment the same Hunger no longer lowers food.

## Not done yet

- Comfort effect, fuel burn times (straw bale, canvas, cabinets, baskets), the special item classes
  (milk bottle curing, hot cocoa, melon juice, rotten tomato throw, popsicle, dog food, horse feed, kelp
  roll slicing, mushroom colonies, rice placing, ropes).
- Knife behaviour beyond the tool data: cutting board recipes, cake slicing, knockback -0.1, no
  block breaking in creative, enchantment limits (fortune excluded, looting allowed).
- Everything with a block entity or state: cooking pot, stove, skillet, cutting board, baskets,
  cabinets, feast and pie blocks, crops, rich soil, canvas signs. See `research/roadmap.md`.
- Food values come from the 1.20 sources of the Forge version and were not compared with the
  Refabricated 2.4.1 bytecode.
- The acceptance world is shared with the other suites: after this slice `Start-Test.ps1` loads every
  `.hmod` in `stage4/extra-mods`. The `stage7` package does not contain this mod.

## Срез 3a: разделочная доска (stage 15)

Блок `farmersdelight:cutting_board` (тонкий, AABB 1..15 x 0..1 x 1..15) получил блок-сущность и поведение в бандле `homestead` (`mixins/homestead/cutting_board.rs`): ПКМ с предметом кладёт один предмет на доску, ПКМ пустой рукой забирает его, ПКМ инструментом нарезает по рецептам `farmersdelight:cutting` (125 рецептов из мода: ножи `c:tools/knives`, ножницы, топор/кирка/лопата по `tool_action`; результаты с `count` и `chance`), результат вылетает вбок от лицевой стороны, инструмент получает 1 урон, при разрушении доски предмет выпадает, предмет рисуется на доске. Формат NBT собственный (`Item`), не `Inventory` оригинала.

**Не сделано и не проверено:** режим выживания (расход предмета, износ ножа, разрушение игроком проверены только командой в творческом режиме); бонус удачи; звук ножа `farmersdelight:block.cutting_board.knife` (проигрывается по имени, ассет мог не попасть в мод); нарезка торта на месте; отрисовка предмета на доске (углы подобраны наугад); частицы; фильтр лишних инструментов (`OFFHAND_EQUIPMENT`); Stove и Skillet.

## Срез 3b: печь и сковорода (stage 16)

**Печь** (`mixins/homestead/stove.rs`, подробности в `farmersdelight-stove.md`): 6 слотов, еда с рецептом `minecraft:campfire_cooking` кладётся ПКМ в первый свободный слот, готовка только при `lit=true`, холодная печь откатывает прогресс на 2 за тик, результат вылетает сверху; розжиг огнивом или огненным шаром, гашение лопатой или ведром воды; содержимое выпадает при ломании игроком. **Сковорода** (`mixins/homestead/skillet.rs`, `fd/skillet.py`, `farmersdelight-skillet.md`): блок на тепловом источнике (магма, горящая печь, костёр), в ней один стек еды готовится по времени `0.2` от рецепта костра, ПКМ пустой рукой забирает, рецепт и предмет объявлены оверлеем (в переносе их не было).

**Не сделано:** шаг игрока по горячей печи и частицы; выбор слота по месту клика; готовка сковородой в руке (анимация, переворот); огненный аспект и сохранение предмета сковороды; свойство `support` и подкладка лотка проверены только по состоянию; формат NBT печи и сковороды собственный; печь, ломаемая не игроком (взрыв, поршень, `setblock air`), теряет содержимое. Команда `setblock … destroy` не вызывает хук разрушения, поэтому тесты ломают блок через `break`.

## Срез 4a: котёл без экрана (stage 17)

Блок `farmersdelight:cooking_pot` (`facing`, `support` none/tray/handle, `waterlogged`) и его предмет объявлены `fd/cooking_pot.py` (в переносе их не было), поведение в `mixins/homestead/cooking_pot.rs` (`farmersdelight-pot.md`). Вместо экрана: ПКМ с предметом кладёт по одному предмету в 6 слотов ингредиентов, нагретый котёл (источник тепла снизу, печь горит) готовит по `farmersdelight:cooking` (26 рецептов, точное совпадение, теги, остатки вроде бутылок), порция попадает в слот блюда, подходящая ёмкость (миска или бутылка) выдаёт одну порцию, блюда без ёмкости берутся пустой рукой, пустая рука возвращает ингредиенты, при ломании всё выпадает.

**Не сделано:** экран (меню на API `open_menu` и `menu_*` есть, но не подключён); слоты 7 и 8 (ёмкость и выход) не используются; автоматизация воронкой и рукой Create; компаратор; переключение рукоятки; кастомное имя; опыт; звук кипения и частицы; сохранение блюда в предмете при добыче (потеря блюда при сломе); формат NBT собственный (`Inventory`, `CookTime`).

## Срез 4b: экран котла (stage 18)

Котёл открывает настоящее меню движка: тип меню `farmersdelight:cooking_pot` (9 слотов, 3 слота данных, выход 8 только на забор) объявлен `fd/cooking_pot.py`, предметы хранятся в контейнере блок-сущности, экран рисует `native_screen_draw` по текстуре `gui/cooking_pot` (стрелка прогресса, значок нагрева, заголовок). Слот блюда (6) на экране скрыт, блюдо рисуется поверх (мод-меню не умеет «слот, из которого нельзя брать»). Поток слотов как в оригинале: блюдо в слоте 6, ёмкость в слоте 7, порции уходят в выход 8; подача рукой нужной ёмкостью осталась. Блок получил шаблон `minecraft:barrel` (его «использование» открывает меню; у шаблона `glass` оно ничего не делало), поэтому `facing` и `open` берутся из бочки.

**Не сделано:** воронки и компаратор (контейнер закрыт для граней), рецептная книга, подсказка блюда, ограничение слота 7 по ёмкости, английские подписи на экране без перевода, звук и частицы, блюдо в предмете при добыче; проверено в творческом режиме.