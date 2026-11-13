# Homestead Horizon

Экспериментальный перенос сборки Homestead на Rust для нативного клиента [Horizon](https://playhorizon.dev): мод Create, Farmer's Delight и ряд мелких модов логики. Репозиторий содержит **только исходный код, скрипты, тесты приёмки и документы**. Чужих модов, текстур, моделей, звуков, клиента и готовых сборок здесь нет, их нужно получить самому (см. «Сборка»).

> Проект неофициальный. Он не связан с Mojang, Microsoft, авторами Create, Farmer's Delight и остальных модов. Minecraft — торговая марка Mojang/Microsoft.

## Состояние

Рабочее, но экспериментальное. Каждая часть принята автоматическими тестами в игре, на живом клиенте проверено только в творческом режиме. Выживание, внешний вид и многие рецепты не проверены.

| Область | Что есть |
| --- | --- |
| Create: кинетика и конвейеры | сеть вращения, ленты, депо, воронки, прессы, мельницы, вентиляторы, пила, дрель, бойлер и паровой двигатель, свисток |
| Create: жидкости | мультиблочный бак, сеть труб, насосы, котёл на баке |
| Create: автоматизация | Mechanical Arm и точки, Redstone Link, выключатели и Nixie, Display Link и Board, Schematicannon и Schematic Table, Packager и Factory Gauge, Linked Controller, Schematic и Schematic and Quill, Stock Link и Frogport |
| Create: декор | около 350 декоративных блоков на ванильных шаблонах |
| Farmer's Delight | еда, разделочная доска, печь, сковорода, котёл с экраном, посевы, пиры |
| Мелкие моды логики | Right Click Harvest, Secure Crops, Too Cheap!, Preferred Gamerules, Crops Love Rain, Better Days |
| Контент-моды | до 45 «данных» модов упаковываются как оболочки (блоки без поведения, структуры, рецепты) |

Не сделано: поезда Create, копикэты, предпросмотр схем, большинство экранов (GUI) Create, сущности и рендер модов, зависящих от слоя сущностей в Horizon mod API. Полный список в `work/homestead-horizon/stage4/research/roadmap.md` и `docs/` внутри `create-source`.

## Структура

```
work/homestead-horizon/
  create-source/create-horizon-<ревизия>/   форк create-horizon (MIT) + наши модули
    mixins/create/                           порт Create (Rust, миксины Horizon)
    mixins/homestead/                        мелкие моды логики
    fd/                                      наложение компонентов Farmer's Delight
    tools/                                   генераторы контента (content.py и др.)
    docs/                                    документы по каждому срезу
  stage4/                                    сборка, приёмка, исследования
    acceptance/                              сценарии приёмки (PowerShell 7)
    research/roadmap.md                      план и известные долги
    Build-*.ps1, Run-Acceptance.ps1, regress.ps1
```

## Сборка (принеси свои файлы)

Чего нет в репозитории и нужно получить самому:

1. Исходники клиента Horizon и `horizon-mixins` (оба MIT) в `work/horizon-source` и `work/mixins-source`, тулчейн Rust и MinGW, как описано в `create-source/.../docs/installation.md`.
2. **Свои копии модов.** Установите сборку Homestead (CurseForge) или нужные моды сами. Скрипты разбирают ваши JAR локально.
3. Оригинальные ресурсы Create: положите JAR Create, который вы скачали сами, в `horizon-mods/create-resources/` (см. `docs/assets.md`). Без него игра использует встроенные нарисованные заготовки.

Дальше:

```powershell
Copy-Item local-settings.example.json local-settings.json   # впишите свои пути
pwsh work/homestead-horizon/stage4/Build-LocalMods.ps1       # собрать .hmod из ваших JAR в локальную папку
pwsh work/homestead-horizon/stage4/build.ps1                 # собрать клиент (~10-15 минут)
pwsh work/homestead-horizon/stage4/Run-Acceptance.ps1 -ReloadOnly
```

`local-settings.json`, `local/`, `private/`, `outputs/` и любые `.hmod`, `.jar`, `.exe` игнорируются git и **не должны публиковаться**: в них чужие ресурсы.

## Ограничения

- Часть рецептов, форм и звуков оценена, а не сверена с оригиналом.
- Ядра Create писались по пересказу исходников, не по локальной копии, и не сверены построчно.
- Запуск двух копий клиента одновременно портит генерацию контента: запускайте одну.
- Первая загрузка с большим числом модов занимает несколько минут: клиент строит кэш контента. Не закрывайте окно.

## Лицензия

Наши добавления: MIT (`LICENSE`). Файлы upstream и сторонних проектов сохраняют свои лицензии: см. `NOTICE.md` и `THIRD_PARTY.md`.
