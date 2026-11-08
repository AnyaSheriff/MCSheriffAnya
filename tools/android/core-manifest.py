"""Производная сборка cdylib в отдельной копии корня, без изменения desktop Cargo."""
import argparse
from pathlib import Path
import tomllib


def convert(text):
    original=tomllib.loads(text)
    assert 'lib' not in original and 'bin' not in original
    assert 'android-core' in original['features']
    lines=text.splitlines(keepends=True)
    result=[];package=False
    for line in lines:
        if line.startswith('['):package=line.strip()=='[package]'
        if package and line.startswith('default-run'):continue
        result.append(line)
        if line.strip()=='[package]':result.append('autobins = false\n')
    output=''.join(result)+'\n[lib]\nname = "mcsa_android_core"\npath = "src/main.rs"\ncrate-type = ["cdylib"]\n'
    changed=tomllib.loads(output)
    expected=dict(original);expected['package']=dict(original['package']);expected['package'].pop('default-run',None);expected['package']['autobins']=False
    expected['lib']={'name':'mcsa_android_core','path':'src/main.rs','crate-type':['cdylib']}
    assert changed==expected,'Unexpected dependency/profile/workspace mutation'
    return output


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('root',type=Path);args=parser.parse_args()
    path=args.root/'Cargo.toml';source=path.read_text();output=convert(source)
    backup=args.root/'Cargo.android-original.toml';assert not backup.exists();backup.write_text(source);path.write_text(output)
