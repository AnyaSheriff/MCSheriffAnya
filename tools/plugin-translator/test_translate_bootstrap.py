"""Проверки команд сборки SDK без запуска внешних инструментов."""
import io
import os
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


class DispatchCompilerTests(unittest.TestCase):
    def test_java21_input_uses_compatible_compiler_without_changing_runtime(self):
        old, new = Path("/opt/jdk17/bin/javac"), Path("/opt/jdk25/bin/javac")
        def version(args, **_):
            value = "17.0.13" if args[0] == str(old) else "25.0.1"
            return subprocess.CompletedProcess(args, 0, "javac " + value + "\n")
        with patch.dict(os.environ, {}, clear=True), patch.object(translate.shutil, "which", return_value=str(old)), \
                patch.object(Path, "glob", return_value=[old, new]), patch.object(subprocess, "run", side_effect=version):
            self.assertEqual(translate.dispatch_javac(21), str(new))
            self.assertEqual(translate.dispatch_javac(11), str(old))

    def test_absent_compatible_compiler_and_explicit_override(self):
        with patch.dict(os.environ, {}, clear=True), patch.object(translate.shutil, "which", return_value=None), \
                patch.object(Path, "glob", return_value=[]):
            with self.assertRaisesRegex(ValueError, r"javac 21.*MCSA_TRANSLATOR_JAVAC"):
                translate.dispatch_javac(21)
        with patch.dict(os.environ, {"MCSA_TRANSLATOR_JAVAC": "/custom/javac"}, clear=True):
            self.assertEqual(translate.dispatch_javac(21), "/custom/javac")


if __name__ == "__main__":
    unittest.main()
