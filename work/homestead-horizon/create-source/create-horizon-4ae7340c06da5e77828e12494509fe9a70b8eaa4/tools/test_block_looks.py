"""Unit tests for block_looks.py (python -m unittest test_block_looks, run from tools/)."""
import json
import tempfile
import unittest
import zipfile
from pathlib import Path

import block_looks as bl

FULL = {'parent': 'minecraft:block/cube_all', 'textures': {'all': 't:block/x'}}
PLUSH = {
    'render_type': 'cutout',
    'elements': [
        {'from': [5.6, 0, 3.1], 'to': [11.6, 4.8, 13.9], 'rotation': {'angle': 0, 'axis': 'y', 'origin': [8, 0, 6]}},
        {'from': [7.4, 2.8, 13.2], 'to': [9.8, 4.2, 20.5], 'rotation': {'angle': 22.5, 'axis': 'x', 'origin': [8, 3, 15]}},
    ],
}
SLABBY = {'elements': [{'from': [0, 0, 0], 'to': [16, 8, 16]}]}
TWO_HALVES = {'elements': [{'from': [0, 0, 0], 'to': [16, 8, 16]}, {'from': [0, 8, 0], 'to': [16, 16, 16]}]}
CARPET = {'elements': [{'from': [0, 0, 0], 'to': [16, 1, 16]}]}
GLASS = {'parent': 'minecraft:block/cube_all', 'render_type': 'cutout'}
CROSS = {'parent': 'minecraft:block/cross', 'textures': {'cross': 't:block/x'}}
CHILD = {'parent': 't:block/base'}
PILLAR = {'elements': [{'from': [4, 0, 4], 'to': [12, 16, 12]}]}
BLOCKSTATE_ONE = {'variants': {'': {'model': 't:block/{name}'}}}
BLOCKSTATE_FACING = {'variants': {
    'facing=north': {'model': 't:block/{name}'}, 'facing=east': {'model': 't:block/{name}', 'y': 90},
    'facing=south': {'model': 't:block/{name}', 'y': 180}, 'facing=west': {'model': 't:block/{name}', 'y': 270}}}


def put(path, doc):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(doc), encoding='utf-8')


def add_block(root, name, template, model, blockstate=BLOCKSTATE_ONE, ns='t'):
    put(root / f'data/{ns}/horizon/block/{name}.json', {'template': template})
    put(root / f'assets/{ns}/models/block/{name}.json', model)
    state = json.loads(json.dumps(blockstate).replace('{name}', name))
    put(root / f'assets/{ns}/blockstates/{name}.json', state)


def declaration(root, name, ns='t'):
    return json.loads((root / f'data/{ns}/horizon/block/{name}.json').read_text(encoding='utf-8'))


class Classify(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def look(self, name):
        return bl.analyze_models(bl.DirReader(self.root), 't', name)

    def test_cube_parent_is_full(self):
        add_block(self.root, 'a', 'minecraft:stone', FULL)
        look = self.look('a')
        self.assertEqual(look.status, 'ok')
        self.assertTrue(look.full)
        self.assertFalse(look.transparent)

    def test_two_halves_cover_the_cube(self):
        add_block(self.root, 'a', 'minecraft:stone', TWO_HALVES)
        self.assertTrue(self.look('a').full)

    def test_half_slab_is_not_full(self):
        add_block(self.root, 'a', 'minecraft:stone', SLABBY)
        look = self.look('a')
        self.assertFalse(look.full)
        self.assertEqual(look.bbox, [0.0, 0.0, 0.0, 1.0, 0.5, 1.0])

    def test_glass_cube_is_transparent(self):
        add_block(self.root, 'a', 'minecraft:stone', GLASS)
        look = self.look('a')
        self.assertTrue(look.full)
        self.assertTrue(look.transparent)

    def test_parent_chain_inside_the_mod(self):
        put(self.root / 'assets/t/models/block/base.json', CARPET)
        add_block(self.root, 'a', 'minecraft:stone', CHILD)
        self.assertEqual(self.look('a').bbox, [0.0, 0.0, 0.0, 1.0, 0.0625, 1.0])

    def test_unknown_model_is_left_alone(self):
        put(self.root / 'data/t/horizon/block/a.json', {'template': 'minecraft:stone'})
        put(self.root / 'assets/t/blockstates/a.json', {'variants': {'': {'model': 'other:block/y'}}})
        self.assertEqual(self.look('a').status, 'unknown')

    def test_model_without_geometry_is_a_geo_block(self):
        add_block(self.root, 'a', 'minecraft:stone', {'textures': {'0': 't:block/x'}})
        look = self.look('a')
        self.assertEqual(look.status, 'ok')
        self.assertTrue(look.geo)
        self.assertFalse(look.full)
        self.assertIsNone(look.bbox)

    def test_facing_union_is_symmetric(self):
        # an off-centre box turned four ways
        add_block(self.root, 'a', 'minecraft:carved_pumpkin',
                  {'elements': [{'from': [0, 0, 8], 'to': [4, 4, 16]}]}, BLOCKSTATE_FACING)
        look = self.look('a')
        self.assertEqual(look.bbox[0], 0.0)
        self.assertEqual(look.bbox[3], 1.0)
        self.assertEqual(look.bbox[2], 0.0)
        self.assertEqual(look.bbox[5], 1.0)


class Decide(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def verdict(self, name, template):
        return bl.decide(template, bl.analyze_models(bl.DirReader(self.root), 't', name))

    def test_full_cube_untouched(self):
        add_block(self.root, 'a', 'minecraft:stone', FULL)
        v = self.verdict('a', 'minecraft:stone')
        self.assertFalse(v.occludes_false)
        self.assertIsNone(v.collision)
        self.assertEqual(v.category, 'ok')

    def test_plush_does_not_occlude_and_gets_a_small_collision(self):
        add_block(self.root, 'plush', 'minecraft:carved_pumpkin', PLUSH, BLOCKSTATE_FACING)
        v = self.verdict('plush', 'minecraft:carved_pumpkin')
        self.assertTrue(v.occludes_false)
        self.assertEqual(v.category, 'oversized')
        self.assertEqual(len(v.collision), 1)
        box = v.collision[0]
        self.assertTrue(all(0.0 <= x <= 1.0 for x in box))
        self.assertLess(box[4], 1.0)

    def test_carpet_on_stone_gets_a_flat_collision(self):
        add_block(self.root, 'c', 'minecraft:stone', CARPET)
        v = self.verdict('c', 'minecraft:stone')
        self.assertTrue(v.occludes_false)
        self.assertEqual(v.collision, [[0.0, 0.0, 0.0, 1.0, 0.0625, 1.0]])

    def test_glass_cube_keeps_collision_but_does_not_occlude(self):
        add_block(self.root, 'g', 'minecraft:stone', GLASS)
        v = self.verdict('g', 'minecraft:stone')
        self.assertTrue(v.occludes_false)
        self.assertIsNone(v.collision)

    def test_plant_has_no_collision(self):
        add_block(self.root, 'p', 'minecraft:stone', CROSS)
        v = self.verdict('p', 'minecraft:stone')
        self.assertTrue(v.occludes_false)
        self.assertEqual(v.collision, [])
        self.assertEqual(v.category, 'plant_empty')

    def test_pressure_plate_that_looks_solid_gets_a_collision(self):
        add_block(self.root, 'solid', 'minecraft:oak_pressure_plate', FULL)
        v = self.verdict('solid', 'minecraft:oak_pressure_plate')
        self.assertEqual(v.collision, [[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]])
        self.assertEqual(v.category, 'empty_solid')

    def test_tall_pillar_on_a_button_gets_its_box(self):
        add_block(self.root, 'pillar', 'minecraft:oak_button', {'elements': [{'from': [2, 0, 2], 'to': [14, 16, 14]}]})
        v = self.verdict('pillar', 'minecraft:oak_button')
        self.assertEqual(v.collision, [[0.125, 0.0, 0.125, 0.875, 1.0, 0.875]])

    def test_thin_pressure_plate_is_left_alone(self):
        add_block(self.root, 'plate', 'minecraft:oak_pressure_plate', CARPET)
        v = self.verdict('plate', 'minecraft:oak_pressure_plate')
        self.assertIsNone(v.collision)
        self.assertEqual(v.category, 'ok')

    def test_state_templates_are_only_counted(self):
        add_block(self.root, 's', 'minecraft:oak_slab', CARPET)
        v = self.verdict('s', 'minecraft:oak_slab')
        self.assertIsNone(v.collision)
        self.assertFalse(v.occludes_false)
        self.assertEqual(v.category, 'state_template')

    def test_geo_block_does_not_occlude_and_keeps_the_template_collision(self):
        add_block(self.root, 'geo', 'minecraft:stone', {'textures': {'0': 't:block/x'}})
        v = self.verdict('geo', 'minecraft:stone')
        self.assertTrue(v.occludes_false)
        self.assertIsNone(v.collision)
        self.assertEqual(v.category, 'geo')

    def test_unresolvable_model_is_unknown(self):
        put(self.root / 'data/t/horizon/block/u.json', {'template': 'minecraft:stone'})
        v = self.verdict('u', 'minecraft:stone')
        self.assertEqual(v.category, 'unknown')
        self.assertFalse(v.occludes_false)


class Overlay(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name) / 'mod'
        self.out = Path(self.tmp.name) / 'out'
        add_block(self.root, 'plush', 'minecraft:carved_pumpkin', PLUSH, BLOCKSTATE_FACING)
        add_block(self.root, 'cube', 'minecraft:stone', FULL)
        add_block(self.root, 'flower', 'minecraft:stone', CROSS)
        add_block(self.root, 'door', 'minecraft:oak_door', CARPET)

    def tearDown(self):
        self.tmp.cleanup()

    def test_only_wrong_blocks_are_written(self):
        counts = bl.process(self.root, self.out)['t']
        written = sorted(p.stem for p in (self.out / 'data/t/horizon/block').glob('*.json'))
        self.assertEqual(written, ['flower', 'plush'])
        self.assertEqual(counts['blocks'], 4)
        self.assertEqual(counts['occludes_false'], 2)
        self.assertEqual(counts['oversized'], 1)
        self.assertEqual(counts['plant_empty'], 1)
        self.assertEqual(counts['state_template'], 1)
        plush = json.loads((self.out / 'data/t/horizon/block/plush.json').read_text(encoding='utf-8'))
        self.assertEqual(plush['template'], 'minecraft:carved_pumpkin')
        self.assertIs(plush['properties']['occludes'], False)
        self.assertIn('default', plush['collision_shapes'])
        flower = json.loads((self.out / 'data/t/horizon/block/flower.json').read_text(encoding='utf-8'))
        self.assertEqual(flower['collision_shapes'], {'default': []})
        # the source is untouched
        self.assertEqual(declaration(self.root, 'plush'), {'template': 'minecraft:carved_pumpkin'})

    def test_no_collision_flag(self):
        bl.process(self.root, self.out, collision=False)
        plush = json.loads((self.out / 'data/t/horizon/block/plush.json').read_text(encoding='utf-8'))
        self.assertNotIn('collision_shapes', plush)
        self.assertIs(plush['properties']['occludes'], False)

    def test_shapes_flag_also_sets_the_outline(self):
        bl.process(self.root, self.out, shapes=True)
        plush = json.loads((self.out / 'data/t/horizon/block/plush.json').read_text(encoding='utf-8'))
        self.assertEqual(plush['shapes'], plush['collision_shapes'])

    def test_existing_properties_and_collision_are_kept(self):
        put(self.root / 'data/t/horizon/block/plush.json', {
            'template': 'minecraft:carved_pumpkin', 'properties': {'light': 3},
            'collision_shapes': {'default': [[0, 0, 0, 1, 0.25, 1]]}})
        bl.process(self.root, self.out)
        plush = json.loads((self.out / 'data/t/horizon/block/plush.json').read_text(encoding='utf-8'))
        self.assertEqual(plush['properties'], {'light': 3, 'occludes': False})
        self.assertEqual(plush['collision_shapes'], {'default': [[0, 0, 0, 1, 0.25, 1]]})

    def test_dry_run_writes_nothing(self):
        counts = bl.process(self.root, self.out, dry_run=True)['t']
        self.assertEqual(counts['written'], 2)
        self.assertFalse(self.out.exists())

    def test_idempotent_on_the_overlaid_result(self):
        bl.process(self.root, self.out)
        # apply the overlay on top of the mod, then run again: nothing left to write
        for path in self.out.rglob('*.json'):
            rel = path.relative_to(self.out)
            (self.root / rel).write_text(path.read_text(encoding='utf-8'), encoding='utf-8')
        again = bl.process(self.root, self.out / 'second')['t']
        self.assertEqual(again['written'], 0)


class Check(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name) / 'mod'
        add_block(self.root, 'plush', 'minecraft:carved_pumpkin', PLUSH, BLOCKSTATE_FACING)
        add_block(self.root, 'cube', 'minecraft:stone', FULL)

    def tearDown(self):
        self.tmp.cleanup()

    def test_unfixed_mod_fails(self):
        violations, checked = bl.check(self.root)
        kinds = sorted(v[2] for v in violations)
        self.assertEqual(kinds, ['occludes', 'oversized'])
        self.assertEqual(checked['t'], 2)

    def test_fixed_mod_passes(self):
        fixed = Path(self.tmp.name) / 'fixed'
        bl.process(self.root, fixed)
        for path in fixed.rglob('*.json'):
            (self.root / path.relative_to(fixed)).write_text(path.read_text(encoding='utf-8'), encoding='utf-8')
        violations, _ = bl.check(self.root)
        self.assertEqual(violations, [])

    def test_empty_collision_on_a_solid_model_fails(self):
        put(self.root / 'data/t/horizon/block/cube.json',
            {'template': 'minecraft:stone', 'collision_shapes': {'default': []}})
        violations, _ = bl.check(self.root)
        self.assertIn('solid_empty', [v[2] for v in violations])

    def test_archive_is_read_like_a_directory(self):
        archive = Path(self.tmp.name) / 'mod.hmod'
        with zipfile.ZipFile(archive, 'w') as z:
            for path in self.root.rglob('*.json'):
                z.write(path, str(path.relative_to(self.root)).replace('\\', '/'))
        violations, checked = bl.check(archive)
        self.assertEqual(checked['t'], 2)
        self.assertEqual(len(violations), 2)
        self.assertEqual(bl.main(['--check', str(archive)]), 1)


class RealData(unittest.TestCase):
    """Runs over the real carried mods when they are on disk (skipped otherwise)."""

    @classmethod
    def setUpClass(cls):
        cls.mods = Path(__file__).resolve().parents[3] / 'pack' / 'mods'

    def test_plushies_all_become_non_occluding(self):
        carry = self.mods / 'perfectplushies' / 'carry'
        if not carry.is_dir():
            self.skipTest('perfectplushies carry not found')
        counts = bl.process(carry, None, dry_run=True)['perfectplushies']
        self.assertGreaterEqual(counts['blocks'], 40)
        self.assertEqual(counts['occludes_false'], counts['blocks'] - counts['unknown'] - counts['other_template'] - counts['state_template'])
        self.assertGreaterEqual(counts['occludes_false'], 40)

    def test_totals_over_all_mods(self):
        if not self.mods.is_dir():
            self.skipTest('pack/mods not found')
        total = bl.new_counts()
        for carry in sorted(self.mods.glob('*/carry')):
            for counts in bl.process(carry, None, dry_run=True).values():
                for key, value in counts.items():
                    total[key] += value
        self.assertGreater(total['blocks'], 9000)
        self.assertGreater(total['occludes_false'], 1500)


if __name__ == '__main__':
    unittest.main()
