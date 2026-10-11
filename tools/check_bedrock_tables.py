"""Проверка готовых таблиц по разрешённым minecraft-data JSON версии 944.
Запуск из корня проекта: python3 tools/check_bedrock_tables.py.
Ничего не записывает и не запускает сервер.
"""
import sys
sys.dont_write_bytecode = True
import json
import struct
import make_bedrock_tables as source

source.prepare_lookup()
java = source.java_states()
states = json.loads((source.BEDROCK / "1.26.10/blockStates.json").read_text())
by_key = {source.bedrock_key(s["name"], {k: v["value"] for k, v in s["states"].items()}): i for i, s in enumerate(states)}
mapping = (source.OUT / "java_to_bedrock.bin").read_bytes()
assert len(mapping) == (max(java) + 1) * 2
for state, (name, props) in java.items():
    bname, bprops = source.LOOKUP[(name, tuple(sorted(props.items())))]
    expected = by_key[source.bedrock_key(bname, bprops)]
    assert struct.unpack_from("<H", mapping, state * 2)[0] == expected, (state, name, props)
items = json.loads((source.ROOT / "tools/data/items.json").read_text())
encoded = (source.OUT / "items_to_bedrock.bin").read_bytes()
missing = {item["name"] for item in items if struct.unpack_from("<h", encoded, item["id"] * 8)[0] == 0}
assert missing == {"air", "furnace_minecart", "spectral_arrow", "knowledge_book", "debug_stick"}, missing
bedrock = {i["name"]: i["id"] for i in json.loads((source.BEDROCK / "1.26.10/items.json").read_text())}
for item in items:
    alias = source.ITEM_ALIASES.get(item["name"])
    if alias:
        network, aux, _ = struct.unpack_from("<hHi", encoded, item["id"] * 8)
        assert (network, aux) == (bedrock[alias], source.item_metadata(item["name"])), item["name"]
print(f"Exact block mappings: {len(java)}; mapped items: {len(items) - len(missing)}; Java-only/air: {sorted(missing)}")
