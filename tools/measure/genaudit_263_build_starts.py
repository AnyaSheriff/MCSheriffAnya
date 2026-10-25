"""Build the optional adapter from our existing release libraries and own source.

Camp placement has no seedmap public export. Its unmodified pure placement
prefix is compiled in a temporary module; no src file is changed. The appended
box accessor exposes only the bounds of our own planned pieces.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile


def build(deps,output):
    repo=Path(__file__).resolve().parents[2]
    source=(repo/'src/world/abandoned_camp.rs').read_text()
    assert source.count('fn wool_stairs(')==1
    prefix=source.split('fn wool_stairs(',1)[0].rsplit('\n///',1)[0]
    assert 'pub fn site_in(' in prefix and 'fn assemble(' in prefix
    prefix=prefix.replace('use crate::blocks;','')
    prefix=prefix.replace('include_str!("abandoned_camp.txt")',
                          'include_str!('+json.dumps(str(repo/'src/world/abandoned_camp.txt'))+')')
    accessor='''
pub fn audit_boxes(site: &Site) -> Vec<[i32;6]> {
    site.pieces.iter().map(|piece| {
        let t=&library().templates[piece.template];
        let corners=[site.world(piece.offset.0,piece.offset.2),
                     site.world(piece.offset.0+t.width-1,piece.offset.2),
                     site.world(piece.offset.0,piece.offset.2+t.length-1),
                     site.world(piece.offset.0+t.width-1,piece.offset.2+t.length-1)];
        [corners.iter().map(|p|p.0).min().unwrap(),site.base_y+piece.offset.1,
         corners.iter().map(|p|p.1).min().unwrap(),corners.iter().map(|p|p.0).max().unwrap(),
         site.base_y+piece.offset.1+t.height-1,corners.iter().map(|p|p.1).max().unwrap()]
    }).collect()
}
'''
    deps=Path(deps).resolve();output=Path(output).resolve()
    output.parent.mkdir(parents=True,exist_ok=True)
    serde=max(deps.glob('libserde_json-*.rlib'),key=lambda f:f.stat().st_mtime)
    with tempfile.TemporaryDirectory(prefix='genaudit-camp-adapter-') as tmp:
        placement=Path(tmp)/'camp-placement.rs';placement.write_text(prefix+accessor)
        env=dict(os.environ,GENAUDIT_CAMP_SOURCE=str(placement))
        subprocess.run(['rustc','--edition=2024','-O','-C','codegen-units=4','--cfg','genaudit_camps',
                        '-L',f'dependency={deps}','--extern',f'seedmap_core={deps}/libseedmap_core.rlib',
                        '--extern',f'serde_json={serde}',str(repo/'tools/measure/genaudit_263_starts.rs'),
                        '-o',str(output)],env=env,check=True)
    return output


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--release-deps',required=True);p.add_argument('--output',required=True)
    a=p.parse_args();print(build(a.release_deps,a.output))
