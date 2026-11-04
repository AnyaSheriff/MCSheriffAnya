"""Офлайн-проверки таблиц, чтобы несовпавшие данные не получали ложные ID."""
import tempfile
import unittest
from pathlib import Path

import generate_registry_26_2 as generator


class RegistryTests(unittest.TestCase):
    def test_name_identity_sparse_ids_and_no_rename_guess(self):
        delta = generator.comparison({"old": 2, "shared": 7}, {"renamed": 3, "shared": 1})
        self.assertEqual(delta["forward_775_to_776"], [-1, -1, -1, -1, -1, -1, -1, 1])
        self.assertEqual(delta["backward_776_to_775"], [-1, 7, -1, -1])
        self.assertEqual(delta["baseline_ids_present"], [2, 7])
        self.assertEqual(delta["removed"], [{"name": "old", "id": 2}])
        self.assertEqual(delta["added"], [{"name": "renamed", "id": 3}])

    def test_duplicate_names_or_ids_rejected(self):
        for values in [[{"name": "a", "id": 0}, {"name": "b", "id": 0}],
                       [{"name": "a", "id": 0}, {"name": "a", "id": 1}]]:
            with self.assertRaises(ValueError):
                generator.named(values)

    def test_every_block_property_participates_in_identity(self):
        old = [{"name": "test", "minStateId": 2, "maxStateId": 3,
                "states": [{"name": "open", "type": "bool", "num_values": 2}]}]
        new = [{"name": "test", "minStateId": 8, "maxStateId": 9,
                "states": [{"name": "open", "type": "enum", "num_values": 2,
                            "values": ["false", "true"]}]}]
        delta = generator.comparison(generator.block_states(old), generator.block_states(new))
        self.assertEqual(delta["forward_775_to_776"][2:], [9, 8])
        self.assertEqual(delta["matched"], 2)
        new[0]["states"][0]["name"] = "other"
        delta = generator.comparison(generator.block_states(old), generator.block_states(new))
        self.assertEqual(delta["matched"], 0)

    def test_block_state_ranges_and_overlaps_rejected(self):
        block = {"name": "a", "minStateId": 0, "maxStateId": 1, "states": []}
        with self.assertRaises(ValueError):
            generator.block_states([block])
        block["maxStateId"] = 0
        with self.assertRaises(ValueError):
            generator.block_states([block, {**block, "name": "b"}])

    def test_missing_field_is_not_the_same_as_explicit_null(self):
        result = generator.records_delta([{"id": 0, "name": "x"}],
                                         [{"id": 1, "name": "x", "extra": None}])
        change = result["changed_shared_fields"][0]["fields"]["extra"]
        self.assertFalse(change["baseline_present"])
        self.assertTrue(change["candidate_present"])
        self.assertNotIn("id", result["changed_shared_fields"][0]["fields"])

    def test_cached_source_hash_and_source_type_are_checked(self):
        with tempfile.TemporaryDirectory() as folder:
            sources = generator.Sources(Path(folder), False, {})
            path = "data/pc/26.2/version.json"
            target = Path(folder) / generator.CANDIDATE[1] / path
            target.parent.mkdir(parents=True)
            target.write_text('{"version": 775}')
            with self.assertRaisesRegex(ValueError, "SHA256"):
                sources.read(generator.CANDIDATE, path)
            for illegal in ["data/pc/26.2/protocol.yml", "data/pc/26.3/blocks.json",
                            "data/pc/26.2/../../secrets.json", "src/index.js"]:
                with self.assertRaisesRegex(ValueError, "Запрещён"):
                    sources.read(generator.CANDIDATE, illegal)


if __name__ == "__main__":
    unittest.main()
