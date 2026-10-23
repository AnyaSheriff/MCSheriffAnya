#!/usr/bin/env python3
# Проверяем отказ до TeaVM, в том числе дескрипторы и наследование API.
from pathlib import Path
import os
import subprocess
import sys
import tempfile
root = Path(__file__).resolve().parent
subprocess.run([sys.executable, str(root / 'build-fixtures.py')], check=True)
cp = str(root / 'target/tool') + os.pathsep + str(root / 'target/lib/*')
subprocess.run(['javac', '-proc:none', '--release', '11', '-cp', cp, '-d', str(root / 'target/tool'), str(root / 'src/test/java/ru/mcsa/translator/InspectorFixtures.java')], check=True)
with tempfile.TemporaryDirectory(dir=root / 'target') as folder:
    subprocess.run(['java', '-cp', cp, 'ru.mcsa.translator.InspectorFixtures', folder], check=True)
    for file in sorted(Path(folder).glob('*.jar')):
        result = subprocess.run(['java', '-cp', cp, 'ru.mcsa.translator.Inspect', str(file), folder + '/out', str(root / 'target/mcsa-bukkit.jar')], capture_output=True, text=True)
        assert result.returncode == 2 and 'ОТКАЗ:' in result.stderr, (file.name, result.stdout, result.stderr)
        print('PASS: отказ', file.stem)
    for name in ('command', 'events', 'scheduler'):
        subprocess.run(['java', '-cp', cp, 'ru.mcsa.translator.Inspect', str(root / 'target' / (name + '.jar')), folder + '/out', str(root / 'target/mcsa-bukkit.jar')], check=True)
print('PASS: 9 отказов, 3 поддерживаемых JAR; входной код на JVM не запускался')
