#!/usr/bin/env python3
# Два собственных плагина; существующие списки fixture не меняются.
from pathlib import Path
import shutil
import sys
from build_guard import run as guarded_run
root = Path(__file__).resolve().parent
guarded_run([sys.executable, str(root / 'translate.py'), '--build-sdk'], check=True)
for name in ('priority-a', 'priority-b'):
    folder = root / 'fixtures' / name
    target = root / 'target/fixtures' / name
    shutil.rmtree(target, ignore_errors=True)
    target.mkdir(parents=True, exist_ok=True)
    guarded_run(['javac', '-proc:none', '--release', '11', '-encoding', 'UTF-8', '-cp', str(root / 'target/mcsa-bukkit.jar'), '-d', str(target), *map(str, folder.glob('*.java'))], check=True)
    guarded_run(['jar', 'cf', str(root / 'target' / (name + '.jar')), '-C', str(target), '.', '-C', str(folder), 'plugin.yml'], check=True)
