# 45 перенесённых модов уровня data (stage17)

`Build-DataMods.ps1` упаковывает `work/homestead-horizon/pack/mods/<мод>/carry` в плоские `.hmod`
(`extra-mods/<id>-<версия>-horizon.1.hmod`, 76 МБ вместе, chipped 16,5 МБ). `Start-Test.ps1` и пакет
подключают все `.hmod` из `extra-mods`. Клиент принял все 45: «content from 48 mod(s)» (45 + Farmer's Delight
+ create + homestead). Приёмка `-DataMods`: для каждого мода с зарегистрированными блоками (20 модов) в мир
ставится выборка из 4 блоков (`setblock`), всего 75 блоков, у каждого мода поставился хотя бы один.
Остальные 25 модов без блоков (структуры, рецепты, функции, ресурсы), их проверяет только строка загрузки.

## Список пропусков

`data-mods-skip.json` пуст: ни один архив не пришлось урезать, загрузчик Horizon сам отбрасывает
неподходящее и пишет это в журнал (`Horizon: ...`). Что осталось за бортом:

- **Блоки, которые `carry` не смог зарегистрировать** (`blocks_unknown` в `carry-report.json`; ассеты есть,
  блока в мире нет): beautify 57 (из 59 останется 2), mcwpaths 57, candlelight 62, bakery 32, chipped 45,
  regions_unexplored 39, suppsquared 28, mcwholidays 26, froglightsreimagined 16, ends_delight 6,
  alcocraftplus 2, natures_delight 1. Причина: пользовательские классы блоков без шаблона в порте.
- **Функции датапаков**: incendium (около 70 функций): неизвестный тег блока `incendium:airs`, частицы `dust`/`flash`
  в старом формате, атрибуты `generic.*`, `item replace`, `summon potion`. Эти способности предметов Incendium не работают.
- **Генерация мира**: 20 «basic feature type», 15 пулов шаблонов «direction», 4 «pool element type» (betterdungeons),
  один пул `kaisyn:village/forest_ruins/town_centers` («unknown block minecraft:grass»): эти элементы выброшены,
  остальные структуры загружены. Мир генерируется ли с ними правильно, не проверялось.
- **Модели**: 24 «standalone model» и около 9 «missing block model» (supplementaries, suppsquared и др.): вместо них
  модель «отсутствует». Текстуры: 5 «missing texture».
- **Замены тегов**: `t_and_t` заменяет 7 тегов `has_structure/*`; записи ванили и regions_unexplored в них выброшены.

## Лицензии

У большинства модов «All Rights Reserved»: архивы только для локального запуска, в git не коммитятся
(`.gitignore`), в пакет попадают в папку `mods` (она в git тоже не входит).

## Не проверено

Поведение блоков (кроме установки), работа структур в мире, выживание, вид моделей, предметы этих модов.
