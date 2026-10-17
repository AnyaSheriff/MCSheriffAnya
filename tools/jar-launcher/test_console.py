#!/usr/bin/env python3
"""Проверки launcher с настоящим PTY и собственной заглушкой ядра, без сети/портов."""
import errno
import json
import os
from pathlib import Path
import pty
import select
import signal
import subprocess
import sys
import tempfile
import termios
import time
import unittest

JAVA = '/usr/lib/jvm/java-25-openjdk/bin/java'
JAR = Path(os.environ.get('MCSA_TEST_JAR', Path(__file__).parent / 'dist/ServerWrapper.jar')).resolve()
PROBE = '''#!/usr/bin/env python3
import json, os, signal, sys

def emit(kind, **data):
    print(json.dumps(dict(kind=kind, **data)), flush=True)

def stop_signal(*_):
    emit('TERM')
    sys.exit(0)

signal.signal(signal.SIGTERM, stop_signal)
emit('READY', stdin_tty=os.isatty(0), stdout_tty=os.isatty(1), args=sys.argv[1:])
print('STDERR_CHECK', file=sys.stderr, flush=True)
if '--exit-now' not in sys.argv:
    for raw in sys.stdin.buffer:
        text = raw.decode('utf-8').rstrip('\\r\\n')
        if not text:
            continue
        emit('COMMAND', text=text)
        if text == 'stop':
            break
emit('SAVED')
sys.exit(7)
'''


class ConsoleTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='mcsa-console-')
        self.addCleanup(self.temp.cleanup)
        self.cwd = Path(self.temp.name)
        binary = self.cwd / 'mcsheriffanya'
        binary.write_text(PROBE)
        binary.chmod(0o755)

    def command(self, entry='wrapper', args=()):
        entry_args = (['-cp', str(JAR), 'pro.gravit.launcher.server.ServerWrapper'] if entry == 'wrapper'
                      else ['-jar', str(JAR)])
        return [JAVA, '-DMCSA_UPDATE=0', '-DSERVER_PORT=25951', '-Dterminal.jline=false'] + entry_args + list(args)

    def start_pty(self, entry='wrapper', args=()):
        master, slave = pty.openpty()
        settings = termios.tcgetattr(slave)
        # Панель передаёт ввод порциями, собственное эхо терминала для проверки отключено.
        settings[3] &= ~(termios.ECHO | termios.ICANON)
        termios.tcsetattr(slave, termios.TCSANOW, settings)
        process = subprocess.Popen(self.command(entry, args), cwd=self.cwd, stdin=slave,
                                   stdout=slave, stderr=slave, start_new_session=True)
        os.close(slave)
        self.addCleanup(os.close, master)
        def cleanup():
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGTERM)
                process.wait(timeout=10)
        self.addCleanup(cleanup)
        self.buffer = b''
        self.events = []
        self.output = b''
        return process, master

    def until(self, master, kind):
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            while b'\n' in self.buffer:
                line, self.buffer = self.buffer.split(b'\n', 1)
                if line.startswith(b'{'):
                    event = json.loads(line)
                    self.events.append(event)
                    if event['kind'] == kind:
                        return event
            if not select.select([master], [], [], 0.1)[0]:
                continue
            try:
                chunk = os.read(master, 65536)
            except OSError as error:
                if error.errno == errno.EIO:
                    break
                raise
            if not chunk:
                break
            self.buffer += chunk
            self.output += chunk
        self.fail('Не получено событие ' + kind)

    def test_panel_pty_has_plain_child_input_and_unsplit_commands(self):
        for entry in ('wrapper', 'jar'):
            with self.subTest(entry=entry):
                process, master = self.start_pty(entry)
                ready = self.until(master, 'READY')
                self.assertFalse(ready['stdin_tty'], 'Ядро не должно включать перерисовку ввода в панели')
                self.assertTrue(ready['stdout_tty'])
                for chunk in (b'say', b' ', b'67\r', b'\n'):
                    os.write(master, chunk)
                    time.sleep(0.03)
                self.assertEqual(self.until(master, 'COMMAND')['text'], 'say 67')
                for byte in 'say Привет 👋\n'.encode():
                    os.write(master, bytes([byte]))
                self.assertEqual(self.until(master, 'COMMAND')['text'], 'say Привет 👋')
                os.write(master, b'stop\n')
                self.until(master, 'SAVED')
                self.assertEqual(process.wait(timeout=10), 7)
                self.assertEqual([e['text'] for e in self.events if e['kind'] == 'COMMAND'],
                                 ['say 67', 'say Привет 👋', 'stop'])
                self.assertIn(b'STDERR_CHECK', self.output)
                self.assertNotIn(b'\x1b[2K', self.output)
                props = (self.cwd / 'config/server.properties').read_text()
                self.assertIn('server-port=25951', props)

    def test_pipe_eof_unicode_arguments_and_child_exit_code(self):
        result = subprocess.run(self.command(args=['--example', 'два слова']), cwd=self.cwd,
                                input='say Привет\nsay 67\n'.encode(), stdout=subprocess.PIPE,
                                stderr=subprocess.STDOUT, timeout=10)
        events = [json.loads(line) for line in result.stdout.splitlines() if line.startswith(b'{')]
        self.assertEqual(result.returncode, 7)
        self.assertEqual(events[0]['args'], ['--example', 'два слова'])
        self.assertEqual([e['text'] for e in events if e['kind'] == 'COMMAND'], ['say Привет', 'say 67'])
        self.assertEqual(events[-1]['kind'], 'SAVED')

    def test_exit_does_not_wait_for_blocked_input_thread(self):
        process, master = self.start_pty(args=['--exit-now'])
        self.until(master, 'SAVED')
        self.assertEqual(process.wait(timeout=5), 7)

    def test_sigterm_reaches_child(self):
        process, master = self.start_pty()
        self.until(master, 'READY')
        process.terminate()
        self.until(master, 'TERM')
        process.wait(timeout=10)


if __name__ == '__main__':
    unittest.main(verbosity=2)
