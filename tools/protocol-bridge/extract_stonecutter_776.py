#!/usr/bin/env python3
"""Один собственный observed template776; полный recipe packet не сохраняется."""
import argparse
import hashlib
import json
from pathlib import Path

SUCCESS_SHA = "a5c5daeceef2cbd4b9747cecc2b67c9e61aa5b8e78dd96f50f38bb443f93942e"


def build(source):
    raw = source.read_bytes()
    assert hashlib.sha256(raw).hexdigest() == SUCCESS_SHA, "source recording changed"
    data = json.loads(raw)
    assert data["protocol"] == 776 and data["success"] and data["server_exit"] == 0 and data["port_released"]
    selected = data["selected_display"]
    assert selected["display_hex"] == "05b702020000"
    assert selected["template_hex"] == "b702020000"
    assert selected["decoded_template"] == {"item": 311, "count": 2, "added": 0, "removed": 0}
    taken = next(row for row in data["snapshots"] if row["stage"] == "taken")
    assert taken["cursor_evidence"] == "server-set-cursor-item"
    assert taken["cursor"] == {"item": 311, "count": 2} and taken["input"] == {"item": 1, "count": 2}
    assert any(row["payload_hex"] == "0105000002010000" for row in data["selected_slot_updates"])
    assert any(row["payload_hex"] == "02b7020000" for row in data["cursor_updates"])
    return {
        "format": 1, "protocol": 776, "source_recording": source.parent.name + "/" + source.name,
        "source_sha256": SUCCESS_SHA, "provenance": data["provenance"],
        "case": "ordinary stonecutter stone→stone_slab",
        "selected": selected, "menu": data["menu"],
        "actions": data["actions"], "selected_slot_updates": data["selected_slot_updates"],
        "cursor_updates": data["cursor_updates"], "final_snapshot": taken,
        "checks": data["checks"],
        "limits": {
            "actual_template_components": "none",
            "existing_known_component_template_composition": "synthetic regression, not separate official observation",
            "public_wire_776": False, "creative_untrusted_776": False,
            "full_recipe_packet_stored": False, "runtime_core_776": False,
            "prior_failure": "verification-stonecutter776-01: observer failed to distinguish predicted empty cursor; preserved outside repo",
        },
    }


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--source", type=Path, required=True)
    p.add_argument("--output", type=Path, default=Path(__file__).resolve().parent / "testdata/stonecutter-26.2-observation.json")
    p.add_argument("--check", action="store_true")
    a = p.parse_args()
    body = json.dumps(build(a.source), ensure_ascii=False, indent=2) + "\n"
    if a.check:
        assert a.output.read_text() == body, "selected fixture stale"
    else:
        a.output.parent.mkdir(parents=True, exist_ok=True)
        a.output.write_text(body)
    print("selected stonecutter776 fixture: " + ("check PASS" if a.check else "written"))


if __name__ == "__main__":
    main()
