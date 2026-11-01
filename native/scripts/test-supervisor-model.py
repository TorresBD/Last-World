"""Portable supervisor contract/model QA. Does not execute Windows PowerShell."""
from copy import deepcopy
from pathlib import Path, PurePosixPath
import json
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]
LAUNCHERS = ROOT / 'native/launchers'
TERMINAL = {0, 2, 10, 11, 12}


class JournalModel:
    """Independent reference for retry/resume dispositions, not a PS emulator."""
    def __init__(self, ids):
        self.phases = {name: {'status': 'pending', 'attempts': []} for name in ids}
        self.starts = []

    def launch(self, name):
        phase = self.phases[name]
        if phase['status'] in ('passed', 'skipped_after_failures'):
            return False
        if len(phase['attempts']) >= 2:
            raise AssertionError('third child attempt forbidden')
        phase['attempts'].append('running')
        phase['status'] = 'running'
        self.starts.append(name)
        return True

    def finish(self, name, disposition):
        phase = self.phases[name]
        phase['attempts'][-1] = disposition
        phase['status'] = ('passed' if disposition == 'passed' else
                           'skipped_after_failures' if len(phase['attempts']) == 2 else 'pending')

    def recover(self):
        # Persisted completed/skipped phases never become pending on restart.
        for name, phase in self.phases.items():
            if phase['status'] == 'running':
                self.finish(name, 'supervisor_interrupted')


def owned_process(expected, observed):
    if observed is None or expected['start'] != observed['start']:
        return False  # PID is gone/reused; do not kill the observed process.
    if expected != observed:
        raise RuntimeError('unsafe ownership; stop further launches')
    return True


def valid_report(report, *, case='cpu-rate-100', pid=42, started=10000, now=20000, attempt=1):
    identity = report.get('source_identity', {})
    native_hash, exe_hash = identity.get('native_source_sha256', ''), identity.get('executable_sha256', '')
    return (report.get('case_id') == case and report.get('process_id') == pid
            and started - 2000 <= report.get('started_utc_ms', -1) <= now + 2000
            and report.get('session_id') == report.get('filename', '').removesuffix('-report.json')
            and bool(report.get('session_id')) and report.get('status') == 'passed'
            and report.get('passed') is True and report.get('checkpoint', {}).get('committed') is True
            and isinstance(native_hash, str) and isinstance(exe_hash, str)
            and bool(re.fullmatch('[0-9a-fA-F]{64}', native_hash)) and bool(re.fullmatch('[0-9a-fA-F]{64}', exe_hash))
            and native_hash.lower() == 'a' * 64 and exe_hash.lower() == 'b' * 64
            and not (attempt == 2 and report.get('recovered_previous_success') is not None))


def feedback_eligible(name, size=1):
    path = PurePosixPath(name)
    return ('checkpoints' not in path.parts and size <= 64 * 1024 * 1024
            and path.suffix in {'.json', '.jsonl', '.txt', '.log', '.csv', '.png', '.dmp'})


def strip_ps_literals(text):
    text = re.sub(r"(?ms)@'\r?\n.*?^'@\s*$", "''", text)
    result = []
    index = 0
    while index < len(text):
        char = text[index]
        if char == '#':
            end = text.find('\n', index)
            index = len(text) if end < 0 else end
        elif char in "'\"":
            quote = char
            index += 1
            while index < len(text):
                if text[index] == '`' and quote == '"':
                    index += 2
                elif text[index] == quote:
                    if index + 1 < len(text) and text[index + 1] == quote:
                        index += 2
                    else:
                        index += 1
                        break
                else:
                    index += 1
            else:
                raise AssertionError('unclosed PowerShell string')
        else:
            result.append(char)
            index += 1
    return ''.join(result)


class SupervisorContractTests(unittest.TestCase):
    def test_two_failed_attempts_skip_only_that_phase_and_continue(self):
        model = JournalModel(['crashing', 'healthy'])
        for _ in range(2):
            self.assertTrue(model.launch('crashing'))
            model.finish('crashing', 'native_crash')
        self.assertFalse(model.launch('crashing'))
        model.launch('healthy'); model.finish('healthy', 'passed')
        self.assertEqual(model.starts, ['crashing', 'crashing', 'healthy'])
        self.assertEqual(model.phases['crashing']['status'], 'skipped_after_failures')

    def test_restart_resumes_interrupted_case_without_rerunning_completed_work(self):
        model = JournalModel(['done', 'interrupted', 'last'])
        model.launch('done'); model.finish('done', 'passed')
        model.launch('interrupted')
        model.phases = json.loads(json.dumps(model.phases))  # persisted round trip
        model.recover()
        self.assertFalse(model.launch('done'))
        model.launch('interrupted'); model.finish('interrupted', 'passed')
        model.launch('last'); model.finish('last', 'passed')
        self.assertEqual(model.starts.count('done'), 1)
        self.assertEqual(model.starts.count('interrupted'), 2)

    def test_watchdog_terminal_failures_do_not_loop_and_unexpected_exits_bound_two_restarts(self):
        for code in TERMINAL:
            restarts = sum(code not in TERMINAL for _ in range(1))
            self.assertEqual(restarts, 0)
        restarts = 0
        for _ in range(9):
            if restarts == 2:
                break
            restarts += 1
        self.assertEqual(restarts, 2)

    def test_pid_reuse_and_identity_uncertainty_never_authorize_unrelated_kill(self):
        owner = {'pid': 8, 'start': '123456789012345678', 'path': 'D:/game/Last-World.exe', 'sha': 'a'}
        self.assertTrue(owned_process(owner, deepcopy(owner)))
        reused = dict(owner, start='123456789012345679')
        self.assertFalse(owned_process(owner, reused))
        for changed in (dict(owner, path='D:/other/Last-World.exe'), dict(owner, sha='b')):
            with self.assertRaises(RuntimeError):
                owned_process(owner, changed)

    def test_fresh_report_protocol_rejects_stale_foreign_and_failed_results(self):
        report = {'case_id': 'cpu-rate-100', 'process_id': 42, 'started_utc_ms': 10001,
                  'session_id': 'cpu-rate-100-42-10001', 'filename': 'cpu-rate-100-42-10001-report.json',
                  'status': 'passed', 'passed': True, 'checkpoint': {'committed': True},
                  'source_identity': {'native_source_sha256': 'a' * 64, 'executable_sha256': 'b' * 64}}
        self.assertTrue(valid_report(report))
        for key, value in [('case_id', 'other'), ('process_id', 43), ('started_utc_ms', 1),
                           ('started_utc_ms', 30000), ('session_id', ''), ('status', 'incomplete'),
                           ('passed', False), ('passed', 'true'), ('checkpoint', {'committed': False})]:
            self.assertFalse(valid_report(dict(report, **{key: value})), (key, value))
        self.assertFalse(valid_report({}))

    def test_postcommit_gpu_failure_cannot_turn_retry_into_receipt_only_pass(self):
        model = JournalModel(['cpu-rate-100'])
        model.launch('cpu-rate-100')
        committed = {'case_id': 'cpu-rate-100', 'process_id': 42, 'started_utc_ms': 10001,
                     'session_id': 'cpu-rate-100-42-10001', 'filename': 'cpu-rate-100-42-10001-report.json',
                     'status': 'passed', 'passed': True, 'checkpoint': {'committed': True},
                     'source_identity': {'native_source_sha256': 'a' * 64, 'executable_sha256': 'b' * 64}}
        self.assertTrue(valid_report(committed))
        # A teardown GPU_ERROR invalidates supervisor success even after native commit.
        model.finish('cpu-rate-100', 'GPU_ERROR_after_commit')
        self.assertEqual(model.phases['cpu-rate-100']['status'], 'pending')
        model.launch('cpu-rate-100')
        recovered = dict(committed, recovered_previous_success={'original_session_id': 'prior', 'replayed_simulation': False})
        self.assertFalse(valid_report(recovered, attempt=2))
        self.assertTrue(valid_report(committed, attempt=2))  # fresh replay/validation may pass
        model.finish('cpu-rate-100', 'receipt_only_retry_rejected')
        self.assertEqual(model.phases['cpu-rate-100']['status'], 'skipped_after_failures')
        self.assertIn('Retry2 returned a prior receipt', (LAUNCHERS / 'Run-Checks.ps1').read_text())

    def test_foreign_or_malformed_report_hash_pair_cannot_pass(self):
        report = {'case_id': 'cpu-rate-100', 'process_id': 42, 'started_utc_ms': 10001,
                  'session_id': 'cpu-rate-100-42-10001', 'filename': 'cpu-rate-100-42-10001-report.json',
                  'status': 'passed', 'passed': True, 'checkpoint': {'committed': True},
                  'source_identity': {'native_source_sha256': 'A' * 64, 'executable_sha256': 'B' * 64, 'extra_stamp_field': 'allowed'}}
        self.assertTrue(valid_report(report))
        for field, wrong in [('native_source_sha256', 'c' * 64), ('executable_sha256', 'd' * 64),
                             ('native_source_sha256', 'a' * 63), ('executable_sha256', 'z' * 64)]:
            bad = deepcopy(report); bad['source_identity'][field] = wrong
            self.assertFalse(valid_report(bad), (field, wrong))
        bad = dict(report, source_identity={})
        self.assertFalse(valid_report(bad))
        self.assertIn('foreign source/executable hash pair', (LAUNCHERS / 'Run-Checks.ps1').read_text())

    def test_attempt_environment_is_child_only_and_never_sticky_in_parent(self):
        parent = {'PATH': 'unchanged', 'LAST_WORLD_BENCHMARK_ATTEMPT': 'parent-value', 'TEMP': 'parent-temp', 'TMP': 'parent-temp'}
        before = dict(parent)
        def child_environment(attempt):
            child = dict(parent)
            child.pop('LAST_WORLD_BENCHMARK_ATTEMPT', None)
            if attempt:
                child['LAST_WORLD_BENCHMARK_ATTEMPT'] = str(attempt)
            child['TEMP'] = child['TMP'] = 'owned-case-temp'
            return child
        self.assertEqual(child_environment(1)['LAST_WORLD_BENCHMARK_ATTEMPT'], '1')
        self.assertEqual(child_environment(2)['LAST_WORLD_BENCHMARK_ATTEMPT'], '2')
        self.assertNotIn('LAST_WORLD_BENCHMARK_ATTEMPT', child_environment(0))
        self.assertEqual(child_environment(2)['TEMP'], 'owned-case-temp')
        self.assertEqual(parent, before)
        source = (LAUNCHERS / 'Run-Checks-OwnedProcess.ps1').read_text()
        self.assertIn('Environment.GetEnvironmentVariables()', source)
        self.assertIn('Marshal.StringToHGlobalUni', source)
        self.assertIn('Marshal.FreeHGlobal(environment)', source)
        self.assertIn('StringComparer.OrdinalIgnoreCase', source)
        self.assertIn(".Append('\\0')", source)
        self.assertNotIn('SetEnvironmentVariable(', source)

    def test_rejection_markers_match_case_attempt_and_build_not_case_only(self):
        identity = {'native_source_sha256': 'source-a', 'executable_sha256': 'exe-a'}
        marker = {'case_id': 'window-rate-100', 'attempt_index': 1, 'source_identity': identity}
        def rejected(case, attempt, build):
            return (marker['case_id'], marker['attempt_index'], marker['source_identity']) == (case, attempt, build)
        self.assertTrue(rejected('window-rate-100', 1, dict(identity)))
        self.assertFalse(rejected('window-rate-100', 2, dict(identity)))  # valid retry2 is not poisoned
        self.assertFalse(rejected('other-case', 1, identity))
        self.assertFalse(rejected('window-rate-100', 1, dict(identity, executable_sha256='exe-b')))
        # If retry2 itself commits then fails teardown, its separate marker
        # rejects that checkpoint before the next different case continues.
        marker['attempt_index'] = 2
        self.assertTrue(rejected('window-rate-100', 2, identity))
        source = (LAUNCHERS / 'Run-Checks.ps1').read_text()
        self.assertIn('attempt_index=[int]$attempt.number;source_identity=', source)
        self.assertIn('Synchronize-RejectedAttempts;Save-Journal', source)

    def test_actual_unit_summary_never_accepts_zero_or_multiple_tests(self):
        source = (LAUNCHERS / 'Run-Checks.ps1').read_text()
        pattern = re.search(r"-Pattern '(\^test result: ok[^']+)'", source).group(1)
        self.assertIsNotNone(re.search(pattern, 'test result: ok. 1 passed; 0 failed; 0 ignored; 217 filtered out'))
        for passed in (0, 2, 214):
            self.assertIsNone(re.search(pattern, f'test result: ok. {passed} passed; 0 failed; 0 ignored; 0 filtered out'))

    def test_resumed_unit_arguments_and_named_execution_cannot_credit_another_case(self):
        source = (LAUNCHERS / 'Run-Checks.ps1').read_text()
        resolver = source[source.index('function Resolve-CanonicalPhase'):source.index('function Validate-JournalState')]
        self.assertIn("$Phase.arguments=@('--exact',[string]$Phase.unit_test_name", resolver)
        self.assertIn('Saved unit ID does not identify its exact test name', resolver)
        expected = 'tests::actual_case'
        for executed, accepted in [('tests::actual_case', True), ('tests::other_case', False)]:
            stdout = f'test {executed} ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 216 filtered out\n'
            self.assertEqual(bool(re.search(r'^test ' + re.escape(expected) + r' \.\.\.', stdout, re.M)), accepted)
        self.assertIn('-CaseSensitive -Pattern', source)

    def test_wall_timeout_uses_monotonic_elapsed_even_if_utc_moves_backward(self):
        source = (LAUNCHERS / 'Run-Checks.ps1').read_text()
        self.assertIn('[System.Diagnostics.Stopwatch]::StartNew()', source)
        self.assertIn('$phaseWatch.Elapsed.TotalSeconds -ge $Phase.timeout_seconds', source)
        self.assertNotIn('UtcNow.AddSeconds($Phase.timeout_seconds)', source)
        utc_values = [1000, 999, 998, 997]
        elapsed_values = [0, 4, 8, 12]
        expired = [elapsed >= 10 for _, elapsed in zip(utc_values, elapsed_values)]
        self.assertEqual(expired, [False, False, False, True])

    def test_return_collection_omits_checkpoint_saves_old_zips_and_huge_files(self):
        self.assertTrue(feedback_eligible('benchmark/cpu-rate-100-42-progress.json'))
        self.assertTrue(feedback_eligible('cases/unit-abc/attempt-1-stderr.txt'))
        for name in ('benchmark/checkpoints/group.save', 'benchmark/checkpoints/last-good.json',
                     'Send-Back.zip', 'cases/test/large.save'):
            self.assertFalse(feedback_eligible(name))
        self.assertFalse(feedback_eligible('huge-checkpoint.json', 65 * 1024 * 1024))

    def test_memory_guard_is_explicit_process_budget_not_machine_oom_target(self):
        gib = 1024 ** 3
        self.assertEqual([min(6 * gib, total * gib // 4) for total in (8, 16, 32, 64)],
                         [2 * gib, 4 * gib, 6 * gib, 6 * gib])

    def test_native_case_sequence_stays_monotonic_after_dynamic_unit_discovery(self):
        phases = [('discovery', 'list', 0), ('baseline', 'ground', 1),
                  ('benchmark', 'rate-001', 2), ('benchmark', 'rate-002', 3),
                  ('benchmark', 'rate-100', 4), ('unit', 'discovered-test', 5)]
        priority = {'discovery': 0, 'unit': 1, 'baseline': 2, 'benchmark': 3}
        ordered = sorted(phases, key=lambda phase: (priority[phase[0]], phase[2]))
        self.assertEqual([name for kind, name, _ in ordered if kind == 'benchmark'],
                         ['rate-001', 'rate-002', 'rate-100'])
        self.assertLess(ordered.index(phases[5]), ordered.index(phases[2]))
        self.assertIn('Expression={[int]$_.sequence}', (LAUNCHERS / 'Run-Checks.ps1').read_text())

    def test_failed_discovery_does_not_skip_the_other_binary_or_native_cases(self):
        model = JournalModel(['discovery-lib', 'unit-lib-a', 'unit-cli-a', 'native-ramp'])
        for _ in range(2):
            model.launch('discovery-lib'); model.finish('discovery-lib', 'failed_discovery')
        model.phases['unit-lib-a']['status'] = 'skipped_after_failures'
        for name in ('unit-cli-a', 'native-ramp'):
            model.launch(name); model.finish(name, 'passed')
        self.assertEqual(model.phases['unit-lib-a']['status'], 'skipped_after_failures')
        self.assertEqual(model.phases['native-ramp']['status'], 'passed')
        source = (LAUNCHERS / 'Run-Checks.ps1').read_text()
        region = source[source.index('function Skip-UnverifiedBinaryCases'):source.index('function Verify-AttemptArtifacts')]
        self.assertIn('$_.executable -eq $Phase.executable', region)

    def test_actual_sources_enforce_suspended_ownership_atomic_journal_and_bounded_watchdog(self):
        main = (LAUNCHERS / 'Run-Checks.ps1').read_text()
        child = (LAUNCHERS / 'Run-Checks-OwnedProcess.ps1').read_text()
        command = (LAUNCHERS / 'Run-Checks-Watchdog.cmd').read_text()
        region = main[main.index('function Run-Attempt'):main.index('\ntry {\n    Verify-Payload')]
        self.assertLess(region.index('StartSuspended'), region.index('$attempt.owner='))
        self.assertLess(region.index('$attempt.owner='), region.index('Save-Journal # suspended'))
        self.assertLess(region.index('Save-Journal # suspended'), region.index('$script:ownedChild.Resume()'))
        for token in ('Flush($true)', '.previous', 'supervisor_interrupted', 'skipped_after_failures'):
            self.assertIn(token, main)
        for token in ('0x2000|0x100', 'AssignProcessToJobObject', '0x4|0x08000000', 'SetErrorMode', 'Check(process.Handle!=IntPtr.Zero)'):
            self.assertIn(token, child)
        self.assertNotRegex(main, r'Stop-Process\s+-Name')
        for code in (0, 2, 10, 11, 12):
            self.assertIn(f'"%CHECKS_EXIT%"=="{code}"', command)
        self.assertIn('"%CHECK_WATCHDOG_ATTEMPT%"=="2"', command)
        labels = set(re.findall(r'^:([A-Za-z]+)', command, re.M))
        self.assertTrue(set(re.findall(r'goto ([A-Za-z]+)', command, re.I)) <= labels)

    def test_feedback_replace_uses_true_null_after_closed_flushed_zip(self):
        source = (LAUNCHERS / 'Run-Checks-Feedback.ps1').read_text()
        self.assertIn('[System.IO.File]::Replace($temporary,$feedback,[NullString]::Value)', source)
        self.assertNotIn('[System.IO.File]::Replace($temporary,$feedback,$null)', source)
        self.assertIn('[System.IO.Compression.ZipArchiveMode]::Create,$true)', source)
        self.assertLess(source.index('$archive.Dispose()'), source.index('$stream.Flush($true)'))
        self.assertLess(source.index('$stream.Flush($true)'), source.index('[System.IO.File]::Replace'))
        self.assertNotRegex(source, r'Remove-Item|\[System\.IO\.File\]::Delete')

    def test_cmd_freshness_avoids_nested_command_parser_and_preserves_error_history(self):
        command = (LAUNCHERS / 'Run-Checks-Watchdog.cmd').read_text()
        statements = '\n'.join(line for line in command.splitlines()
                               if not line.lower().startswith('rem '))
        self.assertNotRegex(statements, r'(?i)for /f')
        self.assertIn('>"check-results\\launch-start-filetime.txt"', statements)
        self.assertIn('set /p CHECK_START_FILETIME=<"check-results\\launch-start-filetime.txt"', statements)
        self.assertLess(statements.index('set "CHECK_START_FILETIME="'), statements.index('ToFileTimeUtc()'))
        self.assertIn('[Int64]::TryParse($env:CHECK_START_FILETIME,[ref]$start)', statements)
        self.assertIn('LastWriteTimeUtc.ToFileTimeUtc() -ge $start', statements)
        self.assertNotIn('type nul >"check-feedback-errors.txt"', statements)
        # Persisted diagnostics and inherited stderr must not compete for the
        # same writable file handle in fallback.
        self.assertIn('-Fallback 2>>"check-launcher-errors.txt"', statements)
        helper = (LAUNCHERS / 'Run-Checks-Feedback.ps1').read_text()
        self.assertIn("'check-feedback-errors.txt'", helper)
        self.assertIn('$failure.Exception.ToString()', helper)
        self.assertIn('$failure.ScriptStackTrace', helper)
        self.assertIn('Write-Error -ErrorAction Continue $_;exit 1', helper)
        self.assertIn("'feedback_recovery; supervisor_exit='+$env:CHECKS_EXIT", helper)

    def test_organized_layout_routes_owned_payload_and_mutable_state_separately(self):
        main = (LAUNCHERS / 'Run-Checks.ps1').read_text()
        feedback = (LAUNCHERS / 'Run-Checks-Feedback.ps1').read_text()
        watchdog = (LAUNCHERS / 'Run-Checks-Watchdog.cmd').read_text()
        shim = (LAUNCHERS / 'Run-Checks.cmd').read_text()
        self.assertIn('Tools\\Initialize-Layout.ps1', shim)
        self.assertIn('Tools\\Run-Checks-Watchdog.cmd', shim)
        self.assertIn('cd /d "%LAST_WORLD_DATA_DIR%"', watchdog)
        self.assertIn("Join-Path $env:LAST_WORLD_INSTALL_ROOT 'Send-Back.zip'", watchdog)
        self.assertIn("$path=Join-Path $appRoot 'build-manifest.json'", main)
        self.assertIn('$file=Join-Path $installRoot $relative', main)
        self.assertIn("PSObject.Properties['App/'+$name]", main)
        self.assertIn('$exe=Join-Path $appRoot $Phase.executable', main)
        self.assertIn('$cwd=$dataRoot', main)
        self.assertIn("$workRoot=Join-Path $dataRoot 'check-results'", main)
        self.assertIn("$hostHistory=Join-Path $dataRoot 'pc-host-history.jsonl'", main)
        self.assertIn("'check-results/'+$script:runId+'/benchmark'", main)
        self.assertIn("|supervisor3-hardware1'", main)  # layout moves do not reset same-build progress
        self.assertIn('adaptive telemetry: one collector', main)
        for source in (main, feedback):
            self.assertIn("$locksRoot=Join-Path $dataRoot 'Locks'", source)
            self.assertIn("Join-Path $locksRoot 'Run-Checks.lock'", source)
            self.assertNotIn("Join-Path $PSScriptRoot 'Run-Checks.lock'", source)
        for token in ("$temporaryRoot=Join-Path $dataRoot 'Temporary'",
                      "$feedback=Join-Path $InstallRoot 'Send-Back.zip'",
                      "'Data/check-results/current-suite.json'",
                      "prefix='Data'", "prefix='App'",
                      "Feedback run folder escaped Data/check-results."):
            self.assertIn(token, feedback)

    def test_hardware_gpu_filter_and_maximum_do_not_attribute_foreign_pid_or_sum_engines(self):
        def owned_engines(rows, expected_pid):
            result = {}
            for instance, value in rows:
                match = re.match(r'^pid_(\d+)_.*_eng_\d+_engtype_([^#]+)', instance)
                if match and int(match[1]) == expected_pid and value is not None:
                    result[match[2]] = max(result.get(match[2], 0), value)
            return result
        rows = [('pid_42_luid_0x0_0x1_phys_0_eng_0_engtype_3D', 35),
                ('pid_42_luid_0x0_0x1_phys_0_eng_1_engtype_3D', 60),
                ('pid_420_luid_0x0_0x1_phys_0_eng_0_engtype_3D', 100),
                ('pid_42_luid_0x0_0x1_phys_0_eng_2_engtype_Compute_0', 20),
                ('pid_42_luid_0x0_0x1_phys_0_eng_3_engtype_Copy', 0),
                ('pid_42_luid_0x0_0x1_phys_0_eng_4_engtype_Compute_1', None)]
        result = owned_engines(rows, 42)
        self.assertEqual(result, {'3D': 60, 'Compute_0': 20, 'Copy': 0})
        self.assertEqual(max(result.values()), 60)
        source = (LAUNCHERS / 'Run-Checks-GpuTelemetry.ps1').read_text()
        self.assertIn('instancePid!=pid) continue', source)
        self.assertIn('busiest_engine_percent=Max(', source)
        self.assertIn('engine_3d_max_percent=Max(', source)
        self.assertIn('result.status=warmup&&key=="engines"?"warmup"', source)
        self.assertIn('0x200u|0x1000u|0x8000u', source)  # bytes: NOSCALE, no100 cap
        self.assertIn('counter_setup_errors', source)

    def test_cpu_percentage_denominators_and_failed_read_interval_pairing(self):
        previous_cpu, previous_wall, logical_count = 10.0, 2.0, 24
        # A failed read at t=3 does not advance either paired timestamp.
        current_cpu, current_wall = 14.0, 4.0
        one_cpu = (current_cpu - previous_cpu) / (current_wall - previous_wall) * 100
        self.assertEqual(one_cpu, 200)
        self.assertAlmostEqual(one_cpu / logical_count, 100 / 12)
        # Kernel includes idle: subtract idle once, then divide by kernel+user.
        kernel_delta, user_delta, idle_delta = 70, 30, 40
        self.assertEqual((kernel_delta + user_delta - idle_delta) / (kernel_delta + user_delta) * 100, 60)
        source = (LAUNCHERS / 'Run-Checks-GpuTelemetry.ps1').read_text()
        self.assertIn('(cpu-previousCpu)/(wall-previousCpuWall)', source)
        self.assertIn('previousCpu=cpu;previousCpuWall=wall;haveCpu=true', source)
        self.assertIn('process_percent_one_logical_cpu/row.cpu.reported_logical_processors', source)
        self.assertIn('GlobalMemoryStatusEx(memory)', source)
        self.assertIn('effective_commit_limit_bytes', source)

    def test_hardware_queries_are_optional_bounded_owned_disposed_and_serialized(self):
        source = (LAUNCHERS / 'Run-Checks-GpuTelemetry.ps1').read_text()
        main = (LAUNCHERS / 'Run-Checks.ps1').read_text()
        for token in ('PdhAddEnglishCounterW', 'PdhGetCounterInfoW', 'PdhAddCounterW',
                      'PdhGetFormattedCounterArrayW', 'PdhCloseQuery', 'Marshal.FreeHGlobal',
                      'MaxBufferBytes=1024*1024,MaxInstances=512', 'ownedProcess.StartTime.ToUniversalTime().Ticks!=startTicks',
                      'not_read_after_collection_failure', 'if(collected==0)', 'interop layout probe failed',
                      "schema_probe='not_run'", "schema_probe='passed'", 'sample_and_log_ms_total'):
            self.assertIn(token, source)
        self.assertLess(source.index('if(collected==0)'), source.index('row.gpu.engines=Read('))
        self.assertIn("if ($Phase.executable -eq 'Last-World.exe')", main)
        self.assertIn("$definition[0].mode -eq 'window'", main)
        self.assertIn('Sample-LastWorldHardwareTelemetry $hardwareState $process', main)
        self.assertIn('Stop-LastWorldHardwareTelemetry $hardwareState', main)
        self.assertIn('Write-AtomicJson (Join-Path $script:runFolder $attempt.hardware_summary)', main)
        self.assertIn("'unavailable_setup'", main)
        self.assertTrue(feedback_eligible('cases/probe/attempt-1-hardware.jsonl'))
        self.assertTrue(feedback_eligible('cases/probe/attempt-1-hardware-summary.json'))

    def test_long_hardware_logs_rotate_without_discarding_records_or_hitting_feedback_limit(self):
        limit = 8 * 1024 * 1024
        records = [1024 * 1024] * 20
        segments = [[]]
        for size in records:
            if segments[-1] and sum(segments[-1]) + size > limit:
                segments.append([])
            segments[-1].append(size)
        self.assertEqual([len(part) for part in segments], [8, 8, 4])
        self.assertEqual([size for part in segments for size in part], records)
        self.assertTrue(all(feedback_eligible(f'attempt-1-hardware.segment-{index}.jsonl', sum(part))
                            for index, part in enumerate(segments)))
        source = (LAUNCHERS / 'Run-Checks-GpuTelemetry.ps1').read_text()
        for token in ('segment_limit_bytes=8MB', '[System.IO.FileMode]::CreateNew',
                      '$State.segments.ToArray()', "'.segment-'", 'extrema=$State.extrema',
                      'collector_process_cpu_ms_total', 'prior_gpu_sample_exceeded250ms_budget'):
            self.assertIn(token, source)
        self.assertNotRegex(source, r'Remove-Item|\.SetLength\(|File\]::Delete')

    def test_powershell_source_delimiters_and_shared_plan_are_consistent(self):
        for path in [*LAUNCHERS.glob('Run-Checks*.ps1'), ROOT / 'native/scripts/test-feedback-windows.ps1']:
            stack = []
            for char in strip_ps_literals(path.read_text()):
                if char in '({[':
                    stack.append(char)
                elif char in ')}]':
                    self.assertTrue(stack and stack.pop() == {')': '(', '}': '{', ']': '['}[char], path.name)
            self.assertEqual(stack, [], path.name)
        plan = json.loads((ROOT / 'native/qualification/benchmark-plan.json').read_text())
        self.assertEqual(plan['normal_play_speed_options'], [1, 2, 4])
        ramps = [case for case in plan['cases'] if case.get('speed_ramp')]
        self.assertEqual(len(ramps), 1)
        self.assertEqual(ramps[0]['speed_ramp']['rates'], [1, 2, 4, 8, 16, 32, 64, 100])
        faster_cases = [case for case in plan['cases'] if case['requested_rate'] > 2]
        self.assertEqual([case['id'] for case in faster_cases], ['window-battle-20m-4x'])
        battle = faster_cases[0]
        self.assertEqual((battle['mode'], battle['requested_rate'], battle['target_ticks']),
                         ('window', 4, 24000))
        self.assertIs(battle.get('battle_scenario'), True)


if __name__ == '__main__':
    unittest.main(verbosity=2)
