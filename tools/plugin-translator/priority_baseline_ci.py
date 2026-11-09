#!/usr/bin/env python3
"""Изолированный настоящий legacy baseline; PASS не означает Bukkit conformance."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
BASE = "a76b0d755a9e4830b0759e7d72563cc667efeda5"
SPEC_SHA = "a53fa2c28485858d24123f0f873457ed66a15d4465d4396c0c6b245fafc91375"
TEST = "plugins::tests::translated_bukkit_legacy_priority_baseline"
TRANSLATOR = ROOT / "tools/plugin-translator"
OUT = ROOT / "priority-evidence"
CAPTURE = ROOT / "tmp/java-priority-baseline"

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def run(name, command):
    OUT.mkdir(exist_ok=True)
    log = OUT / (name + ".log")
    status = OUT / (name + ".status.json")
    if log.exists() or status.exists():
        raise ValueError("Нельзя переписывать прежнюю попытку: " + name)
    with log.open("w") as stream:
        print("RUN", command, flush=True)
        child = subprocess.Popen(command, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        for line in child.stdout:
            print(line, end="", flush=True)
            stream.write(line)
            stream.flush()
        code = child.wait()
    status.write_text(json.dumps({"command":command,"exit_code":code}, indent=2) + "\n")
    if code:
        raise subprocess.CalledProcessError(code, command)


def prepare():
    subprocess.run(["git","merge-base","--is-ancestor",BASE,"HEAD"], cwd=ROOT, check=True)
    spec = TRANSLATOR / "fixtures/priority-expected.json"
    assert sha(spec) == SPEC_SHA
    changed = subprocess.check_output(["git","diff","--name-only",BASE,"HEAD","--","src","wit"], cwd=ROOT, text=True).splitlines()
    assert changed == ["src/plugins/tests.rs"], changed
    assert not CAPTURE.exists(), "Требуется свежий CI checkout без прежних capture"
    OUT.mkdir(exist_ok=True)
    (OUT / "baseline-contract.json").write_text(json.dumps({"base":BASE,"spec_sha256":SPEC_SHA,"native_test":TEST,"production_src_wit_unchanged":True,"baseline_is_not_public_conformance":True},indent=2) + "\n")


def fixtures():
    run("fixtures-build", [sys.executable,"-B",str(TRANSLATOR/"build-priority-fixtures.py")])
    for name in ("priority-a", "priority-b"):
        jar = TRANSLATOR / "target" / (name + ".jar")
        run(name+"-inspect", [sys.executable,"-B",str(TRANSLATOR/"translate.py"),str(jar),"--check-only","--report",str(OUT/(name+"-inspect.json"))])
        run(name+"-translate", [sys.executable,"-B",str(TRANSLATOR/"translate.py"),str(jar),"--output",str(CAPTURE/"plugins"),"--report",str(OUT/(name+"-translation.json"))])


def verify():
    captures = list(CAPTURE.glob("legacy-capture-*.json"))
    assert len(captures) == 1, captures
    capture = captures[0]
    data = json.loads(capture.read_text())
    assert data["wasm_rust_runtime_executed"] is True
    assert data["public_contract_mismatches"] == 6
    assert data["public_verdict_mismatches"] == 2
    run("compare-legacy", [sys.executable,"-B",str(TRANSLATOR/"priority_trace.py"),str(capture),"--require-raw"])
    run("comparator-controls", [sys.executable,"-B",str(TRANSLATOR/"test-priority-trace.py"),str(capture)])
    public = subprocess.run([sys.executable,"-B",str(TRANSLATOR/"priority_trace.py"),str(capture),"--public","--require-raw"], cwd=ROOT, text=True, capture_output=True)
    (OUT/"compare-public.stdout.log").write_text(public.stdout)
    (OUT/"compare-public.stderr.log").write_text(public.stderr)
    print(public.stdout, end="", flush=True)
    assert public.returncode == 1, (public.returncode, public.stderr)
    result = json.loads(public.stdout)
    assert result["passed"] is False
    assert result["mismatches"] == ["a-b-order","a-b-cancel","a-b-uncancel","b-a-order","b-a-cancel","b-a-uncancel"]
    (OUT/"baseline-verification.json").write_text(json.dumps({"actual_native_capture":True,"cohorts":12,"solo_controls":6,"public_order_or_cancellation_mismatches":6,"public_verdict_mismatches":2,"negative_controls_rejected":10,"public_checker_exit_code":public.returncode,"public_checker_failed_for_exact_expected_cases":True,"bukkit_conformance":False},indent=2) + "\n")


def finish():
    OUT.mkdir(exist_ok=True)
    # Только наши выбранные исходники и собственные артефакты, без полного репозитория/target.
    selected = [ROOT/p for p in ["Cargo.toml","Cargo.lock","src/plugins/host.rs","src/plugins/tests.rs","src/plugins/api.rs","wit/mcsa-api.wit",".github/workflows/bukkit-priority-checks.yml"]]
    for folder in ["tools/plugin-translator/mcsa-bukkit/src/main/java","tools/plugin-translator/src/main/java","tools/plugin-translator/fixtures/priority-a","tools/plugin-translator/fixtures/priority-b"]:
        selected += [p for p in (ROOT/folder).rglob("*") if p.is_file()]
    selected += [p for p in TRANSLATOR.iterdir() if p.suffix in {".py",".c",".h",".xml"}]
    selected += [TRANSLATOR/"fixtures/priority-expected.json"]
    source_hashes = {}
    for path in sorted(set(selected)):
        if path.is_file():
            relative = path.relative_to(ROOT)
            dest = OUT/"source"/relative
            dest.parent.mkdir(parents=True,exist_ok=True)
            shutil.copyfile(path,dest)
            source_hashes[str(relative)] = sha(path)
    for path in [TRANSLATOR/"target/mcsa-bukkit.jar", TRANSLATOR/"target/priority-a.jar", TRANSLATOR/"target/priority-b.jar"]:
        if path.is_file():
            dest = OUT/"artifacts"/path.name
            dest.parent.mkdir(parents=True,exist_ok=True)
            shutil.copyfile(path,dest)
    if CAPTURE.exists():
        shutil.copytree(CAPTURE,OUT/"native",dirs_exist_ok=False)
    rust_hashes = {str(p.relative_to(ROOT)):sha(p) for p in sorted((ROOT/"src").rglob("*.rs"))}
    (OUT/"package-rust-sha256.json").write_text(json.dumps(rust_hashes,indent=2)+"\n")
    files = {str(p.relative_to(OUT)):{"sha256":sha(p),"bytes":p.stat().st_size} for p in sorted(OUT.rglob("*")) if p.is_file()}
    receipt = {"source":subprocess.check_output(["git","rev-parse","HEAD"],cwd=ROOT,text=True).strip(),"base":BASE,"job_status":os.environ.get("MCSA_JOB_STATUS"),"run_id":os.environ.get("GITHUB_RUN_ID"),"run_attempt":os.environ.get("GITHUB_RUN_ATTEMPT"),"files":files,"selected_source_sha256":source_hashes,"baseline_only_not_bukkit_conformance":True}
    (OUT/"receipt.json").write_text(json.dumps(receipt,indent=2)+"\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("stage", choices=["prepare","compile","fixtures","test","verify","finish"])
    stage = parser.parse_args().stage
    if stage == "prepare": prepare()
    elif stage == "compile": run("rust-compile", ["cargo","check","--release","--all-targets"])
    elif stage == "fixtures": fixtures()
    elif stage == "test": run("rust-native-test", ["cargo","test","--release","--lib",TEST,"--","--ignored","--exact","--nocapture","--test-threads=1"])
    elif stage == "verify": verify()
    elif stage == "finish": finish()

if __name__ == "__main__":
    main()
