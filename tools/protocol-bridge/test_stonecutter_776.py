"""Синтетические parser tests; НЕ доказательство официального776 поведения."""
import io
import unittest
from probe_stonecutter_776 import selected_display, plain_slot, plain_template, Probe
from smoke import varint


def row(source, item, count=2):
    return varint(2) + varint(source) + varint(5) + varint(item) + varint(count) + b"\0\0"


class Tests(unittest.TestCase):
    def test_selects_only_authored_display_and_filtered_button(self):
        data = b"\0\4" + row(90, 7) + row(1, 8) + row(1, 9) + row(1, 10)
        result = selected_display(data, 1, 9)
        self.assertEqual(result["button_ordinal_for_stone"], 1)
        self.assertEqual(result["entry_ordinal"], 2)
        self.assertEqual(result["display_hex"], "0509020000")
        self.assertEqual(result["template_hex"], "09020000")
        self.assertNotIn("rows", result)
        self.assertEqual(bytes.fromhex(result["display_hex"]),
                         data[result["display_offset"]:result["display_end"]])

    def test_skips_recipe_property_set_without_retaining_it(self):
        data = b"\1\3foo\2\1\2\1" + row(1, 9)
        self.assertEqual(selected_display(data, 1, 9)["decoded_template"]["count"], 2)

    def test_refuses_unknown_prefix_and_tag_membership(self):
        for data in [b"\0\1\0\3tag", b"\0\1\2\1\6", b"\0\0"]:
            with self.assertRaises((ValueError, EOFError)):
                selected_display(data, 1, 9)

    def test_truncation_never_yields_partial_selected_display(self):
        data = b"\0\1" + row(1, 9)
        for n in range(len(data)):
            with self.subTest(length=n), self.assertRaises((ValueError, EOFError)):
                selected_display(data[:n], 1, 9)

    def test_template_is_item_first_slot_is_count_first(self):
        self.assertEqual(plain_template(io.BytesIO(b"\x09\x02\0\0"))["item"], 9)
        self.assertEqual(plain_slot(io.BytesIO(b"\x02\x09\0\0")), {"item": 9, "count": 2})
        for data in [b"\x09\x02\1\0", b"\x09\x02\0\1", b"\x09\0\0\0"]:
            with self.assertRaises(ValueError):
                plain_template(io.BytesIO(data))

    def test_observed_wrong_selected_count_retained_before_failure(self):
        probe = object.__new__(Probe)
        probe.report, probe.stone, probe.slab = {}, 1, 9
        with self.assertRaises(AssertionError):
            probe.observe_packet("declare_recipes", b"\0\1" + row(1, 9, 1))
        self.assertEqual(probe.report["selected_display"]["decoded_template"]["count"], 1)

    def test_extra_packet_remainder_is_not_saved_or_parsed(self):
        result = selected_display(b"\0\2" + row(1, 9) + b"unrelated-not-decoded", 1, 9)
        self.assertEqual(result["template_hex"], "09020000")
        self.assertNotIn("unrelated", str(result))

    def test_cursor_prediction_is_not_counted_as_server_observation(self):
        probe = object.__new__(Probe)
        probe.report = {"actions": []}
        probe.window, probe.revision = 1, 2
        probe.cursor = {"item": 1, "count": 3}
        probe.cursor_evidence = "server-set-cursor-item"
        sent = []
        probe.send = lambda name, payload: sent.append((name, payload))
        probe.click(0)
        self.assertIsNone(probe.cursor)
        self.assertEqual(probe.cursor_evidence, "client-prediction")
        self.assertEqual(sent[0], ("window_click", bytes.fromhex("0102000000000000")))
        probe.observe_packet("set_cursor_item", b"\2" + varint(311) + b"\0\0")
        self.assertEqual(probe.cursor, {"item": 311, "count": 2})
        self.assertEqual(probe.cursor_evidence, "server-set-cursor-item")
        self.assertEqual(probe.report["cursor_updates"][0]["payload_hex"], "02b7020000")


if __name__ == "__main__":
    unittest.main()
