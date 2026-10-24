#!/usr/bin/env python3
# Нативная регрессия платформенного форматирования; WASI проверяет fixture bot.
from pathlib import Path
from build_guard import run
root = Path(__file__).resolve().parent
output = root / 'target/decimal-runtime-test'
output.parent.mkdir(exist_ok=True)
run(['nice', '-n', '19', 'cc', '-std=c11', '-O2', '-fwrapv', '-Wall', '-Wextra', '-Werror', '-I', root, root/'src/test/decimal-runtime.c', '-lm', '-o', output], check=True)
run(['nice', '-n', '19', output], check=True)
