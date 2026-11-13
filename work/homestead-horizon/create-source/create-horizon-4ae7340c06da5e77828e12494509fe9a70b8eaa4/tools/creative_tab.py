"""Adds a creative tab to a carried content mod directory.

Horizon lists a mod's items in the creative inventory only through
`data/<ns>/horizon/creative_tab/<tab>.json` ({title, icon, items}). `horizon-port-pack carry`
does not write one, so Farmer's Delight and the data-tier mods had no tab at all.

    python creative_tab.py [--out <dir>] <mod dir> [<mod dir> ...]

With --out the files are written under <dir> (same relative paths) and the mod dir is left alone.

For every namespace under `data/` that declares horizon items or blocks this writes
`data/<ns>/horizon/creative_tab/<ns>.json` (all items, then all blocks, sorted) and merges a
`itemGroup.<ns>` title into `assets/<ns>/lang/en_us.json` (kept if already present).
"""
import json
import sys
from pathlib import Path


def pretty(ns):
    return ' '.join(part.capitalize() for part in ns.replace('-', '_').split('_'))


def add_tab(root, out=None):
    out = out or root
    count = 0
    for ns_dir in sorted((root / 'data').glob('*')):
        ns = ns_dir.name
        if ns in ('minecraft', 'c'):
            continue
        horizon = ns_dir / 'horizon'
        items = sorted(p.stem for p in (horizon / 'item').glob('*.json')) if (horizon / 'item').is_dir() else []
        blocks = sorted(p.stem for p in (horizon / 'block').glob('*.json')) if (horizon / 'block').is_dir() else []
        entries = [f'{ns}:{n}' for n in items] + [f'{ns}:{n}' for n in blocks if n not in items]
        if not entries:
            continue
        icon = next((e for e in entries if e.split(':')[1] == ns), entries[0])
        key = f'itemGroup.{ns}'
        tab = out / 'data' / ns / 'horizon' / 'creative_tab' / f'{ns}.json'
        tab.parent.mkdir(parents=True, exist_ok=True)
        tab.write_text(json.dumps({'title': key, 'icon': icon, 'items': entries}, indent=2) + '\n', encoding='utf-8')
        lang = root / 'assets' / ns / 'lang' / 'en_us.json'
        data = json.loads(lang.read_text(encoding='utf-8')) if lang.exists() else {}
        if key not in data:
            data[key] = pretty(ns)
            lang = out / 'assets' / ns / 'lang' / 'en_us.json'
            lang.parent.mkdir(parents=True, exist_ok=True)
            lang.write_text(json.dumps(data, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
        count += 1
    return count


if __name__ == '__main__':
    args = sys.argv[1:]
    out = None
    if args[:1] == ['--out']:
        out, args = Path(args[1]), args[2:]
    total = sum(add_tab(Path(arg), out) for arg in args)
    print(f'Wrote {total} creative tab(s)')
