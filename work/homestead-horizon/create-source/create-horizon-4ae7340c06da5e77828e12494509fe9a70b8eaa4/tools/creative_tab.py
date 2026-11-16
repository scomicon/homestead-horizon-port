"""Adds a creative tab to a carried content mod directory.

Horizon lists a mod's items in the creative inventory only through
`data/<ns>/horizon/creative_tab/<tab>.json` ({title, icon, items}). `horizon-port-pack carry`
does not write one, so Farmer's Delight and the data-tier mods had no tab at all.

    python creative_tab.py [--out <dir>] [--resources <dir>]... [--no-resources] [--assets <dir>]...
                           [--list] <mod dir> [<mod dir> ...]

With --out the files are written under <dir> (same relative paths) and the mod dir is left alone.

For every namespace under `data/` that declares horizon items or blocks this writes
`data/<ns>/horizon/creative_tab/<ns>.json` and merges an `itemGroup.<ns>` title into
`assets/<ns>/lang/en_us.json` (kept if already present; otherwise the `name` from `horizon-mod.toml`
when its `id` is the namespace, else the capitalised namespace).

Entries that would show as an empty creative slot are left out of the tab (they stay registered, so
saved worlds, recipes and tags keep working; see docs/creative-tab-audit.md):

  * no `assets/<ns>/items/<id>.json` (crop blocks, technical blocks, wall variants without an item);
  * the item model chain (parents merged, `#alias` resolved, cycles detected) has no geometry: no faces
    and no `builtin/generated` layer, an unresolvable model/parent, or a bare `builtin/entity`;
  * a wall twin whose paired ID is declared in the same namespace: `wall_X` -> `X`,
    `X_wall_sign` -> `X_sign`, `X_wall_hanging_sign` -> `X_hanging_sign`, `X_wall_deco[_N]` -> `X`.
    Stand-alone `*wall*` items without a pair stay.

Models with a custom loader, special renderers and unresolvable vanilla parents (no resource root known)
are kept: the tool cannot prove they are blank. A missing texture is not hidden either (fix the texture).
The icon is taken from the surviving entries (the entry named like the namespace, else the first one);
a namespace with no surviving entry gets no tab. `--resources` points at an unpacked vanilla resource
root (default: work/homestead-horizon/runtime-01/resources when present); `--assets` adds other mod
directories whose assets may be referenced.
"""
import json
import re
import sys
from pathlib import Path

BUILTIN_GENERATED = 'minecraft:builtin/generated'
BUILTIN_ENTITY = 'minecraft:builtin/entity'
RID = re.compile(r'^[a-z0-9_.-]+:[a-z0-9_./-]+$')
DEFAULT_RESOURCES = Path(__file__).resolve().parents[3] / 'runtime-01' / 'resources'


def pretty(ns):
    return ' '.join(part.capitalize() for part in ns.replace('-', '_').split('_'))


def mod_title(root, ns):
    """`name` from horizon-mod.toml when the mod's id is this namespace."""
    toml = root / 'horizon-mod.toml'
    if not toml.is_file():
        return None
    text = toml.read_text(encoding='utf-8-sig')
    mod_id = re.search(r'^id\s*=\s*"([^"]*)"', text, re.M)
    name = re.search(r'^name\s*=\s*"([^"]*)"', text, re.M)
    if mod_id and name and mod_id.group(1) == ns and name.group(1).strip():
        return name.group(1).strip()
    return None


def identifier(value):
    if not isinstance(value, str):
        raise ValueError('resource id must be a string')
    value = value if ':' in value else 'minecraft:' + value
    if not RID.fullmatch(value) or any(p in ('', '.', '..') for p in value.split(':')[1].split('/')):
        raise ValueError('invalid resource id')
    return value


def asset(kind, rid, suffix='.json'):
    ns, name = rid.split(':', 1)
    return f'assets/{ns}/{kind}/{name}{suffix}'


class Store:
    """Reads assets from the mod root, extra mod roots, then vanilla resource roots."""

    def __init__(self, root, extras=(), resources=()):
        self.roots = [root, *extras]
        self.resources = list(resources)
        self.cache = {}

    def path(self, rel):
        for base in self.roots + self.resources:
            candidate = base / rel
            if candidate.is_file():
                return candidate
        return None

    def read(self, rel):
        if rel not in self.cache:
            path = self.path(rel)
            self.cache[rel] = None if path is None else json.loads(path.read_text(encoding='utf-8-sig'))
        return self.cache[rel]

    def exists(self, rel):
        return self.path(rel) is not None

    @property
    def vanilla_known(self):
        return bool(self.resources)


def merge_model(store, rid, trail=()):
    """(merged model, problems). Problems: missing, cycle, invalid, unknown (unprovable vanilla parent)."""
    if rid in trail:
        return {}, {'cycle'}
    if rid == BUILTIN_GENERATED:
        return {'generated': True}, set()
    if rid == BUILTIN_ENTITY:
        return {'entity': True}, set()
    rel = asset('models', rid)
    try:
        doc = store.read(rel)
    except (OSError, ValueError):
        return {}, {'invalid'}
    if doc is None:
        return {}, {'unknown'} if rid.startswith('minecraft:') and not store.vanilla_known else {'missing'}
    if not isinstance(doc, dict):
        return {}, {'invalid'}
    merged, problems = {}, set()
    parent = doc.get('parent')
    if parent is not None:
        try:
            merged, problems = merge_model(store, identifier(parent), trail + (rid,))
        except ValueError:
            return {}, {'invalid'}
    merged = dict(merged)
    textures = doc.get('textures', {})
    merged['textures'] = {**merged.get('textures', {}), **(textures if isinstance(textures, dict) else {})}
    for key in ('elements', 'loader'):
        if key in doc:
            merged[key] = doc[key]
    return merged, problems


def model_state(store, rid):
    """True: has geometry. False: provably blank. None: cannot tell (keep the entry)."""
    try:
        model, problems = merge_model(store, identifier(rid))
    except ValueError:
        return False
    if model.get('loader'):
        return None
    if problems & {'missing', 'cycle', 'invalid'}:
        return False
    if 'unknown' in problems:
        return None
    elements = model.get('elements') or []
    if any(isinstance(e, dict) and e.get('faces') for e in elements if isinstance(e, dict)):
        return True
    if model.get('generated') and any(re.fullmatch(r'layer\d+', str(k)) for k in model['textures']):
        return True
    return False


def node_state(store, node):
    if not isinstance(node, dict):
        return False
    kind = node.get('type', 'minecraft:model')
    if kind == 'minecraft:model':
        return model_state(store, node.get('model'))
    if kind == 'minecraft:empty':
        return False
    if kind in ('minecraft:special', 'minecraft:bundle/selected_item'):
        return None
    children = []
    for key in ('model', 'models', 'on_true', 'on_false', 'fallback', 'cases', 'entries'):
        value = node.get(key)
        for candidate in value if isinstance(value, list) else [value]:
            if isinstance(candidate, dict):
                children.append(candidate.get('model') if 'type' not in candidate else candidate)
    if not children:
        return None
    states = [node_state(store, child) for child in children]
    if any(state is True for state in states):
        return True
    return None if None in states else False


def item_state(store, ns, name):
    """Returns (state, reason): state False means the creative slot would be empty."""
    rel = f'assets/{ns}/items/{name}.json'
    try:
        doc = store.read(rel)
    except (OSError, ValueError):
        return False, 'invalid_item'
    if doc is None:
        return False, 'no_item'
    state = node_state(store, doc.get('model') if isinstance(doc, dict) else None)
    return state, ('no_geometry' if state is False else None)


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


def pick_icon(entries, ns):
    return next((e for e in entries if e.split(':')[1] == ns), entries[0])


def add_tab(root, out=None, resources=(), extras=(), hidden_log=None):
    out = out or root
    store = Store(root, extras, resources)
    count = 0
    for ns_dir in sorted((root / 'data').glob('*')):
        ns = ns_dir.name
        if ns in ('minecraft', 'c'):
            continue
        horizon = ns_dir / 'horizon'
        items = sorted(p.stem for p in (horizon / 'item').glob('*.json')) if (horizon / 'item').is_dir() else []
        blocks = sorted(p.stem for p in (horizon / 'block').glob('*.json')) if (horizon / 'block').is_dir() else []
        declared = set(items) | set(blocks)
        entries, hidden = [], {}
        for name in items + [n for n in blocks if n not in items]:
            if wall_pair(name, declared):
                hidden[name] = 'wall_pair'
                continue
            state, reason = item_state(store, ns, name)
            if state is False:
                hidden[name] = reason
                continue
            entries.append(f'{ns}:{name}')
        if hidden_log is not None:
            hidden_log.update({f'{ns}:{n}': why for n, why in hidden.items()})
        if not entries:
            continue
        key = f'itemGroup.{ns}'
        tab = out / 'data' / ns / 'horizon' / 'creative_tab' / f'{ns}.json'
        tab.parent.mkdir(parents=True, exist_ok=True)
        tab.write_text(json.dumps({'title': key, 'icon': pick_icon(entries, ns), 'items': entries}, indent=2) + '\n',
                       encoding='utf-8')
        lang = root / 'assets' / ns / 'lang' / 'en_us.json'
        data = json.loads(lang.read_text(encoding='utf-8')) if lang.exists() else {}
        if key not in data:
            data[key] = mod_title(root, ns) or pretty(ns)
            lang = out / 'assets' / ns / 'lang' / 'en_us.json'
            lang.parent.mkdir(parents=True, exist_ok=True)
            lang.write_text(json.dumps(data, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
        count += 1
    return count


def main(argv):
    out, resources, extras, listing, use_default = None, [], [], False, True
    args = list(argv)
    mods = []
    while args:
        arg = args.pop(0)
        if arg == '--out':
            out = Path(args.pop(0))
        elif arg == '--resources':
            resources.append(Path(args.pop(0)))
        elif arg == '--assets':
            extras.append(Path(args.pop(0)))
        elif arg == '--no-resources':
            use_default = False
        elif arg == '--list':
            listing = True
        else:
            mods.append(Path(arg))
    if use_default and not resources and (DEFAULT_RESOURCES / 'assets' / 'minecraft').is_dir():
        resources = [DEFAULT_RESOURCES]
    log = {}
    total = sum(add_tab(mod, out, resources, extras, log) for mod in mods)
    reasons = {}
    for why in log.values():
        reasons[why] = reasons.get(why, 0) + 1
    detail = ', '.join(f'{k}={v}' for k, v in sorted(reasons.items())) or 'none'
    print(f'Wrote {total} creative tab(s); left out of tabs: {len(log)} ({detail})')
    if listing:
        for rid, why in sorted(log.items()):
            print(f'  {rid} {why}')


if __name__ == '__main__':
    main(sys.argv[1:])
