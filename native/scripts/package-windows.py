"""Package an already built native Windows fixture; no engine build is implicit."""
from pathlib import Path
from hashlib import sha256
import argparse
import json
import os
import subprocess
import tomllib
import zipfile
from build_identity import native_inputs, source_digest

parser = argparse.ArgumentParser(description=__doc__)
mode = parser.add_mutually_exclusive_group()
mode.add_argument('--river-foundation', action='store_true', help='Create the separately named river-foundation ZIP; preserve the earlier qualification ZIP')
mode.add_argument('--update-bundle', action='store_true', help='Create a merge-only Last-World folder with one checks launcher and no mutable player/session files')
args = parser.parse_args()
root = Path(__file__).resolve().parents[2]
exe = root / 'native/target/x86_64-pc-windows-gnu/release/last-world.exe'
if not exe.is_file():
    raise SystemExit('Build the Windows release first')
inputs = native_inputs(root)
if any(p.stat().st_mtime_ns > exe.stat().st_mtime_ns for p in inputs):
    raise SystemExit('Windows executable predates native source/assets; rebuild before packaging')
source_hash = source_digest(root)
if args.update_bundle:
    stamp_path = exe.parent / 'native-build-stamp.json'
    if not stamp_path.is_file():
        raise SystemExit('Update packaging requires build-release.py source-freeze verification')
    stamp = json.loads(stamp_path.read_text())
    if (stamp.get('source_frozen_through_build') is not True
            or stamp.get('native_source_sha256') != source_hash
            or stamp.get('executable_sha256') != sha256(exe.read_bytes()).hexdigest()):
        raise SystemExit('Release stamp differs from source/executable; rebuild frozen inputs')

if args.update_bundle:
    stage = 'Milestone B small native campaign integration and single PC feedback batch'
    scope = ('Small4.8x3.2km native Milestone B campaign integration with CPU-authoritative gameplay and the existing wgpu renderer. '
             'Run-Game.cmd opens --campaign. Run-Checks.cmd batches CPU campaign/ground reports, Vulkan/DX12 isolated movement checks, and automatic river/campaign rendering. '
             'Initial30 additional mechanism target plus required biology; not the20km/all160 finished release or a full-workload performance guarantee. '
             'See Source/project/MASTER.md for current instructions and Source/project/LEDGER.md for selective history; older project records are historical.')
    filename = 'Last-World-Update-Windows.zip'
    prefix = 'Last-World/'
elif args.river_foundation:
    stage = 'Milestone B river-flow and terrain foundation'
    scope = ('CPU-conserved local river storage/flow, gradual dam filling/overflow/breach and retained coarse editable terrain with partial uploads. '
             'River mode contains zero military/civilian/wildlife agents; the movement benchmark is a separate mode. '
             'Not integrated construction/ecology/naval combat, saves, seasonal environment or the20km campaign.')
    filename = 'Last-World-River-Foundation-Windows.zip'
    prefix = ''
else:
    stage = 'Milestone A PC qualification setup'
    scope = ('Isolated GPU movement, spatial indexing, visibility and rendering. Not the20km integrated military/civilian/ecology campaign. '
             'Qualification scenarios are documentation, not active simulation.')
    filename = 'Last-World-PC-Qualification-Windows.zip'
    prefix = ''

manifest = {
    'application': 'Last World',
    'stage': stage,
    'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip(),
    'native_source_sha256': source_hash,
    'executable_sha256': sha256(exe.read_bytes()).hexdigest(),
    'cargo_lock_sha256': sha256((root / 'native/Cargo.lock').read_bytes()).hexdigest(),
    'scope': scope,
}
if args.river_foundation:
    manifest['river_fixture'] = {'water_grid_cells': [96, 128], 'terrain_cell_meters': 8, 'agents': 0, 'interactive': '--river', 'cpu_correctness_report': '--water-report', 'report_duration_action_ticks': 4800, 'seed': 78421, 'performance_qualification': False}
if args.update_bundle:
    manifest['layout_version'] = 1
    manifest['layout'] = {
        'schema_version': 1,
        'root_launchers': ['Run-Game.cmd', 'Run-Checks.cmd'],
        'application': 'App/Last-World.exe',
        'test_executables': ['App/Last-World-Mechanisms.exe', 'App/Last-World-Cli-Checks.exe'],
        'build_manifest': 'App/build-manifest.json',
        'native_build_stamp': 'App/native-build-stamp.json',
        'benchmark_plan': 'App/benchmark-plan.json',
        'benchmark_unit_tests': 'App/benchmark-unit-tests.json',
        'ui': 'App/ui/', 'licenses': 'App/licenses/',
        'tools': 'Tools/', 'initialization': 'Tools/Initialize-Layout.ps1',
        'watchdog': 'Tools/Run-Checks-Watchdog.cmd',
        'documentation': 'Documentation/', 'source': 'Source/',
        'data': 'Data/', 'feedback': 'Send-Back.zip',
    }
    manifest['update_contract'] = {'folder': 'Last-World', 'layout_version': 1, 'strategy': 'merge_and_replace_owned_files_only', 'deletes_existing_files': False, 'mutable_session_save_settings_files_in_payload': False, 'first_launch_migration': 'Tools/Initialize-Layout.ps1'}
    manifest['pc_feedback'] = {'run': 'Run-Checks.cmd', 'send_back_only': 'Send-Back.zip', 'stages': ['campaign_cpu', 'ground_cpu', 'movement_vulkan', 'movement_dx12', 'automated_river_render', 'automated_campaign_render'], 'cpu_command': '--ground-report', 'river_command': '--river-check --speed 4', 'campaign_command': '--campaign', 'campaign_cpu_command': '--campaign-report', 'campaign_check_command': '--campaign-check --speed 4'}
    manifest['campaign_scope'] = {'map_meters': [4800, 3200], 'gameplay_authority': 'cpu', 'initial_additional_mechanism_target': 30, 'release_additional_mechanism_target': 160, 'hardware_qualification': 'requires_actual_PC_results_for_this_build'}

# The update payload is an explicit set of application-owned files. It never
# walks a user's existing install or includes a fresh console over their log.
payload = {}
def file(source, name):
    if name in payload:
        raise SystemExit('Duplicate packaged path: ' + name)
    payload[name] = source.read_bytes()


def source_snapshot():
    """Archive only reviewed repository inputs, never an installed user folder."""
    exact = [
        'README.md', 'package.json', '.gitignore',
        'project/MASTER.md', 'project/LEDGER.md',
        'native/README.md', 'native/Cargo.toml', 'native/Cargo.lock',
        'native/vendor/PATCHES.md',
        'native/vendor/wgpu/Cargo.toml', 'native/vendor/wgpu/Cargo.toml.orig',
        'native/vendor/wgpu/Cargo.lock', 'native/vendor/wgpu/build.rs',
        'native/vendor/wgpu/README.md', 'native/vendor/wgpu/LICENSE.APACHE',
        'native/vendor/wgpu/LICENSE.MIT',
        # These are the complete local import closure of export-pixels.mjs.
        'dist/config.mjs', 'dist/pixel-forms.mjs',
    ]
    trees = {
        'project': {'.md', '.txt', '.json'},
        'assets/icons': {'.md', '.json'},
        'native/src': {'.rs'},
        'native/shaders': {'.wgsl'},
        'native/assets': {'.png', '.json', '.ttf', '.txt'},
        'native/vendor/wgpu/src': {'.rs', '.wgsl'},
        'native/scripts': {'.py', '.rs', '.mjs', '.ps1'},
        'native/launchers': {'.cmd', '.ps1'},
        'native/examples': {'.rs'},
        'native/qualification': {'.json'},
        # Keep the source gallery and its explicitly labelled design assets.
        # This does not traverse any old PC screenshots/evidence directories.
        'docs': {'.md', '.json', '.mjs', '.html', '.svg', '.png', '.ttf', '.txt'},
        'evidence/milestone-b-integration-2026-10-30': {'.json', '.jsonl', '.txt', '.log'},
        'evidence/benchmark-integration-2026-10-30': {'.json', '.jsonl', '.txt', '.log'},
        'evidence/pc-stress-2026-10-30': {'.json', '.jsonl', '.txt', '.log'},
        'evidence/pc-repair-2026-10-30': {'.json', '.jsonl', '.txt', '.log'},
        'evidence/layout-2026-10-30': {'.json', '.jsonl', '.txt', '.log'},
        'evidence/palette-2026-10-30': {'.json', '.txt', '.log'},
        'evidence/colour-range-2026-10-30': {'.json', '.jsonl', '.txt', '.log'},
        'evidence/pc-review-2026-10-30-run22544': {'.json', '.txt'},
    }
    excluded_parts = {
        '.git', '__pycache__', 'target', 'node_modules', 'downloads', 'runtime',
        'saves', 'settings', 'check-results', 'console.txt', 'settings.json',
        'Send-Back.zip',
    }
    for name in ('project/MASTER.md', 'project/LEDGER.md'):
        record = root / name
        if not record.is_file() or not record.read_text(encoding='utf-8').strip():
            raise SystemExit('Required project record is missing or empty: ' + name)
    unavailable_evidence = []
    selected = {root / name for name in exact}
    for folder, extensions in trees.items():
        base = root / folder
        if not base.is_dir():
            if folder.startswith('evidence/'):
                unavailable_evidence.append(folder)
                continue
            raise SystemExit('Required source snapshot tree is missing: ' + folder)
        for source in base.rglob('*'):
            relative = source.relative_to(root)
            if any(part in excluded_parts or part.startswith('.') for part in relative.parts):
                continue
            if source.is_file() and source.suffix in extensions:
                selected.add(source)
    for source in sorted(selected):
        if not source.is_file() or source.is_symlink() or not source.resolve().is_relative_to(root.resolve()):
            raise SystemExit('Missing, linked or external source snapshot input: ' + str(source))
        name = source.relative_to(root).as_posix()
        if any(part in excluded_parts for part in Path(name).parts):
            raise SystemExit('Mutable/runtime source snapshot input: ' + name)
        file(source, 'Source/' + name)

    manifest['unavailable_historical_evidence'] = unavailable_evidence

    # The source whitelist must never silently omit a compilation dependency
    # added to build_identity.py, including patched vendor includes/manifests.
    compilation_paths = []
    for source in native_inputs(root):
        name = source.relative_to(root).as_posix()
        packaged = payload.get('Source/' + name)
        if packaged is None or packaged != source.read_bytes():
            raise SystemExit('Source snapshot omits or changes native compilation input: ' + name)
        compilation_paths.append(name)

    worktree = subprocess.check_output(['git', 'status', '--porcelain'], cwd=root, text=True)
    readme = (
        'LAST WORLD - SOURCE SNAPSHOT\n\n'
        'This source is inside the same Last-World update folder as the native executable.\n'
        'It is an application-owned reference snapshot, not a second game install or an update service.\n\n'
        f"Snapshot parent commit: {manifest['source_commit']}\n"
        f"Working tree has changes relative to that commit: {bool(worktree.strip())}\n"
        f'Native compilation-input SHA-256: {source_hash}\n'
        f"Executable SHA-256: {manifest['executable_sha256']}\n\n"
        'The commit identifies ancestry; it does not claim that all snapshot files were committed.\n'
        'The outer ../App/build-manifest.json contains a SHA-256/size for every Source file and an aggregate\n'
        'source_snapshot hash. Its native hash identifies the build-stamped compilation inputs;\n'
        'the complete snapshot hash additionally covers scripts, vendor sources, records and docs.\n\n'
        'Included: native Rust, shaders, baked pixel/font assets and licenses, Cargo manifest/lock,\n'
        'the patched wgpu vendor source/PATCHES.md, scripts, examples, launchers, qualification\n'
        'inputs, project records, docs/design references and current B text/JSON evidence.\n'
        'Only dist/config.mjs and dist/pixel-forms.mjs are included for export-pixels.mjs.\n'
        'No old distribution ZIPs, browser runtime, build targets, caches, installed logs, saves,\n'
        'settings or check-results are copied. Old large PC screenshot trees are omitted.\n'
        'Docs/mockups retain one existing world reference and their labelled design previews;\n'
        'these are not new native execution evidence.\n\n'
        'Build from Source with a suitable Rust/Cargo toolchain (see native/Cargo.toml), using\n'
        'cargo build --manifest-path native/Cargo.toml --locked --release. Other Cargo dependencies\n'
        'still require a registry/network or a populated local Cargo cache: this is not a full\n'
        'offline toolchain/registry bundle. Python3.11+ runs the build/package scripts. Windows\n'
        'cross compilation also needs its target/linker. Baked assets are included; optional\n'
        'regeneration uses Node for export-pixels and Pillow/fontTools for licensed glyph baking.\n'
        'The native build identity covers Cargo inputs, Rust/shaders/assets and the patched\n'
        'vendor tree. Scripts/docs are covered by the separate complete snapshot hash.\n'
        'Overlay QA independently recomputes the native identity from packaged Source bytes.\n'
        'The build-release.py helper can record null Git ancestry when repository metadata is\n'
        'absent; this snapshot intentionally has no .git directory. Direct Cargo builds are\n'
        'independent of Git metadata.\n\n'
        'Scope: the current small native Milestone B development slice. Read project/MASTER.md;\n'
        'selectively search project/LEDGER.md and consult docs/MILESTONE_B_VERIFICATION.md for evidence.\n'
        'Master and Ledger are required source files in every future version; old root records are historical.\n'
        'Including source does not qualify production20km streaming, all160 traits, complete\n'
        'mandatory biology, native-PC visuals, throughput, memory or release gameplay.\n'
    )
    payload['Source/SOURCE-README.txt'] = readme.encode('utf-8')
    digest = sha256()
    source_items = [(name, data) for name, data in sorted(payload.items()) if name.startswith('Source/')]
    for name, data in source_items:
        digest.update(name.encode('utf-8'))
        digest.update(b'\0')
        digest.update(data)
    manifest['source_snapshot'] = {
        'schema_version': 1, 'folder': 'Source/',
        'parent_commit': manifest['source_commit'],
        'worktree_dirty': bool(worktree.strip()),
        'native_source_sha256': source_hash,
        'native_inputs': compilation_paths,
        'sha256': digest.hexdigest(),
        'digest_format': 'sha256 of sorted Source/path UTF-8, NUL, then exact file bytes',
        'files': len(source_items), 'bytes': sum(len(data) for _, data in source_items),
        'selection': 'explicit repository file/tree-and-extension whitelist',
        'full_offline_dependency_registry': False,
        'mutable_session_files_included': False,
    }

app_prefix = 'App/' if args.update_bundle else ''
documentation_prefix = 'Documentation/' if args.update_bundle else ''
file(exe, app_prefix + 'Last-World.exe')
if args.update_bundle:
    payload['App/native-build-stamp.json'] = (json.dumps(stamp, indent=2, sort_keys=True)+'\n').encode()
    # Standalone Rust test executables avoid requiring Cargo/Python on the PC.
    # Actual Windows --list discovery remains authoritative at runtime.
    harnesses = {entry['suite']: entry for entry in stamp.get('test_harnesses', [])}
    host_stamp_path = root / 'native/target/release/native-build-stamp.json'
    if set(harnesses) != {'library', 'cli'}:
        raise SystemExit('Build both release test harnesses before packaging')
    if os.name == 'nt' and all(entry.get('test_names') for entry in harnesses.values()):
        host_stamp = stamp
    elif host_stamp_path.is_file():
        host_stamp = json.loads(host_stamp_path.read_text())
    else:
        raise SystemExit('Build the matching host inventory or enumerate actual Windows harnesses before packaging')
    if host_stamp.get('native_source_sha256') != source_hash:
        raise SystemExit('Host test inventory predates current native inputs')
    host_harnesses = {entry['suite']: entry for entry in host_stamp.get('test_harnesses', [])}
    catalog = {'schema_version': 1, 'inventory_source': ('actual Windows runtime inventory' if os.name == 'nt' else 'host_cfg_expected; actual Windows discovery required'),
               'native_source_sha256': source_hash, 'executables': [], 'tests': []}
    for suite, executable_name in [('library', 'Last-World-Mechanisms.exe'), ('cli', 'Last-World-Cli-Checks.exe')]:
        entry = harnesses[suite]
        source = root / entry['path']
        if not source.is_file() or not source.resolve().is_relative_to(root.resolve()) or sha256(source.read_bytes()).hexdigest() != entry['sha256']:
            raise SystemExit('Test executable differs from its frozen build stamp: ' + suite)
        expected = host_harnesses.get(suite, {}).get('test_names', [])
        if not expected or len(expected) != len(set(expected)):
            raise SystemExit('Missing or duplicated host test inventory: ' + suite)
        file(source, 'App/' + executable_name)
        catalog['executables'].append({'id': suite, 'executable': executable_name,
                                       'executable_path': 'App/' + executable_name,
                                       'expected_test_count': len(expected),
                                       'sha256': entry['sha256'], 'inventory_source': catalog['inventory_source']})
        for name in sorted(expected):
            catalog['tests'].append({'id': 'unit-' + suite + '-' + sha256(name.encode()).hexdigest()[:16],
                                     'executable': executable_name, 'test_name': name,
                                     'timeout_seconds': 180})
    catalog['expected_test_count'] = len(catalog['tests'])
    payload['App/benchmark-unit-tests.json'] = (json.dumps(catalog, indent=2, sort_keys=True)+'\n').encode()
    file(root / 'native/qualification/benchmark-plan.json', 'App/benchmark-plan.json')
    manifest['stress_suite'] = {'plan': 'App/benchmark-plan.json', 'mechanism_catalog': 'App/benchmark-unit-tests.json',
                                'max_attempts_per_case': 2, 'resume': 'same_build_journal',
                                'requested_rate_ceiling': 100, 'normal_game_speeds': [1,2,4],
                                'speed_test': 'one_windowed_adaptive_ramp; live_warmup; stop_after_sustained_capacity_limit',
                                'hardware_telemetry': {'helper': 'Tools/Run-Checks-GpuTelemetry.ps1',
                                                       'interval_seconds': 1,
                                                       'cpu_ram': 'owned_native_process_and_system_headroom',
                                                       'gpu': 'optional_WDDM_engine_and_dedicated_shared_allocation_counters',
                                                       'unavailable_is_not_zero': True},
                                'skipped_failed_and_unreached_cases_are_not_passes': True,
                                'test_executables': catalog['executables']}
file(root / 'native/README.md', documentation_prefix + 'README.md')
# Bundled native UI theme, real licensed fonts and cached glyph atlases.
for source in sorted((root / 'native/assets/ui').rglob('*')):
    if source.is_file():
        file(source, app_prefix + 'ui/' + source.relative_to(root / 'native/assets/ui').as_posix())
if not args.update_bundle:
    file(root / 'console.txt', 'console.txt')
if args.update_bundle:
    for name in ['Run-Game.cmd', 'Run-Checks.cmd']:
        file(root / 'native/launchers' / name, name)
    for name in ['Run-Checks.ps1', 'Run-Checks-OwnedProcess.ps1', 'Run-Checks-Feedback.ps1',
                 'Run-Checks-GpuTelemetry.ps1', 'Initialize-Layout.ps1', 'Run-Checks-Watchdog.cmd', 'Check-Progress.cmd']:
        file(root / 'native/launchers' / name, 'Tools/' + name)
else:
    for pattern in ['*.cmd', '*.ps1']:
        for source in sorted((root / 'native/launchers').glob(pattern)):
            file(source, source.name)
for source in sorted((root / 'native/qualification').glob('*.json')):
    file(source, documentation_prefix + 'qualification/' + source.name)
documents = set()
for pattern in ['MILESTONE_A*.md', 'MILESTONE_B*.md', 'GROUND_WATER*.md', 'CAMPAIGN*.md', 'BENCHMARK*.md', 'BATTLE*.md', 'PC_*.md', 'LAYOUT*.md']:
    documents.update((root / 'docs').glob(pattern))
for name in ['TERRAIN_WATER_AND_SEASONS.md', 'RIVER_FLOW_VERIFICATION.md', 'GROUND_WATER_VERIFICATION.md', 'UPDATE_AND_FEEDBACK.md']:
    source = root / 'docs' / name
    if source.is_file():
        documents.add(source)
for source in sorted(documents):
    file(source, documentation_prefix + 'docs/' + source.name)
if args.river_foundation:
    evidence = root / 'evidence/river-foundation-2026-10-29/cpu-interaction-report.json'
    if evidence.is_file():
        file(evidence, 'evidence/river-foundation-2026-10-29/cpu-interaction-report.json')
# Older native diagnostics name this path. Keep a small current pointer rather
# than copying any superseded governing record into new update packages.
payload[documentation_prefix + 'project-records/STATE.md'] = (
    '# Current project records\n\n'
    'Compatibility pointer for older native diagnostics; this is not a STATE record.\n'
    'Read [Master](../../Source/project/MASTER.md) for current instructions and state.\n'
    'Search [Ledger](../../Source/project/LEDGER.md) by ID/tag for history.\n'
).encode('utf-8')

registry = Path(os.environ.get('CARGO_HOME', str(Path.home() / '.cargo'))) / 'registry/src'
notices = []
for package in tomllib.loads((root / 'native/Cargo.lock').read_text())['package']:
    name, version = package['name'], package['version']
    candidates = [root / 'native/vendor/wgpu'] if name == 'wgpu' else list(registry.glob('*/' + name + '-' + version))
    if not candidates:
        continue
    source = candidates[0]
    metadata = tomllib.loads((source / 'Cargo.toml').read_text()).get('package', {})
    notices.append(name + ' ' + version + ' — ' + metadata.get('license', 'See package notices'))
    for pattern in ['LICENSE*', 'COPYING*', 'NOTICE*']:
        for license_file in sorted(source.glob(pattern)):
            if license_file.is_file():
                file(license_file, app_prefix + 'licenses/' + name + '-' + version + '/' + license_file.name)
payload[documentation_prefix + 'THIRD-PARTY-NOTICES.txt'] = ('\n'.join(notices) + '\n').encode()

if args.update_bundle:
    payload['README.txt'] = (
        'LAST WORLD - WINDOWS QUICK GUIDE\n\n'
        'Use these two buttons in this folder:\n'
        '  Run-Game.cmd    Play the native campaign.\n'
        '  Run-Checks.cmd  Run or resume the automatic PC checks.\n\n'
        'Both launchers show the identity of App\\Last-World.exe. Keep App, Tools,\n'
        'Documentation and Source together beside the two buttons. Data is created\n'
        'on first launch for logs, settings, saves and check results.\n\n'
        'TO UPDATE AN EXISTING COPY\n'
        'Close the game and checks. Open the ZIP, then open its Last-World folder.\n'
        'Copy the CONTENTS of that folder into your existing Last-World folder and\n'
        'replace matching files. Do not create a nested Last-World\\Last-World folder.\n'
        'Keep your existing Data folder. The update contains no player data.\n\n'
        'FIRST LAUNCH AFTER AN UPDATE\n'
        'The launcher organizes old logs, settings, saves and check results into Data.\n'
        'Old helpers and conflicting copies are kept safely in Data\\Legacy.\n'
        'The internal files in Data\\Locks can remain after closing; their presence\n'
        'does not mean that a game or check is active. Do not use them as activity indicators.\n\n'
        'PC CHECKS AND FEEDBACK\n'
        'Run-Checks.cmd runs the checks and resumes a matching unfinished batch.\n'
        'This includes one adaptive speed ramp and a rendered 20-minute simulated\n'
        'battle at requested 4x; actual throughput depends on the PC. Failures and\n'
        'unreached cases are recorded rather than counted as passes.\n'
        'When the launcher finishes, send only Send-Back.zip from this folder.\n'
        'That one feedback file is created or replaced by the checks; it is not part\n'
        'of the update ZIP. Logs and detailed results stay in Data.\n\n'
        'FOLDERS\n'
        '  App            Game, test programs, UI assets and build identity.\n'
        '  Tools          Launcher support and check scripts.\n'
        '  Documentation  Detailed guides, project records and third-party notices.\n'
        '  Source         Matching reviewed source snapshot and rebuild information.\n'
        '  Data           Your local mutable game and check data (created on launch).\n\n'
        'BUILD IDENTITY\n'
        f"Native inputs SHA-256: {manifest['native_source_sha256']}\n"
        f"Game EXE SHA-256:     {manifest['executable_sha256']}\n"
        'The full owned-file inventory is App\\build-manifest.json.\n'
    ).encode('utf-8')
    source_snapshot()
    forbidden_roots = {'console.txt', 'campaign-history.jsonl', 'river-history.jsonl', 'ground-history.jsonl', 'benchmark-history.jsonl', 'pc-host-history.jsonl', 'benchmark-results.json', 'workload-scope.json', 'Send-Back.zip'}
    for name in payload:
        source_parts = Path(name.removeprefix('Source/')).parts if name.startswith('Source/') else ()
        if (name in forbidden_roots or name.startswith(('saves/', 'settings/', 'check-results/', 'river-check'))
                or any(part in forbidden_roots | {'saves', 'settings', 'check-results', 'target', '__pycache__', 'node_modules'} for part in source_parts)):
            raise SystemExit('Mutable file cannot be placed in an overlay package: ' + name)
        if '\\' in name or name.startswith('Data/'):
            raise SystemExit('Noncanonical or mutable organized payload path: ' + name)
    if {name for name in payload if '/' not in name} != {'Run-Game.cmd', 'Run-Checks.cmd', 'README.txt'}:
        raise SystemExit('Organized package root must contain only the two launchers and README.txt')
    if source_digest(root) != source_hash:
        raise SystemExit('Native compilation inputs changed while packaging; freeze/rebuild before packaging')
    manifest['payload_files'] = {name: {'sha256': sha256(data).hexdigest(), 'bytes': len(data)} for name, data in sorted(payload.items())}
payload[app_prefix + 'build-manifest.json'] = (json.dumps(manifest, indent=2, sort_keys=True) + '\n').encode()

out = root / 'dist/downloads'
out.mkdir(exist_ok=True)
destination = out / filename
temporary = destination.with_suffix('.zip.part')
with zipfile.ZipFile(temporary, 'w', zipfile.ZIP_DEFLATED, compresslevel=9, strict_timestamps=False) as archive:
    for name, data in sorted(payload.items()):
        archive.writestr(prefix + name, data)
temporary.replace(destination)
print('Packaged', destination)
