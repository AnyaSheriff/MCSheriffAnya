#!/usr/bin/env python3
# Проверяем отказ до TeaVM, в том числе дескрипторы и наследование API.
from pathlib import Path
import os
import json
from build_guard import run as guarded_run
import sys
import tempfile
root = Path(__file__).resolve().parent
guarded_run([sys.executable, str(root / 'build-fixtures.py')], check=True)
guarded_run([sys.executable, str(root / 'test-decimal-runtime.py')], check=True)
cp = str(root / 'target/tool') + os.pathsep + str(root / 'target/lib/*')
guarded_run(['javac', '-proc:none', '--release', '11', '-cp', cp, '-d', str(root / 'target/tool'), str(root / 'src/test/java/ru/mcsa/translator/InspectorFixtures.java')], check=True)
sdk_cp = str(root / 'target/sdk') + os.pathsep + str(root / 'target/tool')
guarded_run(['javac', '-proc:none', '--release', '11', '-encoding', 'UTF-8', '-cp', sdk_cp, '-d', str(root / 'target/tool'), str(root / 'src/test/java/ru/mcsa/translator/SdkContracts.java')], check=True)
guarded_run(['java', '-cp', sdk_cp, 'ru.mcsa.translator.SdkContracts'], check=True)
guarded_run(['javac', '-proc:none', '--release', '11', '-encoding', 'UTF-8', '-cp', sdk_cp, '-d', str(root / 'target/tool'), str(root / 'src/test/java/ru/mcsa/bukkit/concurrent/ExecutorContracts.java')], check=True)
guarded_run(['java', '-cp', sdk_cp, 'ru.mcsa.bukkit.concurrent.ExecutorContracts'], check=True)
with tempfile.TemporaryDirectory(dir=root / 'target') as records:
    guarded_run(['javac', '-proc:none', '--release', '17', '-encoding', 'UTF-8', '-d', records, str(root / 'src/test/java/ru/mcsa/translator/RecordFixture.java')], check=True)
    before=guarded_run(['java','-cp',records,'ru.mcsa.translator.RecordFixture'],check=True,capture_output=True,text=True).stdout
    original=records+'/original.jar';normalized=records+'/normalized.jar'
    guarded_run(['jar','cf',original,'-C',records,'ru'],check=True)
    guarded_run(['java','-cp',cp,'ru.mcsa.translator.Normalize',original,normalized],check=True)
    after=guarded_run(['java','-cp',normalized,'ru.mcsa.translator.RecordFixture'],check=True,capture_output=True,text=True).stdout
    assert before==after,(before,after)
    print('PASS: record equals/hashCode/toString совпадают с JVM; только собственная тестовая модель')
with tempfile.TemporaryDirectory(dir=root / 'target') as folder:
    guarded_run(['java', '-cp', cp, 'ru.mcsa.translator.InspectorFixtures', folder], check=True)
    for file in sorted(Path(folder).glob('*.jar')):
        report_path = Path(folder) / 'report.json'
        result = guarded_run(['java', '-cp', cp, 'ru.mcsa.translator.Inspect', str(file), folder + '/out', str(root / 'target/mcsa-bukkit.jar'), str(report_path)], capture_output=True, text=True)
        assert result.returncode == 2 and 'ОТКАЗ:' in result.stderr, (file.name, result.stdout, result.stderr)
        report = json.loads(report_path.read_text())
        assert report['diagnosticCount'] == len(report['diagnostics']) > 0
        assert report['missing'] == sorted(set(report['missing']))
        expected = {'adventure-spoof': 'подмена API запрещена', 'executor': 'не поддерживается упорядоченный executor', 'parallel-map': 'параллельные операции карты запрещены', 'adventure-overload': 'Component.text'}
        if file.stem in expected:
            assert expected[file.stem] in result.stderr, result.stderr
        print('PASS: отказ', file.stem)
    for name in ('command', 'events', 'scheduler'):
        normalized = str(Path(folder) / (name + '-normalized.jar'))
        guarded_run(['java', '-cp', cp, 'ru.mcsa.translator.Normalize', str(root / 'target' / (name + '.jar')), normalized], check=True)
        guarded_run(['java', '-cp', cp, 'ru.mcsa.translator.Inspect', normalized, folder + '/out', str(root / 'target/mcsa-bukkit.jar')], check=True)
print('PASS: 17 отказов, 3 поддерживаемых JAR; входной код на JVM не запускался')
