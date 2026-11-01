# Milestone A — returned PC-log audit

Date: 2026-10-29, America/Chicago. Read-only audit of Tor's uploaded files. No application, build, benchmark, test, browser or graphics tool was executed by this audit. This report distinguishes tests Tor already ran from this inspection of their records.

## Inputs and scope

| Uploaded file | Lines | Use |
|---|---:|---|
| `/workspace/scratch/5f5799dd0333/upload/console(10).txt` | 11,128 | Append-only historical development output plus the newly appended Windows sessions. |
| `/workspace/scratch/5f5799dd0333/upload/benchmark-history(1).jsonl` | 12 | Three phase-start/result pairs for each of two real-GPU backend runs. |
| `/workspace/scratch/5f5799dd0333/upload/pc-host-history(1).jsonl` | 39 | Windows host, per-process memory samples, backend start/end and exit codes. |

The Windows portion begins at **console line 11067**, timestamp `1793320382238`. Lines 1–11066 are retained development/build/history output. The nearby `=== NATIVE ... package-windows.py` header at lines 11065–11066 belongs to retained packaging history; it does not mean the following Windows logs are a Linux packaging run. The explicit `HOST` records identify the new runtime OS as Windows.

## Current Windows sessions

| Session | Process / run ID | Evidence | Result |
|---|---|---|---|
| Scope description | PID **21416**, no benchmark run ID | Console 11067–11069: SESSION, HOST (`os: windows`), SCOPE. | Scope-description evidence only. No adapter, phase, visual READY or exit-code record for this process is supplied. Do not invent a launch failure or successful exit code. |
| Vulkan isolated benchmark | PID **21300**, run ID **`21300-1793320382327256300`** | Console 11070–11079; benchmark-history lines 1–6; pc-host-history lines 2–20. | Adapter RTX 2080 SUPER, backend Vulkan, driver NVIDIA / driver_info 621.01. Three completed phases at 1,000, 10,000 and 100,000 movement records. Wrapper exit code **0**, pc-host-history line 20 and console 11109. |
| DX12 isolated benchmark | PID **10456**, run ID **`10456-1793320399576869600`** | Console 11080–11089; benchmark-history lines 7–12; pc-host-history lines 21–39. | Adapter RTX 2080 SUPER, backend Dx12, driver 32.0.16.2101. Three completed phases at 1,000, 10,000 and 100,000 movement records. Wrapper exit code **0**, pc-host-history line 39 and console 11128. |

The wrapper host record at pc-host-history line 1 (also console 11090) names AMD Ryzen 9 3900X, 12 cores/24 logical processors, Microsoft Windows 10 Pro build 19045, and 34,266,181,632 visible RAM bytes. Native HOST records report x86_64 and available logical parallelism 24. These are real-PC records, not the earlier eight-thread Linux environment or llvmpipe adapter.

UTC wrapper intervals are:

- Vulkan: `2026-10-30T00:33:02.3080458Z` to `00:33:19.5452653Z`.
- DX12: `2026-10-30T00:33:19.5482661Z` to `00:33:36.7584928Z`.

The PC_HOST records are appended as a block after the native session output in console.txt, so file ordering is not strict timestamp interleaving. Correlate backend/PID/timestamps rather than assuming adjacent log lines came from the same instant.

## Correctness, phase completeness and current errors

All six `phase_started` entries have matching `phase_result` records for the same run ID, backend and agent count. Every result declares `correctness: passed`. The corresponding explicit console `GPU_CHECK` lines are:

| Backend | Count | GPU_CHECK line | Result line | JSONL result line |
|---|---:|---:|---:|---:|
| Vulkan | 1,000 | 11074 | 11075 | 2 |
| Vulkan | 10,000 | 11076 | 11077 | 4 |
| Vulkan | 100,000 | 11078 | 11079 | 6 |
| DX12 | 1,000 | 11084 | 11085 | 8 |
| DX12 | 10,000 | 11086 | 11087 | 10 |
| DX12 | 100,000 | 11088 | 11089 | 12 |

The check text names finite coordinates, observed movement of the first agent, and every active spatial-list link exactly once. It does not prove every agent moved a meaningful distance, correct combat/crowd avoidance, actual feeding/reproduction, native construction or conservation.

There are **zero FATAL, PANIC, GPU_ERROR or SURFACE_ERROR tags in console lines 11067–11128**. No correctness-failure, corrupt-index, dropped-agent, non-finite-position or other error/failure text was found in that current Windows portion. Both backend processes have successful wrapper exits. No started phase is incomplete in the supplied benchmark JSONL.

The returned qualification sequence contains 1k/10k/100k only. It does not include the 5k/25k/50k phases of the older six-population launcher. That absence is not a failed phase: no such start record exists in these two runs. Do not report the older six-phase sweep as newly executed on this PC.

## Retained errors are not current PC failures

The earlier console history contains six FATAL-tag line occurrences and 1,240 GPU_ERROR-tag line occurrences before line 11067. These are log occurrences, potentially duplicated by stdout/file capture, not counts of independent bugs or current Windows failures.

Examples anchoring their origin:

- Console 893–894: an earlier no-adapter failure, timestamp `1793312027137`.
- Console 942–947: retained native Linux `--headless` command and explicit **llvmpipe / device_type Cpu** Vulkan adapter.
- Console 948–960: the earlier WGSL parser rejects the reserved name `target`; subsequent invalid-module/pipeline errors cascade through the retained software-adapter run. The last GPU_ERROR occurrence in the file is at **9648**, before the new Windows sessions.
- Console 10691, 10745 and 10753: explicit Linux HOST records, process 5, available parallelism 8.
- Console 10746: intentional CLI rejection, `Agent count 0 is outside 1..=100000`; wrapper exit 1 at 10748.
- Console 10754: the development environment's Vulkan driver/adapter failure; wrapper exit 1 at 10756.
- Earlier compiler exit-101 records, including 10490 and 10819, are retained build failures. A later Windows cross-build reaches `Finished release` at 11061 and exit 0 at 11063.

Those records remain useful failure history and should be preserved. They must not be attributed to the RTX Windows sessions merely because they share console.txt.

## Visual and workload limits

The new Windows sessions explicitly announce **synchronized isolated headless benchmark** mode (console 11073 and 11083). There is no new Windows READY, PERF, ACTION or SESSION_END record and no supplied screenshot/capture event demonstrating an interactive visual session. A successful headless render-to-texture run does not confirm native window presentation, controls, resizing, visual fidelity, radar/satellite appearance or explosion appearance on Tor's display. An interactive session is **not evidenced**, rather than proven not to have occurred outside these files.

Every current phase result labels itself `isolated_benchmark_only`, `is_integrated_campaign: false`, `military_rules_present: false`, `civilian_and_ecology_rules_present: false`, actual map `[4800,3200]`, and synthetic minimum-50-ms timing policy. Therefore these records establish real-PC execution/correctness of the current isolated GPU benchmark, not a 20 × 20 km native campaign, 10,000 functional military units plus civilians/wildlife, 30 trait mechanisms, game-speed qualification or low-end hardware capability. The SCOPE description explicitly lists the approved targets as not implemented.

Native per-result process RAM/VRAM fields are null; the wrapper separately supplies OS working-set samples and peaks. Those process-memory values are not VRAM measurements. Benchmark quantiles retain at most 8,192 recent samples; phases with more frames are not full-run quantile distributions even though mean/max use all phase samples. The parent report owns the numerical metric summary and comparisons.

## Audit conclusion

The supplied evidence contains two completed real-PC, real-GPU backend runs with all six documented correctness checks passing and wrapper exit 0 for both. No current Windows error is logged. Historical compiler/Linux/software-adapter failures are clearly separable. Interactive visual qualification and the integrated final workload remain outside the supplied evidence. No new run or executable was started for this audit.
