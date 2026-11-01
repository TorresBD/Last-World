# Last World — native Milestone B game and PC checks

## Update and play

Use **Last-World-Update-Windows.zip**. Every current update has the same **Last-World** program folder.

1. Close Last World and any running check batch.
2. Extract the ZIP. Merge the new **Last-World** folder's contents into the existing folder containing **Last-World.exe**, replacing matching program files. Keep the same drive/location; D: is fine.
3. Run **Run-Game.cmd** to open the native game (`Last-World.exe --campaign`). Use the on-screen tools/status/controls for the current slice.

Campaign controls:

- F4 selects Road, F6 selects Trench and F7 selects Camera, then click or drag in the world to place the order. B opens the Build category; Escape cancels placement. Alt+F4 retains the normal window-close behavior.

- Left-drag selects troops in video; double-click selects similar visible troops. Right-click orders the selected units to move, attack an identified enemy or board a friendly transport. Shift queues orders. Delete orders the selection to hold. Ctrl+1–9 assigns groups;1–9 recalls them. Alt+Shift+1–4 stores camera bookmarks; Alt+1–4 recalls them.
- B opens building commands. Choose the actual object icon, then a visible world location. Drag roads, trenches and bridges from their start to endpoint. Escape cancels placement. Category tabs expose economy, defenses, recon, conservation and naval orders.
- WASD/arrows and edge scrolling pan; the wheel zooms around the pointer. Click the minimap to navigate. Space pauses; F1/F2/F3choose1×/2×/4×. Qrecruits a paid unarmed camera trooper; Xrequests the current paid artillery action. Explosion flashes expire after440milliseconds of wall time.
- Eopens known ecology; Fopens its known food web; Iopens logistics; Jopens ecological damage; Hopens searchable help; Popens settings. R/C/O/L/U/V/G/K/Ttoggle radar/video/roads/supply/resources/ecological health/disturbance/known fauna/territory layers. An unavailable layer explains its missing underlying rule. Panels can be closed, moved, pinned and collapsed; layout, scale and accessibility preferences are saved separately.
- Tab cycles observed animal species. F5saves to `saves/last-world.json`; F9restores it. A normal new launch chooses a fresh logged seed; loading preserves the saved seed. Automated checks lock gameplay inputs and use a fixed reference seed. Rejected orders appear as compact feedback and in console.txt.

The native HUD uses the existing wgpu device. It bundles Rajdhani and IBM Plex Sans, their OFL licenses, actual font glyph atlases and the central `ui/theme.json`. Screenshots collected by the campaign check use actual known game state; static design mockups are not native evidence.

The payload excludes existing console/history, settings, saves, previous check results and feedback. Replacing its program files does not reset that data. The build manifest hashes the owned files so a mixed/partial update can be detected. Do not use a mirror/delete operation on your game folder.

The milestone is the connected small native game: construction/logistics/combat and the procedural ecosystem/conservation loop share the same world and integer clock. **Read project/MASTER.md from the source root (Source/project/MASTER.md in an update package) for current implemented/tested/measured coverage and unresolved issues.** A build label or successful automated stage does not establish full milestone acceptance by itself.

## One requested PC run, one returned file

When requested, run **Run-Checks.cmd** once. Leave the automatic test windows open; they close themselves. Send back only **Send-Back.zip** from this game folder. There is no need to choose logs, reports or screenshots yourself.

The supervisor first discovers the actual tests in both packaged Windows test executables with `--list`, compares them with the host-expected catalogue, and runs each actual test in its own process. An exact test must report one passed test; an exit0 with zero tests never passes. It then retains these six baseline stages:

1. `--campaign-report`: CPU integrated campaign correctness scenario, no graphics device. Appends `campaign-history.jsonl` and console records.
2. `--ground-report`: existing deterministic coupled water/soil/biomass correctness report.
3. Vulkan isolated movement checks at1k/10k/100k records,3seconds each.
4. DX12 isolated movement checks at the same counts/durations.
5. `--river-check --speed 4`: automatic river sequence and start/dam/drained captures.
6. `--campaign-check --speed 4`: automatic small campaign sequence, start/built/ecology captures, and15actual HUD/panel screenshots at1366×768,1920×1080and2560×1440.

The same run also executes every isolated case in the bundled `benchmark-plan.json`, including the requested-rate ramp through100×, seasonal/seed checks and larger population cases. These are benchmark requests; normal playable speeds remain1×/2×/4×. The plan sets each case's own wall deadline. Long cases may take hours. Completed phases are not rerun when you reopen the same build; unfinished work resumes from its durable journal and successful native checkpoint. Do not open another game window from this folder during the suite.

Each phase gets at most two attempts. A native crash/failure retries once, then that phase is explicitly skipped with its reason and the other phases continue. Timeouts and memory-guard stops are recorded separately from native crashes. The process guard is the smaller of6GiB and25% of OS-visible physical RAM, with a hard commit limit and one-second process counter sampling. This protects the diagnostic process; it is not a playable pawn cap or a hardware capacity measurement. The CMD watchdog may restart an unexpectedly lost supervisor twice, but never loops on a completed-with-failures result. Uncertain child ownership stops further launches safely.

Retry2 replays the prephase state and repeats required GPU validation. Small attempt/build-specific rejection markers keep a failed teardown checkpoint out of later cases; the supervisor never edits simulation saves. Explicit crash-recovery probes are intentional: the always-crashing probe stays skipped after two attempts, so a finished suite can correctly report failures while demonstrating continuation. No skipped probe is counted as passed.

**Send-Back.zip** contains append-only histories, hardware/driver context, source/executable/plan identities, atomic progress journal, per-attempt raw output, sampled process memory and current owned reports/captures. Native benchmark reports must match the current case/PID/session/time, pass and commit their validated checkpoint. Skipped/failed/unsupported phases are never counted as passed. Large checkpoint saves stay local under `check-results` for resume and are excluded from the return ZIP; exclusion notes are included. A second click leaves an active supervisor alone.

The feedback ZIP is finalized before replacing the previous snapshot. Underlying logs/run folders stay intact. No arbitrary player saves/settings or older ZIPs are packed recursively. If the main PowerShell script cannot start, the CMD attempts a separate minimal log bundle; if that also fails, it says no current ZIP could be created rather than presenting an old one as new. These exceptional branches require their own execution evidence; they are not proved by a successful normal run.

The original four-stage normal workflow was tested on Tor's Windows10/RTX2080SUPER PC. The comprehensive discovery/retry/resume/Job Object supervisor is new. Portable contract/model and source checks run locally; PowerShell and its Win32 helper cannot execute on this host. Its actual Windows behavior remains a PC gate. See `docs/BENCHMARK_SUPERVISION.md` for the protocol and exact evidence limits.

## Scope and performance claims

The current small native world is **4.8×3.2km**. The approved production target remains20×20km with Continental, Archipelago, Large Land Mass and Medium Islands generation controls, plus smaller modes. Initial30 additional trait mechanisms are separate from mandatory biology; all160 remain release scope. Milestone B is a development checkpoint, not the complete20km/all160 release. The target is10,000military **plus** nonmilitary and wildlife; isolated moving records are not a substitute for that integrated workload.

The campaign is CPU-authoritative for gameplay; GPU buffers are display copies. The GPU does not independently move the same campaign animals/pawns or rewrite commander masks. Construction, ecology, navigation, conservation, save/load and tactical results must remain consistent under the single integer20Hz clock. Terrain/foliage use raw arrays, not one ECS entity per tile/tree. Only actual observed state is exposed to the commander; unidentified radar echoes do not reveal live hidden identity.

The separate legacy movement fixture still uses GPU-owned movement and spatial indexing. Its headless loop waits for GPU completion and uses a synthetic minimum50ms frame delta. Its frames/s and synthetic simulated-seconds/s do not establish real-time campaign speed. Software adapters are labeled proxy checks, never a measured low-end result or performance floor. Known buffer payload is not total VRAM, and OS working set is not allocator-only RAM.

Requested1×/2×/4× uses ordinary fixed ticks with retained backlog, not changed rule intervals or silently reduced populations. A seasonal cycle is72,000ticks:1hour at achieved1×. Pause and overload affect the shared calendar. Higher requested speed is not a performance guarantee. Reports keep median/p95/p99/max and workload kind; the prior river build logged a909.9ms interval that requires stage-level investigation. Never infer hitch-free4× or full-game60FPS from an isolated passing check.

## Focused legacy checks

These remain available when a specific investigation needs them; routine feedback uses **Run-Checks.cmd** and one ZIP.

- `Run-River-Test.cmd` / `Last-World.exe --river`: isolated water/ground window with no military/civilian/wildlife agents. Gcloses the hydraulic barrier; Jbreaches/opens; Space pauses; F1/F2/F3choose1×/2×/4×. It is a fixture, not the campaign.
- `Run-Water-Check.cmd` / `--water-report` or `--ground-report`:4800tick deterministic CPU water/ground sequence at seed78421. It records exact finite water/soil/biomass accounts, not rendering/FPS or calibrated natural hydraulics.
- `Run-PC-Qualification.cmd`: older focused Windows host and5second-per-count movement qualification. `Run-Benchmark.cmd` runs the full synchronized movement sweep. Their isolated metrics must stay attached to their workload and executable identity.
- `Last-World.exe --describe`: actual scope metadata without creating a graphics device.
- `--backend vulkan` / `--backend dx12`: select a backend explicitly. `--speed 1`, `--speed 2`, `--speed 4`: starting requested speed. Invalid inputs fail rather than silently reducing requested work.

Existing movement/river controls: WASD/arrows pan, wheel zooms, Space pauses, F1/F2/F3speeds. In the movement fixture1–6changes count, Bruns its visual population sweep, Ftoggles camera vision, Rtoggles radar overlay and Etriggers the temporary explosion flash. Campaign tools use the campaign HUD; population keys do not define a game army cap.

## Visual and architecture continuity

One existing wgpu27 renderer/device is retained; optional Bevy rendering stays disabled and its wgpu30 handles are not mixed with it. Existing procedural ground and noise-grown material atlases are reused. The campaign renderer draws CPU positions and structures as instances and composites screen-space text/buttons on that same device. HUD text/rectangles are UI, not physical sprite art. Organic fauna materials come from the existing atlas. No new image-generation or primitive-stamped physical art was used; both require Tor's prior approval under project/MASTER.md rule 5.5, with the explicit pixel-map exceptions in rule 5.12.

Terrain remains CPU-owned integer millimeter elevations with local changes and dirty buffer/texture updates. The local water/ground model owns finite stocks and uses staged transfers, erosion/deposition, finite foliage detachment and delayed soil drying. Coarse fixture sampling is8m; it is not the planned sparse fine-resolution production world. Water momentum, climate calibration and final balance cannot be inferred from conservation alone. Ground remains brown/disturbed/moist after drainage according to actual stored state, not an instant decorative reset.

Full production parity, all160 mechanism interactions, long-war behavior, named real low-end measurements,20km streaming and final click-to-update delivery remain separate acceptance work. Preserve migration/previous-save guarantees as save schemas evolve. The historical browser game remains a visual/behavioral reference; native PC is the product and primary testing platform. Phone play is currently deferred.

## Build

Rust minimum1.97.1. Preserve Cargo.lock and the pinned engine versions.

```
cargo test --locked
cargo build --locked --release
cargo run --locked --release -- --campaign-report
cargo run --locked --release -- --campaign
cargo run --locked --release -- --campaign-check --speed 4
cargo run --locked --release -- --ground-report
cargo run --locked --release -- --river --speed 1
cargo run --locked --release -- --headless --counts 256,1000 --seconds 1 --check --capture frame.png
```

Windowed/capture and movement commands require a graphics adapter. Captures/readbacks occur only on explicit checks/capture requests; campaign simulation does not require whole-army GPU readback.

`node native/scripts/export-pixels.mjs` reproduces the unchanged procedural material atlas from `dist/pixel-forms.mjs`. The renderer's fullscreen triangle delivers pixel shading; it is not visible as a physical object. Radar/satellite/range/selection shapes are tactical UI.

`vendor/wgpu` retains wgpu27.0.1 with the documented upstream canvas-context compatibility backport; see `vendor/PATCHES.md`. No pin upgrade follows from this milestone.

Release packaging uses `python native/scripts/build-release.py --cargo cargo` (add `--target x86_64-pc-windows-gnu` for the cross-build). This logs compilation and records source hashes before/after the build. Freeze source edits while it runs; use `--force-project` after an interrupted or concurrent-edit build. The overlay packager requires the matching source/executable stamp, so a recent executable timestamp alone cannot qualify stale compiled code.


Updated speed-test policy (Tor,2026-10-30): one existing native window/device runs an adaptive1/2/4/8/16/32/64/100 ramp. It loads once, gives the living simulation15seconds initial warm-up,5seconds settling per rate, and at least two10second measurement windows. Sustained capacity failure stops higher rates; those rates are reported not attempted, not passed. The complete world, authoritative tick and retained debt continue throughout. The72,000-tick seasonal soak, population/seed probes and mechanism checks remain independent correctness/load checks, rather than additional speed comparisons. No achieved100x or4x guarantee.

The previous Send-Back.zip replacement failure is repaired in the launcher; existing feedback is atomically replaced after the new ZIP closes and flushes, and failures retain exception diagnostics. No manual .part handling should be required in the new candidate. Actual Windows validation remains required.


To honor the single-speed-test policy, all other focused smoke/population/seed/recovery probes request only1x; the72,000-tick seasonal soak requests2x and may take roughly30minutes or longer if achieved throughput falls. These phases test load/correctness, not successive speed ceilings. Only the adaptive ramp can ask for4/8/16/32/64/100x. Actual debt and achieved timing remain reported for every phase.


Warm-up begins after the first successfully presented native frame, so initial loading/upload cannot consume the15second allowance. Every measured window requires actual successful presentation; minimized/interrupted windows cannot qualify GPU capacity.1Hz owned-process hardware samples in the feedback archive include CPU time/utilization, process RAM/peaks, system physical/commit headroom and optional Windows GPU engine plus dedicated/shared allocation counters. Missing counters report unavailable, never invented zero. GPU engine maxima are not summed into an overall GPU utilization percentage; dedicated/shared counters are named allocation measurements, not raw GPU execution timing. Logger setup/sample overhead is separate. The game uses its retained device/textures/instance buffers; no artificial VRAM filling.
