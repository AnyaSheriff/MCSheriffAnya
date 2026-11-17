#!/usr/bin/env python3
"""Лёгкие проверки полного отказа, порядка и проекта. Без Java/C/WASM сборки."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from compiler import Refusal, generate, parse
from translate import REPO, build, load, main

FIXTURE = REPO / 'tools/mod-api/fixtures/welcome'
SOURCE = (FIXTURE / 'Welcome.java').read_text(encoding='utf-8')


class CompilerTests(unittest.TestCase):
    def refused(self, source, code=None):
        with self.assertRaises(Refusal) as caught:
            parse(source)
        issue = caught.exception.issue
        self.assertGreaterEqual(issue['line'], 1)
        self.assertGreaterEqual(issue['column'], 1)
        if code:
            self.assertEqual(issue['code'], code)

    def test_fixture_full_parse(self):
        program = parse(SOURCE)
        self.assertEqual(program['class_name'], 'Welcome')
        self.assertEqual([c['event'] for c in program['callbacks']], ['onStart', 'onJoin', 'onJoin', 'onQuit', 'every', 'onStop'])
        self.assertEqual(program['callbacks'][4]['period'], 2)
        self.assertEqual(program['callbacks'][1]['actions'][0]['expression'], [('literal', 'mod:join:1:'), ('player', 'name')])

    def test_all_tokens_consumed(self):
        for extra in ['public class Hidden {}', 'System.exit(0);', 'extra', ';']:
            with self.subTest(extra=extra):
                self.refused(SOURCE + extra)

    def test_hidden_behavior_rejected(self):
        for extra in ['private int count;', 'public void hidden() {}', 'static { System.exit(0); }']:
            with self.subTest(extra=extra):
                self.refused(SOURCE[:-2] + extra + '}')

    def test_no_behavior_dropped_inside_callback(self):
        for extra in ['if (true) {}', 'ctx.unknown("x");', 'Thread.start();', 'return;', 'while (true) {}', 'new Object();', 'ctx.onStart(() -> {});']:
            with self.subTest(extra=extra):
                self.refused(SOURCE.replace('ctx.log("mod-fixture started");', extra))

    def test_fabric_forge_mixin_client_imports(self):
        for api in ['net.fabricmc.api.ModInitializer', 'net.minecraftforge.fml.common.Mod', 'org.spongepowered.asm.mixin.Mixin', 'net.minecraft.client.Minecraft', 'java.lang.reflect.Method', 'java.io.File']:
            with self.subTest(api=api):
                self.refused(SOURCE.replace('import ru.mcsa.modapi.Mod;', 'import ' + api + ';'), 'unsupported-api')

    def test_period_boundaries(self):
        for value in ['1', '2400000']:
            self.assertEqual(parse(SOURCE.replace('every(2,', f'every({value},'))['callbacks'][4]['period'], int(value))
        for value in ['0', '2400001', '010', '-1', '1.5', '2147483648', '2L', '9' * 5000]:
            with self.subTest(value=value[:20]):
                self.refused(SOURCE.replace('every(2,', f'every({value},'))

    def test_utf8_escapes_not_injection(self):
        program = parse(SOURCE.replace('mod-fixture started', 'Аня \\"x\\" \\\\ \\n'))
        c = generate(program)
        self.assertIn('\\320\\220', c)
        self.assertNotIn('Аня', c)
        self.assertNotIn('"x"', c)

    def test_java_unicode_escapes_rejected_everywhere(self):
        self.refused('// \\u000a Thread.start();\n' + SOURCE, 'unicode-escape')

    def test_comments_only_comments(self):
        self.assertEqual(parse('// hi\n/* comment */\n' + SOURCE), parse(SOURCE))
        self.refused(SOURCE + '/* unclosed', 'comment')

    def test_strings_strict(self):
        for value in ['"bad\\q"', '"raw\x00nul"', '"raw\tcontrol"', '"' + 'x' * 4097 + '"']:
            with self.subTest(value=value[:20]):
                self.refused(SOURCE.replace('"mod-fixture started"', value))

    def test_no_player_outside_player_callback(self):
        self.refused(SOURCE.replace('ctx.log("mod-fixture started");', 'ctx.message(player, "x");'))
        self.refused(SOURCE.replace('ctx.log("mod-fixture started");', 'ctx.log(player.name());'))

    def test_reserved_identifiers_and_shadowing(self):
        self.refused(SOURCE.replace('class Welcome', 'class int'))
        self.refused(SOURCE.replace('class Welcome', 'class Context'))
        self.refused(SOURCE.replace('player ->', 'ctx ->'))

    def test_registration_cap(self):
        self.refused(SOURCE.replace('ctx.every(2, () -> {', 'ctx.onStart(() -> {});' * 65 + 'ctx.every(2, () -> {'), 'registration-limit')

    def test_action_cap(self):
        self.refused(SOURCE.replace('ctx.log("mod-fixture started");', 'ctx.log("x");' * 65), 'action-limit')

    def test_expression_cap(self):
        self.refused(SOURCE.replace('"mod-fixture started"', '+'.join(['"x"'] * 33)), 'expression-limit')

    def test_generated_order_scheduler_cleanup_and_unknown_task(self):
        code = generate(parse(SOURCE))
        self.assertLess(code.index('callback_1(who);'), code.index('callback_2(who);'))
        self.assertIn('mcsa_api_scheduler_run_repeating(2,2,5)', code)
        self.assertLess(code.index('mcsa_api_scheduler_cancel(handles[0])'), code.index('callback_5();'))
        self.assertIn('case 5: callback_4(); break;', code)
        self.assertIn('default: break;', code)
        self.assertIn('if(!active) return;', code)

    def test_empty_mod_valid(self):
        source = 'import ru.mcsa.modapi.Mod; import ru.mcsa.modapi.Context; public final class Empty implements Mod { public void initialize(Context ctx) {} }'
        self.assertEqual(parse(source)['callbacks'], [])
        self.assertNotIn('handles[0]', generate(parse(source)))


class ProjectTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.folder = Path(self.temp.name)
        for name in ['mod.toml', 'Welcome.java']:
            (self.folder / name).write_bytes((FIXTURE / name).read_bytes())

    def tearDown(self):
        self.temp.cleanup()

    def test_own_project(self):
        manifest, program, digest = load(self.folder)
        self.assertEqual(manifest['name'], 'mod-welcome')
        self.assertEqual(len(digest), 64)
        self.assertEqual(len(program['callbacks']), 6)

    def test_target_and_api_refusal(self):
        path = self.folder / 'mod.toml'
        original = path.read_text()
        for before, after in [('26.3', '1.21.1'), ('777', '767'), ('mcsa-mod-source-0.1', 'fabric')]:
            with self.subTest(after=after):
                path.write_text(original.replace(before, after))
                with self.assertRaises(Refusal):
                    load(self.folder)
        path.write_text(original)

    def test_extra_manifest_fields_refused(self):
        with (self.folder / 'mod.toml').open('a') as stream:
            stream.write('mixins = ["x"]\n')
        with self.assertRaises(Refusal):
            load(self.folder)

    def test_extra_files_never_silently_dropped(self):
        for name in ['Hidden.java', 'fabric.mod.json', 'assets', 'something.jar']:
            with self.subTest(name=name):
                path = self.folder / name
                path.write_text('')
                with self.assertRaises(Refusal):
                    load(self.folder)
                path.unlink()

    def test_symlink_rejected(self):
        path = self.folder / 'Welcome.java'
        path.unlink()
        path.symlink_to(FIXTURE / 'Welcome.java')
        with self.assertRaises(Refusal):
            load(self.folder)

    def test_path_traversal_refused(self):
        path = self.folder / 'mod.toml'
        path.write_text(path.read_text().replace('Welcome.java', '../Welcome.java'))
        with self.assertRaises(Refusal):
            load(self.folder)

    def test_class_file_name_match(self):
        path = self.folder / 'Welcome.java'
        path.write_text(SOURCE.replace('class Welcome', 'class Other'))
        with self.assertRaises(Refusal):
            load(self.folder)

    def test_cli_reports_refusal_no_output(self):
        (self.folder / 'Welcome.java').write_text(SOURCE + 'hidden();')
        # Отчёт вне проекта: проект не должен содержать неизвестных файлов.
        with tempfile.TemporaryDirectory() as output:
            report = Path(output) / 'report.json'
            self.assertEqual(main([str(self.folder), '--check-only', '--report', str(report)]), 2)
            result = json.loads(report.read_text())
            self.assertFalse(result['ok'])
            self.assertFalse(result['fabric_forge_compatible'])
            self.assertIn('line', result['issue'])
            self.assertEqual(list(Path(output).iterdir()), [report])

    def test_emit_c_requires_full_valid_source(self):
        with tempfile.TemporaryDirectory() as output:
            path = Path(output) / 'result.c'
            self.assertEqual(main([str(self.folder), '--emit-c', str(path)]), 0)
            self.assertIn('callback_1(who)', path.read_text())
            self.assertEqual(main([str(self.folder), '--emit-c', str(path)]), 2)

    def test_jar_is_not_opened(self):
        path = self.folder / 'own-fixture.jar'
        path.write_bytes(b'not an archive')
        with self.assertRaises(Refusal):
            load(path)

    def test_invalid_source_never_invokes_compiler_or_creates_output(self):
        (self.folder / 'Welcome.java').write_text(SOURCE + 'hidden();')
        with tempfile.TemporaryDirectory() as output:
            destination = Path(output) / 'plugins'
            with patch('translate.subprocess.run') as runner:
                with self.assertRaises(Refusal):
                    build(self.folder, destination)
                runner.assert_not_called()
            self.assertFalse(destination.exists())

    def test_missing_toolchain_leaves_no_output(self):
        with tempfile.TemporaryDirectory() as output:
            destination = Path(output) / 'plugins'
            with self.assertRaises(Refusal) as caught:
                build(self.folder, destination, Path(output) / 'absent-tools')
            self.assertEqual(caught.exception.issue['code'], 'toolchain')
            self.assertFalse(destination.exists())

    def test_build_never_replaces_existing_data(self):
        with tempfile.TemporaryDirectory() as output:
            destination = Path(output) / 'mod-welcome'
            destination.mkdir()
            (destination / 'owner-data').write_bytes(b'keep')
            with self.assertRaises(Refusal) as caught:
                build(self.folder, Path(output))
            self.assertEqual(caught.exception.issue['code'], 'destination')
            self.assertEqual((destination / 'owner-data').read_bytes(), b'keep')


if __name__ == '__main__':
    unittest.main()
