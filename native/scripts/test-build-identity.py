"""Bounded build-stamp QA using temporary inputs, never production source edits."""
from pathlib import Path
import sys
import tempfile
import unittest

sys.dont_write_bytecode = True
from build_identity import native_inputs, source_digest


class BuildIdentityTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='last-world-build-identity-')
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        repository = Path(__file__).resolve().parents[2]
        names = (
            'native/Cargo.toml', 'native/Cargo.lock', 'native/src/lib.rs',
            'native/shaders/render.wgsl', 'native/assets/field-reference.json',
            'native/vendor/PATCHES.md', 'native/vendor/wgpu/Cargo.toml',
            'native/vendor/wgpu/Cargo.toml.orig', 'native/vendor/wgpu/build.rs',
            'native/vendor/wgpu/src/lib.rs', 'native/vendor/wgpu/src/util/blit.wgsl',
            'native/qualification/benchmark-plan.json',
            'native/scripts/build-release.py', 'docs/UI_REQUIREMENTS.md',
        )
        # Read current source bytes, then mutate only independent temporary
        # copies. No fixture write ever targets the production repository.
        self.inputs = {name: (repository / name).read_bytes() for name in names}
        for name, data in self.inputs.items():
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)

    def test_vendor_manifest_build_rust_and_wgsl_edits_invalidate_identity(self):
        baseline = source_digest(self.root)
        selected = {p.relative_to(self.root).as_posix() for p in native_inputs(self.root)}
        for name in (
            'native/vendor/wgpu/Cargo.toml',
            'native/vendor/wgpu/Cargo.toml.orig',
            'native/vendor/wgpu/build.rs',
            'native/vendor/wgpu/src/lib.rs',
            'native/vendor/wgpu/src/util/blit.wgsl',
            'native/qualification/benchmark-plan.json',
        ):
            with self.subTest(input=name):
                self.assertIn(name, selected)
                path = self.root / name
                path.write_bytes(self.inputs[name] + b'\nchanged fixture input\n')
                self.assertNotEqual(source_digest(self.root), baseline)
                path.write_bytes(self.inputs[name])
                self.assertEqual(source_digest(self.root), baseline)

    def test_scripts_docs_and_generated_vendor_caches_do_not_change_native_identity(self):
        baseline = source_digest(self.root)
        for name in ('native/scripts/build-release.py', 'docs/UI_REQUIREMENTS.md'):
            (self.root / name).write_bytes(b'changed noncompilation input\n')
        for name in (
            'native/vendor/wgpu/target/debug/dependency-cache.txt',
            'native/vendor/wgpu/__pycache__/tool.pyc',
        ):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b'generated cache is not source\n')
        self.assertEqual(source_digest(self.root), baseline)


if __name__ == '__main__':
    unittest.main(verbosity=2)
