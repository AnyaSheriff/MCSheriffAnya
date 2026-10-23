#!/usr/bin/env python3
# Обычные JAR: только публичное подмножество Bukkit, без зависимости от моста.
from pathlib import Path
import subprocess
import shutil
import sys
root = Path(__file__).resolve().parent
subprocess.run([sys.executable, str(root / 'translate.py'), '--build-sdk'], check=True)
for name in ('command', 'events', 'scheduler'):
    folder = root / 'fixtures' / name
    target = root / 'target/fixtures' / name
    shutil.rmtree(target, ignore_errors=True)
    target.mkdir(parents=True, exist_ok=True)
    subprocess.run(['javac', '-proc:none', '--release', '11', '-encoding', 'UTF-8', '-cp', str(root / 'target/mcsa-bukkit.jar'), '-d', str(target), *map(str, folder.glob('*.java'))], check=True)
    args = ['jar', 'cf', str(root / 'target' / (name + '.jar')), '-C', str(target), '.', '-C', str(folder), 'plugin.yml']
    if (folder / 'config.yml').exists():
        args += ['-C', str(folder), 'config.yml']
    subprocess.run(args, check=True)
