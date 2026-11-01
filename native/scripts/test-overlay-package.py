"""Mutation checks for organized package audit contracts; no native build required."""
from contextlib import contextmanager
from copy import deepcopy
from hashlib import sha256
import importlib.util
from io import BytesIO
import json
from pathlib import Path
import struct
import tempfile
import unittest
import zipfile

from build_identity import native_inputs, source_digest


SCRIPTS = Path(__file__).resolve().parent
ROOT = SCRIPTS.parents[1]
spec = importlib.util.spec_from_file_location('overlay_package', SCRIPTS / 'check-overlay-package.py')
overlay = importlib.util.module_from_spec(spec)
spec.loader.exec_module(overlay)


class BenchmarkContract(unittest.TestCase):
    def setUp(self):
        self.plan = json.loads((ROOT / 'native/qualification/benchmark-plan.json').read_text())

    def test_current_plan_has_one_comparison_and_full_duration_battle(self):
        self.assertEqual(overlay.verify_plan(self.plan), len(self.plan['cases']))

    def test_battle_cannot_be_shortened_or_change_requested_rate(self):
        for field, value in [('target_ticks', 23999), ('requested_rate', 2), ('mode', 'cpu')]:
            with self.subTest(field=field):
                plan = deepcopy(self.plan)
                battle = next(case for case in plan['cases'] if case['id'] == 'window-battle-20m-4x')
                battle[field] = value
                with self.assertRaises(AssertionError):
                    overlay.verify_plan(plan)

    def test_other_probe_cannot_become_an_extra_4x_comparison(self):
        smoke = next(case for case in self.plan['cases'] if case['id'] == 'smoke-cpu')
        smoke['requested_rate'] = 4
        with self.assertRaises(AssertionError):
            overlay.verify_plan(self.plan)

    def test_second_ramp_or_repeated_rate_case_is_rejected(self):
        for duplicate_ramp in [True, False]:
            with self.subTest(duplicate_ramp=duplicate_ramp):
                plan = deepcopy(self.plan)
                extra = deepcopy(next(case for case in plan['cases'] if case.get('speed_ramp')))
                extra['id'] = 'extra-ramp' if duplicate_ramp else 'window-rate-004'
                if not duplicate_ramp:
                    del extra['speed_ramp']
                plan['cases'].append(extra)
                with self.assertRaises(AssertionError):
                    overlay.verify_plan(plan)

    def test_missing_battle_is_rejected(self):
        self.plan['cases'] = [case for case in self.plan['cases'] if case['id'] != 'window-battle-20m-4x']
        with self.assertRaisesRegex(AssertionError, 'Missing the explicit'):
            overlay.verify_plan(self.plan)


@contextmanager
def native_snapshot():
    with tempfile.TemporaryDirectory(prefix='last-world-source-contract-') as directory:
        root = Path(directory)
        for name in ['native/Cargo.toml', 'native/Cargo.lock', 'native/src/main.rs',
                     'native/shaders/render.wgsl', 'native/assets/tiny.json',
                     'native/vendor/PATCHES.md', 'native/qualification/benchmark-plan.json']:
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('fixture bytes for ' + name)
        paths = [path.relative_to(root).as_posix() for path in native_inputs(root)]
        manifest = {'native_source_sha256': source_digest(root), 'source_snapshot': {'native_inputs': paths}}
        data = BytesIO()
        with zipfile.ZipFile(data, 'w') as archive:
            for name in paths:
                archive.writestr('Last-World/Source/' + name, (root / name).read_bytes())
        with zipfile.ZipFile(data) as archive:
            yield root, manifest, archive


class SourceIdentityContract(unittest.TestCase):
    def test_independent_source_inventory_ignores_unowned_build_cache(self):
        with native_snapshot() as (root, manifest, archive):
            cache = root / 'native/target/local-build-cache.txt'
            cache.parent.mkdir(parents=True)
            cache.write_bytes(b'UNOWNED_USER_CACHE')
            self.assertEqual(overlay.verify_packaged_native_source(archive, root, manifest), 7)
            self.assertEqual(cache.read_bytes(), b'UNOWNED_USER_CACHE')

    def test_added_compilation_input_cannot_be_hidden_by_old_inventory(self):
        with native_snapshot() as (root, manifest, archive):
            (root / 'native/src/new_module.rs').write_text('pub fn newly_compiled() {}')
            with self.assertRaisesRegex(AssertionError, 'path set differs'):
                overlay.verify_packaged_native_source(archive, root, manifest)

    def test_extracted_compilation_bytes_must_equal_archive_bytes(self):
        with native_snapshot() as (root, manifest, archive):
            (root / 'native/src/main.rs').write_text('changed after extraction')
            with self.assertRaisesRegex(AssertionError, 'differs from archive'):
                overlay.verify_packaged_native_source(archive, root, manifest)

    def test_source_hash_must_match_the_build_identity(self):
        with native_snapshot() as (root, manifest, archive):
            manifest['native_source_sha256'] = sha256(b'foreign build').hexdigest()
            with self.assertRaisesRegex(AssertionError, 'differ from executable build identity'):
                overlay.verify_packaged_native_source(archive, root, manifest)


class WindowsExecutableContract(unittest.TestCase):
    def test_three_current_build_outputs_are_x64_pe32plus(self):
        release = ROOT / 'native/target/x86_64-pc-windows-gnu/release'
        stamp = json.loads((release / 'native-build-stamp.json').read_text())
        paths = [release / 'last-world.exe'] + [ROOT / entry['path'] for entry in stamp['test_harnesses']]
        self.assertEqual(len(paths), 3)
        for path in paths:
            with self.subTest(path=path.name):
                overlay.verify_windows_pe(path.read_bytes(), path.name)

    def test_dos_stub_or_wrong_architecture_cannot_masquerade_as_windows_build(self):
        data = bytearray(128)
        data[:2] = b'MZ'
        struct.pack_into('<I', data, 0x3c, 64)
        for signature, architecture, optional_kind in [(b'NOPE', 0x8664, 0x20b), (b'PE\0\0', 0x14c, 0x10b)]:
            with self.subTest(signature=signature, architecture=architecture):
                data[64:68] = signature
                struct.pack_into('<H', data, 68, architecture)
                struct.pack_into('<H', data, 88, optional_kind)
                with self.assertRaises(AssertionError):
                    overlay.verify_windows_pe(data, 'invalid.exe')


if __name__ == '__main__':
    unittest.main()
