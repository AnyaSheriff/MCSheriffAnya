#!/usr/bin/env python3
"""Смена сделки при одинаковой оплате: свои команды/пакеты, без чтения JAR.

Запускать только после остановки Cargo и других проб на выбранном порту.
uses берутся из настоящего Merchant Offers после повторного открытия окна.
"""
import argparse
import hashlib
import io
import json
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import time

from probe_container_session import Probe as InventoryProbe
from probe_gameplay import connect
from smoke import read_varint, varint


class Probe(InventoryProbe):
    def __init__(self, sock, protocol, version, names):
        super().__init__(sock, protocol, version, names)
        self.offers = None
        self.stage = "login"

    def observe_packet(self, name, payload):
        start = len(self.observed)
        super().observe_packet(name, payload)
        if name == "trade_list":
            data = io.BytesIO(payload)
            window = read_varint(data.read)
            count = read_varint(data.read)
            assert 0 <= count <= 128

            def ingredient():
                item, amount = read_varint(data.read), read_varint(data.read)
                assert read_varint(data.read) == 0, "probe expects plain input ingredients"
                return {"item": self.names[item], "count": amount}

            offers = []
            for _ in range(count):
                buy, sell = ingredient(), self.slot(data)
                present = data.read(1)
                assert present in (b"\0", b"\1")
                extra = ingredient() if present == b"\1" else None
                disabled = data.read(1)
                assert disabled in (b"\0", b"\1")
                uses, maximum, xp, special, multiplier, demand = struct.unpack(">iiiifi", data.read(24))
                offers.append({"buy": buy, "sell": sell, "extra": extra,
                               "disabled": disabled == b"\1", "uses": uses, "max_uses": maximum,
                               "xp": xp, "special_price": special, "multiplier": multiplier,
                               "demand": demand})
            level, xp = read_varint(data.read), read_varint(data.read)
            regular, restock = data.read(1), data.read(1)
            assert regular in (b"\0", b"\1") and restock in (b"\0", b"\1") and not data.read()
            self.offers = {"window": window, "offers": offers, "level": level, "xp": xp,
                           "regular": regular == b"\1", "can_restock": restock == b"\1"}
            self.observed.append({"name": name, "payload": payload.hex(), "decoded": self.offers})
        for event in self.observed[start:]:
            event["stage"] = self.stage


def clone(value):
    return json.loads(json.dumps(value))


def stack(item, count):
    return {"item": item, "count": count} if count else None


def run_cases(client, command, villager_type, item_ids, report):
    entity = None

    def stage(value):
        client.stage = value

    def opened():
        client.offers = None
        client.send("use_entity", varint(entity) + b"\0\0\0")
        client.pump(5, lambda r: r["name"] == "trade_list")
        client.pump(.3)
        assert client.inventory and client.offers
        assert client.inventory["window"] == client.offers["window"]
        assert len(client.inventory["slots"]) == 39

    def close():
        if client.inventory and client.inventory["window"]:
            client.send("close_window", varint(client.inventory["window"]))
            client.pump(.3)

    def market(label, emeralds=64, cheap_disabled=False):
        nonlocal entity
        stage(label + "/setup")
        close()
        command("clear McsaProbe")
        command("kill @e[type=villager]")
        command("kill @e[type=item]")
        command(f"item replace entity McsaProbe hotbar.0 with emerald {emeralds}")
        client.pump(.5)
        offers = []
        for cost, name, count, maximum in [(64, "diamond", 64, 128), (1, "bread", 1, 0 if cheap_disabled else 128)]:
            offers.append('{buy:{id:"minecraft:emerald",count:' + str(cost)
                          + '},sell:{id:"minecraft:' + name + '",count:' + str(count)
                          + '},uses:0,maxUses:' + str(maximum)
                          + ',xp:0,rewardExp:0b,specialPrice:0,priceMultiplier:0f,demand:0}')
        command('summon villager 2 80 2 {Tags:["mcsa_trade_selection"],NoAI:1b,Invulnerable:1b,'
                'VillagerData:{type:"minecraft:plains",profession:"minecraft:farmer",level:2},'
                'Offers:{Recipes:[' + ','.join(offers) + ']}}')
        event = client.pump(5, lambda r: r["name"] == "spawn_entity" and r.get("entity_type") == villager_type)
        entity = event["entity"]
        client.pump(.3)
        opened()
        assert [o["uses"] for o in client.offers["offers"]] == [0, 0]
        assert [o["sell"] for o in client.offers["offers"]] == [stack("diamond", 64), stack("bread", 1)]

    def selection(label, index, emeralds, result):
        stage(label)
        client.send("select_trade", varint(index))
        client.pump(.4)
        inv = clone(client.inventory)
        report["selections"].append({"stage": label, "selected": index, "inventory": inv})
        assert inv["slots"][:3] == [stack("emerald", emeralds), None, result], (label, inv)
        return inv["revision"]

    def click(label, mode=0, forged=False, revision=None):
        stage(label)
        inv = client.inventory
        prediction = b"\0"
        inv["cursor"] = None
        if forged:
            prediction = b"\1" + varint(item_ids["diamond"]) + varint(64) + b"\0\0"
            inv["cursor"] = stack("diamond", 64)
        sent_revision = inv["revision"] if revision is None else revision
        body = varint(inv["window"]) + varint(sent_revision)
        body += struct.pack(">hb", 2, 0) + varint(mode) + b"\0" + prediction
        client.send("window_click", body)
        client.pump(.5)
        report["clicks"].append({"stage": label, "mode": mode,
                                "prediction": "diamond64" if forged else "empty",
                                "revision_sent": sent_revision,
                                "inventory": clone(client.inventory)})

    def checkpoint(label, payment, bread, diamonds, uses):
        stage(label)
        before = clone(client.inventory)
        assert before["slots"][0] == stack("emerald", payment), (label, "payment", before)
        assert before["slots"][1] is None
        goods = [s for s in before["slots"][3:] + [before["cursor"]] if s]
        actual = {name: sum(s["count"] for s in goods if s["item"] == name) for name in ("bread", "diamond")}
        assert actual == {"bread": bread, "diamond": diamonds}, (label, actual, before)
        close()
        opened()
        actual_uses = [offer["uses"] for offer in client.offers["offers"]]
        assert actual_uses == uses, (label, "uses", actual_uses)
        after = clone(client.inventory)
        all_after = [s for s in after["slots"][3:] + [after["cursor"]] if s]
        for name, count in [("emerald", payment), ("bread", bread), ("diamond", diamonds)]:
            assert sum(s["count"] for s in all_after if s["item"] == name) == count, (label, "reopen", name, after)
        report["checks"].append({"stage": label, "payment_remaining": payment, "bread": bread,
                                 "diamonds": diamonds, "uses": actual_uses,
                                 "before_reopen": before, "after_reopen": after,
                                 "offers_after_reopen": clone(client.offers)})

    market("selection")
    old_revision = selection("initial-expensive", 0, 64, stack("diamond", 64))
    for index, label in [(1, "cheap"), (1, "repeat-cheap"), (0, "back-expensive"), (1, "cheap-again")]:
        selection(label, index, 64, stack("diamond", 64) if index == 0 else stack("bread", 1))
    click("stale-diamond-prediction", forged=True, revision=old_revision)
    checkpoint("one-bread-for-one-emerald", 63, 1, 0, [0, 1])
    selection("cheap-after-reopen", 1, 63, stack("bread", 1))
    click("repeat-output")
    checkpoint("two-bread-for-two-emeralds", 62, 2, 0, [0, 2])

    market("reverse-selection")
    selection("reverse-cheap", 1, 64, stack("bread", 1))
    selection("reverse-expensive", 0, 64, stack("diamond", 64))
    click("expensive-output")
    checkpoint("diamonds-require-all-64-emeralds", 0, 0, 64, [1, 0])
    selection("expensive-with-empty-payment", 0, 0, None)
    click("repeat-empty-output")
    checkpoint("empty-output-cannot-duplicate-diamonds", 0, 0, 64, [1, 0])

    market("insufficient", emeralds=63, cheap_disabled=True)
    selection("expensive-with-63", 0, 63, None)
    click("forged-output-with-insufficient-payment", forged=True)
    checkpoint("insufficient-payment-unchanged", 63, 0, 0, [0, 0])

    market("shift")
    selection("shift-expensive-preview", 0, 64, stack("diamond", 64))
    selection("shift-cheap-selection", 1, 64, stack("bread", 1))
    click("shift-cheap-output", mode=1)
    checkpoint("shift-64-bread-for-64-emeralds", 0, 64, 0, [0, 64])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--jar", type=Path)
    source.add_argument("--test-binary", type=Path)
    parser.add_argument("--version", type=int, choices=[775, 777], required=True)
    parser.add_argument("--data", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--port", type=int)
    args = parser.parse_args()
    args.port = args.port or (26026 if args.test_binary else 26024)
    assert args.port == (26026 if args.test_binary else 26024), "only isolated probe ports allowed"
    sources = json.loads((args.data / "sources.json").read_text())
    for entry in sources["files"] + sources["baseline_files"]:
        assert hashlib.sha256((args.data / entry["file"]).read_bytes()).hexdigest() == entry["sha256"]
    protocol_path = args.data / ("protocol.json" if args.version == 777 else "protocol-26.1.json")
    protocol = json.loads(protocol_path.read_text())
    item_path = args.data / "items.json" if args.version == 777 else Path(__file__).resolve().parents[2] / "tools/data/items.json"
    names = {item["id"]: item["name"] for item in json.loads(item_path.read_text())}
    item_ids = {name: number for number, name in names.items()}
    entity_path = args.data / ("entities.json" if args.version == 777 else "entities-26.1.json")
    villager = next(e["id"] for e in json.loads(entity_path.read_text()) if e["name"] == "villager")
    report = {"protocol": args.version, "target": "core" if args.test_binary else "official",
              "checks": [], "selections": [], "clicks": [], "commands": [],
              "protocol_sha256": hashlib.sha256(protocol_path.read_bytes()).hexdigest(),
              "items_sha256": hashlib.sha256(item_path.read_bytes()).hexdigest(),
              "limits": "Own authored trades only; forged client prediction is not a modification of server preview. "
                        "No claim of crash safety or all trading mechanics."}
    if args.jar:
        expected = {775: "97ccd4c0ed3f81bbb7bfacddd1090b0c56f9bc51", 777: "33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c"}[args.version]
        assert hashlib.sha1(args.jar.read_bytes()).hexdigest() == expected
        report["jar_sha1"] = expected
    else:
        report["test_binary_sha256"] = hashlib.sha256(args.test_binary.read_bytes()).hexdigest()
    with socket.socket() as check:
        check.bind(("127.0.0.1", args.port))
    args.output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="mcsa-trade-selection-") as directory:
        root = Path(directory)
        if args.jar:
            (root / "eula.txt").write_text("eula=true\n")
            (root / "server.properties").write_text(
                f"server-ip=127.0.0.1\nserver-port={args.port}\nonline-mode=false\nwhite-list=false\n"
                "enforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\n"
                "level-seed=1\nview-distance=2\nsimulation-distance=2\ngamemode=creative\nspawn-protection=0\n")
            argv = ["/usr/lib/jvm/java-25-openjdk/bin/java", "-Xmx1G", "-jar", str(args.jar.resolve()), "nogui"]
            ready = "Done ("
        else:
            (root / "mcsa-gameplay-probe.marker").touch()
            argv = [str(args.test_binary.resolve()), "--ignored", "--exact", "gameplay_probe::serve_java_26_3_probe", "--nocapture"]
            ready = "MCSA gameplay probe ready"
        log = args.output / "server.log"
        with log.open("w") as output:
            server = subprocess.Popen(argv, cwd=root, stdin=subprocess.PIPE, stdout=output,
                                      stderr=subprocess.STDOUT, text=True)
            client = None

            def command(value):
                report["commands"].append(value)
                server.stdin.write(value + "\n")
                server.stdin.flush()

            try:
                deadline = time.monotonic() + 120
                while ready not in log.read_text():
                    assert server.poll() is None and time.monotonic() < deadline, "startup failed"
                    time.sleep(.1)
                client = connect(args.port, protocol, lambda s, p: Probe(s, p, args.version, names), args.version)
                client.pump(10, lambda r: r["name"] == "position")
                client.pump(.5)
                for cmd in ["gamerule minecraft:spawn_mobs false", "gamerule minecraft:random_tick_speed 0",
                            "fill -4 79 -4 4 79 4 stone", "tp McsaProbe 0.5 80 0.5", "gamemode survival McsaProbe"]:
                    command(cmd)
                    client.pump(.3)
                run_cases(client, command, villager, item_ids, report)
                assert "Перевод Java-пакета:" not in log.read_text(), "translation failure"
            except BaseException as error:
                report["error"] = str(error)
                raise
            finally:
                if client:
                    report["packets"] = client.observed
                    client.sock.close()
                if server.poll() is None:
                    try:
                        command("stop")
                    except (BrokenPipeError, OSError):
                        pass
                    server.stdin.close()
                    try:
                        server.wait(timeout=30)
                    except subprocess.TimeoutExpired:
                        server.kill()
                        server.wait()
                report["server_exit_code"] = server.returncode
                report["result"] = "PASS" if "error" not in report and server.returncode == 0 else "FAIL"
                (args.output / "result.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    assert report["result"] == "PASS", report.get("error", report["server_exit_code"])
    print(json.dumps({"result": report["result"], "protocol": args.version, "target": report["target"],
                      "checks": len(report["checks"]), "selections": len(report["selections"])}))


if __name__ == "__main__":
    main()
