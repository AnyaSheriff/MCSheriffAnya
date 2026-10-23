#!/usr/bin/env python3
"""Проверка и перевод Bukkit JAR в компонент MCSheriffAnya."""
import argparse
import concurrent.futures
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parent.parent
TARGET = ROOT / "target"
CHAIN = ROOT / ".toolchain"
PACKAGES = {
    "wasi-sdk-34.0-x86_64-linux": "https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-34/wasi-sdk-34.0-x86_64-linux.tar.gz",
    "wasm-tools-1.262.0-x86_64-linux": "https://github.com/bytecodealliance/wasm-tools/releases/download/v1.262.0/wasm-tools-1.262.0-x86_64-linux.tar.gz",
    "wit-bindgen-0.63.0-x86_64-linux": "https://github.com/bytecodealliance/wit-bindgen/releases/download/v0.63.0/wit-bindgen-0.63.0-x86_64-linux.tar.gz",
}

def run(*args, **kwargs):
    subprocess.run([str(a) for a in args], check=True, **kwargs)


def bootstrap():
    TARGET.mkdir(exist_ok=True)
    if not (TARGET / "lib/teavm-tooling-0.12.3.jar").exists():
        run("mvn", "-q", "-f", ROOT / "pom.xml", "dependency:copy-dependencies", "-DoutputDirectory=target/lib")
    def fetch(item):
        name, url = item
        if (CHAIN / name).is_dir():
            return
        CHAIN.mkdir(exist_ok=True)
        archive = CHAIN / (name + ".tar.gz")
        print("Загрузка инструмента:", name, flush=True)
        urllib.request.urlretrieve(url, archive)
        with tarfile.open(archive) as tar:
            tar.extractall(CHAIN, filter="data")
        archive.unlink()
    with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
        list(pool.map(fetch, PACKAGES.items()))
    cp = str(TARGET / "lib/*")
    for source, dest in [(ROOT / "mcsa-bukkit/src/main/java", TARGET / "sdk"), (ROOT / "src/main/java", TARGET / "tool")]:
        shutil.rmtree(dest, ignore_errors=True)
        dest.mkdir(exist_ok=True)
        run("javac", "-proc:none", "--release", "11", "-encoding", "UTF-8", "-cp", cp, "-d", dest, *sorted(source.rglob("*.java")))
    run("jar", "cf", TARGET / "mcsa-bukkit.jar", "-C", TARGET / "sdk", ".")


def port_runtime(c):
    # Только перенос платформенной части TeaVM; код плагина не исполняется на JVM.
    p = c / "definitions.h"
    p.write_text(p.read_text().replace("#ifdef __GNUC__", "#if defined(__GNUC__) && !defined(__wasi__)") + "\n#if defined(__wasi__)\n#include <stdalign.h>\n#endif\n")
    p = c / "exceptions.h"
    p.write_text(p.read_text().replace("#define TEAVM_UNREACHABLE return;", "#define TEAVM_UNREACHABLE __builtin_unreachable();"))
    p = c / "memory.c"
    fallback = """
#if defined(__wasi__)
static void* teavm_virtualAlloc(int64_t size) { void *p=calloc(1,size); if(!p)abort(); return p; }
static void teavm_virtualCommit(void* p,int64_t size) { (void)p;(void)size; }
static void teavm_virtualUncommit(void* p,int64_t size) { memset(p,0,size); }
static int64_t teavm_pageSize(void) { return 65536; }
#endif
"""
    p.write_text(p.read_text().replace("static int64_t teavm_pageCount", fallback + "static int64_t teavm_pageCount"))
    p = c / "fiber.c"
    p.write_text(p.read_text() + "\n#if defined(__wasi__)\nvoid teavm_waitFor(int64_t timeout) { (void)timeout;abort(); }\nvoid teavm_interrupt(void) {}\n#endif\n")
    p = c / "time.c"
    p.write_text(p.read_text() + """
#if defined(__wasi__)
int64_t teavm_currentTimeMillis(void) { struct timespec t;clock_gettime(CLOCK_REALTIME,&t);return (int64_t)t.tv_sec*1000+t.tv_nsec/1000000; }
int64_t teavm_currentTimeNano(void) { struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return (int64_t)t.tv_sec*1000000000+t.tv_nsec; }
#endif
""")
    p = c / "main.c"
    p.write_text(p.read_text().replace("int main(int argc, char** argv)", "int teavm_main(int argc, char** argv)"))


def translate(jar, output, check_only=False):
    if jar.stat().st_size > 64 * 1024 * 1024:
        raise ValueError("JAR больше 64 МиБ")
    with tempfile.TemporaryDirectory(prefix="translate-", dir=TARGET) as temp:
        work = Path(temp)
        cp = str(TARGET / "tool") + os.pathsep + str(TARGET / "lib/*")
        run("java", "-cp", cp, "ru.mcsa.translator.Inspect", jar, work, TARGET / "mcsa-bukkit.jar")
        if check_only:
            return
        import tomllib
        manifest = tomllib.loads((work / "plugin.toml").read_text())
        dest = output / manifest["name"]
        if dest.exists():
            raise ValueError("Папка уже существует; перевод не перезаписывает данные: " + str(dest))
        # Убираем манифест Class-Path и service providers: TeaVM не должен загружать
        # расширения компилятора из входного JAR на JVM переводчика.
        compiler_jar = work / "input-classes.jar"
        with zipfile.ZipFile(jar) as source, zipfile.ZipFile(compiler_jar, "w") as clean:
            for entry in source.infolist():
                if entry.filename.endswith(".class") and not entry.filename.startswith("META-INF/"):
                    clean.writestr(entry.filename, source.read(entry))
        classes = work / "classes"
        classes.mkdir()
        run("javac", "-proc:none", "--release", "11", "-encoding", "UTF-8", "-cp", str(TARGET / "mcsa-bukkit.jar") + os.pathsep + str(compiler_jar), "-d", classes, work / "src/mcsa/generated/Main.java")
        run("java", "-Xmx1g", "-cp", cp, "ru.mcsa.translator.Compile", work / "c", classes, TARGET / "mcsa-bukkit.jar", compiler_jar)
        port_runtime(work / "c")
        wit = work / "wit"
        run(CHAIN / "wit-bindgen-0.63.0-x86_64-linux/wit-bindgen", "c", "--world", "bukkit-plugin", "--rename-world", "plugin", "--out-dir", wit, REPO / "wit/mcsa-api.wit")
        run(CHAIN / "wasi-sdk-34.0-x86_64-linux/bin/clang", "--target=wasm32-wasip2", "-mexec-model=reactor", "-O2", "-mllvm", "-wasm-enable-sjlj", "-mllvm", "-wasm-use-legacy-eh=false", "-mexception-handling", "-Wno-parentheses-equality", "-Wno-pointer-sign", "-Wno-unused-value", "-include", ROOT / "bridge.h", "-I", wit, work / "c/all.c", ROOT / "adapter.c", wit / "plugin.c", wit / "plugin_component_type.o", "-Wl,--max-memory=33554432", "-Wl,-z,stack-size=1048576", "-lsetjmp", "-lm", "-o", work / "plugin.wasm")
        run(CHAIN / "wasm-tools-1.262.0-x86_64-linux/wasm-tools", "validate", "--features", "all", work / "plugin.wasm")
        output.mkdir(parents=True, exist_ok=True)
        # Публикация только полностью собранного компонента, без изменения существующих плагинов.
        with tempfile.TemporaryDirectory(prefix=".java-stage-", dir=output) as staging:
            stage = Path(staging) / manifest["name"]
            stage.mkdir()
            for name in ["plugin.wasm", "plugin.toml"]:
                shutil.copyfile(work / name, stage / name)
            (stage / "data").mkdir()
            if (work / "default-config.yml").exists():
                shutil.copyfile(work / "default-config.yml", stage / "data/config.yml")
            stage.rename(dest)
        print("Готово:", dest, flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("jar", type=Path, nargs="?")
    parser.add_argument("--output", type=Path, default=Path("plugins"), help="Корневая папка плагинов")
    parser.add_argument("--check-only", action="store_true", help="Только статическая проверка; без гарантии компиляции TeaVM")
    parser.add_argument("--build-sdk", action="store_true", help="Собрать SDK и инструменты без перевода")
    args = parser.parse_args()
    try:
        bootstrap()
        if args.jar:
            translate(args.jar.resolve(), args.output.resolve(), args.check_only)
        elif not args.build_sdk:
            parser.error("нужен JAR или --build-sdk")
    except (subprocess.CalledProcessError, OSError, ValueError) as error:
        print("ОТКАЗ:", error, file=sys.stderr)
        return 2
    return 0

if __name__ == "__main__":
    sys.exit(main())
