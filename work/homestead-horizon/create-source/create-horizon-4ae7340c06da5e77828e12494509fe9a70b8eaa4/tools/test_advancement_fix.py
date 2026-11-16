"""Unit tests for advancement_fix.py (python -I -m unittest test_advancement_fix, run from tools/).

The samples are cut down copies of real carried files: Repurposed Structures `ancient_cities`
(`structure`), Regions Unexplored `pioneer` (`biome`) and Incendium `on_sword_get` (`feature`).
"""
import copy
import json
import tempfile
import unittest
from pathlib import Path

import advancement_fix as af


def location_criterion(location):
    return {
        'trigger': 'minecraft:location',
        'conditions': {'player': [{
            'condition': 'minecraft:entity_properties', 'entity': 'this', 'predicate': {'location': location}}]},
    }


ANCIENT_CITIES = {
    'parent': 'repurposed_structures:root',
    'rewards': {'experience': 1000},
    'criteria': {
        'in_ancient_city_ocean': location_criterion({'structure': 'repurposed_structures:ancient_city_ocean'}),
        'in_ancient_city_nether': location_criterion({'structure': 'repurposed_structures:ancient_city_nether'}),
    },
    'requirements': [['in_ancient_city_ocean'], ['in_ancient_city_nether']],
}
PIONEER = {
    'parent': 'regions_unexplored:parent',
    'criteria': {'alpha_grove': location_criterion({'biome': 'regions_unexplored:alpha_grove'})},
}
ON_SWORD_GET = {
    'criteria': {'x': {'trigger': 'minecraft:inventory_changed', 'conditions': {
        'player': [{'condition': 'minecraft:entity_properties', 'entity': 'this',
                    'predicate': {'location': {'feature': 'incendium:forbidden_castle', 'dimension': 'minecraft:the_nether'}}}]}}},
}
INCENDIUM_PLAYER = {
    'criteria': {'x': location_criterion({'biome': 'incendium:ash_barrens',
                                          'player': {'location': {'biome': 'incendium:ash_barrens'}}})},
}


def conditions(doc, name):
    return doc['criteria'][name]['conditions']['player'][0]['predicate']['location']


class FixDocument(unittest.TestCase):
    def test_structure_becomes_structures(self):
        doc = copy.deepcopy(ANCIENT_CITIES)
        changed, stats = af.fix_document(doc, 'ancient_cities')
        self.assertTrue(changed)
        self.assertEqual(stats.renames['structure'], 2)
        loc = conditions(doc, 'in_ancient_city_ocean')
        self.assertEqual(loc, {'structures': 'repurposed_structures:ancient_city_ocean'})
        self.assertNotIn('structure', loc)
        self.assertEqual(stats.neutralised, 0)

    def test_biome_becomes_biomes(self):
        doc = copy.deepcopy(PIONEER)
        changed, stats = af.fix_document(doc, 'pioneer')
        self.assertTrue(changed)
        self.assertEqual(conditions(doc, 'alpha_grove'), {'biomes': 'regions_unexplored:alpha_grove'})

    def test_feature_and_dimension(self):
        doc = copy.deepcopy(ON_SWORD_GET)
        changed, stats = af.fix_document(doc, 'on_sword_get')
        loc = doc['criteria']['x']['conditions']['player'][0]['predicate']['location']
        self.assertEqual(loc, {'structures': 'incendium:forbidden_castle', 'dimension': 'minecraft:the_nether'})
        self.assertEqual(stats.renames['feature'], 1)

    def test_nested_player_location_is_fixed_too(self):
        doc = copy.deepcopy(INCENDIUM_PLAYER)
        changed, stats = af.fix_document(doc, 'incendium')
        loc = conditions(doc, 'x')
        self.assertEqual(loc['biomes'], 'incendium:ash_barrens')
        self.assertEqual(loc['player']['location'], {'biomes': 'incendium:ash_barrens'})
        self.assertEqual(stats.renames['biome'], 2)

    def test_list_and_tag_values_are_kept(self):
        doc = {'criteria': {'a': location_criterion({'biome': ['a:b', '#c:d']}),
                            'b': location_criterion({'structure': '#minecraft:village'})}}
        af.fix_document(doc, 'x')
        self.assertEqual(conditions(doc, 'a')['biomes'], ['a:b', '#c:d'])
        self.assertEqual(conditions(doc, 'b')['structures'], '#minecraft:village')

    def test_location_check_predicate(self):
        doc = {'condition': 'minecraft:location_check', 'predicate': {'biome': 'a:b'}}
        changed, stats = af.fix_document(doc, 'loot')
        self.assertTrue(changed)
        self.assertEqual(doc['predicate'], {'biomes': 'a:b'})

    def test_unchanged_document_reports_no_change(self):
        doc = {'criteria': {'a': location_criterion({'biomes': 'a:b'})}}
        changed, stats = af.fix_document(doc, 'x')
        self.assertFalse(changed)

    def test_idempotent(self):
        doc = copy.deepcopy(ANCIENT_CITIES)
        af.fix_document(doc, 'a')
        again = copy.deepcopy(doc)
        changed, _ = af.fix_document(again, 'a')
        self.assertFalse(changed)
        self.assertEqual(again, doc)


class FailLoudly(unittest.TestCase):
    def test_unsupported_value(self):
        doc = {'criteria': {'a': location_criterion({'biome': {'id': 'x'}})}}
        with self.assertRaises(af.ShapeError):
            af.fix_document(doc, 'x')

    def test_singular_and_plural_together(self):
        doc = {'criteria': {'a': location_criterion({'biome': 'a:b', 'biomes': 'c:d'})}}
        with self.assertRaises(af.ShapeError):
            af.fix_document(doc, 'x')

    def test_unknown_key_is_an_error_in_strict_mode(self):
        doc = {'criteria': {'a': location_criterion({'biomes': 'a:b', 'surprise': 1})}}
        with self.assertRaises(af.ShapeError):
            af.fix_document(doc, 'x')

    def test_unknown_key_is_a_warning_without_strict(self):
        doc = {'criteria': {'a': location_criterion({'biomes': 'a:b', 'surprise': 1})}}
        changed, stats = af.fix_document(doc, 'x', strict=False)
        self.assertEqual(len(stats.warnings), 1)


class Neutralise(unittest.TestCase):
    def test_empty_location_trigger_becomes_impossible(self):
        doc = {'display': {'announce_to_chat': True}, 'criteria': {'a': location_criterion({})}}
        changed, stats = af.fix_document(doc, 'x')
        self.assertTrue(changed)
        self.assertEqual(doc['criteria']['a'], {'trigger': 'minecraft:impossible'})
        self.assertEqual(stats.neutralised, 1)

    def test_technical_advancement_without_display_is_left_alone(self):
        # Incendium keeps an always-true `log_spam_fixer` criterion in its technical advancements on purpose
        doc = {'criteria': {'log_spam_fixer': location_criterion({})}}
        changed, stats = af.fix_document(doc, 'x')
        self.assertFalse(changed)
        self.assertEqual(stats.neutralised, 0)
        self.assertEqual(doc['criteria']['log_spam_fixer']['trigger'], 'minecraft:location')

    def test_hidden_display_without_toast_or_chat_is_left_alone(self):
        doc = {'display': {'announce_to_chat': False, 'show_toast': False}, 'criteria': {'a': location_criterion({})}}
        changed, stats = af.fix_document(doc, 'x')
        self.assertFalse(changed)

    def test_empty_position_is_vacuous(self):
        doc = {'display': {'show_toast': True}, 'criteria': {'a': location_criterion({'position': {}})}}
        af.fix_document(doc, 'x')
        self.assertEqual(doc['criteria']['a']['trigger'], 'minecraft:impossible')

    def test_constrained_location_is_kept(self):
        doc = {'display': {'announce_to_chat': True}, 'criteria': {'a': location_criterion({'dimension': 'minecraft:the_nether'})}}
        af.fix_document(doc, 'x')
        self.assertEqual(doc['criteria']['a']['trigger'], 'minecraft:location')

    def test_tick_root_is_not_touched(self):
        doc = {'criteria': {'root': {'trigger': 'minecraft:tick'}}}
        changed, _ = af.fix_document(doc, 'x')
        self.assertFalse(changed)
        self.assertEqual(doc['criteria']['root'], {'trigger': 'minecraft:tick'})

    def test_other_triggers_are_not_touched(self):
        doc = {'criteria': {'a': {'trigger': 'minecraft:inventory_changed', 'conditions': {}}}}
        changed, _ = af.fix_document(doc, 'x')
        self.assertFalse(changed)

    def test_vacuous_helper(self):
        self.assertTrue(af.is_vacuous({}))
        self.assertTrue(af.is_vacuous({'player': {}}))
        self.assertFalse(af.is_vacuous({'biomes': 'a:b'}))
        self.assertFalse(af.is_vacuous({'light': {'light': {'min': 4}}}))


class ProcessMod(unittest.TestCase):
    def make_mod(self, root):
        adv = root / 'data' / 'repurposed_structures' / 'advancement'
        adv.mkdir(parents=True)
        (adv / 'ancient_cities.json').write_text(json.dumps(ANCIENT_CITIES), encoding='utf-8')
        (adv / 'fine.json').write_text(json.dumps({'criteria': {'a': {'trigger': 'minecraft:tick'}}}), encoding='utf-8')
        pred = root / 'data' / 'incendium' / 'predicate' / 'biome'
        pred.mkdir(parents=True)
        (pred / 'ash.json').write_text(json.dumps({
            'condition': 'minecraft:entity_properties', 'entity': 'this',
            'predicate': {'location': {'biome': 'incendium:ash_barrens'}}}), encoding='utf-8')

    def test_out_dir_gets_only_changed_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            mod = Path(tmp) / 'mod'
            out = Path(tmp) / 'out'
            self.make_mod(mod)
            stats = af.process_mod(mod, out)
            self.assertEqual(stats.files, 3)
            self.assertEqual(stats.changed_files, 2)
            written = sorted(str(p.relative_to(out)).replace('\\', '/') for p in out.rglob('*.json'))
            self.assertEqual(written, ['data/incendium/predicate/biome/ash.json',
                                       'data/repurposed_structures/advancement/ancient_cities.json'])
            fixed = json.loads((out / 'data/repurposed_structures/advancement/ancient_cities.json').read_text(encoding='utf-8'))
            self.assertIn('structures', conditions(fixed, 'in_ancient_city_ocean'))
            # the source stays as it was
            source = json.loads((mod / 'data/repurposed_structures/advancement/ancient_cities.json').read_text(encoding='utf-8'))
            self.assertIn('structure', conditions(source, 'in_ancient_city_ocean'))

    def test_cli_exit_codes(self):
        with tempfile.TemporaryDirectory() as tmp:
            mod = Path(tmp) / 'mod'
            self.make_mod(mod)
            self.assertEqual(af.main(['--out', str(Path(tmp) / 'out'), str(mod)]), 0)
            bad = mod / 'data' / 'x' / 'advancement'
            bad.mkdir(parents=True)
            (bad / 'bad.json').write_text(json.dumps({'criteria': {'a': location_criterion({'biome': {'x': 1}})}}), encoding='utf-8')
            self.assertEqual(af.main(['--out', str(Path(tmp) / 'out2'), str(mod)]), 2)

class Check(unittest.TestCase):
    def test_check_finds_unfixed_files_and_passes_fixed_ones(self):
        with tempfile.TemporaryDirectory() as tmp:
            mod = Path(tmp) / 'mod'
            ProcessMod().make_mod(mod)
            bad = af.check_paths(mod)
            self.assertEqual(len(bad), 2)
            af.process_mod(mod, None)  # in place
            self.assertEqual(af.check_paths(mod), [])
            self.assertEqual(af.main(['--check', str(mod)]), 0)

    def test_check_reads_an_archive(self):
        import zipfile
        with tempfile.TemporaryDirectory() as tmp:
            mod = Path(tmp) / 'mod'
            ProcessMod().make_mod(mod)
            archive = Path(tmp) / 'mod.hmod'
            with zipfile.ZipFile(archive, 'w') as z:
                for path in mod.rglob('*.json'):
                    z.write(path, str(path.relative_to(mod)).replace('\\', '/'))
            self.assertEqual(len(af.check_paths(archive)), 2)
            self.assertEqual(af.main(['--check', str(archive)]), 1)


class RealData(unittest.TestCase):
    """Runs over the real carried mods when they are on disk (skipped otherwise)."""

    def test_no_singular_keys_remain(self):
        mods = Path(__file__).resolve().parents[3] / 'pack' / 'mods'
        if not mods.is_dir():
            self.skipTest('pack/mods not found')
        total = af.Stats()
        with tempfile.TemporaryDirectory() as tmp:
            for carry in sorted(mods.glob('*/carry')):
                out = Path(tmp) / carry.parent.name
                total.merge(af.process_mod(carry, out, strict=True))
            self.assertGreaterEqual(total.renames['structure'] + total.renames['biome'], 40)
            # second pass over the fixed copies changes nothing
            for fixed in sorted(Path(tmp).glob('*')):
                again = af.process_mod(fixed, None, strict=True)
                self.assertEqual(again.changed_files, 0, fixed.name)


if __name__ == '__main__':
    unittest.main()
