"""Host-only live CPU recovery controls; never a hardware/performance benchmark.

Runs a supplied frozen ELF executable, not Cargo. The executable and its real
adjacent build stamp are staged inside one new owned output root because the
native main changes its working directory to the executable's parent. Every
child's logs/reports and the exact supervisor rejection ledger are retained.
No gameplay state, source counters, populations, case costs or rules are edited.
"""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import time

ONCE = "probe-after-commit-crash-once"
ALWAYS = "probe-after-commit-crash-always"
NEXT = "probe-continue-after-skipped"
GROUP = "recovery-controls-78421"
PAIR_KEYS = ("native_source_sha256", "executable_sha256")


def digest(path: Path) -> str:
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def utc() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat()


def atomic_json(path: Path, value: object) -> None:
    pending = path.with_name(path.name + ".part")
    with pending.open("x", encoding="utf-8") as target:
        json.dump(value, target, indent=2, sort_keys=True)
        target.write("\n")
        target.flush()
        os.fsync(target.fileno())
    os.replace(pending, path)
    descriptor = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def checkpoint(path: Path) -> dict | None:
    if not path.exists():
        return None
    # Native load already verifies its exact saved-byte checksum. Here retain
    # the raw envelope hash and inspect only the real controller/campaign data.
    # Never reconstruct or write a campaign save in this helper.
    envelope = json.loads(path.read_text(encoding="utf-8"))
    require(envelope.get("format") == "last-world-campaign", "Wrong checkpoint format")
    require(envelope.get("schema_version") == 2, "Wrong saved envelope schema")
    payload = json.loads(envelope["payload"])
    return {
        "sha256": digest(path),
        "bytes": path.stat().st_size,
        "payload_checksum": envelope["payload_checksum"],
        "last_case": payload["last_case"],
        "attempt": payload["attempt"],
        "completed_cases": payload["controller"]["completed_cases"],
        "processed_tick": payload["campaign"]["data"]["processed_tick"],
        "source_identity": payload["source_identity"],
    }


def disable_child_cores() -> None:
    import resource
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


class Probe:
    def __init__(self, executable: Path, stamp: Path, output: Path, timeout: float):
        require(sys.platform.startswith("linux"), "This helper is a Linux host CPU probe, not a Windows supervisor test")
        require(timeout > 0 and timeout <= 3600, "Timeout must be in (0,3600] seconds")
        require(not output.exists() or not any(output.iterdir()), "Output must be a new or empty owned directory; old results are never deleted")
        output.mkdir(parents=True, exist_ok=True)
        self.root = output.resolve()
        self.runtime = self.root / "runtime"
        self.native = self.root / "native-output"
        self.records = self.root / "attempts"
        for directory in (self.runtime, self.native, self.records):
            directory.mkdir()
        self.timeout = timeout
        self.stamp = json.loads(stamp.read_text(encoding="utf-8"))
        for key in PAIR_KEYS:
            require(re.fullmatch(r"[0-9a-fA-F]{64}", str(self.stamp.get(key, ""))) is not None, "Build stamp needs real SHA256 identities")
        require(self.stamp.get("source_frozen_through_build") is True, "Build stamp does not prove frozen inputs")
        require(digest(executable).lower() == self.stamp["executable_sha256"].lower(), "Executable bytes disagree with adjacent build stamp")
        self.exe = self.runtime / executable.name
        shutil.copy2(executable, self.exe)
        shutil.copy2(stamp, self.runtime / "native-build-stamp.json")
        require(digest(self.exe) == digest(executable), "Staged executable copy changed")
        self.pair = {key: self.stamp[key] for key in PAIR_KEYS}
        self.rejections: list[dict] = []
        self.attempts: list[dict] = []
        self.parent_attempt = os.environ.get("LAST_WORLD_BENCHMARK_ATTEMPT")
        self.summary = {
            "schema_version": 1,
            "correctness_only": True,
            "hardware_performance_qualified": False,
            "windows_supervisor_executed": False,
            "scope": "actual frozen native CPU fault controls; Python writes supervisor-format rejection metadata only",
            "started_utc": utc(),
            "source_executable": str(executable),
            "source_stamp": str(stamp),
            "staged_executable": str(self.exe),
            "source_identity": self.stamp,
            "core_dumps_disabled_for_children": True,
            "child_arguments_never_override_workload": True,
            "attempts": self.attempts,
            "passed": False,
        }
        atomic_json(self.root / "probe-summary.json", self.summary)

    def run(self, case: str, attempt: int, abort: bool) -> dict:
        folder = self.records / f"{len(self.attempts)+1:02d}-{case}-a{attempt}"
        folder.mkdir()
        temporary = folder / "temp"
        temporary.mkdir()
        env = os.environ.copy()
        env.pop("LAST_WORLD_BENCHMARK_ATTEMPT", None)
        env["LAST_WORLD_BENCHMARK_ATTEMPT"] = str(attempt)
        env.update({"TMPDIR": str(temporary), "TEMP": str(temporary), "TMP": str(temporary)})
        args = [str(self.exe), "--benchmark-case", case, "--benchmark-output", str(self.native)]
        row = {
            "case_id": case, "attempt_index": attempt,
            "expected": "post_commit_SIGABRT" if abort else "real_success",
            "started_utc": utc(), "started_utc_ms": time.time_ns() // 1_000_000,
            "arguments": args, "stdout": str((folder/"stdout.log").relative_to(self.root)),
            "stderr": str((folder/"stderr.log").relative_to(self.root)),
            "native_checkpoint_before": checkpoint(self.native/"checkpoints"/f"{GROUP}.save"),
            "passed": False,
        }
        self.attempts.append(row)
        atomic_json(self.root / "probe-summary.json", self.summary)
        print(f"Running actual CPU control {case} attempt{attempt}; expected {row['expected']}", flush=True)
        started = time.monotonic()
        with (folder/"stdout.log").open("wb") as stdout, (folder/"stderr.log").open("wb") as stderr:
            process = subprocess.Popen(args, cwd=self.runtime, env=env, stdout=stdout, stderr=stderr,
                                       start_new_session=True, preexec_fn=disable_child_cores)
            row["process_id"] = process.pid
            try:
                row["exit_code"] = process.wait(timeout=self.timeout)
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait()
                row["timed_out"] = True
                row["exit_code"] = process.returncode
        row["wall_seconds"] = time.monotonic()-started
        row["finished_utc"] = utc()
        atomic_json(folder/"process-result.json", row)
        require(not row.get("timed_out"), "Native recovery control exceeded its monotonic deadline; logs retained")
        require(row["exit_code"] == (-signal.SIGABRT if abort else 0), f"Unexpected native exit {row['exit_code']} for {case} attempt{attempt}")
        reports = list(self.native.glob(f"{case}-{process.pid}-*-report.json"))
        require(len(reports) == 1, "Missing or ambiguous actual owned native report")
        report_path = reports[0]
        report = json.loads(report_path.read_text(encoding="utf-8"))
        require(report_path.name == report.get("session_id", "")+"-report.json", "Native session/file mismatch")
        require(report.get("case_id") == case and report.get("process_id") == process.pid and report.get("attempt") == attempt, "Native case/PID/attempt mismatch")
        require(report.get("source_identity") == self.stamp, "Native report/staged stamp identity mismatch")
        require(all(report["source_identity"].get(k, "").lower() == self.pair[k].lower() for k in PAIR_KEYS), "Native hash pair mismatch")
        require(report.get("passed") is True and report.get("status") == "passed" and report.get("checkpoint", {}).get("committed") is True, "Control never committed a validated actual world/report")
        require(report.get("harness_control_probe") is True and report.get("mode") == "cpu", "Expected a CPU harness control, not gameplay/performance evidence")
        require(report["actual_counts"]["requested_initial_military"] == 128 and report["actual_counts"]["requested_initial_workers"] == 16, "Probe workload changed")
        require(report["actual_counts"]["population_counts_were_not_reduced"] is True, "Native population ownership check failed")
        require(report["metrics"]["committed_ticks"] > 0, "Control replay advanced no actual ordinary ticks")
        require(not report.get("recovered_previous_success"), "This live probe must rerun actual simulation rather than replay a receipt")
        if abort:
            stderr = (folder/"stderr.log").read_text(encoding="utf-8", errors="replace")
            require("BENCHMARK_INTENTIONAL_RECOVERY_PROBE" in stderr, "SIGABRT lacked the explicit after-commit control marker")
            self.rejections.append({
                "case_id": case, "attempt_index": attempt, "source_identity": self.pair,
                "reason": "host_live_probe: actual owned SIGABRT after validated checkpoint/report commit; not a successful supervisor phase",
                "process_id": process.pid, "session_id": report["session_id"],
                "started_utc_ms": row["started_utc_ms"], "recorded_utc": row["finished_utc"],
            })
            ledger = {"schema_version": 1, "rejections": self.rejections}
            # Exact PowerShell Synchronize-RejectedAttempts schema. Native consumes
            # this metadata; no campaign file, stock or controller is fabricated.
            atomic_json(self.native/"rejected-attempts.json", ledger)
            atomic_json(folder/"rejection-ledger-written.json", ledger)
        row["native_report"] = str(report_path.relative_to(self.root))
        row["native_report_sha256"] = digest(report_path)
        row["native_checkpoint_after"] = checkpoint(self.native/"checkpoints"/f"{GROUP}.save")
        row["native_previous_after"] = checkpoint(self.native/"checkpoints"/f"{GROUP}.save.previous")
        row["replay_origin"] = report["replay_origin"]
        row["passed"] = True
        atomic_json(folder/"process-result.json", row)
        atomic_json(self.root/"probe-summary.json", self.summary)
        return report

    def sequence(self) -> None:
        once1 = self.run(ONCE, 1, True)
        once2 = self.run(ONCE, 2, False)
        require(once2["replay_origin"] == "retry_fresh_named_initial_world", "First-case retry did not restore its named initial state")
        require(once1["initial_state"] == once2["initial_state"], "Fresh retry changed the real initial campaign state")
        accepted = self.attempts[-1]["native_checkpoint_after"]
        require(accepted["last_case"] == ONCE and accepted["attempt"] == 2, "Wrong accepted prephase checkpoint")
        always1 = self.run(ALWAYS, 1, True)
        require(always1["initial_state"] == once2["final_state"], "Always-crash case did not start from accepted real state")
        require(self.attempts[-1]["native_previous_after"]["sha256"] == accepted["sha256"], "First failed commit lost its genuine accepted prephase bytes")
        always2 = self.run(ALWAYS, 2, True)
        require(always2["replay_origin"] == "retry_previous_phase_snapshot", "Always-crash retry did not restore previous accepted state")
        require(always2["initial_state"] == always1["initial_state"], "Always-crash retry inherited failed phase effects")
        require(self.attempts[-1]["native_previous_after"]["sha256"] == accepted["sha256"], "Second failed commit overwrote accepted prephase bytes")
        continued = self.run(NEXT, 1, False)
        require(continued["replay_origin"] == "continued_after_rejected_attempt_rollback", "Next phase did not roll back the rejected committed attempt")
        require(continued["initial_state"] == once2["final_state"], "Next case inherited the skipped failed case's world")
        cases = self.attempts[-1]["native_checkpoint_after"]["completed_cases"]
        require(ALWAYS not in cases and ONCE in cases and NEXT in cases, "Failed case leaked into the actual saved controller")
        require(self.attempts[-1]["native_previous_after"]["sha256"] == accepted["sha256"], "Continuation failed to retain the accepted prephase snapshot")
        require(os.environ.get("LAST_WORLD_BENCHMARK_ATTEMPT") == self.parent_attempt, "Helper changed its parent/process-global attempt environment")
        self.summary.update({"passed": True, "finished_utc": utc(), "expected_native_aborts": 3,
                             "real_native_successes": 2, "skipped_case": ALWAYS,
                             "failed_case_absent_from_actual_saved_controller": True,
                             "actual_final_completed_cases": cases,
                             "preserved_accepted_prephase_sha256": accepted["sha256"],
                             "windows_child_environment_behavior": "source-reviewed separately; Windows supervisor not executed by this Linux helper"})
        atomic_json(self.root/"probe-summary.json", self.summary)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--executable", required=True, type=Path, help="actual frozen Linux native executable")
    parser.add_argument("--stamp", type=Path, help="real build stamp; defaults to executable sibling native-build-stamp.json")
    parser.add_argument("--output", required=True, type=Path, help="one new/empty owned root; logs/checkpoints remain here")
    parser.add_argument("--timeout", type=float, default=120, help="monotonic seconds per actual child")
    args = parser.parse_args()
    probe = None
    try:
        executable = args.executable.resolve(strict=True)
        stamp = (args.stamp or executable.parent/"native-build-stamp.json").resolve(strict=True)
        probe = Probe(executable, stamp, args.output, args.timeout)
        probe.sequence()
        print(json.dumps({"passed": True, "correctness_only": True,
                          "summary": str(probe.root/"probe-summary.json")}), flush=True)
        return 0
    except Exception as error:
        if probe is not None:
            probe.summary.update({"passed": False, "finished_utc": utc(), "failure": str(error)})
            atomic_json(probe.root/"probe-summary.json", probe.summary)
        print(f"NATIVE_RECOVERY_PROBE_FAILED: {error}", file=sys.stderr, flush=True)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
