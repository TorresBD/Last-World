"""Verify an organized update ZIP and overlay it without migrating user data."""
from pathlib import Path, PurePosixPath
from hashlib import sha256
import argparse
import json
import struct
import tempfile
import zipfile
from build_identity import native_inputs


ROOT_FILES = {'Run-Game.cmd', 'Run-Checks.cmd', 'README.txt'}
TOOLS = {
    'Run-Checks.ps1', 'Run-Checks-OwnedProcess.ps1', 'Run-Checks-Feedback.ps1',
    'Run-Checks-GpuTelemetry.ps1', 'Initialize-Layout.ps1', 'Run-Checks-Watchdog.cmd', 'Check-Progress.cmd',
}
EXECUTABLES = {'Last-World.exe', 'Last-World-Mechanisms.exe', 'Last-World-Cli-Checks.exe'}
MANIFEST_PATH = 'App/build-manifest.json'
MUTABLE_SENTINELS = {
    'campaign-history.jsonl': b'{"old":"campaign"}\n',
    'console.txt': b'old-session\n[USER_HISTORY_MUST_SURVIVE]\n',
    'river-history.jsonl': b'{"old":"river"}\n',
    'ground-history.jsonl': b'{"old":"ground"}\n',
    'benchmark-history.jsonl': b'{"old":"benchmark"}\n',
    'pc-host-history.jsonl': b'{"old":"host"}\n',
    'settings.json': b'{"drive":"D:","custom":true}\n',
    'saves/ongoing-war.save': b'EXISTING_SAVE_DATA',
    'saves/campaign.json': b'EXISTING_CAMPAIGN_SAVE',
    'saves/campaign.json.prev': b'PREVIOUS_CAMPAIGN_SAVE',
    'check-results/previous/run-summary.json': b'{"previous":true}\n',
    'check-results/current-suite.json': b'{"run_id":"previous","build_key":"old"}\n',
    'check-results/previous/suite-journal.json': b'{"completed_cases":["old-case"]}\n',
    'check-results/previous/benchmark/checkpoints/cpu-baseline-78421.json': b'EXISTING_OWNED_BENCHMARK_CHECKPOINT',
    'Send-Back.zip': b'PREVIOUS_FEEDBACK_SNAPSHOT',
    'settings/ui-layout.json': b'EXISTING_UI_LAYOUT',
    'Data/console.txt': b'EXISTING_ORGANIZED_SESSION',
    'Data/campaign-history.jsonl': b'{"organized":"campaign"}\n',
    'Data/river-history.jsonl': b'{"organized":"river"}\n',
    'Data/ground-history.jsonl': b'{"organized":"ground"}\n',
    'Data/benchmark-history.jsonl': b'{"organized":"benchmark"}\n',
    'Data/pc-host-history.jsonl': b'{"organized":"host"}\n',
    'Data/settings.json': b'{"organized_settings":true}\n',
    'Data/settings/ui-layout.json': b'EXISTING_ORGANIZED_UI_LAYOUT',
    'Data/saves/campaign.json': b'EXISTING_ORGANIZED_SAVE',
    'Data/saves/campaign.json.prev': b'EXISTING_ORGANIZED_PREVIOUS_SAVE',
    'Data/check-results/current-suite.json': b'{"run_id":"organized-old","build_key":"old"}\n',
    'Data/check-results/organized-old/suite-journal.json': b'{"completed_cases":["organized-case"]}\n',
    'Data/check-results/organized-old/benchmark/checkpoints/cpu-baseline-78421.json': b'EXISTING_ORGANIZED_CHECKPOINT',
    'Data/Locks/Run-Checks.lock': b'PERSISTENT_UNLOCKED_LOCK_SENTINEL',
    'Data/Legacy/previous/conflicting-console.txt': b'PREVIOUS_SAFETY_COPY',
    'Source/LOCAL-NOTES.txt': b'UNOWNED_SOURCE_NOTES',
    'Source/console.txt': b'LOCAL_SOURCE_SESSION_LOG',
    'Source/settings/ui-layout.json': b'LOCAL_SOURCE_UI_LAYOUT',
    'Source/saves/ongoing-war.save': b'LOCAL_SOURCE_SAVE',
    'Source/check-results/previous/report.json': b'LOCAL_SOURCE_CHECK_RESULT',
    'Source/native/target/local-build-cache.txt': b'LOCAL_SOURCE_BUILD_CACHE',
}



def verify_project_records(archive):
    """Reject a future update that omits its maintained source-local records."""
    for relative in ('project/MASTER.md', 'project/LEDGER.md'):
        name = 'Last-World/Source/' + relative
        assert name in archive.namelist(), 'Missing source-local project record: ' + relative
        assert archive.read(name).decode('utf-8').strip(), 'Empty source-local project record: ' + relative

    for legacy in ('RULES.md', 'STATE.md', 'DECISIONS.md', 'BLOCKERS.md', 'PARITY.md'):
        assert 'Last-World/Source/' + legacy not in archive.namelist(), 'Superseded source-root governance record: ' + legacy
        if legacy != 'STATE.md':
            assert 'Last-World/Documentation/project-records/' + legacy not in archive.namelist(), 'Superseded documentation governance record: ' + legacy


def verify_packaged_native_source(archive, source_root, manifest):
    """Recompute compilation identity from the independently extracted Source tree."""
    paths = [p.relative_to(source_root).as_posix() for p in native_inputs(source_root)]
    assert paths == manifest['source_snapshot']['native_inputs'], 'Packaged native compilation-input path set differs'
    digest = sha256()
    for name in paths:
        data = (source_root / name).read_bytes()
        assert data == archive.read('Last-World/Source/' + name), 'Extracted native source differs from archive: ' + name
        digest.update(name.encode('utf-8'))
        digest.update(b'\0')
        digest.update(data)
    assert digest.hexdigest() == manifest['native_source_sha256'], 'Packaged native source bytes differ from executable build identity'
    return len(paths)


def verify_windows_pe(data, name):
    assert len(data) >= 64 and data[:2] == b'MZ', 'Missing Windows DOS header: ' + name
    pe_offset = struct.unpack_from('<I', data, 0x3c)[0]
    assert pe_offset + 26 <= len(data) and data[pe_offset:pe_offset + 4] == b'PE\0\0', 'Missing Windows PE header: ' + name
    assert struct.unpack_from('<H', data, pe_offset + 4)[0] == 0x8664, 'Expected x64 Windows PE: ' + name
    assert struct.unpack_from('<H', data, pe_offset + 24)[0] == 0x20b, 'Expected PE32+ optional header: ' + name


def verify_plan(plan):
    cases = plan['cases']
    assert len(cases) == len({c['id'] for c in cases}), 'Duplicate benchmark case IDs'
    ramps = [c for c in cases if c.get('speed_ramp')]
    assert len(ramps) == 1, 'Expected exactly one adaptive speed-ramp process'
    ramp = ramps[0]
    assert ramp['id'] == 'window-speed-ramp' and ramp['mode'] == 'window' and ramp['requested_rate'] == 1
    assert ramp['speed_ramp']['rates'] == [1, 2, 4, 8, 16, 32, 64, 100]
    assert ramp['speed_ramp']['initial_warmup_seconds'] >= 15
    assert ramp['speed_ramp']['settle_seconds'] >= 5
    assert ramp['speed_ramp']['window_seconds'] >= 10
    assert ramp['speed_ramp']['minimum_windows'] >= 2
    battles = [c for c in cases if c['id'] == 'window-battle-20m-4x']
    assert len(battles) == 1, 'Missing the explicit 20-minute 4x rendered battle'
    battle = battles[0]
    assert battle['mode'] == 'window' and battle['requested_rate'] == 4 and battle['target_ticks'] == 24000, 'Battle must render exactly 24000 simulated ticks at requested 4x'
    assert not battle.get('speed_ramp'), 'Battle is a separate full-duration workload, not a speed comparison'
    assert all(0 < c['requested_rate'] <= 2 for c in cases if c is not battle), 'Only the explicit battle may request more than 2x outside the adaptive ramp'
    assert not any(c['id'].startswith(('cpu-rate-', 'window-rate-')) for c in cases), 'Legacy repeated-load speed cases remain'
    assert plan['normal_play_speed_options'] == [1, 2, 4]
    assert plan['benchmark_steps_per_call'] == 64
    return len(cases)


def verify_archive(archive_path):
    with zipfile.ZipFile(archive_path) as archive:
        assert archive.testzip() is None, 'ZIP CRC failure'
        names = archive.namelist()
        assert len(names) == len(set(names)), 'Duplicate ZIP names'
        assert len(names) == len({name.casefold() for name in names}), 'Windows-case-colliding ZIP names'
        for name in names:
            path = PurePosixPath(name)
            assert '\\' not in name and not path.is_absolute() and '..' not in path.parts and path.parts[0] == 'Last-World' and path.as_posix() == name, 'Unsafe or noncanonical ZIP path: ' + name
        manifest = json.loads(archive.read('Last-World/' + MANIFEST_PATH))
        assert manifest['update_contract']['deletes_existing_files'] is False
        assert manifest['update_contract']['mutable_session_save_settings_files_in_payload'] is False
        assert manifest['layout_version'] == manifest['layout']['schema_version'] == manifest['update_contract']['layout_version'] == 1, 'Missing organized layout version'
        layout = manifest['layout']
        for key, value in {
            'root_launchers': ['Run-Game.cmd', 'Run-Checks.cmd'],
            'application': 'App/Last-World.exe',
            'test_executables': ['App/Last-World-Mechanisms.exe', 'App/Last-World-Cli-Checks.exe'],
            'build_manifest': MANIFEST_PATH,
            'native_build_stamp': 'App/native-build-stamp.json',
            'benchmark_plan': 'App/benchmark-plan.json',
            'benchmark_unit_tests': 'App/benchmark-unit-tests.json',
            'ui': 'App/ui/', 'licenses': 'App/licenses/',
            'tools': 'Tools/', 'initialization': 'Tools/Initialize-Layout.ps1',
            'watchdog': 'Tools/Run-Checks-Watchdog.cmd',
            'documentation': 'Documentation/', 'source': 'Source/',
            'data': 'Data/', 'feedback': 'Send-Back.zip',
        }.items():
            assert layout.get(key) == value, 'Organized layout contract differs: ' + key
        expected = manifest['payload_files']
        assert MANIFEST_PATH not in expected, 'Manifest must not hash itself'
        assert set(names) == {'Last-World/' + name for name in expected} | {'Last-World/' + MANIFEST_PATH}, 'Manifest/payload disagreement'
        assert {name for name in expected if '/' not in name} == ROOT_FILES, 'Root must contain only two launchers and README.txt'
        assert {PurePosixPath(name).parts[0] for name in expected if '/' in name} == {'App', 'Tools', 'Documentation', 'Source'}, 'Unexpected organized payload folder or mutable Data payload'
        assert {name.removeprefix('Tools/') for name in expected if name.startswith('Tools/')} == TOOLS, 'Unexpected/missing launcher support tool'
        for name, meta in expected.items():
            data = archive.read('Last-World/' + name)
            assert len(data) == meta['bytes'] and sha256(data).hexdigest() == meta['sha256'], 'Payload hash mismatch: ' + name
        snapshot = manifest.get('source_snapshot')
        assert snapshot and snapshot['folder'] == 'Source/', 'Missing same-folder source snapshot contract'
        assert snapshot['native_source_sha256'] == manifest['native_source_sha256'], 'Source snapshot/executable native identity differs'
        assert snapshot['mutable_session_files_included'] is False
        source_names = sorted(name for name in expected if name.startswith('Source/'))
        verify_project_records(archive)
        for name in [
            'Source/SOURCE-README.txt', 'Source/project/MASTER.md', 'Source/project/LEDGER.md',
            'Source/native/Cargo.toml', 'Source/native/Cargo.lock',
            'Source/native/src/main.rs', 'Source/native/src/lib.rs',
            'Source/native/shaders/render.wgsl', 'Source/native/assets/pixel-atlas.png',
            'Source/native/vendor/PATCHES.md', 'Source/native/vendor/wgpu/Cargo.toml',
            'Source/native/vendor/wgpu/src/lib.rs',
            'Source/native/scripts/package-windows.py',
            'Source/native/scripts/check-overlay-package.py',
            'Source/native/examples/ui-preview.rs',
            'Source/native/qualification/milestone-a-scenarios.json',
            'Source/docs/MILESTONE_B_VERIFICATION.md',
            'Source/dist/config.mjs', 'Source/dist/pixel-forms.mjs',
        ]:
            assert name in expected, 'Missing source snapshot input: ' + name
        assert {name for name in source_names if name.startswith('Source/dist/')} == {'Source/dist/config.mjs', 'Source/dist/pixel-forms.mjs'}, 'Source snapshot includes an unexpected browser distribution/runtime'
        forbidden_parts = {
            '.git', '__pycache__', 'node_modules', 'target', 'downloads', 'runtime',
            'saves', 'settings', 'check-results', 'console.txt', 'settings.json',
            'Send-Back.zip', 'campaign-history.jsonl', 'river-history.jsonl',
            'ground-history.jsonl', 'benchmark-history.jsonl', 'pc-host-history.jsonl',
        }
        digest = sha256()
        source_bytes = 0
        for name in source_names:
            assert not forbidden_parts.intersection(PurePosixPath(name).parts), 'Mutable/runtime file inside Source: ' + name
            assert not name.lower().endswith(('.zip', '.exe', '.dll', '.wasm', '.pyc')), 'Compiled/archive payload inside Source: ' + name
            data = archive.read('Last-World/' + name)
            digest.update(name.encode('utf-8'))
            digest.update(b'\0')
            digest.update(data)
            source_bytes += len(data)
        assert snapshot['files'] == len(source_names) and snapshot['bytes'] == source_bytes, 'Source snapshot size/count mismatch'
        assert snapshot['sha256'] == digest.hexdigest(), 'Source snapshot aggregate hash mismatch'
        for name in ROOT_FILES - {'README.txt'}:
            assert archive.read('Last-World/' + name) == archive.read('Last-World/Source/native/launchers/' + name), 'Root launcher differs from matching Source bytes: ' + name
        for name in TOOLS:
            assert archive.read('Last-World/Tools/' + name) == archive.read('Last-World/Source/native/launchers/' + name), 'Runtime helper differs from matching Source bytes: ' + name
        for name in ['Run-Benchmark.cmd', 'Run-PC-Qualification.cmd', 'Run-River-Test.cmd', 'Run-Visual-Test.cmd', 'Run-Water-Check.cmd', 'Run-PC-Qualification.ps1']:
            assert 'Source/native/launchers/' + name in expected, 'Original launcher missing from Source: ' + name
        for name in ['Documentation/README.md', 'Documentation/THIRD-PARTY-NOTICES.txt', 'Documentation/docs/UPDATE_AND_FEEDBACK.md', 'Documentation/project-records/STATE.md']:
            assert name in expected, 'Missing organized documentation: ' + name
        for source_name in source_names:
            if source_name.startswith('Source/native/assets/ui/'):
                runtime_name = 'App/ui/' + source_name.removeprefix('Source/native/assets/ui/')
                assert runtime_name in expected and archive.read('Last-World/' + runtime_name) == archive.read('Last-World/' + source_name), 'Runtime UI asset differs from matching Source bytes: ' + source_name
        for name in EXECUTABLES:
            assert 'App/' + name in expected, 'Missing Windows executable: ' + name
            verify_windows_pe(archive.read('Last-World/App/' + name), name)
        assert sha256(archive.read('Last-World/App/Last-World.exe')).hexdigest() == manifest['executable_sha256']
        feedback = manifest['pc_feedback']
        assert feedback['run'] == 'Run-Checks.cmd' and feedback['send_back_only'] == 'Send-Back.zip'
        assert feedback['campaign_cpu_command'] == '--campaign-report'
        assert feedback['campaign_check_command'] == '--campaign-check --speed 4'
        stress = manifest['stress_suite']
        assert stress['plan'] == 'App/benchmark-plan.json' and stress['mechanism_catalog'] == 'App/benchmark-unit-tests.json'
        assert stress['hardware_telemetry']['helper'] == 'Tools/Run-Checks-GpuTelemetry.ps1'
        plan_bytes = archive.read('Last-World/App/benchmark-plan.json')
        assert plan_bytes == archive.read('Last-World/Source/native/qualification/benchmark-plan.json'), 'Runtime/compiled benchmark plans differ'
        benchmark_case_count = verify_plan(json.loads(plan_bytes))
        catalog = json.loads(archive.read('Last-World/App/benchmark-unit-tests.json'))
        stamp = json.loads(archive.read('Last-World/App/native-build-stamp.json'))
        assert stamp['native_source_sha256'] == manifest['native_source_sha256'] and stamp['executable_sha256'] == manifest['executable_sha256'] and stamp['source_frozen_through_build'] is True, 'Adjacent native identity differs from frozen payload'
        assert catalog['native_source_sha256'] == manifest['native_source_sha256'], 'Test catalog source identity is stale'
        assert catalog['executables'] and catalog['tests'], 'Empty mechanism test inventory'
        assert 'Windows' in catalog['inventory_source'], 'Cross-target inventory limitation must be explicit'
        harnesses = {entry['suite']: entry for entry in stamp['test_harnesses']}
        assert set(harnesses) == {'library', 'cli'} and len(stamp['test_harnesses']) == 2, 'Frozen harness inventory differs'
        executable_names = set()
        catalog_counts = {}
        for entry in catalog['executables']:
            executable_names.add(entry['executable'])
            assert entry['executable_path'] == 'App/' + entry['executable'], 'Test catalog install-relative executable path differs'
            data = archive.read('Last-World/' + entry['executable_path'])
            assert sha256(data).hexdigest() == entry['sha256'] == harnesses[entry['id']]['sha256'], 'Mechanism executable hash differs'
            assert len(data) == harnesses[entry['id']]['bytes'], 'Mechanism executable size differs'
            catalog_counts[entry['executable']] = sum(t['executable'] == entry['executable'] for t in catalog['tests'])
            assert entry['expected_test_count'] == catalog_counts[entry['executable']], 'Executable test count differs from host inventory metadata'
        assert executable_names == EXECUTABLES - {'Last-World.exe'} and len(catalog['executables']) == 2
        assert len({t['id'] for t in catalog['tests']}) == len(catalog['tests']), 'Duplicate mechanism phase IDs'
        assert len({(t['executable'], t['test_name']) for t in catalog['tests']}) == len(catalog['tests']), 'Duplicate mechanism test names'
        assert all(t['executable'] in executable_names and t['test_name'] and t['timeout_seconds'] > 0 for t in catalog['tests'])
        assert all(catalog_counts.values()), 'Empty executable test catalog'
        assert catalog['expected_test_count'] == len(catalog['tests']) == sum(catalog_counts.values()), 'Total test catalog count differs'
        with tempfile.TemporaryDirectory(prefix='last-world-overlay-') as folder:
            overlay_root = Path(folder)
            for name, data in MUTABLE_SENTINELS.items():
                target = overlay_root / 'Last-World' / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(data)
            archive.extractall(overlay_root)
            native_input_count = verify_packaged_native_source(archive, overlay_root / 'Last-World/Source', manifest)
            for name, data in MUTABLE_SENTINELS.items():
                assert (overlay_root / 'Last-World' / name).read_bytes() == data, 'Existing user data overwritten during ZIP merge: ' + name
    return {'archive': str(archive_path), 'layout_version': 1, 'crc': 'passed', 'payload_hashes': 'passed', 'source_snapshot': 'included_and_verified', 'source_files': len(source_names), 'source_bytes': source_bytes, 'native_source_bytes': 'independently_rehashed', 'native_compilation_inputs': native_input_count, 'windows_pe_executables': len(EXECUTABLES), 'benchmark_cases': benchmark_case_count, 'adaptive_speed_ramps': 1, 'battle_case': 'window-battle-20m-4x:24000ticks', 'catalog_test_counts': catalog_counts, 'catalog_tests': len(catalog['tests']), 'zip_merge_existing_root_and_Data_logs_settings_saves_checkpoints_feedback_and_unowned_source_files': 'preserved', 'first_launch_migration': 'not_run_by_zip_overlay_audit', 'mutable_sentinels': len(MUTABLE_SENTINELS), 'entries': len(names)}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('archive', type=Path)
    args = parser.parse_args()
    print(json.dumps(verify_archive(args.archive)))
