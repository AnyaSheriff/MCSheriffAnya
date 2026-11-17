"""Новый helper обязан связывать именно новый build/trace/source, не старый ELF."""
import hashlib
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import worldgen_stage_ci_263 as ci


class StageCiTests(unittest.TestCase):
    def test_build_receipt_binds_source_binary_trace_and_lock(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            (root / 'Cargo.lock').write_bytes(b'fixed-lock')
            elf, trace = root / 'server', root / 'trace.gz'
            elf.write_bytes(b'\x7fELFown-new-source'); trace.write_bytes(b'own-stage-data')
            receipt = {'source': 'a' * 40, 'elf_sha256': ci.sha(elf), 'trace_sha256': ci.sha(trace),
                       'cargo_lock_sha256': ci.sha(root / 'Cargo.lock')}
            with patch.object(ci, 'ROOT', root):
                ci.validate_build(receipt, 'a' * 40, elf, trace)
                for field in receipt:
                    invalid = {**receipt, field: 'wrong'}
                    with self.subTest(field=field), self.assertRaises(ValueError):
                        ci.validate_build(invalid, 'a' * 40, elf, trace)
                elf.write_bytes(b'old-or-replaced-binary')
                with self.assertRaises(ValueError):
                    ci.validate_build(receipt, 'a' * 40, elf, trace)
                receipt['elf_sha256'] = ci.sha(elf)
                with self.assertRaises(ValueError):
                    ci.validate_build(receipt, 'a' * 40, elf, trace)

    def test_old_baseline_guard_is_not_reused_or_weakened(self):
        self.assertEqual(ci.baseline.SOURCE, '177fd36c55d9ebad4ebbedb0564e2ad3f6dfe3ff')
        with self.assertRaises(ValueError):
            ci.baseline.validate_source_diff('M\tsrc/world/mod.rs\n')
        self.assertNotIn('src/world/mod.rs', ci.baseline.ALLOWED_ADDITIONS)


if __name__ == '__main__':
    unittest.main()
