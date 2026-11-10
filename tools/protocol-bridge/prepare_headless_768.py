#!/usr/bin/env python3
"""Только закреплённые metadataJSON, разрешённыйprotocolJSON и opaque serverJAR."""
import argparse
import hashlib
import json
from pathlib import Path
import urllib.request
from capture_native_768 import metadata, opaque_sha, SERVER_SHA1
from generate_client_chain import locked


def download(url, path, sha1, size=None):
    with urllib.request.urlopen(url,timeout=90) as response:
        raw=response.read(64*1024*1024+1)
    if len(raw)>64*1024*1024 or (size is not None and len(raw)!=size) or hashlib.sha1(raw).hexdigest()!=sha1:
        raise ValueError('pinned input SHA1/size mismatch')
    path.write_bytes(raw)


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
    a.output.mkdir(parents=True,exist_ok=False)
    tools=Path(__file__).resolve().parent
    pin=json.loads((tools/'capture-768-pins.json').read_text())
    download(pin['entry']['url'],a.output/'version.json',pin['entry']['sha1'])
    meta=metadata(a.output/'version.json')
    server=meta['downloads']['server']
    download(server['url'],a.output/'server.jar',SERVER_SHA1,server['size'])
    opaque_sha(a.output/'server.jar',SERVER_SHA1)
    record=json.loads((tools/'client-chain-sources.json').read_text())['versions']['768']['files']['protocol']
    locked(a.output,'protocol.json',record,True)
    rows={p.name:{'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()}
        for p in a.output.iterdir() if p.is_file()}
    (a.output/'inputs.json').write_text(json.dumps({'files':rows,'client_downloaded':False,'executed':False,'jar_opened':False},indent=2)+'\n')

if __name__=='__main__':main()
