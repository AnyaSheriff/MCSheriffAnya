"""Проверки команд сборки SDK без запуска внешних инструментов."""
import io
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stderr
from pathlib import Path
from unittest.mock import patch

import translate


class BootstrapTests(unittest.TestCase):
    def setUp(self):
        workspace = self.enterContext(tempfile.TemporaryDirectory(dir=translate.ROOT))
        self.target = Path(workspace) / "target"
        self.enterContext(patch.object(translate, "TARGET", self.target))
        self.run_command = self.enterContext(patch.object(translate, "guarded_run"))
        self.download = self.enterContext(patch.object(translate.urllib.request, "urlretrieve"))
        self.enterContext(patch.object(sys, "argv", ["translate.py", "--build-sdk"]))
        self.maven_command = [
            "mvn", "-q", "-f", str(translate.ROOT / "pom.xml"),
            "org.apache.maven.plugins:maven-dependency-plugin:3.6.1:copy-dependencies",
            "-DoutputDirectory=target/lib",
        ]

    def tearDown(self):
        self.download.assert_not_called()

    def test_build_sdk_resolves_dependencies_before_compilation(self):
        self.assertEqual(translate.main(), 0)
        first, *remaining = self.run_command.call_args_list
        self.assertEqual(first.args, (self.maven_command,))
        self.assertEqual(first.kwargs, {"check": True})
        self.assertEqual([call.args[0][0] for call in remaining], ["javac", "javac", "jar"])

    def test_build_sdk_uses_existing_dependency_cache(self):
        library = self.target / "lib"
        library.mkdir(parents=True)
        (library / "teavm-tooling-0.12.3.jar").touch()

        self.assertEqual(translate.main(), 0)
        commands = [call.args[0][0] for call in self.run_command.call_args_list]
        self.assertEqual(commands, ["javac", "javac", "jar"])

    def test_cached_wasm_toolchain_needs_no_java_or_maven(self):
        chain = self.target.parent / ".toolchain"
        for name in translate.PACKAGES:
            (chain / name).mkdir(parents=True)
        with patch.object(translate, "CHAIN", chain):
            translate.bootstrap_wasm_toolchain()
        self.run_command.assert_not_called()
        self.assertFalse(self.target.exists())

    def test_build_sdk_stops_on_dependency_resolution_failure(self):
        def fail(command, *, check):
            if check:
                raise subprocess.CalledProcessError(1, command)

        self.run_command.side_effect = fail
        stderr = io.StringIO()
        with redirect_stderr(stderr):
            self.assertEqual(translate.main(), 2)

        self.run_command.assert_called_once_with(self.maven_command, check=True)
        self.assertIn("ОТКАЗ:", stderr.getvalue())
        self.assertIn(self.maven_command[4], stderr.getvalue())
        self.assertFalse((self.target / "sdk").exists())


if __name__ == "__main__":
    unittest.main()
