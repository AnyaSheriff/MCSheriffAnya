"""Чистый контроль подлинного попадания/HP/огня; исходы сложения чар не угадывает."""

import math

MELEE_KIND = "minecraft:player_attack"
FIRE_KIND = "minecraft:on_fire"
TICKS_PER_SEC = 20.0
EPS = 1e-6


def _fail(msg):
    raise ValueError("invalid evidence: " + msg)


def _finite(x):
    return isinstance(x, (int, float)) and not isinstance(x, bool) and math.isfinite(x)


def _ordered_window(seq, what, lo_at, hi_at, hp_check=None):
    prev = None
    for e in seq:
        at = e.get("at")
        if not _finite(at):
            _fail(what + ": nonfinite timestamp")
        if prev is not None and at < prev - EPS:
            _fail(what + ": timestamps not ordered")
        prev = at
        if at < lo_at - EPS:
            _fail(what + ": event before window start")
        if at > hi_at + EPS:
            _fail(what + ": event outside end")
        if hp_check is not None:
            hp_check(e)


def qualify(evidence):
    if not isinstance(evidence, dict):
        _fail("evidence must be a dict")
    for key in ("start_at", "attack_at", "end_at", "victim", "attacker", "initial_hp",
                "gear_verified", "health", "damage", "burning"):
        if key not in evidence:
            _fail("missing key: " + key)

    start_at = evidence["start_at"]
    attack_at = evidence["attack_at"]
    end_at = evidence["end_at"]
    victim = evidence["victim"]
    attacker = evidence["attacker"]
    if not _finite(start_at) or not _finite(attack_at) or not _finite(end_at):
        _fail("nonfinite end bounds")
    if not start_at < attack_at < end_at or not end_at - attack_at > EPS:
        _fail("end bounds equal or reversed")
    if (not isinstance(victim, int) or isinstance(victim, bool)
            or not isinstance(attacker, int) or isinstance(attacker, bool)
            or victim == attacker):
        _fail("victim/attacker ids invalid")
    initial_hp = evidence["initial_hp"]
    if (not isinstance(initial_hp, list) or len(initial_hp) != 2
            or any(not _finite(h) or h != 20.0 for h in initial_hp)):
        _fail("initial HP must be 20.0 for both entities")
    gear = evidence["gear_verified"]
    if (not isinstance(gear, list) or len(gear) != 2
            or any(not isinstance(g, bool) or g is not True for g in gear)):
        _fail("gear_verified must be strictly True for both entities")

    health, damage, burning = evidence["health"], evidence["damage"], evidence["burning"]
    for name, seqs in (("health", health), ("damage", damage), ("burning", burning)):
        if not isinstance(seqs, list) or len(seqs) != 2 or any(not isinstance(s, list) for s in seqs):
            _fail(name + " must be two per-slot lists")

    if any(not entries or not _finite(entries[0].get("at")) for entries in health):
        _fail("health: missing actual finite seeded receipt")
    seed_at = min(entries[0]["at"] for entries in health)
    melee_hp = None
    victim_final = None

    def hp_ok(e):
        if not _finite(e.get("hp")):
            _fail("health: nonfinite HP")

    for slot in (0, 1):
        entries = health[slot]
        if not entries:
            _fail("health: missing seeded entry")
        _ordered_window(entries, "health", seed_at, end_at, hp_ok)
        first = entries[0]
        if first["at"] > start_at or first["hp"] != 20.0:
            _fail("health: actual HP20 seed must precede recording")
        for e in entries:
            if e["at"] <= attack_at + EPS and e["hp"] != 20.0:
                _fail("health: HP changed before melee")
        if slot == 0:  # victim track
            post = [e for e in entries if e["at"] > attack_at + EPS and e["hp"] < 20.0]
            if not post:
                _fail("health: missing post-melee victim HP update")
            if post[0]["hp"] >= 20.0 or post[0]["hp"] <= 0.0:
                _fail("health: victim HP did not strictly decrease after melee")
            prev = post[0]["hp"]
            for e in entries[entries.index(post[0])+1:]:
                if e["hp"] > prev + EPS or e["hp"] <= 0.0:
                    _fail("health: victim HP healed or nonpositive after melee")
                prev = e["hp"]
            melee_hp = 20.0 - post[0]["hp"]
            victim_final = post[-1]["hp"]
        else:  # attacker track must stay at 20
            if any(e["hp"] != 20.0 for e in entries):
                _fail("health: attacker HP must stay 20.0")

    sigs, fire_counts, cycles = [], [], []
    for obs in (0, 1):
        dmg = damage[obs]
        _ordered_window(dmg, "damage[%d]" % obs, attack_at, end_at)
        melees, fires, sig = [], [], []
        for d in dmg:
            kind = d.get("kind")
            entity, cause, direct = d.get("entity"), d.get("cause"), d.get("direct")
            if not isinstance(kind, str) or not kind.startswith("minecraft:"):
                _fail("damage: kind must include the minecraft: prefix")
            if kind == MELEE_KIND:
                melees.append(d)
            elif kind == FIRE_KIND:
                fires.append(d)
            else:
                _fail("damage: unexpected harm kind: " + kind)
            if any(type(v) is not int or v < 0 for v in (entity, cause, direct)):
                _fail("damage: bad attribution fields")
            sig.append((kind, entity, cause, direct))
        if len(melees) != 1:
            _fail("damage: expected exactly one player_attack per observer")
        m = melees[0]
        if m["entity"] != victim or m["cause"] != attacker + 1 or m["direct"] != attacker + 1:
            _fail("damage: player_attack not attributed to victim by attacker")
        if m["at"] < attack_at - EPS:
            _fail("damage: player_attack before authored attack")
        for f in fires:
            if f["entity"] != victim or f["cause"] != 0 or f["direct"] != 0:
                _fail("damage: on_fire must be environmental (cause=direct=0) on victim")
            if not f["at"] > m["at"] + EPS:
                _fail("damage: on_fire damage before or at the player_attack")
        sigs.append(sorted(sig))
        fire_counts.append(len(fires))

    if sigs[0] != sigs[1]:
        _fail("observers disagree on damage counts/kinds/roles")
    first_loss = next(e for e in health[0] if e["hp"] < 20.0)
    actual_melee = next(d for d in damage[0] if d["kind"] == MELEE_KIND)
    if first_loss["at"] < actual_melee["at"] - EPS:
        _fail("health decreased before actual received melee")

    for obs in (0, 1):
        flags = burning[obs]
        _ordered_window(flags, "burning[%d]" % obs, seed_at, end_at)
        for e in flags:
            b = e.get("burning")
            if not isinstance(b, bool):
                _fail("burning: flag must be boolean")
            if b and e["at"] < attack_at - EPS:
                _fail("burning: flag true before authored attack")
        on = off = None
        active = False
        for e in flags:
            if e["burning"] and not active:
                if on is not None:
                    _fail("burning: multiple ignition cycles / reignition")
                on = e["at"]
                active = True
            elif not e["burning"] and active:
                off = e["at"]
                active = False
        if on is None:
            cycles.append(None)
            continue
        if off is None or not off > on:
            _fail("burning: ignition never turned off")
        if not off < end_at - EPS:
            _fail("burning: ignition not turned off before end")
        cycles.append((on, off))

    if (cycles[0] is None) != (cycles[1] is None):
        _fail("observers disagree on ignition presence")
    for obs in (0, 1):
        if cycles[obs] is None:
            if fire_counts[obs] > 0:
                _fail("on_fire damage recorded without ignition")
            if melee_hp <= 0.0:
                _fail("no ignition without positive melee and gear proof")

    return {
        "health": [[e["hp"] for e in health[0]], [e["hp"] for e in health[1]]],
        "fire_hits": fire_counts,
        "burn_cycles": [None if c is None else 1 for c in cycles],
        "burn_duration_ticks": [None if c is None else int(round((c[1] - c[0]) * TICKS_PER_SEC))
                                for c in cycles],
        "melee_hp": melee_hp,
        "victim_final_hp": victim_final,
    }
