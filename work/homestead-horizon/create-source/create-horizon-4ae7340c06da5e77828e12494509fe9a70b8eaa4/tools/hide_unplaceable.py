"""Hides carried items that cannot work in Horizon (modded boats) from the player.

    python hide_unplaceable.py [--policy <file>] [--out <dir>] <mod dir> [<mod dir> ...]
    python hide_unplaceable.py --check [--policy <file>] <built .hmod or mod dir> ...   (exit 1 on a leftover)

The policy (tools/unplaceable_items.json) lists, per mod namespace, fnmatch patterns over item names. For every
matching item declared in `data/<ns>/horizon/item/` the tool

  * prints `DROP <relative path>` on stdout for every file the caller must leave out of the archive:
    each recipe of the mod that names a hidden item anywhere (result or ingredient), each recipe advancement
    (`data/<ns>/advancement/recipes/**`) that grants only dropped recipes, and, only when the policy says
    `drop_items: true`, the item declaration with its `assets/<ns>/items` and `assets/<ns>/models/item` files;
  * rewrites `<out>/data/<ns>/horizon/creative_tab/<ns>.json` without the hidden items when that file exists
    (run it after creative_tab.py with the same --out directory).

The items stay registered by default (`drop_items: false`) so saved worlds and item tags that name them keep
loading. A one-line summary goes to stderr. Build-DataMods.ps1 feeds the DROP lines into its per-mod drop list.
"""
import fnmatch
import json
import os
import sys
from pathlib import Path

DEFAULT_POLICY = Path(__file__).with_name('unplaceable_items.json')


def load_policy(path):
    doc = json.loads(Path(path).read_text(encoding='utf-8'))
    return {ns: rule for ns, rule in doc.items() if not ns.startswith('_') and isinstance(rule, dict)}


def hidden_items(root, ns, rule):
    items_dir = root / 'data' / ns / 'horizon' / 'item'
    if not items_dir.is_dir():
        return []
    patterns = rule.get('patterns') or []
    return sorted(p.stem for p in items_dir.glob('*.json') if any(fnmatch.fnmatch(p.stem, pat) for pat in patterns))


def mentions(node, ids):
    """True when any string inside the JSON node is one of the hidden item ids."""
    if isinstance(node, str):
        return node in ids
    if isinstance(node, dict):
        return any(mentions(v, ids) for v in node.values())
    if isinstance(node, list):
        return any(mentions(v, ids) for v in node)
    return False


def rel(root, path):
    return str(path.relative_to(root)).replace('\\', '/')


def process(root, policy, out=None):
    """Returns (drop list of relative paths, summary dict)."""
    drops = []
    summary = {'items': 0, 'recipes': 0, 'recipe_advancements': 0, 'tab_entries': 0, 'item_files': 0}
    for ns, rule in sorted(policy.items()):
        names = hidden_items(root, ns, rule)
        if not names:
            continue
        ids = {f'{ns}:{n}' for n in names}
        summary['items'] += len(names)
        recipe_ids = set()
        for folder in ('recipe', 'recipes'):
            base = root / 'data' / ns / folder
            if not base.is_dir():
                continue
            for path in sorted(base.rglob('*.json')):
                try:
                    doc = json.loads(path.read_text(encoding='utf-8'))
                except (OSError, ValueError):
                    continue
                if mentions(doc, ids):
                    drops.append(rel(root, path))
                    recipe_ids.add(f'{ns}:' + str(path.relative_to(base).with_suffix('')).replace('\\', '/'))
                    summary['recipes'] += 1
        adv = root / 'data' / ns / 'advancement' / 'recipes'
        if adv.is_dir():
            for path in sorted(adv.rglob('*.json')):
                try:
                    doc = json.loads(path.read_text(encoding='utf-8'))
                except (OSError, ValueError):
                    continue
                granted = (doc.get('rewards') or {}).get('recipes') if isinstance(doc, dict) else None
                if isinstance(granted, list) and granted and all(g in recipe_ids for g in granted):
                    drops.append(rel(root, path))
                    summary['recipe_advancements'] += 1
        if rule.get('drop_items'):
            for name in names:
                for candidate in (root / 'data' / ns / 'horizon' / 'item' / f'{name}.json',
                                  root / 'assets' / ns / 'items' / f'{name}.json',
                                  root / 'assets' / ns / 'models' / 'item' / f'{name}.json'):
                    if candidate.is_file():
                        drops.append(rel(root, candidate))
                        summary['item_files'] += 1
        if out is not None:
            tab = out / 'data' / ns / 'horizon' / 'creative_tab' / f'{ns}.json'
            if tab.is_file():
                doc = json.loads(tab.read_text(encoding='utf-8'))
                before = list(doc.get('items') or [])
                doc['items'] = [e for e in before if e not in ids]
                summary['tab_entries'] += len(before) - len(doc['items'])
                if doc['items'] != before:
                    tab.write_text(json.dumps(doc, indent=2) + '\n', encoding='utf-8')
    return sorted(set(drops)), summary


def check_archive(path, policy):
    """Problems left in a built archive (.hmod/.zip) or mod dir: hidden items still listed in a creative tab,
    or recipes that still mention a hidden item. Returns a list of (file, what)."""
    import zipfile
    path = Path(path)
    files = {}
    if path.is_file():
        with zipfile.ZipFile(path) as z:
            for name in z.namelist():
                if name.endswith('.json'):
                    files[name] = z.read(name).decode('utf-8')
    else:
        for file in path.rglob('*.json'):
            # Deeply nested mod data (repurposed_structures template pools) passes MAX_PATH once unpacked:
            # open through the \\?\ long-path prefix on Windows.
            real = str(file.resolve())
            if os.name == 'nt' and not real.startswith('\\\\?\\'):
                real = '\\\\?\\' + real
            with open(real, encoding='utf-8') as handle:
                files[str(file.relative_to(path)).replace('\\', '/')] = handle.read()
    problems = []
    for ns, rule in sorted(policy.items()):
        patterns = rule.get('patterns') or []
        prefix = f'data/{ns}/horizon/item/'
        names = sorted(n[len(prefix):-5] for n in files if n.startswith(prefix) and any(fnmatch.fnmatch(n[len(prefix):-5], p) for p in patterns))
        if not names:
            continue
        ids = {f'{ns}:{n}' for n in names}
        for name, text in sorted(files.items()):
            try:
                doc = json.loads(text)
            except ValueError:
                continue
            if name.startswith(f'data/{ns}/horizon/creative_tab/') and isinstance(doc, dict):
                shown = [e for e in doc.get('items') or [] if e in ids]
                if shown:
                    problems.append((name, f'{len(shown)} hidden item(s) in the creative tab'))
            elif name.startswith((f'data/{ns}/recipe/', f'data/{ns}/recipes/')) and mentions(doc, ids):
                problems.append((name, 'recipe mentions a hidden item'))
    return problems


def main(argv):
    args = list(argv)
    policy_path = DEFAULT_POLICY
    out = None
    do_check = False
    while args and args[0].startswith('--'):
        flag = args.pop(0)
        if flag == '--policy':
            policy_path = Path(args.pop(0))
        elif flag == '--out':
            out = Path(args.pop(0))
        elif flag == '--check':
            do_check = True
        else:
            print(f'unknown option {flag}', file=sys.stderr)
            return 2
    policy = load_policy(policy_path)
    if do_check:
        count = 0
        for arg in args:
            try:
                problems = check_archive(arg, policy)
            except RuntimeError as error:  # the bundled python can lack zlib and cannot read deflated archives
                print(f'cannot read {arg}: {error}. Extract the archive and pass the folder.', file=sys.stderr)
                return 2
            count += len(problems)
            print(f'{Path(arg).name}: {len(problems)} problem(s)')
            for name, what in problems[:20]:
                print(f'  {name}: {what}')
        return 1 if count else 0
    total = {}
    for arg in args:
        drops, summary = process(Path(arg), policy, out)
        for path in drops:
            print('DROP ' + path)
        for key, value in summary.items():
            total[key] = total.get(key, 0) + value
    print('hide_unplaceable: ' + ', '.join(f'{k}={v}' for k, v in total.items()), file=sys.stderr)
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
