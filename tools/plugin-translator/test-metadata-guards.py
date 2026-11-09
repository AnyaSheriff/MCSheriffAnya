#!/usr/bin/env python3
# Контракты настоящих Java-метаданных и статический gate четырёх API; не WASM runtime proof.
from pathlib import Path
import json
import os
import tempfile
from build_guard import run as guarded_run
root = Path(__file__).resolve().parent
cp = str(root / 'target/tool') + os.pathsep + str(root / 'target/lib/*')
sdk_cp = str(root / 'target/sdk') + os.pathsep + cp
for name in ('MetadataGuardContracts', 'MetadataGuardFixtures'):
    guarded_run(['javac', '-proc:none', '--release', '11', '-encoding', 'UTF-8', '-cp', sdk_cp, '-d', str(root / 'target/tool'), str(root / ('src/test/java/ru/mcsa/translator/' + name + '.java'))], check=True)
guarded_run(['java', '-cp', sdk_cp, 'ru.mcsa.translator.MetadataGuardContracts'], check=True)
with tempfile.TemporaryDirectory(dir=root / 'target') as folder:
    guarded_run(['java', '-cp', cp, 'ru.mcsa.translator.MetadataGuardFixtures', folder], check=True)
    for name in ('supported', 'overload', 'lore', 'legacy-name'):
        report = Path(folder) / 'report.json'
        result = guarded_run(['java', '-cp', cp, 'ru.mcsa.translator.Inspect', str(Path(folder) / (name + '.jar')), folder + '/out', str(root / 'target/mcsa-bukkit.jar'), str(report)], capture_output=True, text=True)
        diagnostics = json.loads(report.read_text())['diagnosticCount']
        if name == 'supported':
            assert result.returncode == 0 and diagnostics == 0, result.stderr
        else:
            assert result.returncode == 2 and diagnostics > 0 and 'не поддерживается org/bukkit/inventory/meta/ItemMeta.' in result.stderr, result.stderr
print('PASS: 4 metadata guards приняты; неверный overload, hasLore и legacy getter отклонены')
