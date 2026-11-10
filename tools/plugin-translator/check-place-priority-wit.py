#!/usr/bin/env python3
"""Реальная проверка WIT parser и C signatures, без запуска чужих плагинов."""
from pathlib import Path
import tempfile
import translate
from place_priority_wit import prepare

translate.bootstrap(toolchain=True)
with tempfile.TemporaryDirectory(prefix='priority-wit-', dir=translate.TARGET) as folder:
    root = Path(folder)
    wit = prepare(root / 'package')
    translate.run(translate.CHAIN / 'wit-bindgen-0.63.0-x86_64-linux/wit-bindgen', 'c', '--world', 'staged-place-bukkit-plugin', '--rename-world', 'plugin', '--out-dir', root / 'generated', wit)
    header = (root / 'generated/plugin.h').read_text()
    for name in ['exports_mcsa_priority_events_block_break_begin', 'exports_mcsa_priority_events_block_break_phase', 'exports_mcsa_priority_events_block_break_finish', 'exports_mcsa_api_events_on_block_break', 'exports_mcsa_place_priority_events_block_place_begin', 'exports_mcsa_place_priority_events_block_place_phase', 'exports_mcsa_place_priority_events_block_place_finish']:
        assert name in header, name
    assert 'can_build' in header and 'cancelled' in header
    print(header)
    # Реальный C compiler проверяет definitions adapter против generated signatures.
    translate.run(translate.CHAIN / 'wasi-sdk-34.0-x86_64-linux/bin/clang', '--target=wasm32-wasip2', '-fsyntax-only', '-Werror', '-DMCSA_STAGED_BLOCK_BREAK=1', '-DMCSA_STAGED_BLOCK_PLACE=1', '-I', root / 'generated', translate.ROOT / 'adapter.c')
    print('PLACE_PRIORITY_WIT_REAL_GENERATION_PASS')
