"""Fixes location predicates of carried data (advancements, predicates, loot tables) for Horizon 26.3.

Mods written for 1.20.1/1.21.1 name the place condition of a location predicate with the singular
keys `structure` and `biome`. Horizon 26.3 reads only `structures` and `biomes`
(horizon-client/src/advancements.rs `location()`), silently ignores every other key, and then the
predicate is empty and therefore always true. A `minecraft:location` criterion that holds only such a
predicate is met on the first check, so a fresh world grants dozens of structure and biome
advancements at once (1000 XP each for Repurposed Structures).

    python advancement_fix.py [--out <dir>] [--no-strict] [--report <file>] <mod dir> [<mod dir> ...]
    python advancement_fix.py --check <built .hmod or mod dir> ...      (exit 1 when a file would still change)

With --out only the changed files are written there (same relative paths as in the mod dir) and the mod
dir is left alone, exactly like creative_tab.py. Without --out the files are changed in place.

What it does, in every JSON file under data/<ns>/{advancement,advancements,predicate,predicates,
loot_table,loot_tables}:
  1. In every location predicate (the value of `location`, `stand_on`, `steps_on`,
     `movement_affected_by`, and the `predicate` of a `location_check` condition) it renames
     `biome` -> `biomes`, `structure` -> `structures` and the pre-1.19 `feature` -> `structures`. The value
     (id, `#tag` or list) is kept: 26.3 matches a string, a tag or a list.
  2. It fails loudly (exit code 2) on shapes it cannot map: a singular key whose value is not a string or a
     list of strings, a singular and a plural key side by side, or an unknown key in a location predicate
     (26.3 would ignore it and could leave the predicate always true). `player` is the one tolerated
     unknown key (Incendium nests a stray `player` there; 26.3 ignores it). `--no-strict` turns the errors
     into warnings.
  3. A `minecraft:location` criterion whose whole condition is a vacuous (always true) location predicate is
     neutralised to `minecraft:impossible`, so it can never be granted instantly. `minecraft:tick` criteria
     (root advancements) are left alone.
"""
import json
import sys
from pathlib import Path

DIRS = ('advancement', 'advancements', 'predicate', 'predicates', 'loot_table', 'loot_tables')
LOCATION_KEYS = ('location', 'stand_on', 'steps_on', 'movement_affected_by')
RENAME = {'biome': 'biomes', 'structure': 'structures', 'feature': 'structures'}
KNOWN = {'position', 'dimension', 'biomes', 'structures', 'can_see_sky', 'smokey', 'light', 'block', 'fluid'}
TOLERATED = {'player'}
LOCATION_TRIGGERS = ('minecraft:location', 'location')


class ShapeError(Exception):
    pass


class Stats:
    def __init__(self):
        self.files = 0
        self.changed_files = 0
        self.renames = {'biome': 0, 'structure': 0, 'feature': 0}
        self.neutralised = 0
        self.by_kind = {}
        self.warnings = []

    def merge(self, other):
        self.files += other.files
        self.changed_files += other.changed_files
        for k, v in other.renames.items():
            self.renames[k] += v
        self.neutralised += other.neutralised
        for k, v in other.by_kind.items():
            self.by_kind[k] = self.by_kind.get(k, 0) + v
        self.warnings += other.warnings


def fix_location(pred, where, stats, strict):
    """Rewrites one location predicate dict in place; returns True when it changed."""
    changed = False
    for old, new in RENAME.items():
        if old not in pred:
            continue
        value = pred[old]
        ok = isinstance(value, str) or (isinstance(value, list) and all(isinstance(x, str) for x in value))
        if not ok:
            raise ShapeError(f'{where}: `{old}` has an unsupported value {json.dumps(value)[:80]}')
        if new in pred:
            raise ShapeError(f'{where}: both `{old}` and `{new}` are present')
        del pred[old]
        pred[new] = value
        stats.renames[old] += 1
        changed = True
    for key in list(pred):
        if key not in KNOWN and key not in TOLERATED:
            message = f'{where}: unknown location key `{key}` (26.3 ignores it)'
            if strict:
                raise ShapeError(message)
            stats.warnings.append(message)
    return changed


def is_vacuous(pred):
    """True when a location predicate constrains nothing (26.3 reads it as always true)."""
    if not isinstance(pred, dict):
        return False
    for key in KNOWN:
        value = pred.get(key)
        if value in (None, {}, [], ''):
            continue
        if key == 'position' and isinstance(value, dict) and all(v in (None, {}, '') for v in value.values()):
            continue
        return False
    return True


def walk(node, where, stats, strict):
    changed = False
    if isinstance(node, dict):
        if node.get('condition') in ('minecraft:location_check', 'location_check') and isinstance(node.get('predicate'), dict):
            changed |= fix_location(node['predicate'], where + '/predicate', stats, strict)
        for key, value in node.items():
            if key in LOCATION_KEYS and isinstance(value, dict):
                changed |= fix_location(value, f'{where}/{key}', stats, strict)
            changed |= walk(value, f'{where}/{key}', stats, strict)
    elif isinstance(node, list):
        for index, item in enumerate(node):
            changed |= walk(item, f'{where}[{index}]', stats, strict)
    return changed


def location_of(condition):
    """The location predicate a single player condition is made of, or None."""
    if not isinstance(condition, dict):
        return None
    if condition.get('condition') in ('minecraft:entity_properties', 'entity_properties'):
        pred = condition.get('predicate')
        if isinstance(pred, dict) and set(pred) == {'location'} and isinstance(pred['location'], dict):
            return pred['location']
    if condition.get('condition') in ('minecraft:location_check', 'location_check'):
        pred = condition.get('predicate')
        if isinstance(pred, dict) and not condition.get('offsetX') and not condition.get('offsetY') and not condition.get('offsetZ'):
            return pred
    return None


def neutralise(doc, where, stats):
    """minecraft:location criteria that are always true become minecraft:impossible."""
    changed = False
    criteria = doc.get('criteria') if isinstance(doc, dict) else None
    if not isinstance(criteria, dict):
        return False
    # Technical advancements (no display) are mod plumbing: Incendium keeps an always-true
    # "log_spam_fixer" criterion in them on purpose, so only advancements the player sees are neutralised.
    display = doc.get('display')
    if not isinstance(display, dict) or not (display.get('announce_to_chat', True) or display.get('show_toast', True)):
        return False
    for name, criterion in criteria.items():
        if not isinstance(criterion, dict) or criterion.get('trigger') not in LOCATION_TRIGGERS:
            continue
        conditions = criterion.get('conditions') or {}
        player = conditions.get('player') if isinstance(conditions, dict) else None
        if player is None and not conditions:
            vacuous = True  # no condition at all: fires at once
        elif isinstance(player, list) and player:
            vacuous = all(location_of(c) is not None and is_vacuous(location_of(c)) for c in player)
        else:
            vacuous = False
        if vacuous:
            criteria[name] = {'trigger': 'minecraft:impossible'}
            stats.neutralised += 1
            stats.warnings.append(f'{where}: criterion `{name}` was always true, now minecraft:impossible')
            changed = True
    return changed


def fix_document(doc, where, strict=True):
    """Fixes one parsed JSON document in place. Returns (changed, stats)."""
    stats = Stats()
    changed = walk(doc, where, stats, strict)
    changed |= neutralise(doc, where, stats)
    return changed, stats


def process_mod(root, out=None, strict=True):
    stats = Stats()
    data = root / 'data'
    if not data.is_dir():
        return stats
    for ns_dir in sorted(data.glob('*')):
        for dirname in DIRS:
            base = ns_dir / dirname
            if not base.is_dir():
                continue
            for path in sorted(base.rglob('*.json')):
                rel = path.relative_to(root)
                try:
                    doc = json.loads(path.read_text(encoding='utf-8'))
                except (OSError, ValueError) as error:
                    raise ShapeError(f'{rel}: not valid JSON ({error})')
                stats.files += 1
                changed, one = fix_document(doc, str(rel).replace('\\', '/'), strict)
                stats.merge(one)
                if changed:
                    stats.changed_files += 1
                    stats.by_kind[dirname] = stats.by_kind.get(dirname, 0) + 1
                    target = (out / rel) if out else path
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_text(json.dumps(doc, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
    return stats


def check_paths(path):
    """Files of a built archive (.hmod/.zip) or mod dir that advancement_fix would still change. Returns a list."""
    import copy
    import zipfile
    path = Path(path)
    bad = []

    def examine(name, raw):
        try:
            doc = json.loads(raw)
        except ValueError:
            bad.append((name, 'not valid JSON'))
            return
        changed, stats = fix_document(copy.deepcopy(doc), name, strict=False)
        if changed:
            what = ', '.join(f'{k}={v}' for k, v in stats.renames.items() if v) or 'vacuous criterion'
            bad.append((name, what))

    def wanted(parts):
        return len(parts) >= 4 and parts[0] == 'data' and parts[2] in DIRS and parts[-1].endswith('.json')

    if path.is_file():
        with zipfile.ZipFile(path) as z:
            for name in sorted(z.namelist()):
                if wanted(name.split('/')):
                    examine(name, z.read(name).decode('utf-8'))
    else:
        for file in sorted(path.rglob('*.json')):
            name = str(file.relative_to(path)).replace('\\', '/')
            if wanted(name.split('/')):
                examine(name, file.read_text(encoding='utf-8'))
    return bad


def main(argv):
    out = None
    strict = True
    report = None
    do_check = False
    args = list(argv)
    while args and args[0].startswith('--'):
        flag = args.pop(0)
        if flag == '--out':
            out = Path(args.pop(0))
        elif flag == '--no-strict':
            strict = False
        elif flag == '--report':
            report = Path(args.pop(0))
        elif flag == '--check':
            do_check = True
        else:
            print(f'unknown option {flag}', file=sys.stderr)
            return 2
    if do_check:
        violations = 0
        for arg in args:
            try:
                bad = check_paths(arg)
            except RuntimeError as error:  # the bundled python can lack zlib and cannot read deflated archives
                print(f'cannot read {arg}: {error}. Extract the archive and pass the folder.', file=sys.stderr)
                return 2
            violations += len(bad)
            print(f'{Path(arg).name}: {len(bad)} file(s) still need advancement_fix')
            for name, what in bad[:20]:
                print(f'  {name}: {what}')
        return 1 if violations else 0
    total = Stats()
    try:
        for arg in args:
            total.merge(process_mod(Path(arg), out, strict))
    except ShapeError as error:
        print(f'advancement_fix: {error}', file=sys.stderr)
        return 2
    summary = (f'advancement_fix: {total.files} files checked, {total.changed_files} changed, '
               f'renames biome={total.renames["biome"]} structure={total.renames["structure"]} '
               f'feature={total.renames["feature"]}, neutralised={total.neutralised}, '
               f'changed by folder: {dict(sorted(total.by_kind.items()))}')
    print(summary)
    for message in total.warnings[:50]:
        print('  warning: ' + message, file=sys.stderr)
    if report:
        report.write_text(summary + '\n' + '\n'.join(total.warnings) + '\n', encoding='utf-8')
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
