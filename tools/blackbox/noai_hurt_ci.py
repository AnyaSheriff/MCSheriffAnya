#!/usr/bin/env python3
"""CI-only fixed hurt/control observations; strict comparison remains diagnostic."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import combat_ci as common
from noai_hurt_audit import SCENARIOS, audit

ROOT=common.ROOT
PIN=ROOT/'tools/blackbox/ci/noai-hurt-runtime-pin.json'
OWN_FILES=['tools/blackbox/noai_hurt_ci.py','tools/blackbox/noai_hurt_audit.py',
           'tools/blackbox/test_noai_hurt.py','tools/blackbox/ci/noai-hurt-runtime-pin.json',
           '.github/workflows/noai-hurt-observe.yml','tools/blackbox/noai_ambient_audit.py',
           'tools/blackbox/ci/noai-hurt-observe.md']


def verdict(strict, scoped, record_exit, analyze_exit):
    if (record_exit or analyze_exit or strict['errors'] or
            any(r.get('warnings') or any(r['decode_errors']) for r in strict['comparisons'])):
        return 'infrastructure-failed'
    return scoped['status']


def execute(artifact, selection, work, output, java):
    if work.exists() or (output/'ci-receipt.json').exists():
        raise ValueError('fresh work/output required')
    work.mkdir(parents=True);data=work/'data';data.mkdir();results=work/'recordings'
    receipt={'status':'infrastructure-failed','selection':selection,'scenarios':SCENARIOS,
             'source_head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
             'commands':[], 'series_policy':'Exactly eight fixed pairs per side, 400 ticks per pair, one fist attack at +100; no retries to success.'}
    def interrupted(sig, frame):
        raise InterruptedError(f'CI interrupted: {sig}')
    previous={sig:signal.signal(sig,interrupted) for sig in (signal.SIGTERM,signal.SIGINT)}
    try:
        common.snapshot_sources(output,SCENARIOS)
        for name in OWN_FILES:
            source=common.checked_file(ROOT/name);target=output/'source'/name
            target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(source,target)
        runtime,build_receipt=common.verify_artifact(artifact,selection)
        shutil.copyfile(build_receipt,output/'build-receipt.json')
        binary=work/'mcsheriffanya';shutil.copyfile(runtime,binary);binary.chmod(0o700)
        receipt['java_version']=subprocess.check_output([java,'-version'],stderr=subprocess.STDOUT,text=True,timeout=10)
        if not re.search(r'version "25(?:[.\"]|$)',receipt['java_version']):
            raise ValueError('Java25 required')
        sources=json.loads(common.SOURCES.read_text());jar=work/'server.jar'
        common.fetch(sources['official_jar'],jar)
        for entry in sources['json_files']:
            common.fetch(entry,data/entry['file'])
        command=[sys.executable,str(ROOT/'tools/blackbox/record777.py'),'--data',str(data),
                 '--jar',str(jar),'--binary',str(binary),'--output',str(results),'--java',java,
                 '--max-seconds','900','--version','777','--isolate-vitals',*SCENARIOS]
        receipt['commands'].append(command);common.write_json(output/'ci-receipt.json',receipt)
        receipt['record_exit']=common.run_tool(command,output/'recorder.log',960)
        if receipt['record_exit']:
            receipt['early_watchdog_errors']=common.cleanup_manifest(results)
        command=[sys.executable,str(ROOT/'tools/blackbox/analyze777.py'),str(results),'--protocol',str(data/'protocol.json')]
        receipt['commands'].append(command)
        receipt['analyze_exit']=common.run_tool(command,output/'analyzer.log',180)
        strict=common.audit_results(results,SCENARIOS,selection['pin']['runtime_sha256'])
        receipt['strict_diagnostic']=strict
        common.write_json(output/'strict-diagnostic.json',strict)
        scoped=audit(results,data)
        common.write_json(results/'noai-hurt-observations.json',scoped)
        receipt['status']=verdict(strict,scoped,receipt['record_exit'],receipt['analyze_exit'])
        receipt['scope']=scoped['claim']
    except Exception as error:
        receipt['status']='infrastructure-failed';receipt['error']=type(error).__name__+': '+str(error)
    finally:
        for sig in previous:
            signal.signal(sig,signal.SIG_IGN)
        try:
            receipt['watchdog_errors']=common.cleanup_manifest(results)
            receipt['final_ports_free']=common.ports_free(common.PORTS)
            if receipt['watchdog_errors'] or not all(receipt['final_ports_free'].values()):
                receipt['status']='infrastructure-failed'
        except Exception as error:
            receipt['status']='infrastructure-failed';receipt['cleanup_error']=str(error)
        common.write_json(output/'ci-receipt.json',receipt)
        try:
            common.collect(output,results,data)
        except Exception as error:
            receipt['status']='infrastructure-failed';receipt['collection_error']=str(error)
            common.write_json(output/'ci-receipt.json',receipt)
        for sig,handler in previous.items():
            signal.signal(sig,handler)
    return receipt


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('mode',choices=('pin','run'))
    p.add_argument('--output',type=Path,required=True);p.add_argument('--artifact',type=Path)
    p.add_argument('--work',type=Path);p.add_argument('--java',default='java');a=p.parse_args()
    if os.environ.get('GITHUB_ACTIONS')!='true':
        p.error('CI-only entrypoint')
    a.output.mkdir(parents=True,exist_ok=True)
    try:
        if a.mode=='pin':
            pin=json.loads(PIN.read_text())
            result=common.preflight(pin,a.output)
            with open(os.environ['GITHUB_OUTPUT'],'a') as stream:
                stream.write(f'build_run_id={pin["build_run_id"]}\nartifact_name={result["artifact_name"]}\n')
        else:
            if not a.artifact or not a.work:
                p.error('run requires --artifact and --work')
            result=execute(a.artifact,json.loads((a.output/'build-selection.json').read_text()),a.work.resolve(),a.output.resolve(),a.java)
            if result['status']!='observations-valid':
                raise SystemExit(1)
    except Exception as error:
        common.write_json(a.output/'entrypoint-error.json',{'status':'infrastructure-failed','error':type(error).__name__+': '+str(error)})
        raise SystemExit(1)


if __name__=='__main__':
    main()
