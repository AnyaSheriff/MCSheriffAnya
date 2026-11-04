"""Отказ на неизвестных source ID до кодирования MissingTarget."""
import unittest
from generate_static_776 import MISSING, dense_count, direct_mapping


class StaticTests(unittest.TestCase):
    def test_sentinel_and_overflow_are_not_silently_cast(self):
        for number in [-1, 65535, 65536, 2 ** 32, True]:
            with self.assertRaises(ValueError):
                dense_count({"x": number})

    def test_holes_and_duplicate_source_ids_are_rejected(self):
        for values in [{"a": 0, "b": 2}, {"a": 1}, {"a": 0, "b": 0}, {}]:
            with self.assertRaises(ValueError):
                dense_count(values)

    def test_missing_target_is_a_present_source_without_matching_name(self):
        self.assertEqual(direct_mapping({"a": 0, "b": 1}, {"b": 0, "c": 1}),
                         (MISSING, 0))

    def test_direct_pair_keeps_common_objects_absent_from_third_version(self):
        baseline = {"a": 0}
        middle = {"a": 0, "new": 1}
        modern = {"new": 0, "a": 1}
        self.assertEqual(direct_mapping(middle, baseline)[1], MISSING)
        self.assertEqual(direct_mapping(middle, modern)[1], 0)
        self.assertEqual(direct_mapping(modern, middle)[0], 1)

    def test_target_holes_and_sentinel_rejected_too(self):
        for target in [{"x": 65535}, {"a": 0, "b": 2}]:
            with self.assertRaises(ValueError):
                direct_mapping({"a": 0}, target)


if __name__ == "__main__":
    unittest.main()
