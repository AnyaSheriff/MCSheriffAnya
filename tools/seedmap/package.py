"""Упаковка скриптов и WASM без fetch, модулей, CDN и сервера."""
from pathlib import Path
import base64
import json
import shutil

root = Path(__file__).resolve().parent
dist = root / "dist"
glue = (dist / "seedmap.js").read_text()
wasm = base64.b64encode((dist / "seedmap_bg.wasm").read_bytes()).decode("ascii")
worker = glue + "\nconst WASM_BASE64 = " + json.dumps(wasm) + ";\n" + (root / "worker.js").read_text()
(dist / "runtime.js").write_text("window.SEEDMAP_WORKER = " + json.dumps(worker, ensure_ascii=False) + ";\n")
for name in ("index.html", "app.js"):
    shutil.copyfile(root / name, dist / name)
