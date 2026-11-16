"""Checks for creative_tab.py on synthetic mod directories. Run: python -I creative_tab_checks.py

No game, no archives, no network; everything lives in a temporary directory.
"""
import importlib.util
import json
import sys
import tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location('creative_tab', Path(__file__).with_name('creative_tab.py'))
ct = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ct)


def put(root, rel, doc=None):
    path = root / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(doc if doc is not None else {}), encoding='utf-8')


def declare(root, ns, name, kind='item', model=None, item_json=True, geometry=True):
    put(root, f'data/{ns}/horizon/{kind}/{name}.json')
    if item_json:
        put(root, f'assets/{ns}/items/{name}.json', {'model': {'type': 'minecraft:model', 'model': f'{ns}:item/{name}'}})
    if model is not False:
        put(root, f'assets/{ns}/models/item/{name}.json',
            model or ({'parent': 'minecraft:item/generated', 'textures': {'layer0': f'{ns}:item/{name}'}}
                      if geometry else {'parent': 'minecraft:builtin/entity'}))


def run(root, **kwargs):
    log = {}
    out = root.parent / (root.name + '-out')
    count = ct.add_tab(root, out, hidden_log=log, **kwargs)
    tab = out / f'data/t/horizon/creative_tab/t.json'
    return count, (json.loads(tab.read_text(encoding='utf-8')) if tab.is_file() else None), log, out


def main():
    checks = 0
    with tempfile.TemporaryDirectory(prefix='creative-tab-checks-') as temp:
        base = Path(temp)
        vanilla = base / 'vanilla'
        put(vanilla, 'assets/minecraft/models/item/generated.json', {'parent': 'builtin/generated'})
        put(vanilla, 'assets/minecraft/models/block/cube_all.json',
            {'elements': [{'faces': {'north': {'texture': '#all'}}}]})

        root = base / 'mod'
        declare(root, 't', 't')                                             # namespace-named icon candidate
        declare(root, 't', 'good')
        declare(root, 't', 'oak_sign')
        declare(root, 't', 'oak_wall_sign', item_json=False)                # wall twin: hidden by pair
        declare(root, 't', 'oak_hanging_sign')
        declare(root, 't', 'oak_wall_hanging_sign')                         # wall twin
        declare(root, 't', 'lone_wall_deco_1')                              # no pair: stays
        declare(root, 't', 'wall_lamp')                                     # pair `lamp` undeclared: stays
        declare(root, 't', 'plushie', geometry=False)                       # bare builtin/entity
        declare(root, 't', 'plant', kind='block', item_json=False)          # crop block without an item
        declare(root, 't', 'missing_parent', model={'parent': 't:block/nowhere'})
        declare(root, 't', 'external', model={'parent': 'other:item/model'})  # other mod absent: blank
        declare(root, 't', 'looped', model={'parent': 't:item/looped'})
        declare(root, 't', 'cube', model={'parent': 'minecraft:block/cube_all', 'textures': {'all': 't:block/x'}})
        declare(root, 't', 'loader', model={'loader': 'x:custom'})
        declare(root, 't', 'no_texture', model={'parent': 'minecraft:item/generated',
                                               'textures': {'layer0': 't:item/absent'}})  # texture not checked
        declare(root, 't', 'empty_node', item_json=False, model=False)
        put(root, 'assets/t/items/empty_node.json', {'model': {'type': 'minecraft:empty'}})
        declare(root, 't', 'special', item_json=False, model=False)
        put(root, 'assets/t/items/special.json', {'model': {'type': 'minecraft:special', 'base': 't:item/x'}})
        declare(root, 't', 'cond', item_json=False, model=False)
        put(root, 'assets/t/items/cond.json', {'model': {
            'type': 'minecraft:condition', 'on_true': {'type': 'minecraft:model', 'model': 't:item/t'},
            'on_false': {'type': 'minecraft:model', 'model': 't:item/plushie'}}})
        declare(root, 't', 'bad_json', item_json=False, model=False)
        (root / 'assets/t/items/bad_json.json').write_text('{', encoding='utf-8')

        count, tab, log, out = run(root, resources=[vanilla])
        shown = {e.split(':')[1] for e in tab['items']}
        assert count == 1
        assert shown == {'t', 'good', 'oak_sign', 'oak_hanging_sign', 'lone_wall_deco_1', 'wall_lamp', 'cube',
                         'loader', 'no_texture', 'special', 'cond'}, shown
        checks += 1
        assert log['t:oak_wall_sign'] == 'wall_pair' and log['t:oak_wall_hanging_sign'] == 'wall_pair'
        assert log['t:plant'] == 'no_item'
        assert {log[f't:{n}']: n for n in ('plushie', 'missing_parent', 'external', 'looped', 'empty_node')}.keys() \
            == {'no_geometry'}
        assert log['t:bad_json'] == 'invalid_item'
        checks += 1
        assert tab['icon'] == 't:t'                                          # entry named like the namespace
        assert tab['title'] == 'itemGroup.t'
        lang = json.loads((out / 'assets/t/lang/en_us.json').read_text(encoding='utf-8'))
        assert lang['itemGroup.t'] == 'T'
        checks += 1

        # Icon is re-picked from survivors when the namespace-named entry is blank.
        root2 = base / 'mod2'
        declare(root2, 't', 't', geometry=False)
        declare(root2, 't', 'b_item')
        declare(root2, 't', 'a_item')
        _, tab2, _, _ = run(root2, resources=[vanilla])
        assert tab2['icon'] == 't:a_item' and 't:t' not in tab2['items'], tab2
        checks += 1

        # No survivor: no tab, no title written.
        root3 = base / 'mod3'
        declare(root3, 't', 'only_blank', geometry=False)
        count3, tab3, log3, out3 = run(root3, resources=[vanilla])
        assert count3 == 0 and tab3 is None and 't:only_blank' in log3
        assert not (out3 / 'assets/t/lang/en_us.json').exists()
        checks += 1

        # Unprovable vanilla parent without a resource root is kept; the same model with a root and a
        # missing file is hidden.
        root4 = base / 'mod4'
        declare(root4, 't', 'vanilla_child', model={'parent': 'minecraft:block/cube_all', 'textures': {'all': 't:x'}})
        declare(root4, 't', 'vanilla_gone', model={'parent': 'minecraft:block/not_a_model'})
        _, tab4, log4, _ = run(root4)
        assert {e.split(':')[1] for e in tab4['items']} == {'vanilla_child', 'vanilla_gone'}
        _, tab5, log5, _ = run(root4, resources=[vanilla])
        assert [e.split(':')[1] for e in tab5['items']] == ['vanilla_child'] and 't:vanilla_gone' in log5
        checks += 1

        # Another mod directory passed as --assets satisfies a cross-mod parent.
        other = base / 'other'
        put(other, 'assets/other/models/item/model.json', {'elements': [{'faces': {'up': {'texture': '#t'}}}]})
        _, tab6, _, _ = run(root, resources=[vanilla], extras=[other])
        assert 't:external' in tab6['items']
        checks += 1

        # Title comes from horizon-mod.toml only when the id is the namespace.
        root7 = base / 'mod7'
        declare(root7, 't', 'item')
        (root7 / 'horizon-mod.toml').write_text('id = "t"\nname = "Test Mod"\n', encoding='utf-8')
        _, _, _, out7 = run(root7, resources=[vanilla])
        assert json.loads((out7 / 'assets/t/lang/en_us.json').read_text(encoding='utf-8'))['itemGroup.t'] == 'Test Mod'
        (root7 / 'horizon-mod.toml').write_text('id = "other"\nname = "Test Mod"\n', encoding='utf-8')
        _, _, _, out7 = run(root7, resources=[vanilla])
        assert json.loads((out7 / 'assets/t/lang/en_us.json').read_text(encoding='utf-8'))['itemGroup.t'] == 'T'
        checks += 1

        # An existing title is kept.
        root8 = base / 'mod8'
        declare(root8, 't', 'item')
        put(root8, 'assets/t/lang/en_us.json', {'itemGroup.t': 'Mine', 'x': 'y'})
        _, _, _, out8 = run(root8, resources=[vanilla])
        assert not (out8 / 'assets/t/lang/en_us.json').exists()              # nothing to merge, source kept
        assert json.loads((root8 / 'assets/t/lang/en_us.json').read_text(encoding='utf-8')) == {
            'itemGroup.t': 'Mine', 'x': 'y'}
        checks += 1

    # hide_unplaceable keeps the icon on a surviving entry.
    spec2 = importlib.util.spec_from_file_location('hide_unplaceable', Path(__file__).with_name('hide_unplaceable.py'))
    hide = importlib.util.module_from_spec(spec2)
    spec2.loader.exec_module(hide)
    with tempfile.TemporaryDirectory(prefix='creative-tab-checks-') as temp:
        root = Path(temp) / 'mod'
        declare(root, 't', 'oak_boat')
        declare(root, 't', 'stone')
        put(root, 'data/t/horizon/creative_tab/t.json',
            {'title': 'itemGroup.t', 'icon': 't:oak_boat', 'items': ['t:oak_boat', 't:stone']})
        hide.process(root, {'t': {'patterns': ['*_boat']}}, out=root)
        doc = json.loads((root / 'data/t/horizon/creative_tab/t.json').read_text(encoding='utf-8'))
        assert doc['items'] == ['t:stone'] and doc['icon'] == 't:stone', doc
        checks += 1
    print(f'creative_tab_checks: {checks} groups passed')


if __name__ == '__main__':
    sys.exit(main())
