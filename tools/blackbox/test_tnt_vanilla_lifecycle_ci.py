"""Offline acceptance guards; no Minecraft process."""
import unittest
from tnt_vanilla_lifecycle_ci import entity_fields, entity_state, game_time, validate


def receipt():
    rows = [{'label': label, 'gametime': 20 + dt,
             'entity': 'observable' if dt <= 20 else 'not-observable',
             'fields': {'uuid': [1, 2, 3, 4], 'fuse': 80 - dt if dt <= 20 else 0},
             'witness': '[Server] MCSA_WITNESS_DIRT' if dt == 0 else ''}
            for label, dt in zip(('primed-frozen', 'after20', 'after100', 'after200', 'reheld-after220'), (0, 20, 100, 200, 220))]
    return {'case': 'held', 'observations': rows, 'loaded_witness': '[Server] MCSA_COORDINATE_READABLE',
            'cleanup': {'exit_code': 0, 'method': 'stop-and-eof', 'group_gone': True},
            'port': 26154, 'ports_free': {'26154': True}}


class Guards(unittest.TestCase):
    def test_time_requires_one_explicit_acknowledgment(self):
        self.assertEqual(game_time('The time is 123\n[Server] barrier'), 123)
        self.assertEqual(game_time('[16:37:27] [Server thread/INFO]: System chat: The game time is 1 tick(s)'), 1)
        for text in ('unknown', 'The time is 1\nThe time is 2',
                     'The time is 1\nThe game time is 1 tick(s)',
                     'The game time is 1', 'The game time is 1 day(s)'):
            with self.assertRaises(ValueError):
                game_time(text)

    def test_absence_is_only_not_observable(self):
        self.assertEqual(entity_state('No entity was found'), 'not-observable')
        self.assertEqual(entity_state('TNT has the following entity data: {fuse:80s, UUID:[I;1,2,3,4], Pos:[1d,2d,3d]}'), 'observable')
        with self.assertRaises(ValueError):
            entity_state('Unknown or incomplete command')

    def test_reject_empty_or_contradictory_nbt(self):
        for text in ('TNT has the following entity data: {}',
                     'TNT has the following entity data: {} No entity was found',
                     'fuse:80s, Fuse:80s, UUID:[I;1,2,3,4], Pos:[1d,2d,3d]',
                     'fuse:80s, UUID:[I;1,2,3], Pos:[1d,2d,3d]',
                     'fuse:80s, UUID:[I;1,2,3,4], Pos:[NaNd,2d,3d]'):
            with self.assertRaises(ValueError):
                entity_fields(text)

    def test_control_identity_and_progress(self):
        for field, value in (('uuid', [4, 3, 2, 1]), ('fuse', 80)):
            changed = receipt()
            changed['observations'][1]['fields'][field] = value
            with self.assertRaises(ValueError):
                validate(changed)

    def test_empty_port_check_rejected(self):
        changed = receipt()
        changed['ports_free'] = {}
        with self.assertRaises(ValueError):
            validate(changed)

    def test_readability_required(self):
        changed = receipt()
        changed.pop('loaded_witness')
        with self.assertRaises(ValueError):
            validate(changed)

    def test_valid_control(self):
        self.assertEqual(validate(receipt()), 'observation-complete-not-parity')

    def test_missing_or_shifted_observation_rejected(self):
        for mutation in ('missing', 'shifted'):
            value = receipt()
            if mutation == 'missing':
                value['observations'].pop()
            else:
                value['observations'][2]['gametime'] += 1
            with self.assertRaises(ValueError):
                validate(value)

    def test_control_must_start_with_entity_and_block(self):
        for field, value in (('entity', 'not-observable'), ('witness', '')):
            changed = receipt()
            changed['observations'][0][field] = value
            with self.assertRaises(ValueError):
                validate(changed)

    def test_held_control_must_end_without_observable_tnt(self):
        changed = receipt()
        changed['observations'][-1]['entity'] = 'observable'
        with self.assertRaises(ValueError):
            validate(changed)
        changed['case'] = 'released'
        self.assertEqual(validate(changed), 'observation-complete-not-parity')

    def test_cleanup_is_strict(self):
        for field, value in (('exit_code', 1), ('method', 'term-after-stop-timeout'), ('group_gone', False)):
            changed = receipt()
            changed['cleanup'][field] = value
            with self.assertRaises(ValueError):
                validate(changed)
        changed = receipt()
        changed['ports_free']['26154'] = False
        with self.assertRaises(ValueError):
            validate(changed)


if __name__ == '__main__':
    unittest.main()
