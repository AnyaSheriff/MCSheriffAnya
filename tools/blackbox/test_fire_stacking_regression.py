# -*- coding: utf-8 -*-
"""Тесты компактного валидатора регрессии «огненного стека».

Загружают исходный (original.json) и фактический (actual.json) фикстуры;
при отсутствии файлов рядом — синтетическая заглушка (guard), собранная
строго по форме из expected.summary: она НЕ является реальной записью
захвата и помечена как синтетическая. Проверяются позитивный проход и
отклонение мутаций: дубликат пары, пропущенная пара, неизвестная сторона,
неудачная строка, ошибки, неверный статус, HP, fire_hits, присутствие
burn_cycles, длительность дальше 1 тика, bool вместо длительности,
отсутствующий ключ; отдельно — что допуск ±1 тик принимается.
"""

import copy
import json
import unittest
from pathlib import Path

import fire_stacking_regression as fsr

_SIDES = ("official", "ours")


def _load_expected():
    """Ожидаемый фикстур: original.json рядом, иначе путь к артефакту аудита."""
    local = Path(__file__).with_name("original.json")
    if local.exists():
        with open(local, encoding="utf-8") as f:
            return json.load(f)
    fallback = Path(__file__).resolve().parents[2] / "tools" / "blackbox" / "ci" / "fire-stacking-original-777.json"
    with open(fallback, encoding="utf-8") as f:
        return json.load(f)


def _load_actual(expected):
    """Фактический фикстур: actual.json рядом, иначе синтетическая заглушка по форме.

    Синтетика содержит ровно 12 копий expected.summary (6 сценариев ×
    official/ours) со статусом qualified; это охранный вариант, а НЕ
    реальная запись захвата.
    """
    local = Path(__file__).with_name("actual.json")
    if local.exists():
        with open(local, encoding="utf-8") as f:
            return json.load(f)
    rows = []
    for case in expected["cases"]:
        for side in _SIDES:
            rows.append({
                "scenario": case["scenario"],
                "side": side,
                "status": "qualified",
                "observed": {"summary": copy.deepcopy(case["summary"])},
            })
    return {
        "schema": "fire-stacking-discovery-777-v1",
        "status": "qualified-discovery",
        "cases": rows,
    }


class FireStackingRegressionTest(unittest.TestCase):
    """Позитивный проход и отклонение каждой мутации через ValueError."""

    @classmethod
    def setUpClass(cls):
        cls.expected = _load_expected()
        cls.actual = _load_actual(cls.expected)

    def _mut(self):
        """Глубокая копия actual для точечной мутации в тесте."""
        return copy.deepcopy(self.actual)

    def _reject(self, actual):
        with self.assertRaises(ValueError):
            fsr.validate(actual, self.expected)

    def test_positive(self):
        """Исходные фикстуры проходят валидацию без исключений."""
        fsr.validate(self.actual, self.expected)
        self.assertEqual(len(self.expected["cases"]), 6)
        self.assertEqual(len(self.actual["cases"]), 12)

    def test_duplicate_pair_rejected(self):
        actual = self._mut()
        actual["cases"][-1] = copy.deepcopy(actual["cases"][0])
        self._reject(actual)

    def test_missing_pair_rejected(self):
        actual = self._mut()
        actual["cases"].pop(1)
        self._reject(actual)

    def test_unknown_side_rejected(self):
        actual = self._mut()
        actual["cases"][0]["side"] = "theirs"
        self._reject(actual)

    def test_failed_row_rejected(self):
        actual = self._mut()
        actual["cases"][0]["status"] = "failed"
        self._reject(actual)

    def test_errors_rejected(self):
        actual = self._mut()
        actual["errors"] = ["boom"]
        self._reject(actual)

    def test_wrong_status_rejected(self):
        actual = self._mut()
        actual["status"] = "partial-discovery"
        self._reject(actual)

    def test_wrong_hp_rejected(self):
        actual = self._mut()
        actual["cases"][0]["observed"]["summary"]["victim_final_hp"] += 0.001
        self._reject(actual)

    def test_wrong_fire_hits_rejected(self):
        actual = self._mut()
        actual["cases"][0]["observed"]["summary"]["fire_hits"][0] += 1
        self._reject(actual)

    def test_wrong_burn_cycles_presence_rejected(self):
        """Присутствие горения: int↔None в burn_cycles обязан расходиться с expected."""
        actual = self._mut()
        actual["cases"][0]["observed"]["summary"]["burn_cycles"] = [None, 1]
        self._reject(actual)

    def test_duration_more_than_one_tick_rejected(self):
        actual = self._mut()
        actual["cases"][0]["observed"]["summary"]["burn_duration_ticks"][0] += 2
        self._reject(actual)

    def test_bool_as_duration_rejected(self):
        actual = self._mut()
        actual["cases"][0]["observed"]["summary"]["burn_duration_ticks"][0] = True
        self._reject(actual)

    def test_missing_key_rejected(self):
        actual = self._mut()
        del actual["cases"][0]["observed"]["summary"]["melee_hp"]
        self._reject(actual)

    def test_malformed_expected_summary_and_side_rejected(self):
        expected = copy.deepcopy(self.expected)
        del expected['cases'][0]['summary']['health']
        with self.assertRaises(ValueError):
            fsr.validate(self.actual, expected)
        actual = self._mut()
        actual['cases'][0]['side'] = []
        self._reject(actual)

    def test_plus_minus_one_tick_accepted(self):
        """Допуск ±1 тик (округление времён приёма) принимается, не расширяется."""
        actual = self._mut()
        ticks = actual["cases"][0]["observed"]["summary"]["burn_duration_ticks"]
        ticks[0] -= 1
        ticks[1] += 1
        fsr.validate(actual, self.expected)
        beyond = self._mut()
        beyond["cases"][0]["observed"]["summary"]["burn_duration_ticks"][0] -= 2
        self._reject(beyond)


if __name__ == "__main__":
    unittest.main(verbosity=2)
