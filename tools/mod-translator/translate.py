#!/usr/bin/env python3
"""Перевод только OWN-source ModSDK subset в mcsa:api WASM; не Fabric/Forge/JAR."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib

from compiler import Refusal, generate, parse

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parent.parent
# Только закреплённые инструменты существующей цепочки; Java/Bukkit не нужны.
CHAIN = REPO / 'tools/plugin-translator/.toolchain'


def load(folder):
    folder = Path(folder).resolve()
    manifest_path = folder / 'mod.toml'
    if manifest_path.is_symlink() or not manifest_path.is_file() or manifest_path.stat().st_size > 16384:
        raise Refusal('manifest', 'Нужен обычный mod.toml до 16 КиБ')
    try:
        manifest = tomllib.loads(manifest_path.read_text(encoding='utf-8'))
    except (ValueError, UnicodeError) as error:
        raise Refusal('manifest', f'Неверный mod.toml: {error}') from error
    required = {'name', 'version', 'api', 'minecraft', 'protocol', 'source'}
    if set(manifest) != required:
        raise Refusal('manifest-fields', 'Нужны ровно поля name/version/api/minecraft/protocol/source; чужие loader-метаданные не переводятся')
    if not isinstance(manifest['name'], str) or not re.fullmatch('[a-z0-9_-]{1,32}', manifest['name']):
        raise Refusal('name', 'Имя: a-z, 0-9, _ и -, от 1 до 32 символов')
    if not isinstance(manifest['version'], str) or not re.fullmatch('[A-Za-z0-9_.+-]{1,64}', manifest['version']):
        raise Refusal('version', 'Версия: до 64 ASCII символов, без пробелов')
    if manifest['api'] != 'mcsa-mod-source-0.1':
        raise Refusal('api', 'Поддержан только собственный API mcsa-mod-source-0.1')
    if manifest['minecraft'] != '26.3' or type(manifest['protocol']) is not int or manifest['protocol'] != 777:
        raise Refusal('target', 'Поддержано только ядро 26.3 / protocol 777; адаптация 1.21.1 не проверена')
    if not isinstance(manifest['source'], str) or not re.fullmatch('[A-Za-z_][A-Za-z_0-9]*[.]java', manifest['source']):
        raise Refusal('source-path', 'source должен быть одним именем .java внутри папки')
    source_path = folder / manifest['source']
    if source_path.is_symlink() or not source_path.is_file() or source_path.stat().st_size > 65536:
        raise Refusal('source-file', 'Нужен обычный собственный .java до 64 КиБ, без symlink')
    # Не игнорируем дополнительные исходники, mixins, assets или loader manifest.
    # Полный проект должен состоять ровно из явно поддерживаемых двух файлов.
    if {path.name for path in folder.iterdir()} != {'mod.toml', manifest['source']}:
        raise Refusal('project-content', 'В проекте есть дополнительные файлы; они не будут молча отброшены. Нужны только mod.toml и source')
    try:
        source = source_path.read_text(encoding='utf-8')
    except UnicodeError as error:
        raise Refusal('source-encoding', 'Исходник должен быть UTF-8') from error
    program = parse(source)
    if manifest['source'] != program['class_name'] + '.java':
        raise Refusal('source-class', 'Имя public class должно совпадать с .java')
    return manifest, program, hashlib.sha256(source.encode('utf-8')).hexdigest()


def build(folder, output, toolchain=CHAIN):
    manifest, program, digest = load(folder)
    output = Path(output).resolve()
    destination = output / manifest['name']
    if destination.exists() or destination.is_symlink():
        raise Refusal('destination', 'Папка результата уже существует; данные не перезаписываются')
    toolchain = Path(toolchain).resolve()
    bindgen = toolchain / 'wit-bindgen-0.63.0-x86_64-linux/wit-bindgen'
    clang = toolchain / 'wasi-sdk-34.0-x86_64-linux/bin/clang'
    validate = toolchain / 'wasm-tools-1.262.0-x86_64-linux/wasm-tools'
    if any(not path.is_file() for path in [bindgen, clang, validate]):
        raise Refusal('toolchain', 'Нет закреплённых wit-bindgen 0.63.0 / wasi-sdk 34 / wasm-tools 1.262.0; подготовьте их в CI')
    with tempfile.TemporaryDirectory(prefix='mcsa-mod-') as temporary:
        work = Path(temporary)
        generated = generate(program)
        (work / 'mod.c').write_text(generated, encoding='utf-8')
        subprocess.run([str(bindgen), 'c', '--world', 'plugin', '--out-dir', str(work), str(REPO / 'wit/mcsa-api.wit')], check=True)
        subprocess.run([str(clang), '--target=wasm32-wasip2', '-mexec-model=reactor', '-O2', '-Wall', '-Wextra', '-Werror', '-Wno-unused-function', '-I', str(work), str(work / 'mod.c'), str(work / 'plugin.c'), str(work / 'plugin_component_type.o'), '-Wl,--max-memory=33554432', '-Wl,-z,stack-size=1048576', '-o', str(work / 'plugin.wasm')], check=True)
        subprocess.run([str(validate), 'validate', '--features', 'all', str(work / 'plugin.wasm')], check=True)
        output.mkdir(parents=True, exist_ok=True)
        # Публикуем только готовый компонент целиком, без изменения прежних данных.
        with tempfile.TemporaryDirectory(prefix='.mod-stage-', dir=output) as staging:
            stage = Path(staging) / manifest['name']
            stage.mkdir()
            shutil.copyfile(work / 'plugin.wasm', stage / 'plugin.wasm')
            (stage / 'plugin.toml').write_text(f'name = "{manifest["name"]}"\nversion = "{manifest["version"]}"\napi = "0.1"\nmain = "plugin.wasm"\n', encoding='utf-8')
            receipt = dict(format=1, source_sha256=digest, compiler_profile='mcsa-mod-source-0.1', target='26.3/777', callbacks=len(program['callbacks']), wasm_sha256=hashlib.sha256((stage / 'plugin.wasm').read_bytes()).hexdigest(), wit_sha256=hashlib.sha256((REPO / 'wit/mcsa-api.wit').read_bytes()).hexdigest(), generated_c_sha256=hashlib.sha256(generated.encode('utf-8')).hexdigest())
            (stage / 'conversion.json').write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
            (stage / 'data').mkdir()
            stage.rename(destination)
    return destination


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('project', type=Path, help='Папка с СОБСТВЕННЫМ mod.toml и .java; чужие JAR не открываются')
    parser.add_argument('--check-only', action='store_true', help='Полный разбор source-subset; без проверки WASM-сборки')
    parser.add_argument('--emit-c', type=Path, help='Сохранить C после полного разбора; без компиляции')
    parser.add_argument('--output', type=Path, default=Path('plugins'))
    parser.add_argument('--toolchain', type=Path, default=CHAIN)
    parser.add_argument('--report', type=Path, help='Русский JSON-отчёт об успехе или точном отказе')
    args = parser.parse_args(argv)
    report = dict(format=1, profile='mcsa-mod-source-0.1', fabric_forge_compatible=False, ok=False)
    try:
        manifest, program, digest = load(args.project)
        report.update(name=manifest['name'], source_sha256=digest, callbacks=len(program['callbacks']))
        if args.emit_c:
            if args.emit_c.exists():
                raise Refusal('destination', 'C-файл уже существует')
            args.emit_c.write_text(generate(program), encoding='utf-8')
        elif not args.check_only:
            report['output'] = str(build(args.project, args.output, args.toolchain))
        report['ok'] = True
        report['verified'] = 'source-subset-only' if args.check_only or args.emit_c else 'wasm-component-validated'
        print('Готово: ' + report['verified'])
    except Refusal as error:
        report['issue'] = error.issue
        print('ОТКАЗ: ' + str(error), file=sys.stderr)
    except (OSError, subprocess.CalledProcessError) as error:
        report['issue'] = dict(code='build-or-io', message=str(error))
        print('ОТКАЗ: ' + str(error), file=sys.stderr)
    if args.report:
        args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    return 0 if report['ok'] else 2


if __name__ == '__main__':
    sys.exit(main())
