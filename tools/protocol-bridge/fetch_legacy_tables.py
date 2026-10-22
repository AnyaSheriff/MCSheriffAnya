#!/usr/bin/env python3
"""Получить только закреплённые JSON1.12.2, без исходных реализаций."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import urllib.request

ALLOWED={'dataPaths.json','protocol.json','version.json','items.json','blocks.json','enchantments.json','legacy.json'}
URL=re.compile(r'https://raw\.githubusercontent\.com/PrismarineJS/minecraft-data/[0-9a-f]{40}/data/(?:dataPaths\.json|pc/(?:1\.12(?:\.2)?|common)/[A-Za-z]+\.json)\Z')
def main():
    p=argparse.ArgumentParser();p.add_argument('--directory',type=Path,required=True);a=p.parse_args()
    source=Path(__file__).with_name('legacy-sources-1.12.2.json');manifest=json.loads(source.read_text())
    assert {e['file'] for e in manifest['files']}==ALLOWED and len(manifest['files'])==len(ALLOWED)
    a.directory.mkdir(parents=True,exist_ok=True)
    for e in manifest['files']:
        assert URL.fullmatch(e['url']) and 0<e['bytes']<=8*1024*1024
        path=a.directory/e['file']
        if path.exists() and hashlib.sha256(path.read_bytes()).hexdigest()==e['sha256']:continue
        with urllib.request.urlopen(e['url'],timeout=30)as response:data=response.read(8*1024*1024+1)
        assert len(data)==e['bytes'] and hashlib.sha256(data).hexdigest()==e['sha256']
        json.loads(data);path.write_bytes(data)
    (a.directory/'sources.json').write_text(source.read_text())
    print('Verified',len(ALLOWED),'JSON files for1.12.2')
if __name__=='__main__':main()
