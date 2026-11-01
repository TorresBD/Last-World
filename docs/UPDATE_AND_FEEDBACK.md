# Last World — one-folder update and one-file PC feedback

Current organized update: root Run-Game.cmd and Run-Checks.cmd; App holds binaries/manifests, Tools support scripts, Data persistent state and locks, Documentation/Source references. Copy CONTENTS into the actual existing folder. Send only root Send-Back.zip. See LAYOUT_AND_LAUNCHERS.md and project/MASTER.md (Source/project/MASTER.md in an update package) for current paths/results; older flat-path/version details below are historical. New supervisor prints native progress every10seconds. The72000tick requested2x season may take longer than30minutes; previous PC run passed42m30.7s at~1.4148x. Optional Tools/Check-Progress.cmd is read-only and produces separate Send-Back-Progress.zip when diagnosing an active run.


Tor requested larger work batches, fewer handoffs and a repeatable PC workflow. This is the delivery contract for this and subsequent native updates.

## Update

1. Close Last World and any running check batch.
2. Extract `Last-World-Update-Windows.zip`. Copy the contents of its `Last-World` folder into the existing folder containing `Last-World.exe`, replacing matching program files. If both folders already use the name `Last-World`, merge the new folder over the old one. Keep the current drive/location, including D:.
3. Run `Run-Checks.cmd`.

The update contains application files, launchers, build identity and documentation. It contains no fresh `console.txt`, generated histories, settings, saves, feedback ZIP or check results. File replacement therefore does not reset those existing files. No cleanup/mirror operation deletes unlisted files. A running executable must be closed so Windows can replace it.

The included manifest hashes the owned payload. The check runner detects incomplete/mixed updates before launching tests. Old separately named qualification/river downloads remain available as historical builds; the update bundle uses the stable folder/file contract above.

## One run

`Run-Checks.cmd` now runs the comprehensive resumable supervisor without manual scene selection. It discovers each actual Windows mechanism/CLI test and runs them individually, retains these baseline scenes, then runs every native stress/rate/population/seed/season case in the bundled plan:

- Deterministic CPU ground/water report, including finite water/soil/biomass and drying behavior.
- Vulkan movement checks at1k/10k/100k agents.
- DX12 movement checks at1k/10k/100k agents.
- An automatic river-render sequence at requested4×: close the barrier at20simulated seconds, breach at80seconds, finish at180seconds. It captures three actual renderer frames and writes a conservation/result report. Leave the window open; it closes itself.
- The CPU campaign correctness and automatic campaign-render/HUD capture phases remain included.

The military/civilian/ecology campaign is not qualified by these isolated scenes. A missing/failed backend is recorded; independent remaining stages are still attempted. The automatic river check records capture effects and requested versus submitted speed; captures are not a steady-FPS qualification.

The runner holds one per-install lock, owns only its child processes, applies bounded timeouts/process memory guards and records failure/cleanup details. Every phase gets two attempts, then is explicitly skipped after failures while other phases continue. An interrupted same-build suite resumes its atomic journal without rerunning completed/skipped phases. A second invocation leaves an active supervisor alone. Host console records are merged after verified owned-child exit; native stderr is retained separately. Full protocol and current Windows-execution limitations are in `BENCHMARK_SUPERVISION.md`.

For the current native campaign additions, only the CPU `--campaign-report` wall deadline is600seconds, allowing the strict late integrated-state save/load and next-tick comparison to finish. The campaign-render deadline stays240seconds; other stage budgets are unchanged. This harness deadline does not change the20Hz gameplay clock,24,000-tick correctness ceiling, pawn counts, rule gates or performance acceptance. A completed correctness run is not proof of requested game speed or60FPS.

## One return file

Send back only **`Send-Back.zip`** from the game folder. It gathers histories, all owned case attempts, actual Windows inventory/drift, journals, host/process/source identities, summaries and fresh reports/captures. Native checkpoint saves remain local for resume and are omitted from feedback, with explicit exclusion notes. No manual file selection is needed.

The ZIP is finalized before replacing the previous feedback snapshot. Underlying append-only logs and run folders remain. Prior feedback ZIPs are never packed recursively. The allowlist does not collect unrelated player files. If the main PowerShell script cannot start or parse, the CMD wrapper attempts an independent minimal diagnostic ZIP; if that also fails, it reports that no current ZIP could be produced rather than presenting an old archive as new.

The first returned batch now tests the successful Windows CMD/PowerShell execution and one-file collection path: all four stages exited0, with three fresh captures and a final report. See `COUPLED_PC_BATCH_RESULTS.md`. Failure/timeout/duplicate-launch/fallback branches were not exercised by that successful run. Local sentinel overlay checks establish preservation separately; a current-only returned console does not prove a previous folder was merged on the PC. The measured river hitch remains open even though the functional stages passed.


Updated speed-test policy (Tor,2026-10-30): one existing native window/device runs an adaptive1/2/4/8/16/32/64/100 ramp. It loads once, gives the living simulation15seconds initial warm-up,5seconds settling per rate, and at least two10second measurement windows. Sustained capacity failure stops higher rates; those rates are reported not attempted, not passed. The complete world, authoritative tick and retained debt continue throughout. The72,000-tick seasonal soak, population/seed probes and mechanism checks remain independent correctness/load checks, rather than additional speed comparisons. No achieved100x or4x guarantee.

The previous Send-Back.zip replacement failure is repaired in the launcher; existing feedback is atomically replaced after the new ZIP closes and flushes, and failures retain exception diagnostics. No manual .part handling should be required in the new candidate. Actual Windows validation remains required.


To honor the single-speed-test policy, all other focused smoke/population/seed/recovery probes request only1x; the72,000-tick seasonal soak requests2x and may take roughly30minutes or longer if achieved throughput falls. These phases test load/correctness, not successive speed ceilings. Only the adaptive ramp can ask for4/8/16/32/64/100x. Actual debt and achieved timing remain reported for every phase.


Warm-up begins after the first successfully presented native frame, so initial loading/upload cannot consume the15second allowance. Every measured window requires actual successful presentation; minimized/interrupted windows cannot qualify GPU capacity.1Hz owned-process hardware samples in the feedback archive include CPU time/utilization, process RAM/peaks, system physical/commit headroom and optional Windows GPU engine plus dedicated/shared allocation counters. Missing counters report unavailable, never invented zero. GPU engine maxima are not summed into an overall GPU utilization percentage; dedicated/shared counters are named allocation measurements, not raw GPU execution timing. Logger setup/sample overhead is separate. The game uses its retained device/textures/instance buffers; no artificial VRAM filling.
