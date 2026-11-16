"""Read-only creative-tab audit. Run with python -I tab_audit.py [--format json].

Only --output writes a report. Archives are extracted into an automatically removed
system temporary directory, outside the repository. No imports from carried mods,
builds, game launches, network access or third-party Python packages are needed.
Windows' bundled Python can lack zlib: extraction then uses .NET in PowerShell.
"""
import argparse
import base64
from collections import Counter, defaultdict
from datetime import date
import fnmatch
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import zipfile


CS = Path(__file__).resolve().parents[1]
WORK = CS.parents[1]
REPO = WORK.parents[1]
ST = WORK / 'stage4'
RID = re.compile(r'^[a-z0-9_.-]+:[a-z0-9_./-]+$')
DECL = re.compile(r'^data/([^/]+)/horizon/(?:item|block)/(.+)\.json$')
BLANK = {'missing_item', 'missing_model', 'empty_geometry', 'entity_without_geometry',
         'invalid_json', 'model_cycle', 'empty_item_model', 'unregistered'}


def read_json(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))


def digest(path):
    with path.open('rb') as handle:
        return hashlib.file_digest(handle, 'sha256').hexdigest()


def identifier(value):
    if not isinstance(value, str):
        raise ValueError('resource id must be a string')
    value = value if ':' in value else 'minecraft:' + value
    if not RID.fullmatch(value) or any(p in ('', '.', '..') for p in value.split(':')[1].split('/')):
        raise ValueError('invalid resource id')
    return value


def asset(kind, value, suffix='.json'):
    ns, name = identifier(value).split(':', 1)
    return f'assets/{ns}/{kind}/{name}{suffix}'


def validate_members(archive):
    """Reject traversal, links and Windows aliases before either extractor runs."""
    seen = set()
    for member in archive.infolist():
        name = member.filename
        parts = PurePosixPath(name).parts
        if (not parts or name.startswith('/') or '\\' in name or ':' in name
                or any(p in ('..', '.') or p.rstrip(' .') != p for p in parts)
                or stat.S_ISLNK(member.external_attr >> 16)):
            raise ValueError('unsafe archive member')
        key = name.rstrip('/').casefold()
        if key in seen:
            raise ValueError('duplicate archive member')
        seen.add(key)


def extract_all(paths, temp):
    jobs = []
    for number, path in enumerate(paths):
        with zipfile.ZipFile(path) as archive:
            validate_members(archive)
        jobs.append((path, temp / str(number)))
    if zipfile.zlib is not None:
        for path, target in jobs:
            with zipfile.ZipFile(path) as archive:
                archive.extractall(target)
    else:
        if os.name != 'nt':
            raise ValueError('Python without zlib requires Windows/.NET extraction')
        # WindowsApps' pwsh alias cannot always be started by bundled Python.
        shell = shutil.which('powershell') or shutil.which('pwsh')
        if not shell:
            raise ValueError('PowerShell is required for Python without zlib')
        # Paths go through a JSON environment value, never through shell code.
        command = r"""
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$jobs = $env:TAB_AUDIT_JOBS | ConvertFrom-Json
foreach ($job in $jobs) {
    $root = '\\?\' + [IO.Path]::GetFullPath($job.target)
    [IO.Directory]::CreateDirectory($root) | Out-Null
    $zip = [IO.Compression.ZipFile]::OpenRead($job.source)
    try {
        foreach ($entry in $zip.Entries) {
            $dest = [IO.Path]::Combine($root, $entry.FullName.Replace('/', '\'))
            if ($entry.FullName.EndsWith('/')) {
                [IO.Directory]::CreateDirectory($dest) | Out-Null
                continue
            }
            [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($dest)) | Out-Null
            $src = $entry.Open()
            try {
                $out = [IO.File]::Create($dest)
                try { $src.CopyTo($out) } finally { $out.Dispose() }
            } finally { $src.Dispose() }
        }
    } finally { $zip.Dispose() }
}
"""
        encoded = base64.b64encode(command.encode('utf-16-le')).decode('ascii')
        env = dict(os.environ)
        env['TAB_AUDIT_JOBS'] = json.dumps([{'source': str(p), 'target': str(t)} for p, t in jobs])
        result = subprocess.run([shell, '-NoProfile', '-NonInteractive', '-EncodedCommand', encoded],
                                stdin=subprocess.DEVNULL, env=env, timeout=180,
                                text=True, capture_output=True)
        if result.returncode:
            # Do not leak absolute paths in diagnostic output/report.
            raise ValueError('PowerShell/.NET archive extraction failed')
    return [target for _, target in jobs]


class Store:
    def __init__(self, roots, resources):
        self.files = {}
        self.collisions = set()
        self.resources = resources
        self.cache = {}
        for root in roots:
            for path in root.rglob('*'):
                if path.is_file():
                    rel = path.relative_to(root).as_posix()
                    if (rel.startswith('assets/') and rel in self.files
                            and digest(path) != digest(self.files[rel])):
                        self.collisions.add(rel)
                    self.files[rel] = path

    def path(self, rel):
        if rel in self.files:
            return self.files[rel]
        for root in self.resources:
            path = root / rel
            if path.is_file():
                return path
        return None

    def exists(self, rel):
        return self.path(rel) is not None

    def read(self, rel):
        if rel not in self.cache:
            path = self.path(rel)
            if path is None:
                raise FileNotFoundError(rel)
            self.cache[rel] = read_json(path)
        return self.cache[rel]


def merge_model(store, rid, trail=()):
    """Resolve parents before textures; child texture aliases may fill parent faces."""
    rid = identifier(rid)
    if rid in trail:
        return {}, [('model_cycle', rid)]
    if rid == 'minecraft:builtin/generated':
        return {'generated': True}, []
    if rid == 'minecraft:builtin/entity':
        return {'entity': True}, []
    rel = asset('models', rid)
    if not store.exists(rel):
        return {}, [('missing_model', rel)]
    try:
        doc = store.read(rel)
        if not isinstance(doc, dict):
            raise ValueError('model must be an object')
        parent = doc.get('parent')
        merged, issues = merge_model(store, parent, trail + (rid,)) if parent else ({}, [])
    except (ValueError, TypeError):
        return {}, [('invalid_json', rel)]
    merged = dict(merged)
    merged['textures'] = {**merged.get('textures', {}), **doc.get('textures', {})}
    for key in ('elements', 'loader'):
        if key in doc:
            merged[key] = doc[key]
    return merged, issues


def resolve_texture(value, textures):
    seen = set()
    while isinstance(value, str) and value.startswith('#'):
        key = value[1:]
        if key in seen or key not in textures:
            return None
        seen.add(key)
        value = textures[key]
    return value


def inspect_model(store, rid):
    model, issues = merge_model(store, rid)
    textures = model.get('textures', {})
    if not isinstance(textures, dict):
        return [('invalid_json', identifier(rid))], False
    for key, value in textures.items():
        resolved = resolve_texture(value, textures)
        if resolved is None:
            issues.append(('texture_alias', f'{identifier(rid)}#{key}'))
        else:
            try:
                rel = asset('textures', resolved, '.png')
                if not store.exists(rel):
                    issues.append(('missing_texture', rel))
            except ValueError:
                issues.append(('invalid_texture', f'{identifier(rid)}#{key}'))
    faces = []
    for element in model.get('elements', []) or []:
        if isinstance(element, dict):
            faces.extend((element.get('faces') or {}).values())
    for face in faces:
        if isinstance(face, dict) and isinstance(face.get('texture'), str):
            resolved = resolve_texture(face['texture'], textures)
            if resolved is None:
                issues.append(('texture_alias', identifier(rid) + ':' + face['texture']))
            elif not store.exists(asset('textures', resolved, '.png')):
                issues.append(('missing_texture', asset('textures', resolved, '.png')))
    has_geometry = bool(faces) or (model.get('generated') and any(
        re.fullmatch(r'layer\d+', key) for key in textures))
    if model.get('loader'):
        issues.append(('custom_loader', str(model['loader'])))
        return issues, None  # a custom renderer may supply geometry
    if not has_geometry and not any(code in ('missing_model', 'model_cycle', 'invalid_json') for code, _ in issues):
        issues.append(('entity_without_geometry' if model.get('entity') else 'empty_geometry', identifier(rid)))
    return issues, has_geometry


def inspect_node(store, node):
    if not isinstance(node, dict):
        return [('invalid_json', 'item model node is not an object')], False
    kind = node.get('type', 'minecraft:model')
    if kind == 'minecraft:model':
        try:
            return inspect_model(store, node['model'])
        except (ValueError, KeyError, TypeError):
            return [('invalid_json', 'invalid model id')], False
    if kind == 'minecraft:empty':
        return [('empty_item_model', kind)], False
    if kind in ('minecraft:special', 'minecraft:bundle/selected_item'):
        return [('dynamic_renderer', kind)], None
    children = []
    for key in ('model', 'models', 'on_true', 'on_false', 'fallback', 'cases', 'entries'):
        value = node.get(key)
        for candidate in value if isinstance(value, list) else [value]:
            if isinstance(candidate, dict):
                children.append(candidate.get('model') if 'type' not in candidate else candidate)
    if not children:
        return [('unsupported_item_node', str(kind))], None
    issues, states = [], []
    for child in children:
        found, state = inspect_node(store, child)
        issues.extend(found)
        states.append(state)
    # Issues in an optional branch do not prove the default GUI icon is blank.
    if any(state is True for state in states):
        issues = [('branch_' + code if code in BLANK else code, detail) for code, detail in issues]
        return issues, True
    return issues, None if None in states else False


def wall_pair(name, declared):
    candidates = []
    if name.startswith('wall_'):
        candidates.append(name[5:])
    for suffix, replacement in (('_wall_hanging_sign', '_hanging_sign'), ('_wall_sign', '_sign')):
        if name.endswith(suffix):
            candidates.append(name[:-len(suffix)] + replacement)
    base = re.sub(r'_wall_deco(?:_\d+)?$', '', name)
    if base != name:
        candidates.append(base)
    return next((c for c in candidates if c in declared), None)


def policy_rules():
    patterns = read_json(CS / 'tools/unplaceable_items.json')
    hidden = defaultdict(set)
    sources = []
    for policy in sorted(ST.glob('batch*-policy.json')):
        sources.append((policy.name, digest(policy)))
        for rule in read_json(policy).get('mods', []):
            ns = rule['namespace']
            hidden[ns].update(rule.get('drop_items', []))
            # Builder reports retain the exact names removed by recipe pruning.
            batch = WORK / ('pack-' + policy.name.split('-')[0])
            report = batch / ('overlay-' + rule['key'] + '.json')
            if rule.get('drop_uncraftable_items') and report.is_file():
                sources.append((report.relative_to(WORK).as_posix(), digest(report)))
                for dropped in read_json(report).get('dropped_files', []):
                    match = DECL.fullmatch(dropped)
                    if match and match[1] == ns:
                        hidden[ns].add(match[2])
            elif rule.get('drop_uncraftable_items'):
                raise ValueError('missing overlay report for drop_uncraftable_items: ' + rule['key'])
    sources.append(('tools/unplaceable_items.json', digest(CS / 'tools/unplaceable_items.json')))
    return patterns, hidden, sources


def audit_archive(path, root, store, policies):
    patterns, hidden, _ = policies
    declarations = defaultdict(set)
    for rel in store.files:
        match = DECL.fullmatch(rel)
        if match:
            declarations[match[1]].add(match[2])
    result = {'archive': path.name, 'sha256': digest(path), 'tabs': [], 'entries': [], 'icons': []}
    for tab in sorted(root.glob('data/*/horizon/creative_tab/*.json')):
        rel = tab.relative_to(root).as_posix()
        doc = read_json(tab)
        if not isinstance(doc, dict) or not isinstance(doc.get('items'), list):
            raise ValueError('invalid tab schema: ' + rel)
        result['tabs'].append(rel)
        for role, raw in [('entry', v) for v in doc['items']] + [('icon', doc.get('icon'))]:
            issues = []
            try:
                rid = identifier(raw)
                ns, name = rid.split(':', 1)
            except ValueError:
                raise ValueError('invalid tab entry: ' + rel) from None
            if name not in declarations[ns] and ns != 'minecraft':
                issues.append(('unregistered', rid))
            item = asset('items', rid)
            if not store.exists(item):
                issues.append(('missing_item', item))
            else:
                try:
                    found, _ = inspect_node(store, store.read(item)['model'])
                    issues.extend(found)
                except (ValueError, KeyError, TypeError):
                    issues.append(('invalid_json', item))
            pair = wall_pair(name, declarations[ns])
            if pair:
                issues.append(('wall_pair', f'{ns}:{pair}'))
            if (name in hidden[ns] or any(fnmatch.fnmatchcase(name, p)
                    for p in patterns.get(ns, {}).get('patterns', []))):
                issues.append(('policy_hidden', rid))
            collisions = [item] if item in store.collisions else []
            if collisions:
                issues.append(('resource_collision', item))
            entry = {'id': rid, 'tab': rel, 'issues': sorted(set(issues))}
            result['entries' if role == 'entry' else 'icons'].append(entry)
    result['counts'] = counts(result['entries'])
    return result


def counts(entries):
    total = Counter()
    total['entries'] = len(entries)
    for row in entries:
        codes = {code for code, _ in row['issues']}
        total.update(codes)
        if codes & BLANK:
            total['blank_candidate'] += 1
        if codes & (BLANK | {'policy_hidden', 'wall_pair'}):
            total['filter_candidate'] += 1
    return dict(total)


def markdown(result):
    mods = result['mods']
    total = counts([e for m in mods for e in m['entries']])
    lines = ['# Аудит вкладок креатива', '', '## Сделано', '',
             f"Статический аудит текущих runtime `.hmod`, {result['date']}. "
             'Архивы распакованы во временную папку вне репозитория и удалены после чтения. '
             'Сборка и игра не запускались. Исходные архивы проверены SHA-256 до и после аудита.', '',
             f"Архивов: **{len(mods)}**; вкладок: **{sum(len(m['tabs']) for m in mods)}**; "
             f"записей: **{total['entries']}**. Кандидатов на пустую иконку: "
             f"**{total.get('blank_candidate', 0)}**. Нарушений политик: **{total.get('policy_hidden', 0)}**.", '',
             'Это диагностика ресурсов, а не подтверждение игрового рендера. '
             'Отсутствующая текстура может дать missing-texture, а не пустую ячейку; '
             'настенный дубликат может иметь исправную иконку. Категории пересекаются.', '',
             'Ресурсы Minecraft разрешались по локальному подготовленному снимку `runtime-01/resources`; '
             f"доступных дополнительных корней: {result['resource_roots']}. "
             'Ссылки между `.hmod` проверяются по общему индексу. '
             f"Конфликтующих путей assets с разным содержимым: {len(result['collisions'])}. "
             'Общие data-теги и отдельные манифесты архивов не считаются конфликтами ресурсов.', '',
             '| Архив | Вкладки | Записи | Нет items JSON | Нет модели/родителя | Нет текстуры | Пустая геометрия | builtin/entity без геометрии | Настенная пара | Скрыто политикой | Кандидаты пустых |',
             '| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |']
    for mod in mods:
        c = mod['counts']
        values = [len(mod['tabs'])] + [c.get(k, 0) for k in ('entries', 'missing_item', 'missing_model',
                  'missing_texture', 'empty_geometry', 'entity_without_geometry', 'wall_pair',
                  'policy_hidden', 'blank_candidate')]
        lines.append('| ' + mod['archive'] + ' | ' + ' | '.join(map(str, values)) + ' |')
    lines += ['', '## Правила для creative_tab.py', '',
              '1. До выбора `icon` отфильтровать записи, у которых нет '
              '`assets/<ns>/items/<id>.json`. Не добавлять автоматически все объявления блоков.',
              '2. Рекурсивно проверить item-модель и родителей по общему набору ресурсов '
              '(включая Minecraft). Объединять текстуры родителей с переопределениями потомков, '
              'разрешать `#alias`, обнаруживать циклы. Само наличие JSON и `particle` не означает наличие иконки. '
              'Для обычной модели нужны faces, для `builtin/generated` — `layerN`; '
              '`builtin/entity` без геометрии требует настоящего special renderer или замены модели.',
              '3. Исключать настенный ID только при наличии объявленного парного ID в том же namespace: '
              '`wall_X → X`, `X_wall_sign → X_sign`, `X_wall_hanging_sign → X_hanging_sign`, '
              '`X_wall_deco` / `X_wall_deco_N → X`. Не удалять все имена с `wall`: '
              'многие самостоятельные декорации имеют рабочие иконки и не имеют пары.',
              '4. Применять `unplaceable_items.json` (`fnmatchcase` по имени), явные '
              '`drop_items` из всех `batch*-policy.json` и точные удаления из отчётов overlay '
              'для `drop_uncraftable_items`. Не считать каждый предмет без рецепта скрытым. '
              'Генерировать вкладки после overlay, проверять финальный `.hmod`.',
              '5. Выбирать `icon` только из оставшихся проверенных записей; если их нет — '
              'не создавать вкладку. Удалённые из вкладки ID сохранять зарегистрированными '
              'для совместимости миров, если отдельная политика не требует удаления регистрации.',
              '6. Не скрывать автоматически custom-loader/special-модели: отмечать их отдельно '
              'для проверки поддерживаемого рендера. Недостающие текстуры лучше исправлять, '
              'а не выдавать за доказанный пустой слот.', '',
              '## Особое внимание', '']
    for namespace in ('regions_unexplored', 'perfectplushies', 'phomesteadplushies', 'mcwholidays'):
        rows = [e for m in mods for e in m['entries'] if e['id'].startswith(namespace + ':')]
        c = counts(rows)
        lines.append(f"- `{namespace}`: записей {c['entries']}, кандидатов пустых "
                     f"{c.get('blank_candidate', 0)}, настенных пар {c.get('wall_pair', 0)}, "
                     f"нарушений политик {c.get('policy_hidden', 0)}; "
                     f"объединение рекомендуемого фильтра {c.get('filter_candidate', 0)}.")
    lines += ['', '## Полные списки по модам', '',
              '`missing_item` — нет нового items JSON; `missing_model` — отсутствует модель или родитель; '
              '`missing_texture` — отсутствует PNG; `empty_geometry` — нет faces и генерируемых слоёв; '
              '`entity_without_geometry` — builtin/entity без собственной геометрии; '
              '`wall_pair` — найден парный не-настенный ID; `policy_hidden` — нарушение политики. '
              'Прочие типы/ветки/рендереры и ошибки выводятся явно. '
              'Все ID с проблемами перечислены без усечения; ID без замечаний не перечисляются.', '']
    for mod in mods:
        lines += [f"### {mod['archive']}", '', f"SHA-256: `{mod['sha256']}`.", '']
        rows = [e for e in mod['entries'] if e['issues']]
        if not rows:
            lines += ['Замечаний к записям нет.' if mod['tabs'] else 'Вкладок нет; записей для проверки нет.', '']
        for row in rows:
            detail = '; '.join(f'{code}: {value}' for code, value in row['issues'])
            lines.append(f"- `{row['id']}` — {detail}.")
        bad_icons = [e for e in mod['icons'] if e['issues']]
        if bad_icons:
            lines += ['', 'Значки вкладок (отдельно от числа записей):']
            for row in bad_icons:
                lines.append(f"- `{row['id']}`: " + '; '.join(f'{c}: {v}' for c, v in row['issues']) + '.')
        lines.append('')
    lines += ['## Источники политик', '']
    for name, sha in result['policies']:
        lines.append(f'- `{name}` — SHA-256 `{sha}`.')
    if result['collisions']:
        lines += ['', '## Конфликты ресурсов', ''] + [f'- `{r}`' for r in result['collisions']]
    lines += ['', '## Отличается от оригинала', '',
              'Проверяется порт Horizon и его политики, а не Java-клиент. '
              'Наличие модели/текстур не подтверждает точность переноса.', '',
              '## Не проверено', '',
              'Фактическое изображение каждого предмета в игре, прозрачность PNG, '
              'визуальная корректность transforms, выбор условных веток, поведение специальных '
              'рендереров, производительность перелистывания и применимость модели после кэша. '
              'Ресурсы из импортируемого Create JAR и подключённых ресурспаков вне `.hmod` '
              'не индексируются: такая внешняя ссылка отражается как отсутствующая в данном наборе. '
              'Отдельный мод Create, вплавленный в бинарник, не является `.hmod` этой выборки.', '',
              '## Повторный запуск', '', 'Из корня проекта, PowerShell:', '', '```powershell',
              "$cs = 'work/homestead-horizon/create-source/create-horizon-4ae7340c06da5e77828e12494509fe9a70b8eaa4'",
              "$python = 'work/llvm-mingw/llvm-mingw-20261104-msvcrt-x86_64/python/bin/python.exe'",
              '& $python -I "$cs/tools/tab_audit.py" --self-test',
              '& $python -I "$cs/tools/tab_audit.py" --format markdown',
              '# Для машинного разбора: --format json. Код 0 = аудит выполнен; 1 = --fail-on-issues; 2 = ошибка ввода.',
              '```', '']
    return '\n'.join(lines)


def self_test():
    class MemoryStore:
        def __init__(self, docs):
            self.docs = docs
        def exists(self, rel):
            return rel in self.docs
        def read(self, rel):
            return self.docs[rel]
    docs = {
        'assets/t/models/base.json': {'textures': {'side': '#all'}, 'elements': [
            {'faces': {'north': {'texture': '#side'}}}]},
        'assets/t/models/child.json': {'parent': 't:base', 'textures': {'all': 't:test'}},
        'assets/t/textures/test.png': None,
        'assets/t/models/particle.json': {'textures': {'particle': 't:test'}},
        'assets/t/models/cycle.json': {'parent': 't:cycle'},
        'assets/t/models/entity.json': {'parent': 'builtin/entity'},
        'assets/t/models/generated.json': {'parent': 'builtin/generated', 'textures': {'layer0': 't:test'}},
    }
    store = MemoryStore(docs)
    assert inspect_model(store, 't:child') == ([], True)
    assert inspect_model(store, 't:generated') == ([], True)
    assert ('empty_geometry', 't:particle') in inspect_model(store, 't:particle')[0]
    assert inspect_model(store, 't:cycle')[0][0][0] == 'model_cycle'
    assert inspect_model(store, 't:absent')[0][0][0] == 'missing_model'
    assert inspect_model(store, 't:entity')[0][0][0] == 'entity_without_geometry'
    assert wall_pair('oak_wall_hanging_sign', {'oak_hanging_sign'}) == 'oak_hanging_sign'
    assert wall_pair('purple_ornament_wall_deco_2', {'purple_ornament'}) == 'purple_ornament'
    assert wall_pair('standalone_wall_deco_1', {'standalone_wall_deco_1'}) is None
    assert resolve_texture('#a', {'a': '#b', 'b': '#a'}) is None
    docs.pop('assets/t/textures/test.png')
    assert any(c == 'missing_texture' for c, _ in inspect_model(store, 't:child')[0])
    try:
        identifier('t:../escape')
        raise AssertionError('traversal accepted')
    except ValueError:
        pass
    print('self-test: 12 checks passed')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--mods', type=Path, default=ST / 'runtime/game/horizon-mods')
    parser.add_argument('--resources', type=Path, action='append', default=[])
    parser.add_argument('--format', choices=('json', 'markdown'), default='markdown')
    parser.add_argument('--output', type=Path)
    parser.add_argument('--fail-on-issues', action='store_true')
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if hasattr(sys.stdout, 'reconfigure'):
        sys.stdout.reconfigure(encoding='utf-8')
    if args.self_test:
        self_test()
        return 0
    paths = sorted(args.mods.glob('*.hmod'))
    if not paths:
        raise ValueError('no .hmod archives')
    resources = args.resources or [WORK / 'runtime-01/resources']
    if any(not (r / 'assets/minecraft').is_dir() for r in resources):
        raise ValueError('prepared resources with assets/minecraft required')
    policies = policy_rules()
    before = {p.name: digest(p) for p in paths}
    with tempfile.TemporaryDirectory(prefix='tab-audit-') as temp_name:
        temp = Path(temp_name).resolve()
        if temp.is_relative_to(REPO):
            raise ValueError('system temporary directory must be outside repository')
        roots = extract_all(paths, temp)
        store = Store(roots, resources)
        result = {'date': date.today().isoformat(), 'resource_roots': len(resources), 'policies': policies[2],
                  'collisions': sorted(store.collisions),
                  'mods': [audit_archive(p, r, store, policies) for p, r in zip(paths, roots)]}
        if before != {p.name: digest(p) for p in paths} or paths != sorted(args.mods.glob('*.hmod')):
            raise ValueError('archive set changed during audit; retry after other work finishes')
    output = json.dumps(result, ensure_ascii=False, indent=2) + '\n' if args.format == 'json' else markdown(result)
    if args.output:
        target = args.output.resolve()
        if not target.is_relative_to(REPO):
            raise ValueError('report must stay inside repository')
        target.write_text(output, encoding='utf-8')
    else:
        print(output, end='')
    return int(args.fail_on_issues and any(e['issues'] for m in result['mods']
                                          for e in m['entries'] + m['icons']))


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (OSError, ValueError, zipfile.BadZipFile, subprocess.SubprocessError) as error:
        # OSError strings may contain a private absolute path.
        print('tab_audit: ' + (str(error) if isinstance(error, ValueError) else type(error).__name__), file=sys.stderr)
        sys.exit(2)
