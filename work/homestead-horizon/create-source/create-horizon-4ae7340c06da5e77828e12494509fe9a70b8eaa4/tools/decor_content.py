"""Decorative and material blocks of Create 6.0.10 (stone palettes, metal decor, glass, windows,
copper tiles/shingles, rose quartz, cardboard, casings, trapdoors), declared from decor_catalog.json.

Every block is a vanilla-template block (no Rust logic). Models are vanilla-textured stand-ins:
the player-supplied JAR replaces them through the usual import (names in DECOR_BLOCKS must be in
catalog.BLOCKS). Nothing here copies original artwork. See docs/decor-content.md.
"""
import json
from pathlib import Path

CATALOG = json.loads(Path(__file__).with_name('decor_catalog.json').read_text(encoding='utf-8'))
BLOCK_DEFS = CATALOG['blocks']
DECOR_BLOCKS = [b['id'] for b in BLOCK_DEFS]
# content.py rewrites this tag after the reference tags; pass pickaxe_values() into that write.
DEFERRED_TAGS = ('minecraft/tags/block/mineable/pickaxe.json',)
PICKAXE_TAG = DEFERRED_TAGS[0]

CUTOUT = 'minecraft:cutout'
FACINGS = {'north': 0, 'east': 90, 'south': 180, 'west': 270}


def _tex(name):
    return f'minecraft:block/{name}'


def _ref(name):
    return f'create:block/{name}'


# ---- block declaration ---------------------------------------------------
def block_json(b):
    out = {'template': b['template'], 'item': True, 'properties': dict(b.get('properties', {}))}
    if b['kind'] == 'axis_h':
        out['state_properties'] = {'axis': {'type': 'enum', 'values': ['x', 'z']}}
        out['default_state'] = {'axis': 'z'}
    if b.get('components'):
        out['components'] = dict(b['components'])
    return out


# ---- blockstates ---------------------------------------------------------
def _stairs_states(i):
    angle = {'east': 0, 'south': 90, 'west': 180, 'north': 270}
    variants = {}
    for facing, a in angle.items():
        for half in ('bottom', 'top'):
            for shape in ('straight', 'inner_left', 'inner_right', 'outer_left', 'outer_right'):
                model = _ref(i if shape == 'straight' else f"{i}_{shape.split('_')[0]}")
                left = shape.endswith('left')
                if half == 'bottom':
                    y, x = (a - 90) % 360 if left else a, 0
                else:
                    y, x = a if (left or shape == 'straight') else (a + 90) % 360, 180
                v = {'model': model, 'uvlock': True}
                if x:
                    v['x'] = x
                if y:
                    v['y'] = y
                variants[f'facing={facing},half={half},shape={shape}'] = v
    return {'variants': variants}


def _wall_states(i):
    parts = [{'when': {'up': 'true'}, 'apply': {'model': _ref(i + '_post')}}]
    for side, y in (('north', 0), ('east', 90), ('south', 180), ('west', 270)):
        for height, suffix in (('low', '_side'), ('tall', '_side_tall')):
            apply = {'model': _ref(i + suffix), 'uvlock': True}
            if y:
                apply['y'] = y
            parts.append({'when': {side: height}, 'apply': apply})
    return {'multipart': parts}


def _pane_states(i):
    def part(model, y=0, when=None):
        apply = {'model': _ref(f'{i}_{model}')}
        if y:
            apply['y'] = y
        return {'apply': apply, **({'when': when} if when else {})}
    return {'multipart': [
        part('post'),
        part('side', 0, {'north': 'true'}), part('side', 90, {'east': 'true'}),
        part('side_alt', 0, {'south': 'true'}), part('side_alt', 90, {'west': 'true'}),
        part('noside', 0, {'north': 'false'}), part('noside_alt', 0, {'east': 'false'}),
        part('noside_alt', 90, {'south': 'false'}), part('noside', 270, {'west': 'false'}),
    ]}


def _trapdoor_states(i):
    variants = {}
    for facing, y in FACINGS.items():
        for half in ('bottom', 'top'):
            closed = {'model': _ref(f'{i}_{half}')}
            if y:
                closed['y'] = y
            variants[f'facing={facing},half={half},open=false'] = closed
        opened = {'model': _ref(f'{i}_open')}
        if y:
            opened['y'] = y
        variants[f'facing={facing},half=bottom,open=true'] = opened
        variants[f'facing={facing},half=top,open=true'] = {'model': _ref(f'{i}_open'), 'x': 180, 'y': (y + 180) % 360}
    return {'variants': variants}


def blockstate(b):
    i, kind = b['id'], b['kind']
    one = {'variants': {'': {'model': _ref(i)}}}
    if kind in ('cube', 'glass', 'column'):
        return one
    if kind == 'axis_h':
        return {'variants': {'axis=x': {'model': _ref(i), 'y': 90}, 'axis=z': {'model': _ref(i)}}}
    if kind == 'pillar':
        return {'variants': {'axis=y': {'model': _ref(i)},
                             'axis=x': {'model': _ref(i + '_horizontal'), 'x': 90, 'y': 90},
                             'axis=z': {'model': _ref(i + '_horizontal'), 'x': 90}}}
    if kind == 'slab':
        return {'variants': {'type=bottom': {'model': _ref(i)}, 'type=top': {'model': _ref(i + '_top')},
                             'type=double': {'model': _ref(b['full'])}}}
    if kind == 'stairs':
        return _stairs_states(i)
    if kind == 'wall':
        return _wall_states(i)
    if kind in ('pane', 'bars'):
        return _pane_states(i)
    if kind == 'ladder':
        return {'variants': {f'facing={f}': {'model': _ref(i), **({'y': y} if y else {})}
                             for f, y in {'north': 0, 'south': 180, 'west': 270, 'east': 90}.items()}}
    if kind == 'scaffolding':
        return {'variants': {'bottom=false': {'model': _ref(i)}, 'bottom=true': {'model': _ref(i + '_horizontal')}}}
    if kind == 'trapdoor':
        return _trapdoor_states(i)
    raise ValueError(f'unknown kind {kind} for {i}')


# ---- models ---------------------------------------------------------------
def models(b):
    """Yield (name, json) for assets/create/models/block/<name>.json."""
    i, kind, t = b['id'], b['kind'], b['tex']
    cut = {'render_type': CUTOUT}
    if kind in ('cube', 'axis_h'):
        yield i, {'parent': 'minecraft:block/cube_all', 'textures': {'all': _tex(t['all'])}}
    elif kind == 'glass':
        yield i, {'parent': 'minecraft:block/cube_all', 'textures': {'all': _tex(t['all'])}, **cut}
    elif kind == 'column':
        yield i, {'parent': 'minecraft:block/cube_column', 'textures': {'end': _tex(t['end']), 'side': _tex(t['side'])}}
    elif kind == 'pillar':
        tex = {'end': _tex(t['end']), 'side': _tex(t['side'])}
        yield i, {'parent': 'minecraft:block/cube_column', 'textures': tex}
        yield i + '_horizontal', {'parent': 'minecraft:block/cube_column_horizontal', 'textures': tex}
    elif kind in ('slab', 'stairs'):
        tex = {k: _tex(t['all']) for k in ('bottom', 'top', 'side')}
        parents = {'slab': {'': 'slab', '_top': 'slab_top'},
                   'stairs': {'': 'stairs', '_inner': 'inner_stairs', '_outer': 'outer_stairs'}}[kind]
        for suffix, parent in parents.items():
            yield i + suffix, {'parent': f'minecraft:block/{parent}', 'textures': dict(tex)}
    elif kind == 'wall':
        tex = {'wall': _tex(t['all'])}
        for suffix, parent in (('_post', 'template_wall_post'), ('_side', 'template_wall_side'),
                               ('_side_tall', 'template_wall_side_tall'), ('_inventory', 'wall_inventory')):
            yield i + suffix, {'parent': f'minecraft:block/{parent}', 'textures': dict(tex)}
    elif kind in ('pane', 'bars'):
        tex = {'pane': _tex(t.get('pane', t.get('all'))), 'edge': _tex(t.get('edge', t.get('all')))}
        for suffix in ('post', 'side', 'side_alt', 'noside', 'noside_alt'):
            yield f'{i}_{suffix}', {'parent': f'minecraft:block/template_glass_pane_{suffix}', 'textures': dict(tex), **cut}
    elif kind == 'ladder':
        yield i, {'parent': 'minecraft:block/ladder',
                  'textures': {'texture': _tex(t['all']), 'particle': _tex(t['all'])}, **cut}
    elif kind == 'scaffolding':
        tex = {'top': _tex(t['top']), 'side': _tex(t['side']), 'bottom': _tex(t['bottom']), 'particle': _tex(t['top'])}
        yield i, {'parent': 'minecraft:block/scaffolding_stable', 'textures': dict(tex), **cut}
        yield i + '_horizontal', {'parent': 'minecraft:block/scaffolding_unstable', 'textures': dict(tex), **cut}
    elif kind == 'trapdoor':
        for suffix in ('bottom', 'top', 'open'):
            yield f'{i}_{suffix}', {'parent': f'minecraft:block/template_trapdoor_{suffix}',
                                    'textures': {'texture': _tex(t['all'])}, **cut}
    else:
        raise ValueError(f'unknown kind {kind} for {i}')


def item_model(b):
    i, kind, t = b['id'], b['kind'], b['tex']
    if kind == 'wall':
        return {'parent': _ref(i + '_inventory')}
    if kind == 'trapdoor':
        return {'parent': _ref(i + '_bottom')}
    if kind in ('pane', 'bars', 'ladder'):
        return {'parent': 'minecraft:item/generated', 'textures': {'layer0': _tex(t.get('pane', t.get('all')))}}
    return {'parent': _ref(i)}


# ---- loot -----------------------------------------------------------------
def _silk():
    return {'type': 'minecraft:match_tool', 'predicate': {'predicates': {'minecraft:enchantments': [
        {'enchantments': 'minecraft:silk_touch', 'levels': {'min': 1}}]}}}


def loot(b):
    i, kind = b['id'], b['loot']
    survives = {'type': 'minecraft:survives_explosion'}
    item = {'type': 'minecraft:item', 'name': f'create:{i}'}
    if kind == 'self':
        return {'type': 'minecraft:block', 'pools': [{'rolls': 1, 'bonus_rolls': 0.0, 'conditions': [survives], 'entries': [item]}]}
    if kind == 'silk':
        return {'type': 'minecraft:block', 'pools': [{'rolls': 1, 'bonus_rolls': 0.0, 'conditions': [_silk()], 'entries': [item]}]}
    if kind == 'slab':
        double = {'type': 'minecraft:block_state_property', 'block': f'create:{i}', 'properties': {'type': 'double'}}
        return {'type': 'minecraft:block', 'pools': [{'rolls': 1, 'bonus_rolls': 0.0, 'entries': [{
            'type': 'minecraft:item', 'name': f'create:{i}', 'functions': [
                {'function': 'minecraft:set_count', 'count': 2.0, 'add': False, 'conditions': [double]},
                {'function': 'minecraft:explosion_decay'}]}]}]}
    if kind == 'bound':
        return {'type': 'minecraft:block', 'pools': [
            {'rolls': 1, 'bonus_rolls': 0.0, 'entries': [{'type': 'minecraft:alternatives', 'children': [
                {'type': 'minecraft:item', 'name': f'create:{i}', 'conditions': [_silk()]},
                {'type': 'minecraft:item', 'name': 'minecraft:string', 'conditions': [survives]}]}]},
            {'rolls': 1, 'bonus_rolls': 0.0,
             'conditions': [{'type': 'minecraft:inverted', 'term': _silk()}, survives],
             'entries': [{'type': 'minecraft:item', 'name': 'create:cardboard_block'}]}]}
    raise ValueError(f'unknown loot kind {kind} for {i}')


# ---- tags / lang -----------------------------------------------------------
def tag_values(base_tags=None):
    """{'create/tags/item/...json': {'values': [...]}} merged over base_tags (the reference tag map)."""
    base_tags = base_tags or {}
    merged = {}
    for path, ids in CATALOG['tags'].items():
        old = list(base_tags.get(path, {}).get('values', []))
        merged[path] = {**base_tags.get(path, {}), 'values': old + [v for v in ids if v not in old]}
    return merged


def pickaxe_values(base_tags=None):
    """Complete value list for minecraft/tags/block/mineable/pickaxe.json (reference + decor)."""
    return tag_values(base_tags)[PICKAXE_TAG]['values']


def lang_en():
    return {f'block.create.{name}': title for name, title in CATALOG['lang'].items()}


# ---- entry points ------------------------------------------------------------
def declare(write, model=None, base_tags=None):
    """Write every block, item model, loot table, recipe and tag. `model` is unused (kept for the
    shared helper signature); `base_tags` is the reference tag map so shared tags are merged."""
    for b in BLOCK_DEFS:
        i = b['id']
        write(f'data/create/horizon/block/{i}.json', block_json(b))
        write(f'assets/create/blockstates/{i}.json', blockstate(b))
        for name, data in models(b):
            write(f'assets/create/models/block/{name}.json', data)
        write(f'assets/create/models/item/{i}.json', item_model(b))
        write(f'assets/create/items/{i}.json', {'model': {'type': 'minecraft:model', 'model': f'create:item/{i}'}})
        write(f'data/create/loot_table/blocks/{i}.json', loot(b))
    recipes(write)
    for path, data in tag_values(base_tags).items():
        if path not in DEFERRED_TAGS:
            write('data/' + path, data)


def recipes(write):
    for name, data in CATALOG['recipes'].items():
        write(f'data/create/recipe/palettes/{name}.json', data)


def owned_model_paths():
    """Model files declared here; fallback_textures.retarget must leave them alone."""
    seen = set()
    declare(lambda path, _value: seen.add(path) if path.startswith('assets/create/models/') else None)
    return seen


OWNED_MODEL_PATHS = owned_model_paths()


if __name__ == '__main__':
    import sys
    out = Path(sys.argv[1])

    def dump(path, value):
        target = out / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(json.dumps(value, indent=2) + '\n', encoding='utf-8', newline='\n')
    declare(dump)
    print(f'{len(DECOR_BLOCKS)} blocks, {len(CATALOG["recipes"])} recipes written to {out}')

