import math
import unittest

from fire_stacking_qualifier import qualify

MELEE = "minecraft:player_attack"
FIRE = "minecraft:on_fire"
CAUSE = 201  # attacker(200) + 1


def base_evidence():
    return {
        "start_at": 0.999,
        "attack_at": 1.0,
        "end_at": 5.0,
        "victim": 100,
        "attacker": 200,
        "initial_hp": [20.0, 20.0],
        "gear_verified": [True, True],
        "health": [
            [{"at": 0.999, "hp": 20.0}, {"at": 1.05, "hp": 17.0}, {"at": 1.25, "hp": 14.0}],
            [{"at": 0.999, "hp": 20.0}],
        ],
        "damage": [
            [{"at": 1.0, "entity": 100, "kind": MELEE, "cause": CAUSE, "direct": CAUSE},
             {"at": 1.2, "entity": 100, "kind": FIRE, "cause": 0, "direct": 0},
             {"at": 1.4, "entity": 100, "kind": FIRE, "cause": 0, "direct": 0}],
            [{"at": 1.0, "entity": 100, "kind": MELEE, "cause": CAUSE, "direct": CAUSE},
             {"at": 1.3, "entity": 100, "kind": FIRE, "cause": 0, "direct": 0},
             {"at": 1.45, "entity": 100, "kind": FIRE, "cause": 0, "direct": 0}],
        ],
        "burning": [
            [{"at": 0.999, "burning": False}, {"at": 1.1, "burning": True},
             {"at": 2.0, "burning": False}],
            [{"at": 1.05, "burning": False}, {"at": 1.15, "burning": True},
             {"at": 1.95, "burning": False}],
        ],
    }


def no_ignition_ev():
    ev = base_evidence()
    ev["burning"] = [[], []]
    ev["damage"] = [
        [{"at": 1.0, "entity": 100, "kind": MELEE, "cause": CAUSE, "direct": CAUSE}],
        [{"at": 1.0, "entity": 100, "kind": MELEE, "cause": CAUSE, "direct": CAUSE}],
    ]
    ev["health"] = [[{"at": 0.999, "hp": 20.0}, {"at": 1.05, "hp": 17.0}],
                    [{"at": 0.999, "hp": 20.0}]]
    return ev


class QualifierTests(unittest.TestCase):
    def assert_bad(self, ev):
        with self.assertRaises(ValueError):
            qualify(ev)

    def test_positive_burning(self):
        s = qualify(base_evidence())
        self.assertEqual(s["health"], [[20.0, 17.0, 14.0], [20.0]])
        self.assertEqual(s["fire_hits"], [2, 2])
        self.assertEqual(s["burn_cycles"], [1, 1])
        self.assertEqual(s["burn_duration_ticks"], [18, 16])
        self.assertEqual(s["melee_hp"], 3.0)
        self.assertEqual(s["victim_final_hp"], 14.0)

    def test_no_ignition(self):
        s = qualify(no_ignition_ev())
        self.assertEqual(s["fire_hits"], [0, 0])
        self.assertEqual(s["burn_cycles"], [None, None])
        self.assertEqual(s["burn_duration_ticks"], [None, None])
        self.assertEqual(s["melee_hp"], 3.0)
        self.assertEqual(s["victim_final_hp"], 17.0)

    def test_missing_realhit(self):
        ev = base_evidence()
        ev["damage"][0] = [d for d in ev["damage"][0] if d["kind"] != MELEE]
        self.assert_bad(ev)

    def test_missing_hp_update(self):
        ev = base_evidence()
        ev["health"][0] = [{"at": 0.999, "hp": 20.0}]
        self.assert_bad(ev)

    def test_missing_gear(self):
        ev = base_evidence()
        ev["gear_verified"] = [True, False]
        self.assert_bad(ev)

    def test_causal_mismatch_hit_before_authored(self):
        ev = base_evidence()
        for side in ev["damage"]:
            side[0]["at"] = 0.5
        self.assert_bad(ev)

    def test_causal_mismatch_fire_not_after_melee(self):
        ev = base_evidence()
        ev["damage"][0][1]["at"] = 1.0
        self.assert_bad(ev)

    def test_causal_mismatch_wrong_cause(self):
        ev = base_evidence()
        for side in ev["damage"]:
            side[0]["cause"] = 1
            side[0]["direct"] = 1
        self.assert_bad(ev)

    def test_observer_missing_hit(self):
        ev = base_evidence()
        ev["damage"][1] = [d for d in ev["damage"][1] if d["kind"] != MELEE]
        self.assert_bad(ev)

    def test_extra_source(self):
        ev = base_evidence()
        extra = {"at": 1.6, "entity": 100, "kind": "minecraft:arrow", "cause": 3, "direct": 3}
        ev["damage"][0].append(dict(extra))
        ev["damage"][1].append(dict(extra))
        self.assert_bad(ev)

    def test_extra_harm_on_attacker(self):
        ev = base_evidence()
        extra = {"at": 1.6, "entity": 200, "kind": FIRE, "cause": 0, "direct": 0}
        ev["damage"][0].append(dict(extra))
        ev["damage"][1].append(dict(extra))
        self.assert_bad(ev)

    def test_unresolved_burn(self):
        ev = base_evidence()
        for flags in ev["burning"]:
            del flags[-1]  # ends still burning
        self.assert_bad(ev)

    def test_reignite(self):
        ev = base_evidence()
        ev["burning"] = [
            [{"at": 1.05, "burning": False}, {"at": 1.1, "burning": True},
             {"at": 1.8, "burning": False}, {"at": 2.5, "burning": True}],
            [{"at": 1.05, "burning": False}, {"at": 1.15, "burning": True},
             {"at": 1.8, "burning": False}, {"at": 2.5, "burning": True}],
        ]
        self.assert_bad(ev)

    def test_preexisting_burn(self):
        ev = base_evidence()
        for flags in ev["burning"]:
            flags[0] = {"at": 0.5, "burning": True}
        self.assert_bad(ev)

    def test_ignition_disagreement(self):
        ev = base_evidence()
        ev["burning"][1] = []
        self.assert_bad(ev)

    def test_hp_heal(self):
        ev = base_evidence()
        ev["health"][0][2]["hp"] = 18.0
        self.assert_bad(ev)

    def test_attacker_hp_drop(self):
        ev = base_evidence()
        ev["health"][1].append({"at": 1.2, "hp": 19.0})
        self.assert_bad(ev)

    def test_nan_inputs(self):
        ev = base_evidence()
        ev["health"][0][1]["hp"] = float("nan")
        self.assert_bad(ev)
        ev2 = base_evidence()
        ev2["attack_at"] = float("nan")
        self.assert_bad(ev2)

    def test_out_of_time_health(self):
        ev = base_evidence()
        ev["health"][0].append({"at": 6.0, "hp": 12.0})
        self.assert_bad(ev)

    def test_out_of_time_damage(self):
        ev = base_evidence()
        ev["damage"][0].append({"at": 5.5, "entity": 100, "kind": FIRE, "cause": 0, "direct": 0})
        self.assert_bad(ev)

    def test_out_of_time_burning(self):
        ev = base_evidence()
        ev["burning"][0].append({"at": 5.5, "burning": False})
        self.assert_bad(ev)

    def test_health_before_seed(self):
        ev = base_evidence()
        ev["health"][0].append({"at": 0.5, "hp": 20.0})
        self.assert_bad(ev)

    def test_real_earlier_seed_receipts_and_duplicate_burning_flag(self):
        ev = base_evidence()
        ev["health"][0][0]["at"] = 0.4
        ev["health"][1][0]["at"] = 0.6
        ev["burning"][0].insert(2, {"at": 1.2, "burning": True})
        qualify(ev)

    def test_hp_cannot_decrease_before_received_melee(self):
        ev = base_evidence()
        for ds in ev['damage']:
            ds[0]['at'] = 1.1
        self.assert_bad(ev)

    def test_healing_to_twenty_cannot_be_filtered_out(self):
        ev = base_evidence()
        ev['health'][0].append({'at': 2.0, 'hp': 20.0})
        self.assert_bad(ev)

    def test_reversed_end_bounds(self):
        ev = base_evidence()
        ev["end_at"] = 0.5
        self.assert_bad(ev)

    def test_equal_end_bounds(self):
        ev = base_evidence()
        ev["end_at"] = ev["attack_at"]
        self.assert_bad(ev)

    def test_missing_key(self):
        ev = base_evidence()
        del ev["burning"]
        self.assert_bad(ev)


if __name__ == "__main__":
    unittest.main()
