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
        self.assertEqual(manifest['sdk_abi'], 'mcsa:api@0.1.0')
        self.assertEqual(len(digest), 64)
        self.assertEqual(len(program['callbacks']), 6)

    def test_legacy_manifest_loads_as_same_project(self):
        explicit = load(self.folder)
        path = self.folder / 'mod.toml'
        path.write_text(path.read_text().replace('sdk_abi = "mcsa:api@0.1.0"\n', ''))
        self.assertEqual(load(self.folder), explicit)

    def test_unsupported_sdk_abi_rejected_before_source_parse(self):
        path = self.folder / 'mod.toml'
        original = path.read_text()
        for value in ['"mcsa:api@0.2.0"', '"mcsa-mod-source-0.1"', '""', '1', '["mcsa:api@0.1.0"]']:
            with self.subTest(value=value):
                path.write_text(original.replace('"mcsa:api@0.1.0"', value))
                with patch('translate.parse') as parse_source:
                    with self.assertRaises(Refusal) as caught:
                        load(self.folder)
                    self.assertEqual(caught.exception.issue['code'], 'sdk-abi')
                    parse_source.assert_not_called()

    def test_target_and_api_refusal(self):
        path = self.folder / 'mod.toml'
        original = path.read_text()
        for before, after, code in [('26.3', '1.21.1', 'target'), ('777', '767', 'target'),
                                    ('mcsa-mod-source-0.1', 'fabric', 'api')]:
            with self.subTest(after=after):
                path.write_text(original.replace(before, after))
                with self.assertRaises(Refusal) as caught:
                    load(self.folder)
                self.assertEqual(caught.exception.issue['code'], code)
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

    def test_cli_reports_abi_profile_and_target(self):
        with tempfile.TemporaryDirectory() as output:
            report = Path(output) / 'report.json'
            self.assertEqual(main([str(self.folder), '--check-only', '--report', str(report)]), 0)
            result = json.loads(report.read_text())
            self.assertEqual(result['profile'], 'mcsa-mod-source-0.1')
            self.assertEqual(result['sdk_abi'], 'mcsa:api@0.1.0')
            self.assertEqual(result['target'], '26.3/777')

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



class ChatCompilerTests(unittest.TestCase):
    def setUp(self):
        self.fixture = REPO / 'tools/mod-api/fixtures/chat'
        self.source = (self.fixture / 'ChatAudit.java').read_text()

    def test_typed_chat_fixture_and_existing_abi(self):
        manifest, program, _ = load(self.fixture)
        self.assertEqual(manifest['sdk_abi'], 'mcsa:api@0.1.0')
        self.assertEqual([c['event'] for c in program['callbacks']], ['onChat', 'onChat'])
        self.assertEqual(program['callbacks'][0]['actions'][0]['expression'], [
            ('literal', 'chat:first:'), ('chat', 'playerName'), ('literal', ':'), ('chat', 'message')])
        self.assertEqual(program['callbacks'][1]['actions'][1]['operation'], 'cancel')

    def test_original_text_and_sender_use_borrowed_wit_fields(self):
        code = generate(parse(self.source))
        self.assertIn('append(message->ptr,message->len);', code)
        self.assertIn('append(who->name.ptr,who->name.len);', code)
        self.assertIn('append(who->uuid.ptr,who->uuid.len);', code)
        self.assertNotIn('plugin_string_free', code)

    def test_cancel_is_monotonic_and_callbacks_are_ordered(self):
        code = generate(parse(self.source))
        self.assertEqual(code.count('*cancelled=true;'), 2)
        self.assertNotIn('*cancelled=false', code)
        self.assertLess(code.index('callback_0(who,message,&cancelled);'), code.index('callback_1(who,message,&cancelled);'))
        self.assertIn('bool cancelled=false; if(active)', code)
        self.assertIn('MCSA_API_TYPES_CHAT_RESULT_CANCEL : MCSA_API_TYPES_CHAT_RESULT_PASS', code)

    def test_observer_without_cancel_is_supported(self):
        code = generate(parse(self.source.replace('chat.cancel();', '')))
        self.assertNotIn('*cancelled=true;', code)
        self.assertIn('bool cancelled=false;', code)

    def test_unsupported_chat_mutations_are_rejected(self):
        for action in ['chat.replace("x");', 'chat.uncancel();', 'chat.cancel(false);', 'ctx.message(chat, "x");']:
            with self.subTest(action=action), self.assertRaises(Refusal):
                parse(self.source.replace('chat.cancel();', action))

    def test_event_getters_do_not_cross_types(self):
        for expression in ['chat.name()', 'chat.uuid()', 'chat.player()', 'chat.isCancelled()', 'player.name()']:
            with self.subTest(expression=expression), self.assertRaises(Refusal):
                parse(self.source.replace('chat.playerName()', expression))
        with self.assertRaises(Refusal):
            parse(SOURCE.replace('player.name()', 'player.message()'))

    def test_cancel_is_not_available_in_lifecycle(self):
        with self.assertRaises(Refusal):
            parse(SOURCE.replace('ctx.log("mod-fixture started");', 'chat.cancel();'))

    def test_cancel_counts_toward_existing_action_limit(self):
        with self.assertRaises(Refusal) as caught:
            parse(self.source.replace('chat.cancel();', 'chat.cancel();' * 65))
        self.assertEqual(caught.exception.issue['code'], 'action-limit')

    def test_new_fixture_still_requires_full_source_parse(self):
        with self.assertRaises(Refusal):
            parse(self.source + 'hidden();')


class CommandCompilerTests(unittest.TestCase):
    fixture = REPO / 'tools/mod-api/fixtures/command'
    source = (fixture / 'CommandAudit.java').read_text(encoding='utf-8')

    def test_two_typed_commands_and_ownership_in_generated_c(self):
        _, program, _ = load(self.fixture)
        self.assertEqual([(c['command'], c['level']) for c in program['callbacks']],
                         [('mod_echo', 0), ('mod_admin', 4)])
        code = generate(program)
        self.assertEqual(code.count('mcsa_api_command_register(&spec,&err)'), 2)
        self.assertIn('append(caller->name.ptr,caller->name.len);', code)
        self.assertIn('if(args->len>1) append(args->ptr[1].ptr,args->ptr[1].len);', code)
        self.assertIn('plugin_string_dup_n(&ret->message.val,buffer,used);', code)
        self.assertLess(code.index('callback_1(caller,name,args,ret);'),
                        code.index('mcsa_api_types_sender_free(caller);'))

    def test_bad_names_levels_and_argument_indexes_are_refused(self):
        for old, new, code in [
            ('"mod_echo"', '"/mod_echo"', 'command-name'),
            ('"mod_echo"', '"ModEcho"', 'command-name'),
            ('"mod_echo"', '"mod_admin"', 'duplicate-command'),
            ('"mod_echo", 0', '"mod_echo", 5', 'command-level'),
            ('"mod_echo", 0', '"mod_echo", "0"', 'command-level'),
            ('argument(1)', 'argument(32)', 'command-argument'),
            ('argument(1)', 'argument(01)', 'command-argument'),
            ('senderName()', 'uuid()', 'unsupported-command-field'),
        ]:
            with self.subTest(new=new), self.assertRaises(Refusal) as err:
                parse(self.source.replace(old, new))
            self.assertEqual(err.exception.issue['code'], code)

    def test_reply_is_scoped_and_unique(self):
        cases = [
            (SOURCE.replace('ctx.log("mod-fixture started");', 'ctx.reply(command,"x");'), 'unsupported-command-action'),
            (self.source.replace('ctx.log("command fixture ran");', 'ctx.reply(command,"second");'), 'duplicate-reply'),
            (self.source.replace('ctx.log("command fixture ran");', 'ctx.message(command,"x");'), 'unsupported-source'),
        ]
        for source, code in cases:
            with self.subTest(code=code), self.assertRaises(Refusal) as err:
                parse(source)
            self.assertEqual(err.exception.issue['code'], code)


if __name__ == '__main__':
    unittest.main()
