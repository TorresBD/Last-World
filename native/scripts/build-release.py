"""Build a logged release and reject inputs that change during compilation."""
from pathlib import Path
from datetime import datetime, timezone
from hashlib import sha256
import argparse
import json
import os
import subprocess
import sys
from build_identity import native_inputs, source_digest

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--target')
parser.add_argument('--cargo', default=os.environ.get('CARGO', 'cargo'))
parser.add_argument('--force-project', action='store_true',
                    help='Recompile this project after an interrupted or concurrent-edit build')
parser.add_argument('--test-harnesses', action='store_true',
                    help='Also build standalone release library/CLI test runners for the supervised PC suite')
args = parser.parse_args()
root = Path(__file__).resolve().parents[2]
if args.force_project:
    (root / 'native/src/lib.rs').touch()
before = source_digest(root)
before_times = {str(p): p.stat().st_mtime_ns for p in native_inputs(root)}
command = [sys.executable, str(root / 'native/scripts/run.py'), args.cargo,
           'build', '--manifest-path', str(root / 'native/Cargo.toml'), '--locked', '--release']
if args.target:
    command += ['--target', args.target]
result = subprocess.run(command, cwd=root)
if result.returncode:
    raise SystemExit(result.returncode)
test_harnesses = []
if args.test_harnesses:
    test_command = [sys.executable, str(root / 'native/scripts/run.py'), args.cargo,
                    'test', '--manifest-path', str(root / 'native/Cargo.toml'), '--locked',
                    '--release', '--no-run', '--lib', '--bin', 'last-world', '--message-format=json']
    if args.target:
        test_command += ['--target', args.target]
    built = subprocess.run(test_command, cwd=root, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    print(built.stdout, end='')
    if built.returncode:
        raise SystemExit(built.returncode)
    artifacts = {}
    for line in built.stdout.splitlines():
        try:
            artifact = json.loads(line)
        except (ValueError, TypeError):
            continue
        if artifact.get('reason') != 'compiler-artifact' or not artifact.get('profile', {}).get('test') or not artifact.get('executable'):
            continue
        target_name = artifact.get('target', {}).get('name')
        suite = {'last_world_native': 'library', 'last-world': 'cli'}.get(target_name)
        if suite:
            artifacts[suite] = Path(artifact['executable'])
    if set(artifacts) != {'library', 'cli'}:
        raise SystemExit('Expected exactly the library and CLI test executable artifacts; no release stamp accepted')
    for suite, path in sorted(artifacts.items()):
        entry = {'suite': suite, 'path': path.relative_to(root).as_posix(),
                 'sha256': sha256(path.read_bytes()).hexdigest(), 'bytes': path.stat().st_size,
                 'inventory_source': None, 'test_names': []}
        # A host inventory is an expected cross-target list, not proof that the
        # Windows cfg selected identical tests. The PC supervisor enumerates its
        # actual Windows runners before launching exact individual test cases.
        if not args.target or (os.name == 'nt' and 'windows' in args.target):
            listed = subprocess.run([str(path), '--list', '--format', 'terse'], cwd=root,
                                    capture_output=True, text=True, check=True)
            entry['test_names'] = [line[:-6] for line in listed.stdout.splitlines() if line.endswith(': test')]
            if not entry['test_names']:
                raise SystemExit('Empty host test inventory; no release stamp accepted')
            entry['inventory_source'] = ('actual Windows runtime inventory' if os.name == 'nt' else 'host_cfg_expected; Windows runtime discovery required')
        test_harnesses.append(entry)
after = source_digest(root)
after_times = {str(p): p.stat().st_mtime_ns for p in native_inputs(root)}
if before != after or before_times != after_times:
    raise SystemExit('Native inputs changed during compilation; no build stamp was accepted. '
                     'Freeze edits and rerun with --force-project.')
directory = root / 'native/target'
if args.target:
    directory /= args.target
directory /= 'release'
windows = ('windows' in args.target) if args.target else os.name == 'nt'
exe = directory / ('last-world.exe' if windows else 'last-world')
if not exe.is_file():
    raise SystemExit('Expected release executable is missing; no build stamp accepted')
head = subprocess.run(['git', 'rev-parse', 'HEAD'], cwd=root, capture_output=True, text=True)
status = subprocess.run(['git', 'status', '--porcelain'], cwd=root, capture_output=True, text=True)
stamp = {
    'schema_version': 1,
    'target': args.target or 'host',
    'built_at_utc': datetime.now(timezone.utc).isoformat(),
    'source_commit': head.stdout.strip() if head.returncode == 0 else None,
    'native_source_sha256': after,
    'executable_sha256': sha256(exe.read_bytes()).hexdigest(),
    'source_frozen_through_build': True,
    'worktree_dirty': bool(status.stdout.strip()) if status.returncode == 0 else None,
    'test_harnesses': test_harnesses,
}
path = directory / 'native-build-stamp.json'
temporary = path.with_suffix('.json.tmp')
temporary.write_text(json.dumps(stamp, indent=2) + '\n')
os.replace(temporary, path)
line = '[BUILD_STAMP] ' + json.dumps(stamp, sort_keys=True)
with (root / 'console.txt').open('a') as log:
    log.write(line + '\n')
print(line)
