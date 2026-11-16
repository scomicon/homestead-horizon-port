"""Unit tests for hide_unplaceable.py (python -m unittest test_hide_unplaceable, run from tools/)."""
import json
import tempfile
import unittest
from pathlib import Path

import hide_unplaceable as hu

POLICY = {'ru': {'patterns': ['*_boat', '*_chest_boat'], 'drop_items': False}}


def put(path, doc):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(doc), encoding='utf-8')


def make_mod(root):
    for name in ('baobab_boat', 'baobab_chest_boat', 'sign'):
        put(root / f'data/ru/horizon/item/{name}.json', {'template': 'minecraft:stick'})
        put(root / f'assets/ru/items/{name}.json', {})
    put(root / 'data/ru/recipe/baobab_boat.json', {
        'type': 'minecraft:crafting_shaped', 'result': {'item': 'ru:baobab_boat'}})
    put(root / 'data/ru/recipe/baobab_chest_boat.json', {
        'type': 'minecraft:crafting_shapeless', 'ingredients': [{'item': 'minecraft:chest'}, {'item': 'ru:baobab_boat'}],
        'result': {'item': 'ru:baobab_chest_boat'}})
    put(root / 'data/ru/recipe/sign.json', {'result': {'item': 'ru:sign'}})
    put(root / 'data/ru/advancement/recipes/transportation/baobab_boat.json', {
        'rewards': {'recipes': ['ru:baobab_boat']}, 'criteria': {}})
    put(root / 'data/ru/advancement/recipes/transportation/baobab_chest_boat.json', {
        'rewards': {'recipes': ['ru:baobab_chest_boat']}, 'criteria': {}})
    put(root / 'data/ru/advancement/recipes/misc/sign.json', {'rewards': {'recipes': ['ru:sign']}, 'criteria': {}})
    put(root / 'data/ru/horizon/creative_tab/ru.json', {
        'title': 'itemGroup.ru', 'icon': 'ru:sign',
        'items': ['ru:baobab_boat', 'ru:baobab_chest_boat', 'ru:sign']})


class Hide(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name) / 'mod'
        self.out = Path(self.tmp.name) / 'tabs'
        make_mod(self.root)
        # the creative tab as creative_tab.py would have written it into --out
        put(self.out / 'data/ru/horizon/creative_tab/ru.json', {
            'title': 'itemGroup.ru', 'icon': 'ru:sign',
            'items': ['ru:baobab_boat', 'ru:baobab_chest_boat', 'ru:sign']})

    def tearDown(self):
        self.tmp.cleanup()

    def test_drops_recipes_and_their_advancements_only(self):
        drops, summary = hu.process(self.root, POLICY, self.out)
        self.assertEqual(drops, [
            'data/ru/advancement/recipes/transportation/baobab_boat.json',
            'data/ru/advancement/recipes/transportation/baobab_chest_boat.json',
            'data/ru/recipe/baobab_boat.json',
            'data/ru/recipe/baobab_chest_boat.json'])
        self.assertEqual(summary['items'], 2)
        self.assertEqual(summary['recipes'], 2)
        self.assertEqual(summary['recipe_advancements'], 2)

    def test_items_stay_registered_by_default(self):
        drops, _ = hu.process(self.root, POLICY, self.out)
        self.assertFalse([d for d in drops if '/horizon/item/' in d or d.startswith('assets/')])

    def test_creative_tab_loses_the_hidden_items(self):
        hu.process(self.root, POLICY, self.out)
        tab = json.loads((self.out / 'data/ru/horizon/creative_tab/ru.json').read_text(encoding='utf-8'))
        self.assertEqual(tab['items'], ['ru:sign'])

    def test_drop_items_policy_also_drops_the_declarations(self):
        policy = {'ru': {'patterns': ['*_boat', '*_chest_boat'], 'drop_items': True}}
        drops, summary = hu.process(self.root, policy, self.out)
        self.assertIn('data/ru/horizon/item/baobab_boat.json', drops)
        self.assertIn('assets/ru/items/baobab_chest_boat.json', drops)
        self.assertNotIn('data/ru/horizon/item/sign.json', drops)

    def test_other_namespaces_are_untouched(self):
        drops, summary = hu.process(self.root, {'other': {'patterns': ['*_boat']}}, self.out)
        self.assertEqual(drops, [])
        self.assertEqual(summary['items'], 0)

    def test_source_files_are_never_modified(self):
        before = (self.root / 'data/ru/recipe/baobab_boat.json').read_text(encoding='utf-8')
        hu.process(self.root, POLICY, self.out)
        self.assertEqual((self.root / 'data/ru/recipe/baobab_boat.json').read_text(encoding='utf-8'), before)

    def test_cli_prints_drop_lines(self):
        import io
        import contextlib
        policy_file = Path(self.tmp.name) / 'policy.json'
        policy_file.write_text(json.dumps(POLICY), encoding='utf-8')
        buffer = io.StringIO()
        with contextlib.redirect_stdout(buffer):
            code = hu.main(['--policy', str(policy_file), '--out', str(self.out), str(self.root)])
        self.assertEqual(code, 0)
        lines = [l for l in buffer.getvalue().splitlines() if l.startswith('DROP ')]
        self.assertEqual(len(lines), 4)

class Check(unittest.TestCase):
    def test_check_reports_leftovers_and_accepts_a_clean_archive(self):
        import zipfile
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / 'mod'
            make_mod(root)
            self.assertEqual(len(hu.check_archive(root, POLICY)), 3)  # 2 recipes + the creative tab
            drops, _ = hu.process(root, POLICY, None)
            archive = Path(tmp) / 'clean.hmod'
            with zipfile.ZipFile(archive, 'w') as z:
                for path in root.rglob('*.json'):
                    name = str(path.relative_to(root)).replace('\\', '/')
                    if name in drops:
                        continue
                    if name.startswith('data/ru/horizon/creative_tab/'):
                        doc = json.loads(path.read_text(encoding='utf-8'))
                        doc['items'] = [e for e in doc['items'] if not e.endswith('_boat') and not e.endswith('_chest_boat')]
                        z.writestr(name, json.dumps(doc))
                    else:
                        z.write(path, name)
            self.assertEqual(hu.check_archive(archive, POLICY), [])


class RealData(unittest.TestCase):
    def test_regions_unexplored_boats(self):
        carry = Path(__file__).resolve().parents[3] / 'pack' / 'mods' / 'regions_unexplored' / 'carry'
        if not carry.is_dir():
            self.skipTest('regions_unexplored carry not found')
        drops, summary = hu.process(carry, hu.load_policy(hu.DEFAULT_POLICY), None)
        self.assertEqual(summary['items'], 32)
        # 33 recipe files have oat in their name, but oak_boat crafts the vanilla minecraft:oak_boat from
        # Regions Unexplored planks: that one works and stays
        self.assertEqual(summary['recipes'], 32)
        self.assertEqual(summary['recipe_advancements'], 32)
        self.assertEqual(len(drops), 64)
        self.assertNotIn('data/regions_unexplored/recipe/oak_boat.json', drops)

    def test_other_mods_have_nothing_to_hide(self):
        mods = Path(__file__).resolve().parents[3] / 'pack' / 'mods'
        if not mods.is_dir():
            self.skipTest('pack/mods not found')
        policy = hu.load_policy(hu.DEFAULT_POLICY)
        for carry in sorted(mods.glob('*/carry')):
            if carry.parent.name == 'regions_unexplored':
                continue
            drops, _ = hu.process(carry, policy, None)
            self.assertEqual(drops, [], carry.parent.name)


if __name__ == '__main__':
    unittest.main()
