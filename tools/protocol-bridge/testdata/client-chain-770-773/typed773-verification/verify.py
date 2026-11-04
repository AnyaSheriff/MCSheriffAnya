"""Проверяет результаты продолжения; это не открывает допуск нижним клиентам."""
import hashlib
import json
from pathlib import Path
import re

here = Path(__file__).resolve().parent
base = here.parent
root = here.parents[4]
target = Path('/home/lenox/.local/share/mcsa-java-26.3-probe/s26-mv5-target')
def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
def read(path):
    return json.loads(path.read_text())
rust = (here/'rust-final.log').read_text()
rows = re.findall(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;', rust)
assert sum(int(a) for a,_ in rows) == 3166
assert sum(int(b) for _,b in rows) == 92
assert 'warning:' not in rust
assert 'Ran 237 tests' in (here/'python-final.log').read_text() and '\nOK\n' in (here/'python-final.log').read_text()
for name in ['clippy-workspace.log','clippy-root.log','build.log']:
    log = (here/name).read_text()
    assert 'Finished `release`' in log and 'warning:' not in log and 'error:' not in log, name
public = read(base/'typed773-public-final/result.json')
assert public['status'] == 'pass' and len(public['checks']) == 24
binary = target/'release/deps/mcsheriffanya-b5a369944de6d5e0'
binary_hash = sha(binary)
fixtures = {774:'f49c99cd5a0d5e41bf07a95be35ca2c18ddca5e1c5759d80afedd9dce872da41',
            776:'b315a5d410c666ec1b0bc4b8a38635ec38c765693decc46f15ed82752810bab3'}
evidence = {}
for client in [774,776]:
    for primary in [775,777]:
        cell = {}
        for kind,count in [('gui',6),('gameplay',11),('dimensions',5),('password',5)]:
            run = 'typed773-regression-v2' if kind == 'gui' and client == 774 else 'typed773-regression-v3'
            folder = base/run/f'{client}-{kind}{primary}'
            report = read(folder/'result.json')
            assert report['status'] == 'pass' and report['server_exit'] == 0
            assert report['binary_sha256'] == binary_hash
            assert report['primary'] == primary and report.get('client',report.get('protocol')) == client
            assert len(report['checks']) == count
            if kind == 'gui':
                assert report['payload_source_sha256'] == fixtures[client]
                assert len(report['payloads']) == 196 and all(p['post_pong'] for p in report['payloads'])
                assert report['payload_post_pong'] and len(report['statue_poses']) == 4
                assert [c['case'] for c in report['checks']] == ['cancel','drop-one','drop-stack','swap','release','jab']
            if kind == 'gameplay':
                assert report['first_server_exit'] == 0
            cell[kind] = dict(path=str(folder.relative_to(root)), checks=count, result_sha256=sha(folder/'result.json'))
        evidence[f'{client}/{primary}'] = cell
sources = [root/p for p in [
    'tools/protocol-bridge/src/client_773.rs','tools/protocol-bridge/src/client_773_tests.rs',
    'tools/protocol-bridge/src/generated_payload_773.rs','tools/protocol-bridge/generate_payload_773.py',
    'tools/protocol-bridge/payload-773-report.json','tools/protocol-bridge/src/adjacent.rs',
    'tools/protocol-bridge/src/adjacent_tests.rs','tools/protocol-bridge/src/component_776.rs',
    'tools/protocol-bridge/src/entities.rs','tools/protocol-bridge/src/items.rs',
    'tools/protocol-bridge/src/lib.rs','tools/protocol-bridge/src/nested_items.rs',
    'tools/protocol-bridge/src/recipes.rs','tools/protocol-bridge/live_client.py',
    'tools/protocol-bridge/prepare_registry_client.py',
    'tools/protocol-bridge/testdata/client-chain-770-773/CONTINUED.md','docs/multiversion-plan.md','docs/java-26.3-content.md']]

result = dict(client_translation_only=True, lower_admission_complete=False,
              public_admission={str(p):{str(c):('open' if c in (774,776) else 'closed') for c in [774,776,773,772,771,770]} for p in [775,777]},
              checks=dict(rust=3166,core=2908,startup=3,protocol=244,seedmap=11,ignored=92,python=237,
                          clippy_workspace='PASS -D warnings',clippy_root='PASS -D warnings',release='PASS',public=24),
              scope='Bounded adjacent773 inventory/recipe/particle/trade/metadata foundation only; full Play and live lower-client checklist unfinished',
              evidence=evidence, original_payloads=fixtures,
              binaries=dict(test_sha256=binary_hash,release_sha256=sha(target/'release/mcsheriffanya')),
              new_source_sha256={str(p.relative_to(root)):sha(p) for p in sources},
              retained_failures=['targeted-first.log: test-format compile error',
                                 'typed773-regression/774-gui775: Xvfb/NVIDIA before connection',
                                 'typed773-regression-final/774-gui775: obsolete774 bee-prefix fixture',
                                 'typed773-regression-v2/776-gui775: old GLFW override lacks preedit callback',
                                 'client773-prepare.log: initial Java-version assertion excluded1.21.10'],
              limitations=['773 attributes/anger/bee prefix and missing-content projections incomplete',
                           '773 full connection/chunk/Play integration and every live checklist still absent',
                           '772-770 inherit unfinished773; no older gameplay profiles or new admission gates',
                           'No fresh official-server run; existing observations reused'])
proof_dirs = [here, *(base/name for name in ['typed773-public-final','typed773-regression',
              'typed773-regression-final','typed773-regression-v2','typed773-regression-v3'])]
result['artifact_sha256'] = {str(p.relative_to(root)):sha(p) for d in proof_dirs for p in d.rglob('*') if p.is_file()}
(base/'typed773-verification.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result['checks']))
