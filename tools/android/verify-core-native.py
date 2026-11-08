"""Сверка собственных загрузчиков/полных ядер, двух разных ревизий и исходной сборки."""
import importlib.util
import json
from pathlib import Path
import re
import sys
import tomllib

spec=importlib.util.spec_from_file_location('native',Path(__file__).with_name('verify-native.py'));native=importlib.util.module_from_spec(spec);spec.loader.exec_module(native)


def validate_log(path,library):
    text=path.read_text();assert 'GLIBC_' not in text and '(TEXTREL)' not in text
    needed=re.findall(r'\(NEEDED\).*?\[([^\]]+)\]',text)
    assert 'libc.so' in needed and set(needed)<={'libc.so','libm.so','libdl.so','liblog.so','libandroid.so'}
    if library:
        for name in ['mcsa_core_api_v1','mcsa_core_main_v1','mcsa_core_build_id_v1']:assert name in text,name


def main(root):
    head=(root/'source-head.txt').read_text().strip();assert re.fullmatch('[0-9a-f]{40}',head)
    original=tomllib.loads((root/'Cargo.original.toml').read_text());derived=tomllib.loads((root/'Cargo.core.toml').read_text())
    for section in ['dependencies','dev-dependencies','target','profile','workspace','features','lints']:assert original[section]==derived[section],section
    assert derived['lib']=={'name':'mcsa_android_core','path':'src/main.rs','crate-type':['cdylib']} and not derived['package']['autobins']
    result={'format':2,'source_head':head,'api':24,'host_api':1,'core_revision':1,'runtimes':{},'updates':{'2':{'build_id':head+'-r2','runtimes':{}}}}
    for abi,machine in [('armeabi-v7a',40),('arm64-v8a',183),('x86_64',62)]:
        host=native.inspect((root/abi/'libmcsa.so').read_bytes(),machine);validate_log(root/f'elf-host-{abi}.txt',False)
        core=native.inspect((root/abi/'libmcsa_core.so').read_bytes(),machine,library=True);validate_log(root/f'elf-core-{abi}-r1.txt',True)
        newer=native.inspect((root/'updates/2'/abi/'libmcsa_core.so').read_bytes(),machine,library=True);validate_log(root/f'elf-core-{abi}-r2.txt',True)
        assert core['sha256']!=newer['sha256'],'Revisions must be distinct native builds, not renamed copies'
        assert host['bytes']<1_000_000 and core['bytes']>1_000_000 and newer['bytes']>1_000_000
        features='default, plugins enabled' if abi=='arm64-v8a' else 'no-default, plugins disabled'
        result['runtimes'][abi]={'host':host,'core':core,'features':features,'build_id':head+'-r1'}
        result['updates']['2']['runtimes'][abi]=newer
    (root/'receipt.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))


if __name__=='__main__':main(Path(sys.argv[1]))
