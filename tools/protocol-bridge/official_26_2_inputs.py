"""Проверка входов обычных776-проб. Ничего не скачивает и не запускает."""
import hashlib
import json
import re
from urllib.parse import urlparse

from generate_registry_26_2 import CANDIDATE, DEFAULT_OUTPUT, Sources


def arguments(parser):
    from pathlib import Path
    parser.add_argument("--server-metadata", type=Path,
                        help="официальный version JSON26.2; обязателен для776")
    parser.add_argument("--registry-cache", type=Path,
                        help="кэш generate_registry_26_2; обязателен для776")


def load(args):
    if args.server_metadata is None or args.registry_cache is None:
        raise ValueError("776 требует --server-metadata и --registry-cache")
    body = args.server_metadata.read_bytes()
    metadata = json.loads(body)
    if metadata.get("id") != "26.2":
        raise ValueError("Ожидался официальный metadata26.2")
    download = metadata["downloads"]["server"]
    if not re.fullmatch(r"[0-9a-f]{40}", download["sha1"]):
        raise ValueError("Неверный SHA1 официального JAR")
    url = urlparse(download["url"])
    if url.scheme != "https" or url.hostname not in ("piston-data.mojang.com", "launcher.mojang.com"):
        raise ValueError("Неофициальный URL сервера")
    # Только контрольная сумма файла. ZIP/JAR не открывается как архив.
    with args.jar.open("rb") as source:
        if hashlib.file_digest(source, "sha1").hexdigest() != download["sha1"]:
            raise ValueError("JAR не соответствует официальному metadata")
    locked = json.loads(DEFAULT_OUTPUT.read_text())["sources"]
    sources = Sources(args.registry_cache, False, locked)
    protocol = sources.read(CANDIDATE, "data/pc/26.2/protocol.json")
    entities = sources.read(CANDIDATE, "data/pc/26.2/entities.json")
    provenance = {
        "metadata_sha256": hashlib.sha256(body).hexdigest(),
        "server_url": download["url"], "jar_sha1": download["sha1"],
        "candidate_sources": sources.manifest,
        "metadata_requirement": "version metadata must be obtained through the official launcher manifest",
    }
    return protocol, entities, provenance
