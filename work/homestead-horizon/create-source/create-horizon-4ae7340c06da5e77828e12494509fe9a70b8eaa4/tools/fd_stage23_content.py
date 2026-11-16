"""Content check for the Farmer's Delight stage 23 slice (fd/stage23.py).

Runs `stage23.patch` on a COPY of an overlay directory (the output of fd/overlay.py, for
example `work/homestead-horizon/fd-overlay`) and checks that what it writes fits the carried
assets: every declared block has a blockstate whose variants only name properties the block has
and whose models exist, a loot table, a language line; the basket shapes are proper turns of the
open box; the cabinets cover all six facings. Nothing in the overlay itself is changed.

    python tools/fd_stage23_content.py <overlay dir>        # the check on a copy
    python tools/fd_stage23_content.py --selftest          # only the geometry checks
"""
import importlib.util
import json
import re
import shutil
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('fd_stage23', HERE.parent / 'fd' / 'stage23.py')
stage23 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(stage23)

NS = stage23.NAMESPACE

# State properties the vanilla templates bring (the declaration may not repeat them).
TEMPLATE_PROPERTIES = {
    'minecraft:barrel': {'facing', 'open'},
    'minecraft:chorus_plant': {'north', 'east', 'south', 'west', 'up', 'down'},
    'minecraft:carved_pumpkin': {'facing'},
    'minecraft:white_carpet': set(),
    'minecraft:brown_mushroom': set(),
    'minecraft:red_mushroom': set(),
}

problems = []


def check(condition, message):
    if not condition:
        problems.append(message)


def selftest():
    """The turned basket: floor and walls, one rotation per facing."""
    up = stage23.turned_boxes(stage23.BASKET_UP, 'up')
    check(up[0] == [0.0, 0.0, 0.0, 1.0, 0.125, 1.0], f'up floor is {up[0]}')
    for facing in stage23.FACINGS:
        boxes = stage23.turned_boxes(stage23.BASKET_UP, facing)
        check(len(boxes) == 5, f'{facing}: {len(boxes)} boxes')
        volume = sum((b[3] - b[0]) * (b[4] - b[1]) * (b[5] - b[2]) for b in boxes)
        base = sum((b[3] - b[0]) * (b[4] - b[1]) * (b[5] - b[2]) for b in up)
        check(abs(volume - base) < 1e-9, f'{facing}: volume {volume} differs from {base}')
        check(all(0.0 <= v <= 1.0 for b in boxes for v in b), f'{facing}: a box leaves the cell')
    # The floor lies on the side opposite to the opening.
    floors = {
        'down': (4, 0.875, 1.0),   # y from 14/16
        'north': (5, 0.875, 1.0),  # z from 14/16
        'south': (5, 0.0, 0.125),
        'east': (3, 0.0, 0.125),   # x up to 2/16
        'west': (0, 0.875, 1.0),
    }
    for facing, (axis, low, high) in floors.items():
        floor = stage23.turned_boxes(stage23.BASKET_UP, facing)[0]
        slab = (floor[axis - 3], floor[axis]) if axis > 2 else (floor[axis], floor[axis + 3])
        check(abs(slab[0] - low) < 1e-9 and abs(slab[1] - high) < 1e-9,
              f'{facing}: floor spans {slab}, want {(low, high)}')


def state_keys(blockstate):
    """The property names a blockstate file mentions (variants keys and multipart `when`)."""
    names = set()
    for key in blockstate.get('variants', {}):
        for part in filter(None, key.split(',')):
            names.add(part.split('=')[0])
    for part in blockstate.get('multipart', []):
        when = part.get('when', {})
        for condition in ([when] if 'OR' not in when else when['OR']):
            names.update(condition.keys())
    return names


def models_of(blockstate):
    found = []
    for value in blockstate.get('variants', {}).values():
        for entry in (value if isinstance(value, list) else [value]):
            found.append(entry['model'])
    for part in blockstate.get('multipart', []):
        apply = part['apply']
        for entry in (apply if isinstance(apply, list) else [apply]):
            found.append(entry['model'])
    return found


def check_overlay(overlay):
    assets = overlay / f'assets/{NS}'
    data = overlay / f'data/{NS}'
    lang = json.loads((assets / 'lang/en_us.json').read_text(encoding='utf-8'))
    for name, block in stage23.blocks().items():
        where = f'{name}'
        template = block['template']
        check(template in TEMPLATE_PROPERTIES, f'{where}: no known property list for {template}')
        own = set(block.get('state_properties', {}))
        check(not own & TEMPLATE_PROPERTIES.get(template, set()),
              f'{where}: redeclares a template property {own & TEMPLATE_PROPERTIES[template]}')
        path = assets / f'blockstates/{name}.json'
        check(path.exists(), f'{where}: no blockstate')
        if path.exists():
            state = json.loads(path.read_text(encoding='utf-8'))
            unknown = state_keys(state) - own - TEMPLATE_PROPERTIES.get(template, set())
            check(not unknown, f'{where}: blockstate names unknown properties {sorted(unknown)}')
            for model in models_of(state):
                model_path = assets / 'models' / (model.split(':', 1)[-1] + '.json')
                check(model_path.exists(), f'{where}: missing model {model}')
        check((data / f'loot_table/blocks/{name}.json').exists(), f'{where}: no loot table')
        check(f'block.{NS}.{name}' in lang, f'{where}: no language line')
        shapes = block.get('shapes')
        if shapes and shapes.get('property') == 'facing':
            check(set(shapes['values']) == set(stage23.FACINGS), f'{where}: shapes miss a facing')
        for key in ('hardness', 'explosion_resistance'):
            value = block['properties'].get(key)
            check(value is None or value >= 0, f'{where}: {key} {value}')
        sound = block['properties'].get('sounds')
        check(sound is None or re.fullmatch(r'minecraft:[a-z_]+', sound), f'{where}: sounds {sound}')
    for wood in stage23.WOODS:
        state = json.loads((assets / f'blockstates/{wood}_cabinet.json').read_text(encoding='utf-8'))
        for facing in stage23.FACINGS:
            for opened in ('false', 'true'):
                key = f'facing={facing},open={opened}'
                check(key in state['variants'], f'{wood}_cabinet: no variant {key}')
    rope_tag = json.loads((data / 'tags/block/ropes.json').read_text(encoding='utf-8'))
    check(f'{NS}:rope' in rope_tag['values'], 'the ropes tag lacks the rope')
    for tag, wanted in (('climbable', f'{NS}:rope'), ('mushroom_grow_block', f'{NS}:rich_soil')):
        values = json.loads((overlay / f'data/minecraft/tags/block/{tag}.json').read_text(encoding='utf-8'))
        check(wanted in values['values'], f'minecraft:{tag} lacks {wanted}')
    axe = json.loads((overlay / 'data/minecraft/tags/block/mineable/axe.json').read_text(encoding='utf-8'))
    for wood in stage23.WOODS:
        check(f'{NS}:{wood}_cabinet' in axe['values'], f'mineable/axe lacks the {wood} cabinet')
    check(f'{NS}:basket' in axe['values'], 'mineable/axe lacks the basket')


def main(argv):
    selftest()
    if argv and argv[0] != '--selftest':
        overlay = Path(argv[0]).resolve()
        with tempfile.TemporaryDirectory() as temp:
            copy = Path(temp) / 'overlay'
            shutil.copytree(overlay, copy)
            stage23.patch(copy)
            check_overlay(copy)
    for problem in problems:
        print('PROBLEM:', problem)
    print(f'fd stage23 content check: {"FAILED" if problems else "ok"} ({len(problems)} problems)')
    return 1 if problems else 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
