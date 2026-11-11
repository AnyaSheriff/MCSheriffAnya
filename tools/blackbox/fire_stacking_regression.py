# -*- coding: utf-8 -*-
"""Компактный чистый валидатор регрессии «огненного стека» (fire stacking).

Сравнивает НОВЫЕ квалифицированные blackbox-наблюдения (actual) с уже
независимо проверенными ИСХОДНЫМИ наблюдениями (expected). Валидатор НЕ
разбирает байты и НЕ маскирует различия таймингов компаратора; он также
НЕ заявляет полную packet- или gameplay-эквивалентность. Доказательство
износа (wear) здесь не проверяется — оно покрывается отдельным аудитом
wire-артефактов. Метаданные expected сохраняются без изменений; никакие
хэши или провенанс не изобретаются.
"""

_REQUIRED = frozenset({
    "health", "fire_hits", "burn_cycles",
    "burn_duration_ticks", "melee_hp", "victim_final_hp",
})
_SIDES = frozenset({"official", "ours"})


def _eq_health(a, b, ctx):
    """Точное равенство вложенных массивов f32-значений HP (структура и значения)."""
    if not isinstance(a, list) or not isinstance(b, list) or len(a) != len(b):
        raise ValueError(f"{ctx}: health — ожидался список той же формы")
    for x, y in zip(a, b):
        if isinstance(x, list) != isinstance(y, list):
            raise ValueError(f"{ctx}: health — несовпадение вложенности списка")
        if isinstance(x, list):
            _eq_health(x, y, ctx)
        elif (isinstance(x, bool) or isinstance(y, bool)
              or not isinstance(x, float) or not isinstance(y, float) or x != y):
            raise ValueError(f"{ctx}: health — значения HP должны совпадать точно (f32)")


def _eq_int_list(label, a, b):
    """Точное равенство списков int/None; bool недопустим."""
    if not isinstance(a, list) or not isinstance(b, list) or len(a) != len(b):
        raise ValueError(f"{label}: ожидался список той же длины")
    for i, (x, y) in enumerate(zip(a, b)):
        if not (x is None and y is None) and (
                isinstance(x, bool) or isinstance(y, bool)
                or not isinstance(x, int) or not isinstance(y, int) or x != y):
            raise ValueError(f"{label}[{i}]: ожидались одинаковые int/None (bool недопустим)")


def _ticks(label, a, b):
    """burn_duration_ticks: None↔None обязателен; иначе Python-int в пределах ±1 тика."""
    if not isinstance(a, list) or not isinstance(b, list) or len(a) != len(b):
        raise ValueError(f"{label}: ожидался список той же длины")
    for i, (x, y) in enumerate(zip(a, b)):
        if y is None:
            if x is not None:
                raise ValueError(f"{label}[{i}]: при None в expected фактическое значение обязано быть None")
            continue
        if (isinstance(x, bool) or isinstance(y, bool)
                or not isinstance(x, int) or not isinstance(y, int) or abs(x - y) > 1):
            raise ValueError(f"{label}[{i}]: ожидался Python-int (не bool) в пределах ±1 тика от expected")


def validate(actual, expected):
    """Проверяет actual против expected; при нарушении формы или значений поднимает ValueError.

    Требует: actual.status ровно "qualified-discovery" и отсутствие ошибок;
    ровно 12 уникальных пар (scenario, side) для всех 6 имён сценариев
    (official/ours), каждый статус "qualified"; в каждой строке
    row.observed.summary — точное совпадение с expected.case.summary по
    health/fire_hits/burn_cycles/melee_hp/victim_final_hp, а
    burn_duration_ticks — либо None↔None, либо Python-int в пределах ±1 тика
    (допуск обусловлен округлением времён приёма наблюдателем и не расширяется).
    """
    if not isinstance(actual, dict) or not isinstance(expected, dict):
        raise ValueError("actual и expected должны быть словарями")
    if actual.get("status") != "qualified-discovery":
        raise ValueError("actual.status должен быть ровно 'qualified-discovery'")
    errors = actual.get("errors", [])
    if not isinstance(errors, list) or errors:
        raise ValueError("actual.errors должен отсутствовать или быть пустым списком")
    exp_cases = expected.get("cases")
    if not isinstance(exp_cases, list) or len(exp_cases) != 6:
        raise ValueError("expected.cases: ожидался непустой список")
    exp = {}
    for c in exp_cases:
        if (not isinstance(c, dict) or not isinstance(c.get("scenario"), str)
                or not isinstance(c.get("summary"), dict) or set(c["summary"]) != _REQUIRED
                or c["scenario"] in exp):
            raise ValueError("expected.cases: некорректная строка или дубликат имени сценария")
        exp[c["scenario"]] = c["summary"]
    rows = actual.get("cases")
    if not isinstance(rows, list) or len(rows) != 12:
        raise ValueError("actual.cases: ожидался список ровно из 12 строк")
    seen = set()
    for row in rows:
        if (not isinstance(row, dict) or not isinstance(row.get("side"), str)
                or row["side"] not in _SIDES or row.get("status") != "qualified"):
            raise ValueError("actual.cases: неверная строка, side или статус не 'qualified'")
        name, side = row.get("scenario"), row.get("side")
        if not isinstance(name, str) or (name, side) in seen:
            raise ValueError("actual.cases: отсутствует или дублируется пара (scenario, side)")
        seen.add((name, side))
        if name not in exp:
            raise ValueError(f"actual.cases: неизвестный сценарий {name!r}")
        obs = row.get("observed")
        s = obs.get("summary") if isinstance(obs, dict) else None
        if not isinstance(s, dict):
            raise ValueError(f"{name}/{side}: отсутствует observed.summary")
        e = exp[name]
        if set(s) != _REQUIRED:
            raise ValueError(f"{name}/{side}: неверный набор ключей summary")
        ctx = f"{name}/{side}"
        _eq_health(s["health"], e["health"], ctx)
        _eq_int_list(f"{ctx}.fire_hits", s["fire_hits"], e["fire_hits"])
        _eq_int_list(f"{ctx}.burn_cycles", s["burn_cycles"], e["burn_cycles"])
        _ticks(f"{ctx}.burn_duration_ticks", s["burn_duration_ticks"], e["burn_duration_ticks"])
        for k in ("melee_hp", "victim_final_hp"):
            v = s[k]
            if isinstance(v, bool) or not isinstance(v, float) or v != e[k]:
                raise ValueError(f"{ctx}: {k} должен точно совпадать (f32, bool недопустим)")
    if seen != {(n, sd) for n in exp for sd in _SIDES}:
        raise ValueError("actual.cases: есть отсутствующие или лишние пары (scenario, side)")
