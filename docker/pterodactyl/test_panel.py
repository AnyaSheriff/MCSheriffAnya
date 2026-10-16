"""Проверки без Docker, сети и запуска Minecraft: python3 docker/pterodactyl/test_panel.py."""

import json
import os
from pathlib import Path
import selectors
import shlex
import signal
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
EGG = json.loads((Path(__file__).with_name("egg-mcsheriffanya.json")).read_text())


class PanelTests(unittest.TestCase):
    def setUp(self):
        (ROOT / "tmp").mkdir(exist_ok=True)
        self.temp = tempfile.TemporaryDirectory(prefix="panel-test-", dir=ROOT / "tmp")
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)

    def launch(self, startup, **variables):
        env = dict(os.environ, STARTUP=startup, TEST_CONTAINER=str(self.directory), **variables)
        # Подменяем только cd: образ требует /home/container, тест пишет внутри копии.
        command = 'cd() { builtin cd "$TEST_CONTAINER"; }; source "$1"'
        process = subprocess.Popen(
            ["bash", "-c", command, "panel-test", str(Path(__file__).with_name("entrypoint.sh"))],
            env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            text=True,
        )
        self.addCleanup(self.finish, process)
        return process

    @staticmethod
    def finish(process):
        if process.poll() is None:
            process.kill()
        process.communicate()

    def test_expansion_stdin_workdir_and_exec(self):
        probe = self.directory / "probe.py"
        probe.write_text(
            'import json, os, sys\n'
            'print(json.dumps([os.getpid(), os.getcwd(), sys.argv[1:], sys.stdin.read()]))\n'
        )
        value = 'Тест with spaces; $(touch injected) `touch injected2` "quoted"'
        process = self.launch(
            f'{shlex.quote(sys.executable)} {shlex.quote(str(probe))} "{{{{MESSAGE}}}}" {{{{PORT}}}}',
            MESSAGE=value, PORT="25577",
        )
        stdout, stderr = process.communicate("help\nstop\n", timeout=5)
        self.assertEqual(process.returncode, 0, stderr)
        banner, payload = stdout.splitlines()
        self.assertTrue(banner.startswith("container@mcsa~ "))
        self.assertIn("25577", banner)
        self.assertNotIn("{{", banner)
        self.assertEqual(json.loads(payload), [process.pid, str(self.directory), [value, "25577"], "help\nstop\n"])
        self.assertFalse((self.directory / "injected").exists())
        self.assertFalse((self.directory / "injected2").exists())

    def test_sigterm_reaches_executed_process(self):
        probe = self.directory / "probe_sigterm.py"
        probe.write_text(
            'import signal, sys\n'
            'signal.signal(signal.SIGTERM, lambda *_: sys.exit(23))\n'
            'print("READY", flush=True)\n'
            'sys.stdin.read()\n'
        )
        process = self.launch(f'{shlex.quote(sys.executable)} {shlex.quote(str(probe))}')
        # Таймер ограничивает ожидание READY даже при поломке entrypoint.
        with selectors.DefaultSelector() as selector:
            selector.register(process.stdout, selectors.EVENT_READ)
            output = b""
            while b"READY\n" not in output:
                self.assertTrue(selector.select(timeout=5), "Probe did not become ready")
                chunk = os.read(process.stdout.fileno(), 4096)
                self.assertTrue(chunk, "Probe exited before READY")
                output += chunk
        process.send_signal(signal.SIGTERM)
        process.wait(timeout=5)
        self.assertEqual(process.returncode, 23)

    def test_empty_command_fails(self):
        process = self.launch("   ")
        _, stderr = process.communicate(timeout=5)
        self.assertNotEqual(process.returncode, 0)
        self.assertIn("STARTUP must contain an executable", stderr)

    def test_install_seeds_config_and_preserves_existing_data(self):
        script = EGG["scripts"]["installation"]["script"].replace(
            "/mnt/server", shlex.quote(str(self.directory))
        )

        def install():
            subprocess.run(["sh", "-eu"], input=script, text=True, capture_output=True, check=True, timeout=5)

        install()
        files = json.loads(EGG["config"]["files"])
        for filename, parser in files.items():
            text = (self.directory / filename).read_text()
            values = dict(line.split("=", 1) for line in text.splitlines() if "=" in line)
            self.assertEqual(parser["parser"], "properties")
            self.assertTrue(parser["find"].keys() <= values.keys())
        config = self.directory / "config/server.properties"
        config.write_text("motd=Мой сервер\nserver-port=25577\n")
        world = self.directory / "world/keep.dat"
        world.write_bytes(b"existing world")
        install()
        self.assertEqual(config.read_text(), "motd=Мой сервер\nserver-port=25577\n")
        self.assertEqual(world.read_bytes(), b"existing world")
        declared = {item["env_variable"] for item in EGG["variables"]}
        for parser in files.values():
            for value in parser["find"].values():
                if value.startswith("{{server.environment."):
                    self.assertIn(value.removeprefix("{{server.environment.").removesuffix("}}"), declared)


if __name__ == "__main__":
    unittest.main()
