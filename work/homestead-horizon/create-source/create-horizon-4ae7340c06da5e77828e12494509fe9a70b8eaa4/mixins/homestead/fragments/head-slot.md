# Убрать атрибут и слот головы у блоков с шаблоном `minecraft:carved_pumpkin`: правки для интегратора

Обоснование и сравнение вариантов см. в `CS/docs/head-slot-attribute.md`. Выбран **Вариант 1** (снятие `minecraft:attribute_modifiers`, `minecraft:equippable` и `minecraft:compostable` в JSON блока через `"components"`):
- без патча движка и без пересборки `horizon-client.exe`;
- сохраняет `facing`, физику, звуки и коллизию без побочных эффектов (`glazed_terracotta` дала бы `requires_tool: true` + `stone` + `PUSH`; `jack_o_lantern` — `light: 15`; `loom`/`furnace`/`stonecutter`/`lectern` — открытие GUI по ПКМ).

Общие файлы ниже не редактировались; их применяет интегратор.

## 1. `CS/tools/block_looks.py`

1. После `EPS = 1e-6` (строка 64) добавить:

```python
HEAD_STRIP_KEYS = (
    'minecraft:attribute_modifiers',
    'minecraft:equippable',
    'minecraft:compostable',
)


def missing_head_strips(decl):
    """Component removals needed on a carved_pumpkin-templated block item so it cannot be worn on the head."""
    if not isinstance(decl, dict) or decl.get('template') != 'minecraft:carved_pumpkin':
        return []
    if decl.get('item') is False:
        return []
    comps = decl.get('components') if isinstance(decl.get('components'), dict) else {}
    return ['!' + k for k in HEAD_STRIP_KEYS if k not in comps and ('!' + k) not in comps]
```

2. В `new_counts()` (строка 374) добавить `'head_stripped': 0`:

```python
def new_counts():
    return {'blocks': 0, 'occludes_false': 0, 'oversized': 0, 'plant_empty': 0, 'empty_solid': 0,
            'geo': 0, 'state_template': 0, 'slab_mismatch': 0, 'other_template': 0, 'unknown': 0,
            'head_stripped': 0, 'written': 0}
```

3. В `process()` (строки 388-421) вычислять `head_strips` для **любого** `carved_pumpkin`-блока с `item != False` (независимо от того, полный это куб или `unknown`-модель):

```python
        if not isinstance(decl, dict) or 'template' not in decl:
            continue
        head_strips = missing_head_strips(decl)
        if head_strips:
            c['head_stripped'] += 1
        look = analyze_models(reader, ns, name)
        verdict = decide(decl['template'], look)
        ...
        want_collision = collision and verdict.collision is not None and 'collision_shapes' not in decl
        want_occ = verdict.occludes_false and (decl.get('properties') or {}).get('occludes') is not False
        want_head = bool(head_strips)
        if not (want_collision or want_occ or want_head):
            continue
        c['written'] += 1
        if dry_run:
            continue
        new = json.loads(json.dumps(decl))
        if want_head:
            comps = new.get('components')
            if not isinstance(comps, dict):
                comps = {}
            for remove_key in head_strips:
                comps[remove_key] = {}
            new['components'] = comps
```

4. В `check(path)` (строки 435-445) проверять `missing_head_strips(decl)` **до** `if look.status != 'ok': continue`:

```python
        for ns, name in block_ids(reader):
            decl = reader.read_json(f'data/{ns}/horizon/block/{name}.json')
            if not isinstance(decl, dict) or 'template' not in decl:
                continue
            template = decl['template']
            if template in STATE_TEMPLATES or (template not in FULL_TEMPLATES and template not in NO_COLLISION_TEMPLATES):
                continue
            head_strips = missing_head_strips(decl)
            if head_strips:
                violations.append((ns, name, 'head_slot', 'carved_pumpkin template without ' + ', '.join(head_strips)))
            look = analyze_models(reader, ns, name)
            if look.status != 'ok':
                if head_strips:
                    checked[ns] = checked.get(ns, 0) + 1
                continue
            checked[ns] = checked.get(ns, 0) + 1
```

## 2. `CS/tools/test_block_looks.py`

1. В `Overlay.test_only_wrong_blocks_are_written` (строка 217) проверить ещё и:
   ```python
   self.assertEqual(plush['components'], {
       '!minecraft:attribute_modifiers': {},
       '!minecraft:equippable': {},
       '!minecraft:compostable': {},
   })
   ```
2. В `Check.test_unfixed_mod_fails` (строка 274) обновить ожидаемый список нарушений:
   ```python
   self.assertEqual(kinds, ['head_slot', 'occludes', 'oversized'])
   ```
3. В `Check.test_archive_is_read_like_a_directory` (строка 298) обновить счётчик нарушений с `2` на `3`:
   ```python
   self.assertEqual(len(violations), 3)
   ```
4. Добавить новые тесты в `Overlay` и `Check`:
   - полный куб и `unknown`-модель на `minecraft:carved_pumpkin` всё равно получают `components` с `!`-удалениями (и проваливают `--check` до обработки);
   - блок с `"item": false` не получает `components` (иначе `horizon-mods` отклонит его с ошибкой `"item is false, so the block has no item to take components"`);
   - существующие `components` в декларации (например, пользовательский `minecraft:max_stack_size` или явный `minecraft:equippable`) сохраняются и не перезаписываются;
   - повторный запуск `process()` на уже обработанном результате пишет `0` файлов (идемпотентность).

## 3. Farmer's Delight: `CS/fd/overlay.py` и `CS/fd/stage23.py`

1. В `CS/fd/overlay.py:211-215` (`cutting_board`):
   ```python
        board.write_text(json.dumps({
            'template': 'minecraft:carved_pumpkin',
            'properties': {'hardness': 0.5, 'occludes': False, 'render_shape': 'MODEL', 'sounds': 'minecraft:oak_planks'},
            'shapes': {'default': shape}, 'collision_shapes': {'default': shape},
            'components': {'!minecraft:attribute_modifiers': {}, '!minecraft:equippable': {}, '!minecraft:compostable': {}}},
            indent=2) + '\n', encoding='utf-8')
   ```
2. В `CS/fd/stage23.py:125-132` (`half_tatami_mat`):
   ```python
        'half_tatami_mat': {
            'template': 'minecraft:carved_pumpkin',
            'item': True,
            'properties': {'hardness': 0.2, 'occludes': False, 'render_shape': 'MODEL',
                           'sounds': 'minecraft:white_wool'},
            'shapes': {'default': _blocks(TATAMI_HALF)},
            'collision_shapes': {'default': _blocks(TATAMI_HALF)},
            'components': {'!minecraft:attribute_modifiers': {}, '!minecraft:equippable': {}, '!minecraft:compostable': {}},
        },
   ```

## 4. `ST/Build-Batch1.ps1`

`Build-DataMods.ps1` (строка 42) и `Build-DataMods-batch2.ps1` (строка 124) уже вызывают `block_looks.py --out $tabs ...`.
В `ST/Build-Batch1.ps1` (сразу после строки 114 `& $python -I (Join-Path $create 'tools/creative_tab.py') --out $tabs $overlay | Out-Null`) добавить:

```powershell
    & $python -I (Join-Path $create 'tools/block_looks.py') --out $tabs $overlay | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "block_looks failed for $id" }
```

*(Также, если в `Test-HomesteadFixes2.ps1` проверяются только `*-horizon.*.hmod`, стоит либо расширить проверку `--check` на `*-batch1.*.hmod` / `*-batch2.*.hmod`, либо проверять их в `-Batch1`/`-Batch2`.)*

## 5. Проверка после применения интегратором

1. Юнит-тесты из `CS/tools`:
   ```powershell
   $env:PYTHONUTF8 = '1'
   & "<workspace>\work\llvm-mingw\llvm-mingw-20261104-msvcrt-x86_64\python\bin\python.exe" -m unittest test_block_looks
   ```
2. Пересборка архивов `.hmod`:
   `pwsh ST/Build-DataMods.ps1`, `pwsh ST/Build-Batch1.ps1`, `pwsh ST/Build-DataMods-batch2.ps1`, `pwsh ST/Build-FarmersDelight.ps1`.
3. Статическая проверка распакованных архивов через `block_looks.py --check` (включая `Test-HomesteadFixes2.ps1` через `Run-Acceptance.ps1 -Fixes2`) и визуальная проверка в игре на вкладке `Mcwholidays` (`Purple Ornament Wall Deco 2`) — строка `When on Head: -100% Waypoint Transmit Range` и возможность надеть в слот шлема должны отсутствовать.
