# Returned Windows stress results — 2026-10-30

Status: **measured** on Tor’s named development PC. This is an audit of returned evidence, with no new build, binary execution, game run or test run. Every result below belongs to the older returned executable `6bac0962…` / native source `2c367d4d…`. Later code changes have no new PC measurement here.

The stress suite finished **251 phases passed and 11 skipped after two failed attempts**, out of 262. Its exact terminal status is `completed_with_failures`, exit `10`, `passed:false`, `performance_qualification:false`, `unsafe_cleanup:false`. Passing a phase records its correctness protocol; it does not promise requested speed, frame latency, full-game coverage or a release pass.

## Input identity and archive handling

| Input | SHA-256 |
|---|---|
| `Send-Back-20960-639289951711982289.zip.part` | `b9b69c88463b3d93e5c1cd03b06b78e50f839a857a24ccce33f8f0dbb9981732` |
| `Send-Back-17524-639289951735827847.zip.part` | `b3796427c58673a7ca3e67bc4a4e0edc5b862700c564971cedfa446bdcc47c23` |

Both `.zip.part` files are valid ZIPs with 669 entries and valid member CRCs. Names match, and every member byte matches except `feedback-bundle-notes.json`. The first note says `supervisor terminal exit 10`; the second says `supervisor_watchdog_exhausted_or_preparation_failed; no suite pass inferred`. They describe packaging of the same retained suite. The `.part` finalization is a launcher/bundling bug, **not missing return data**. `check-launcher-errors.txt` also records `The filename, directory name, or volume label syntax is incorrect.` This does not erase the native failures or convert the suite to a pass.

| Identity | Returned value |
|---|---|
| Run | `20261030T214513275Z-20960` |
| Windows executable SHA-256 | `6bac0962241beebcd60ab3476bc2bf1bd17a06304343687adbb5f92e67e790d9` |
| Native source SHA-256 | `2c367d4d424db6d168468757ac43616c8612e98070189b23ac093643314011d5` |
| Build key | `c3f41014caebcd533d2b102877745c8c5935d370d48cbaf9aeb25d429254e6ab` |
| Embedded benchmark plan SHA-256 | `4e6e36e9d53244cb83289bd76d857488b794ae73f89899fff45b8a4da6e7a70d` |
| Unit catalogue SHA-256 | `eb3b45cfd0322f52a60d824c5d4220e132ee51dfb8dd14d58d5c752ca4655992` |

The manifest packaging commit is `d910929df617f420d58c6ed6fa2026bd7e682da1`; the native build stamp names `44e645fc638e78afd0252b220d9acc28e1a73c52`. The executable and native content hashes, including the supervisor’s owned-process executable hashes, bind this evidence. Commit ancestry alone is not the build identity. Exact archive/member hashes are retained in `evidence/pc-stress-2026-10-30/review-summary.json`.

## Hardware and measurement meaning

AMD Ryzen 9 3900X, 12 cores / 24 logical processors; Windows 10 Pro build 19045; NVIDIA GeForce RTX 2080 SUPER, driver `32.0.16.2101`; 34,266,181,632 bytes visible physical RAM. WMI’s 32-bit AdapterRAM field truncates and is not a verified total-VRAM measurement. These are measurements on the development PC, not real weaker-hardware qualification.

The tables report CPU fixed 20 Hz rule-tick times, **not GPU frame time**. Quantiles are complete whole-case nearest-rank distributions. Achieved speed uses committed simulation time divided by measured phase wall time. OS process peak working set/private bytes include setup/checkpoint portions of the attempt; they are not allocator-only memory or total VRAM. Native RSS is null on Windows. No GPU timestamp timings are available.

The map is 4.8 × 3.2 km. The rate ramp starts with 10,000 military + 2,000 workers + one scout, live ecology and up to 5,000 materialized **existing** wildlife identities. Deaths are not replaced. Phases continue one evolving world and preserve clock debt; this is not eight independent runs of identical state.

## CPU requested-rate ramp

| Requested | Achieved | Requested sustained | Tick p50 / p95 / p99 / max (ms) | Debt start → end (ticks) | Peak WS / private (MiB) |
|---:|---:|:---:|---|---:|---:|
| 1× | 0.997× | Yes | 9.07 / 10.52 / 55.91 / 74.39 | 0 → 1 | 69 / 63 |
| 2× | 1.997× | Yes | 10.27 / 13.46 / 59.45 / 65.41 | 1 → 2 | 259 / 168 |
| 4× | 3.816× | No | 10.65 / 13.45 / 61.01 / 65.51 | 2 → 76 | 520 / 93 |
| 8× | 3.511× | No | 10.81 / 56.48 / 96.93 / 115.25 | 76 → 1,959 | 275 / 234 |
| 16× | 3.473× | No | 10.85 / 12.85 / 85.84 / 91.65 | 1,959 → 7,038 | 569 / 687 |
| 32× | 3.454× | No | 10.90 / 69.80 / 84.29 / 90.35 | 7,038 → 18,673 | 576 / 577 |
| 64× | 3.013× | No | 11.28 / 77.22 / 129.57 / 163.61 | 18,673 → 43,285 | 585 / 531 |
| 100× | 3.033× | No | 11.87 / 32.47 / 108.93 / 118.05 | 43,285 → 84,213 | 586 / 711 |

All eight CPU rate phases passed their correctness protocol. Only requested 1× and 2× were marked sustained. Measurement windows were approximately 20–21 seconds; the ramp ends at tick 9,522. At request 100× it achieved 3.033× and retained 84,213 pending ticks. The larger requests did not establish 100× capacity.

## Fresh named CPU population cases

All three request 100× and passed correctness. None sustained the request. Tactical records include military, workers and one scout; the populations were not clamped to renderer capacity.

| Military + workers | Tactical records | Achieved | Tick p50 / p95 / p99 / max (ms) | Final debt | Peak WS / private (MiB) | Longest advance batch |
|---|---:|---:|---|---:|---:|---:|
| 50,000 + 10,000 | 60,001 | 1.845× | 22.11 / 51.06 / 83.05 / 98.65 | 61,661 | 436 / 518 | 2.59 s |
| 100,000 + 20,000 | 120,001 | 1.159× | 40.57 / 51.47 / 91.76 / 107.07 | 60,537 | 274 / 275 | 2.93 s |
| 200,000 + 40,000 | 240,001 | 0.567× | 82.76 / 109.59 / 147.96 / 153.98 | 68,613 | 791 / 1025 | 6.48 s |

Initial debt was zero. Measured windows were 31.410 / 30.624 / 34.502 seconds for 50k / 100k / 200k military respectively. Additional seed cases 78422 and 78423 passed correctness, achieving 4.088× and 4.179× against requested 32×, with 17,137 and 16,709 pending ticks.

## Exact eleven skipped phases

Each was attempted twice; a skipped phase is not a pass.

| Count | Phase IDs | Native cause and interpretation |
|---:|---|---|
| 8 | `benchmark-window-rate-001`, `-002`, `-004`, `-008`, `-016`, `-032`, `-064`, `-100` | Both attempts: `No grown material for "bunker", team 0`; exit 1. Genuine window harness/material failure, not GPU saturation. Partial frame diagnostics exist but do not qualify these phases. |
| 1 | `benchmark-cpu-season-78421` | Both attempts: `AlreadyPresent`; exit 1 at tick 22,899. Genuine deterministic native failure before the requested full soak. |
| 1 | `benchmark-window-pop-100000-20000` | Expected declared capacity rejection: 120,000 military + workers exceeds the 100,000 tactical display-agent capacity. No world population construction, clamping, render start, FPS or hardware saturation measurement. |
| 1 | `benchmark-probe-after-commit-crash-always` | Expected recovery control: both owned attempts intentionally crash after report/checkpoint commit, exit `-1073740791`, then the phase is skipped. |

The material and `AlreadyPresent` failures had no supervisor timeout, guard stop or native-crash classification. The capacity check is an unsupported declared renderer configuration, and the always-crash phase is an intentional control; neither is a successful workload measurement.

## Windows tests, baseline checks and recovery

Actual Windows discovery found **224 mechanism tests + 3 CLI tests**, with no missing/additional names or inventory drift; all **227 individually executed Windows unit phases passed**. Two discovery phases, six baseline phases and 16 benchmark phases also passed, giving 251 passes.

The six baseline phases were `baseline-campaign-cpu`, `baseline-ground-cpu`, `baseline-movement-vulkan`, `baseline-movement-dx12`, `baseline-river-render` and `baseline-campaign-render`. The two movement backends each checked 1k/10k/100k isolated movement; those are separate from integrated campaign performance. The focused campaign CPU fixture is explicitly **512 military + 64 workers**, not the default population performance case. It passed all 14 gameplay/correctness gates through tick 17,320, including exact late save/load continuation from 17,280 to 17,320 with a 91,373,432-byte save. This focused success does not establish that the natural 10k+2k stress world reached those milestones.

Recovery control `probe-after-commit-crash-once`: attempt 1 intentionally crashed after a valid commit, attempt 2 passed with exit 0. `probe-after-commit-crash-always`: two intentional crashes, then skip. The independent following `probe-continue-after-skipped` phase passed on attempt 1. Valid reports written before intentional crashes did not override the supervisor’s crashed-attempt status. These controls validate observed retry/skip/continue behavior, not gameplay throughput.

## Baseline native campaign frames and UI

The separate successful `campaign-check-4040-1793397024819-report.json` reports tick 2,400, conservation true, three baseline captures and 1,995 accepted frames. Frame interval median / p95 / p99 / max was **10.8345 / 30.6421 / 119.6625 / 1,186.9854 ms**. Mean was 15.0381 ms. These include the native baseline capture path and are not hitch-free 4× qualification. GPU execution time and GPU-completed speed are null.

CPU wall stage mean / max: simulation **11.861 / 145.195 ms**; upload **1.705 / 1,112.859 ms**; capture **1.334 / 2,509.443 ms**; encode/submit **0.229 / 0.607 ms**; present **0.954 / 8.039 ms**; total work **16.151 / 2,549.701 ms**. Stage maxima can occur on different frames and cannot be added or treated as GPU execution. Event-gap timing includes OS/event scheduling.

Native UI composition measured median / p95 / p99 / max **2.401 / 11.842 / 17.198 / 23.712 ms** (157 samples; mean 3.706 ms), including explicit capture variants. This exceeds the 1 ms CPU target on those composition samples. GPU UI execution time remains null, so the separate 1 ms GPU target is unmeasured. The return contains 15 actual UI captures (five panel states at 1366×768, 1920×1080 and 2560×1440), plus baseline campaign/river captures: 21 PNG members at archive root. Their presence is recorded; this numerical audit does not claim a screenshot visual-quality pass. Raw screenshots remain in the input archives.

## Natural evolving-world coverage and remaining qualification

The successful rate ramp reached tick 9,522 with two completed jobs/two buildings, trait 46 committed twice and two traps. Combat totals were 575 shots and 42 deaths. No actual ore delivery, captive pair, release or conservation offspring was reached.

Both failed seasonal attempts continued from tick 9,522 and failed at tick 22,899 with `AlreadyPresent`. They achieved approximately 2.163× against requested 100× over approximately 309 measured seconds; final debt was 687,982 / 688,675 ticks. One seasonal boundary was crossed, but **zero full 72,000-tick cycles** were completed. The native report explicitly marks seasons `not_reached`.

Failed-phase diagnostics observed eight buildings, 11 completed jobs (nine in that phase), trait 12 committed twice and trait 46 five times, four habitat nodes, two passages and five traps. Nests, dens, shelters, dams, reefs and wallows remained zero. These retained diagnostics are observations, not passed natural-world qualification. Logistics/economy and conservation remained `not_reached`; mutation and sanctuary were `unimplemented`; traits were `partially_instrumented`. Physical mechanisms tested in focused fixtures do not establish natural occurrence of every mechanism.

The reports explicitly do not qualify full 20 km gameplay, all 160 traits, million-wildlife operation, a full seasonal soak, weaker real hardware, 100× speed or the GPU UI budget. Repair results in later builds must be kept separate from these historical measurements.

## Evidence locations

Within either returned archive, primary evidence is `check-results/current-suite.json`, `check-results/20261030T214513275Z-20960/run-summary.json`, `suite-journal.json`, matching `benchmark/*-report.json`, case attempt stderr/stdout, actual Windows inventories, `pc-host-history.jsonl`, `native-build-stamp.json`, and `campaign-check-4040-1793397024819-report.json`. Histories also retain the older four-stage `20261030T021220123Z-23228` batch; it is historical and is not counted as part of this 262-phase suite.

The compact machine-readable audit is `evidence/pc-stress-2026-10-30/review-summary.json`. Raw archives and screenshot members were not copied or altered by this audit.
