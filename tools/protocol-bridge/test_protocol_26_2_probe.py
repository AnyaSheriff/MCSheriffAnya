"""Проверки нового анализатора на прежних собственных наблюдениях, без Java."""
import json
from pathlib import Path
import unittest

from analyze_protocol_26_2 import Reader, analyze, attribute_slot, game_packet, success

DATA = Path(__file__).with_name("testdata")


class ProbeTests(unittest.TestCase):
    def test_existing_777_session_uuid_and_profile(self):
        raw = (DATA / "login-success-26.3.bin").read_bytes()
        result = success(raw)
        self.assertEqual(result["username"], "McsaProbe")
        self.assertNotEqual(result["player_uuid"], result["session_uuid"])
        for broken in [raw[:-1], raw + b"\0", (DATA / "login-success-26.1.2.bin").read_bytes()]:
            with self.assertRaises(ValueError):
                success(broken)

    def test_known_join_none_and_previous_creative(self):
        raw = (DATA / "play-login-26.3-none.bin").read_bytes()
        current = game_packet(raw, "login", "optional_varint")
        legacy_hypothesis = game_packet(raw, "login", "legacy_byte")
        self.assertIsNone(current["previous_mode"])
        self.assertEqual(legacy_hypothesis["previous_mode"], 0)
        self.assertFalse(current["online_mode"])
        raw = (DATA / "play-login-26.3-previous-creative.bin").read_bytes()
        result = game_packet(raw, "login", "optional_varint")
        self.assertEqual((result["mode"], result["previous_mode"]), (0, 1))
        self.assertEqual(game_packet(raw, "login", "legacy_byte")["previous_mode"], 2)

    def test_join_does_not_ignore_trailing_data(self):
        raw = (DATA / "play-login-26.3-none.bin").read_bytes()
        for broken in [raw[:-1], raw + b"\0"]:
            with self.assertRaises(ValueError):
                game_packet(broken, "login", "optional_varint")

    def test_all_75_existing_attribute_observations_parse_exactly(self):
        observations = json.loads((DATA / "attribute-id-observations.json").read_text())
        for side, count in [("old", 35), ("new", 40)]:
            samples = observations[side]["cases"]
            values = [attribute_slot(bytes.fromhex(case["slot_hex"])) for case in samples]
            self.assertEqual({attribute for _, attribute in values}, set(range(count)))
            self.assertEqual(len({item for item, _ in values}), 1)
            self.assertEqual(len(values), count)

    def test_attribute_payload_must_be_the_requested_modifier(self):
        sample = json.loads((DATA / "attribute-id-observations.json").read_text())["new"]["cases"][0]
        raw = bytes.fromhex(sample["slot_hex"])
        for broken in [raw[:-1], raw + b"\0", raw.replace(b"mcsa:probe", b"other:name")]:
            with self.assertRaises(ValueError):
                attribute_slot(broken)

    def test_oversize_varint_and_boolean_rejected(self):
        for raw in [b"\xff\xff\xff\xff\x10", b"\x80\x80\x80\x80\x80"]:
            with self.assertRaises(ValueError):
                Reader(raw).uint()
        with self.assertRaises(ValueError):
            Reader(b"\x02").boolean()

    def test_actual_776_session_and_all_40_attributes(self):
        fixture = json.loads((DATA / "protocol-26.2-observations.json").read_text())
        result = analyze(fixture["login"], fixture["sessions"], fixture["attributes"])
        expected = {k: v for k, v in fixture["verified"].items() if k != "input_sha256"}
        self.assertEqual(result, expected)
        self.assertEqual(result["spawn_info_encoding"], "legacy_byte")
        self.assertEqual(result["attribute_count"], 40)
        self.assertEqual(result["attribute_ids"]["minecraft:movement_speed"], 26)
        self.assertEqual(result["attribute_ids"]["minecraft:bounciness"], 9)
        packet = next(p for p in fixture["login"]["packets"] if "test_profile_payload_hex" in p)
        self.assertEqual((DATA / "login-success-26.2.bin").read_bytes(),
                         bytes.fromhex(packet["test_profile_payload_hex"]))

    def test_missing_attribute_and_failed_run_cannot_be_called_verified(self):
        fixture = json.loads((DATA / "protocol-26.2-observations.json").read_text())
        fixture["attributes"]["cases"].pop()
        with self.assertRaisesRegex(ValueError, "Не все"):
            analyze(fixture["login"], fixture["sessions"], fixture["attributes"])
        fixture["sessions"]["exit_code"] = 1
        with self.assertRaisesRegex(ValueError, "Неуспешная"):
            analyze(fixture["login"], fixture["sessions"], fixture["attributes"])


if __name__ == "__main__":
    unittest.main()
