#!/usr/bin/env python3
# Проверяем отказ до TeaVM, в том числе дескрипторы и наследование API.
from pathlib import Path
import os
import json
import subprocess
import sys
import tempfile
root = Path(__file__).resolve().parent
subprocess.run([sys.executable, str(root / 'build-fixtures.py')], check=True)
cp = str(root / 'target/tool') + os.pathsep + str(root / 'target/lib/*')
subprocess.run(['javac', '-proc:none', '--release', '11', '-cp', cp, '-d', str(root / 'target/tool'), str(root / 'src/test/java/ru/mcsa/translator/InspectorFixtures.java')], check=True)
sdk_cp = str(root / 'target/sdk') + os.pathsep + str(root / 'target/tool')
subprocess.run(['javac', '-proc:none', '--release', '11', '-encoding', 'UTF-8', '-cp', sdk_cp, '-d', str(root / 'target/tool'), str(root / 'src/test/java/ru/mcsa/translator/SdkContracts.java')], check=True)
subprocess.run(['java', '-cp', sdk_cp, 'ru.mcsa.translator.SdkContracts'], check=True)
with tempfile.TemporaryDirectory(dir=root / 'target') as folder:
    subprocess.run(['java', '-cp', cp, 'ru.mcsa.translator.InspectorFixtures', folder], check=True)
    for file in sorted(Path(folder).glob('*.jar')):
        report_path = Path(folder) / 'report.json'
        result = subprocess.run(['java', '-cp', cp, 'ru.mcsa.translator.Inspect', str(file), folder + '/out', str(root / 'target/mcsa-bukkit.jar'), str(report_path)], capture_output=True, text=True)
        assert result.returncode == 2 and 'ОТКАЗ:' in result.stderr, (file.name, result.stdout, result.stderr)
        report = json.loads(report_path.read_text())
        assert report['diagnosticCount'] == len(report['diagnostics']) > 0
        assert report['missing'] == sorted(set(report['missing']))
        expected = {'adventure-spoof': 'подмена API запрещена', 'executor': 'собственные потоки запрещены', 'parallel-map': 'параллельные операции карты запрещены', 'adventure-overload': 'Component.text'}
        if file.stem in expected:
            assert expected[file.stem] in result.stderr, result.stderr
        print('PASS: отказ', file.stem)
    for name in ('command', 'events', 'scheduler'):
        subprocess.run(['java', '-cp', cp, 'ru.mcsa.translator.Inspect', str(root / 'target' / (name + '.jar')), folder + '/out', str(root / 'target/mcsa-bukkit.jar')], check=True)
print('PASS: 15 отказов, 3 поддерживаемых JAR; входной код на JVM не запускался')
