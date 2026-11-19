#!/usr/bin/env python3
"""CI-only: собрать две собственные source-fixture в настоящие WASM-компоненты."""
import argparse
from pathlib import Path
import tempfile

from translate import CHAIN, REPO, build


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--toolchain', type=Path, default=CHAIN)
    parser.add_argument('--chat', action='store_true', help='Также собрать собственный пример отмены чата')
    parser.add_argument('--output', type=Path, default=REPO / 'tmp/mod-test/plugins')
    args = parser.parse_args()
    fixture = REPO / 'tools/mod-api/fixtures/welcome'
    for suffix in ['', '-b']:
        with tempfile.TemporaryDirectory(prefix='own-mod-fixture-') as temporary:
            project = Path(temporary)
            manifest = (fixture / 'mod.toml').read_text(encoding='utf-8').replace('mod-welcome', 'mod-welcome' + suffix)
            source = (fixture / 'Welcome.java').read_text(encoding='utf-8')
            if suffix:
                source = source.replace('mod:', 'mod-b:').replace('every(2,', 'every(3,')
            (project / 'mod.toml').write_text(manifest, encoding='utf-8')
            (project / 'Welcome.java').write_text(source, encoding='utf-8')
            print(build(project, args.output, args.toolchain), flush=True)

    if args.chat:
        print(build(REPO / 'tools/mod-api/fixtures/chat', args.output, args.toolchain), flush=True)


if __name__ == '__main__':
    main()
