#!/usr/bin/env python3
"""Строгая проверка собственного BossBar API до компиляции TeaVM."""
from pathlib import Path
import argparse
import json
import os
import shutil
import tempfile
from build_guard import run as guarded_run

root = Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--output', type=Path)
args = parser.parse_args()
if args.output:
    args.output.mkdir(parents=True, exist_ok=True)
cp = str(root / 'target/tool') + os.pathsep + str(root / 'target/lib/*')
guarded_run(['javac', '-proc:none', '--release', '11', '-encoding', 'UTF-8', '-cp', cp,
             '-d', str(root / 'target/tool'),
             str(root / 'src/test/java/ru/mcsa/translator/BossBarFixtures.java')], check=True)
with tempfile.TemporaryDirectory(dir=root / 'target') as folder:
    guarded_run(['java', '-cp', cp, 'ru.mcsa.translator.BossBarFixtures', folder], check=True)
    failures = {
        'flag-field': 'не поддерживается org/bukkit/boss/BarFlag.DARKEN_SKY',
        'flag-method': 'не поддерживается org/bukkit/boss/BossBar.addFlag',
        'keyed': 'не поддерживается org/bukkit/Bukkit.createBossBar',
        'wrong-overload': 'не поддерживается org/bukkit/boss/BossBar.getProgress()F',
    }
    for name in ('supported', *failures):
        report = Path(folder) / (name + '-report.json')
        result = guarded_run(['java', '-cp', cp, 'ru.mcsa.translator.Inspect',
                              str(Path(folder) / (name + '.jar')), folder + '/out',
                              str(root / 'target/mcsa-bukkit.jar'), str(report)],
                             capture_output=True, text=True)
        diagnostics = json.loads(report.read_text())['diagnosticCount']
        if name == 'supported':
            assert result.returncode == 0 and diagnostics == 0, result.stderr
        else:
            assert result.returncode == 2 and diagnostics > 0, result.stderr
            assert failures[name] in result.stderr, result.stderr
        print(name + ': exit=' + str(result.returncode) + ', diagnostics=' + str(diagnostics))
        if args.output:
            shutil.copyfile(report, args.output / report.name)
            (args.output / (name + '-stderr.log')).write_text(result.stderr)
print('PASS: BossBar API принят; flag field/method, KeyedBossBar и неверный overload отклонены')
