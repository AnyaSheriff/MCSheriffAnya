#!/usr/bin/env python3
"""Один ordinary stone→stone_slab display776 и действие в камнерезе.

Переиспользует наш Client/connect. Не пишет полный Update Recipes, registry
или мир: transient prefix разбирается до одного выбранного display, остальное
выбрасывается. Не читает JAR как архив. По умолчанию только plan.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import signal
import socket
import struct
import subprocess
import tempfile
import time

import official_26_2_inputs
from probe_776_payloads import table
from probe_gameplay import Client, connect, mapping
from smoke import read_varint, varint


def exact(data, size):
    value = data.read(size)
    if len(value) != size:
        raise EOFError("truncated ordinary stonecutter field")
    return value


def uint(data, maximum=65536):
    value = read_varint(data.read)
    if not 0 <= value <= maximum:
        raise ValueError(("ordinary field outside limit", value, maximum))
    return value


def skip_string(data):
    exact(data, uint(data, 32767))


def plain_template(data):
    item, count = uint(data), uint(data, 64)
    added, removed = uint(data, 256), uint(data, 256)
    if item == 0 or count == 0 or added or removed:
        raise ValueError(("only component-free template permitted", item, count, added, removed))
    return {"item": item, "count": count, "added": added, "removed": removed}


def selected_display(payload, stone, slab):
    """Сохраняет единственный display, не всю таблицу и не соседние рецепты.

    Предыдущие stonecutter entries нужны только для button ordinal. Если
    встречается неизвестная форма, не угадываем длину: проба честно останавливается.
    Источник входных ID — pinned JSON, output bytes — только actual packet.
    """
    if len(payload) > 8 * 1024 * 1024:
        raise ValueError("recipe packet too large")
    data = io.BytesIO(payload)
    for _ in range(uint(data, 4096)):
        skip_string(data)
        for _ in range(uint(data)):
            uint(data)
    row_count = uint(data)
    button = 0
    for row in range(row_count):
        entry_start = data.tell()
        size = uint(data, 65537)
        if size == 0:
            # Нельзя определить членство stone через неизвестный tag registry.
            raise ValueError(("tag input not permitted in bounded probe", row, entry_start))
        inputs = [uint(data) for _ in range(size - 1)]
        display_start = data.tell()
        kind = uint(data, 10)
        if kind == 4:
            result = {"item": uint(data), "count": 1, "added": 0, "removed": 0}
            template_start = None
        elif kind == 5:
            template_start = data.tell()
            result = plain_template(data)
        else:
            raise ValueError(("unsupported preceding display", row, display_start, kind))
        applies = stone in inputs
        if applies and result["item"] == slab:
            end = data.tell()
            return {
                "packet_payload_sha256": hashlib.sha256(payload).hexdigest(),
                "packet_payload_length": len(payload),
                "entry_ordinal": row, "button_ordinal_for_stone": button,
                "entry_offset": entry_start, "display_offset": display_start,
                "display_end": end, "input_ids": inputs, "display_type": kind,
                "display_hex": payload[display_start:end].hex(),
                "template_hex": None if template_start is None else payload[template_start:end].hex(),
                "decoded_template": result,
                "scope": "Only selected stone→stone_slab display retained; packet remainder discarded",
            }
        if applies:
            button += 1
    raise ValueError("selected stone→stone_slab recipe absent")


def plain_slot(data):
    count = uint(data, 64)
    if count == 0:
        return None
    item = uint(data)
    added, removed = uint(data, 256), uint(data, 256)
    if item == 0 or added or removed:
        raise ValueError(("unexpected component in authored menu", item, added, removed))
    return {"item": item, "count": count}


class Probe(Client):
    def __init__(self, sock, protocol, stone, slab, report):
        super().__init__(sock, protocol)
        self.stone, self.slab, self.report = stone, slab, report
        self.window = None
        self.revision = 0
        self.slots = {}
        self.cursor = None
        self.cursor_evidence = "initial-unobserved"
        self.menu_type = None

    def confirm_teleport(self, teleport, values):
        self.send("teleport_confirm", varint(teleport))

    def observe_packet(self, name, payload):
        data = io.BytesIO(payload)
        if name == "declare_recipes":
            if "selected_display" in self.report:
                raise ValueError("unexpected repeated recipe announcement")
            selected = selected_display(payload, self.stone, self.slab)
            self.report["selected_display"] = selected
            # Сначала сохраняется actual выбранное наблюдение; не подгоняем его.
            assert selected["display_type"] == 5, "observed result is not ItemStackTemplate"
            assert selected["decoded_template"] == {
                "item": self.slab, "count": 2, "added": 0, "removed": 0,
            }, "selected recipe differs from authored expectation"
        elif name == "open_window":
            self.window, self.menu_type = uint(data), uint(data)
            self.slots, self.cursor = {}, None
            self.cursor_evidence = "menu-reset-unobserved"
            self.report["menu"] = {"window": self.window, "type": self.menu_type}
        elif name == "window_items":
            window, revision = uint(data), uint(data)
            slots = {i: plain_slot(data) for i in range(uint(data, 256))}
            cursor = plain_slot(data)
            if data.read():
                raise ValueError("trailing inventory bytes")
            if self.window is not None and window == self.window:
                self.slots, self.cursor, self.revision = slots, cursor, revision
                self.cursor_evidence = "server-window-items"
        elif name == "set_slot":
            window = read_varint(data.read)  # signed special windows are legal
            revision = uint(data)
            place = struct.unpack(">h", exact(data, 2))[0]
            value = plain_slot(data)
            if data.read():
                raise ValueError("trailing slot bytes")
            if window == self.window:
                self.slots[place], self.revision = value, revision
                if place in (0, 1):
                    self.report["selected_slot_updates"].append(
                        {"slot": place, "revision": revision, "payload_hex": payload.hex(), "value": value})
            elif window == -1 and place == -1:
                self.cursor = value
                self.cursor_evidence = "server-set-slot"
        elif name == "set_cursor_item":
            value = plain_slot(data)
            if data.read():
                raise ValueError("trailing cursor bytes")
            self.cursor = value
            self.cursor_evidence = "server-set-cursor-item"
            self.report.setdefault("cursor_updates", []).append(
                {"payload_hex": payload.hex(), "value": value})
        # Не сохраняем payload UpdateRecipes/recipe books/chunks/registry.

    def wait(self, label, condition, seconds=5):
        deadline = time.monotonic() + seconds
        while not condition():
            if time.monotonic() >= deadline:
                raise TimeoutError(label)
            self.pump(.1)

    def click(self, place):
        assert self.window is not None
        payload = (varint(self.window) + varint(self.revision)
                   + struct.pack(">hb", place, 0) + b"\0\0\0")
        self.report["actions"].append({"action": "window_click", "slot": place,
                                       "revision": self.revision, "payload_hex": payload.hex(),
                                       "predicted_cursor": None})
        self.send("window_click", payload)
        # Этот click действительно сообщает серверу cursorItem=None. Если это
        # совпало с результатом, сервер может не повторять cursor update.
        # Не выдаём локальное предсказание за полученный packet.
        self.cursor = None
        self.cursor_evidence = "client-prediction"

    def snapshot(self, label):
        self.report["snapshots"].append({
            "stage": label, "revision": self.revision,
            "input": self.slots.get(0), "output": self.slots.get(1),
            "cursor": self.cursor,
            "cursor_evidence": self.cursor_evidence,
            "authored_player_slots": {str(k): v for k, v in self.slots.items()
                                      if k >= 2 and v is not None},
        })


COMMANDS = [
    "gamerule minecraft:spawn_mobs false",
    "gamerule minecraft:random_tick_speed 0",
    "fill -4 79 -4 4 79 4 stone",
    "tp McsaProbe 0.5 80 0.5",
    "gamemode survival McsaProbe",
    "clear McsaProbe",
    "setblock 2 80 2 stonecutter",
    "item replace entity McsaProbe hotbar.0 with stone 3",
]


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--jar", type=Path, required=True)
    official_26_2_inputs.arguments(p)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--execute", action="store_true")
    a = p.parse_args()
    a.output = a.output.resolve()
    a.output.mkdir(parents=True, exist_ok=False)
    plan = {"protocol": 776, "port": 26028, "execute": a.execute,
            "commands": COMMANDS, "actions": ["open stonecutter", "put3 stone in input",
              "select observed stone_slab ordinal", "observe2 slabs", "take2 slabs; input becomes2"],
            "retention": "one selected display/template; selected menu slots only; no whole recipes/registry/world dump",
            "expected_window": "normally30–90s; startup limit120s; each observation5s; cleanup30s"}
    (a.output / "plan.json").write_text(json.dumps(plan, ensure_ascii=False, indent=2) + "\n")
    if not a.execute:
        print(json.dumps({"planned": True, "launched": False, "plan": str(a.output / "plan.json")}))
        return
    def interrupted(signum, _frame):
        # Внешний runner может завершить Python по deadline; finally остановит
        # именно нашу отдельную Java group, а не чужие серверы.
        raise KeyboardInterrupt(f"probe interrupted by signal {signum}")
    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGINT, interrupted)
    cpus = sorted(os.sched_getaffinity(0))[:2]
    assert len(cpus) == 2
    os.sched_setaffinity(0, cpus)
    protocol, _, provenance = official_26_2_inputs.load(a)
    items = {row["name"]: row["id"] for row in table(a.registry_cache, "items")}
    report = {"protocol": 776, "cpus": cpus, "provenance": provenance,
              "selected_slot_updates": [], "actions": [], "snapshots": [], "checks": [],
              "executed_commands": [], "success": False}
    report["packet_ids"] = {name: mapping(protocol, "play", side)[name]
                           for side, names in [("toClient", ["declare_recipes", "open_window", "set_slot"]),
                                               ("toServer", ["enchant_item", "window_click"])] for name in names}
    with socket.socket() as check:
        check.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        check.bind(("127.0.0.1", 26028))
    with tempfile.TemporaryDirectory(prefix="mcsa-stonecutter776-") as directory:
        root = Path(directory)
        (root / "eula.txt").write_text("eula=true\n")
        (root / "server.properties").write_text(
            "server-ip=127.0.0.1\nserver-port=26028\nonline-mode=false\nwhite-list=false\n"
            "enforce-secure-profile=false\nnetwork-compression-threshold=-1\n"
            "level-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\n"
            "gamemode=creative\nspawn-protection=0\nmax-players=2\n")
        log = a.output / "server.log"
        with log.open("w") as logfile:
            server = subprocess.Popen(
                ["/usr/lib/jvm/java-25-openjdk/bin/java", "-Xmx1G", "-jar", str(a.jar.resolve()), "nogui"],
                cwd=root, stdin=subprocess.PIPE, stdout=logfile, stderr=subprocess.STDOUT,
                text=True, start_new_session=True)
            report["server_pid"] = server.pid
            client = None

            def command(text):
                report["executed_commands"].append(text)
                server.stdin.write(text + "\n")
                server.stdin.flush()

            try:
                deadline = time.monotonic() + 120
                while "Done (" not in log.read_text():
                    assert server.poll() is None and time.monotonic() < deadline, "startup failed"
                    time.sleep(.1)
                client = connect(26028, protocol,
                                 lambda s, p: Probe(s, p, items["stone"], items["stone_slab"], report), 776)
                client.wait("initial recipe/position absent",
                            lambda: client.position is not None and "selected_display" in report, 10)
                for text in COMMANDS:
                    command(text)
                    client.pump(.2)
                client.send("block_place", b"\0" + struct.pack(">Q", 2 << 38 | 2 << 12 | 80)
                            + b"\1" + struct.pack(">fff", .5, .5, .5) + b"\0\0\1")
                client.wait("stonecutter menu did not open", lambda: client.window is not None)
                # Это отдельная гипотеза по нашим775/777 observations; failure сохраняется.
                assert client.menu_type == 24, ("observed776 stonecutter menu type", client.menu_type)
                stone3 = {"item": items["stone"], "count": 3}
                client.wait("authored stone not in menu", lambda: stone3 in client.slots.values())
                places = [i for i, value in client.slots.items() if i >= 2 and value == stone3]
                assert len(places) == 1
                client.snapshot("opened")
                client.click(places[0])
                client.wait("stone not picked up", lambda: client.cursor == stone3)
                client.click(0)
                client.wait("stone not inserted", lambda: client.slots.get(0) == stone3 and client.cursor is None)
                selected = report["selected_display"]["button_ordinal_for_stone"]
                action = varint(client.window) + varint(selected)
                report["actions"].append({"action": "select_recipe", "ordinal": selected,
                                          "payload_hex": action.hex()})
                client.send("enchant_item", action)
                slabs2 = {"item": items["stone_slab"], "count": 2}
                client.wait("selected output differs", lambda: client.slots.get(1) == slabs2)
                client.snapshot("selected")
                client.click(1)
                client.wait("selected output not taken", lambda: client.cursor == slabs2
                            and client.cursor_evidence.startswith("server-")
                            and client.slots.get(0) == {"item": items["stone"], "count": 2})
                client.snapshot("taken")
                report["checks"] = ["selected-component-free-template", "actual-stonecutter-menu",
                                    "authored-input3", "selected-output2", "take-output-consumes-one-input"]
                report["success"] = True
            except BaseException as error:
                report["error"] = repr(error)
                if client:
                    client.snapshot("failure")
                raise
            finally:
                if client:
                    client.sock.close()
                if server.poll() is None:
                    try:
                        command("stop")
                        server.stdin.close()
                        server.wait(timeout=30)
                    except (BrokenPipeError, subprocess.TimeoutExpired):
                        os.killpg(server.pid, signal.SIGKILL)
                        server.wait()
                report["server_exit"] = server.returncode
                with socket.socket() as check:
                    try:
                        check.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
                        check.bind(("127.0.0.1", 26028))
                        report["port_released"] = True
                    except OSError as error:
                        report["port_released"] = False
                        report["cleanup_error"] = repr(error)
                report["success"] = report["success"] and server.returncode == 0 and report["port_released"]
                (a.output / "result.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    assert report["success"]
    print(json.dumps({"checks": len(report["checks"]), "success": True, "port_released": True}))


if __name__ == "__main__":
    main()
