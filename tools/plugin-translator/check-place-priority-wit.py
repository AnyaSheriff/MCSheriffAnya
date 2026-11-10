#!/usr/bin/env python3
"""Реальная проверка WIT parser и C signatures, без запуска чужих плагинов."""
from pathlib import Path
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
import translate
from place_priority_wit import prepare

out=translate.REPO / 'place-wit-evidence'
out.mkdir(exist_ok=False)
status='failure'
try:
    translate.bootstrap(toolchain=True)
    with tempfile.TemporaryDirectory(prefix='place-priority-wit-', dir=translate.TARGET) as folder:
        root = Path(folder)
        try:
            wit = prepare(root / 'package')
            translate.run(translate.CHAIN / 'wit-bindgen-0.63.0-x86_64-linux/wit-bindgen', 'c', '--world', 'staged-place-bukkit-plugin', '--rename-world', 'plugin', '--out-dir', root / 'generated', wit)
            header = (root / 'generated/plugin.h').read_text()
            for name in ['exports_mcsa_priority_events_block_break_begin', 'exports_mcsa_priority_events_block_break_phase', 'exports_mcsa_priority_events_block_break_finish', 'exports_mcsa_api_events_on_block_break', 'exports_mcsa_place_priority_events_block_place_begin', 'exports_mcsa_place_priority_events_block_place_phase', 'exports_mcsa_place_priority_events_block_place_finish']:
                assert name in header, name
            assert 'can_build' in header and 'cancelled' in header
            print(header,flush=True)
            # Реальный C compiler проверяет definitions adapter против generated signatures.
            translate.run(translate.CHAIN / 'wasi-sdk-34.0-x86_64-linux/bin/clang', '--target=wasm32-wasip2', '-fsyntax-only', '-Werror', '-DMCSA_STAGED_BLOCK_BREAK=1', '-DMCSA_STAGED_BLOCK_PLACE=1', '-I', root / 'generated', translate.ROOT / 'adapter.c')
            print('PLACE_PRIORITY_WIT_REAL_GENERATION_PASS',flush=True)
            status='success'
        finally:
            for name in ['generated','package']:
                if (root/name).exists():shutil.copytree(root/name,out/name)
finally:
    def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
    tracked=subprocess.check_output(['git','ls-files','tools/plugin-translator','wit','.github/workflows/bukkit-place-wit-checks.yml'],cwd=translate.REPO,text=True).splitlines()
    sources={}
    for name in tracked:
        p=translate.REPO/name
        if p.is_file():
            target=out/'source'/name;target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,target)
            sources[name]={'sha256':sha(p),'bytes':p.stat().st_size}
    tools={}
    for name in ['wit-bindgen-0.63.0-x86_64-linux/wit-bindgen','wasi-sdk-34.0-x86_64-linux/bin/clang','wasm-tools-1.262.0-x86_64-linux/wasm-tools']:
        p=translate.CHAIN/name
        if p.is_file():
            version=subprocess.run([str(p),'--version'],capture_output=True,text=True,check=False)
            tools[name]={'sha256':sha(p),'bytes':p.stat().st_size,'version':version.stdout,'version_stderr':version.stderr,'version_exit_code':version.returncode}
    (out/'receipt.json').write_text(json.dumps({'source':subprocess.check_output(['git','rev-parse','HEAD'],cwd=translate.REPO,text=True).strip(),'status':status,'run_id':os.environ.get('GITHUB_RUN_ID'),'attempt':os.environ.get('GITHUB_RUN_ATTEMPT'),'tool_urls':translate.PACKAGES,'tools':tools,'source_files':sources,'generated_files':{str(p.relative_to(out)):{'sha256':sha(p),'bytes':p.stat().st_size} for d in ['generated','package'] for p in sorted((out/d).rglob('*')) if p.is_file()},'native_rust_or_fixture_execution':False},indent=2)+'\n')
