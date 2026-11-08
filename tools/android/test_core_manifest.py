import importlib.util
from pathlib import Path
import tomllib
import unittest

spec=importlib.util.spec_from_file_location('core_manifest',Path(__file__).with_name('core-manifest.py'))
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)


class CoreManifestTest(unittest.TestCase):
    def test_only_target_changes_dependencies_profiles_workspace_stay_identical(self):
        source=(Path(__file__).resolve().parents[2]/'Cargo.toml').read_text();original=tomllib.loads(source);converted=tomllib.loads(module.convert(source))
        for name in ['dependencies','dev-dependencies','target','profile','workspace','features','lints']:self.assertEqual(original[name],converted[name],name)
        self.assertFalse(converted['package']['autobins']);self.assertNotIn('default-run',converted['package']);self.assertEqual(converted['lib']['crate-type'],['cdylib'])

    def test_refuses_to_overwrite_existing_library_target(self):
        with self.assertRaises(AssertionError):module.convert('[package]\nname="test"\n[features]\nandroid-core=[]\n[lib]\npath="src/lib.rs"\n')


if __name__=='__main__':unittest.main()
