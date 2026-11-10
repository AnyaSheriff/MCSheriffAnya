#!/usr/bin/env python3
"""Реальная проверка WIT parser и C signatures, без запуска чужих плагинов."""
from pathlib import Path
import tempfile
import translate
from priority_wit import prepare

translate.bootstrap(toolchain=True)
with tempfile.TemporaryDirectory(prefix='priority-wit-', dir=translate.TARGET) as folder:
    root = Path(folder)
    wit = prepare(root / 'package')
    translate.run(translate.CHAIN / 'wit-bindgen-0.63.0-x86_64-linux/wit-bindgen', 'c', '--world', 'staged-bukkit-plugin', '--rename-world', 'plugin', '--out-dir', root / 'generated', wit)
    header = (root / 'generated/plugin.h').read_text()
    for name in ['exports_mcsa_priority_events_block_break_begin', 'exports_mcsa_priority_events_block_break_phase', 'exports_mcsa_priority_events_block_break_finish', 'exports_mcsa_api_events_on_block_break']:
        assert name in header, name
    print(header)
    print('PRIORITY_WIT_REAL_GENERATION_PASS')
