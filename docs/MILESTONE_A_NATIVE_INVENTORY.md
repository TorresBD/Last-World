# Milestone A — native source inventory

Date: 2026-10-29 (Tor's client timezone). Scope: static inspection of the existing native benchmark. No build, test, game, browser, capture, performance run, dependency edit, or artwork generation was performed for this report. The current source is not being claimed freshly compiled by this inventory. Historical test evidence is identified separately below.

## Current truth

The native application is an isolated movement/rendering/visibility benchmark, not native campaign parity. It uses minimal Bevy for a resource and update system, winit for the window/input loop, and a single custom wgpu renderer/device. It does not yet implement the economy, construction orders, military combat, native ecology, conservation, campaign UI or save/load loop. Tor's approved target is now a 20 × 20 km large map, 10,000 military pawns plus non-military agents and wildlife, and 30 additional trait mechanisms plus mandatory biology; none of those integrated workloads is established by this benchmark.

## Ownership and execution

| Area | Source evidence | Current behavior and limits |
|---|---|---|
| ECS | `native/src/runtime.rs:12`, `app():97` | One `Control` resource drives clock, camera, requested count, overlays and benchmark phases. No military/wildlife population is spawned as Bevy ECS entities. `MinimalPlugins` plus an `Update` system are used. |
| GPU/device | `native/src/gpu.rs:39`, `Gpu::new():138` | One owned instance, adapter, device and queue. Vulkan, DX12, Metal and Browser WebGPU are enabled; high-performance adapter preference and no forced fallback. No explicit command-line backend selector. |
| Dependency boundary | `native/Cargo.toml:15–16` | Bevy `=0.20.1`, default features disabled, only `std` and `multi_threaded`; direct wgpu `=27.0.1` with local vendor patch. Bevy rendering is not enabled. The previously recorded wgpu-30 dependency of optional Bevy rendering remains an integration question, not a reason to create a second renderer or silently change pins. |
| Adapter limits | `native/src/gpu.rs:167–181` | Logs `AdapterInfo`, rejects fewer than six storage-buffer bindings, requests empty optional features and limits based on adapter resolution. It does not yet derive configurable scene/cache capacities or RAM/VRAM budgets from the adapter. |
| Military-like agents | `Agent:17`, creation `gpu.rs:231–259`; `compute.wgsl::movement` | GPU owns position, facing, ID, route goal, team, visual role and linked-list next pointer. CPU constructs 100,000 records at startup even when fewer are active. No CPU authoritative soldier state, health, inventory, combat, construction or economic behavior exists here. The two role values select appearance, not a complete worker/military simulation. |
| Static objects | `scene():65` | Fifteen hardcoded structures/ships/beacon objects, with three team-0 sensor entries appended. These are a visual fixture, not player construction. |
| Ground | `Terrain::new():136`; `Gpu::new():186–318` | CPU generates terrain and RGBA, uploads them, then the local `Terrain` is dropped at the end of construction. `Gpu` does not retain the CPU authoritative terrain arrays. There is no native crater, foundation-leveling, filling/smoothing, dirty-region upload, terrain save or live route invalidation path yet. |
| Render path | `Gpu::render():541`; `render.wgsl` | Full-screen terrain/object pass and one instanced troop draw. Troop vertices read GPU agent storage directly. Hidden/offscreen troop triangles collapse in the vertex shader; all active instances still incur vertex invocation cost. No MDI dependency is required. |
| Readback | `Gpu::read_buffer():610`, `capture():633` | Explicit diagnostics/capture allocate staging buffers and call `map_async`, then synchronously poll/wait before copying results. Interactive `render()` performs no whole-army readback. These readbacks are deliberate blocking diagnostic paths, not zero-cost asynchronous gameplay coupling. |

## Clock and update details

`runtime.rs::fixed_clock` is named fixed clock but is an `Update` system with a custom floating accumulator, not Bevy `FixedUpdate` or a full authoritative integer-clock schedule. It clamps frame delta to 0.25 s, caps accumulated requested game time at 4 s, consumes 0.05 s steps (20 Hz), caps one update at 80 steps, and increments `time: f32` and `total_steps: u64`. Speed controls double/halve the requested speed from 1× through 32×; these benchmark controls are not an approved product speed cap.

`Gpu::render` receives the whole step batch. The movement kernel loops that many steps inside each agent invocation, then spatial binning and combined visibility operate on the resulting state. There are no per-substep tactical collisions/attacks to qualify yet. `total_steps` advances before surface acquisition: a lost/outdated/error surface can return before `gpu.render` consumes the batch, so the benchmark clock and agent advancement can diverge on that path. This needs an explicit simulation/render separation before production rules depend on the clock.

The headless loop forces each submitted frame delta to at least 0.05 s (`runtime.rs:405`), renders, and waits for all GPU work. Consequently its frame count is not directly interactive frame pacing or a normal real-time achieved-speed measure. Its report currently omits actual game ticks/achieved speed. Windowed phase transitions use 15 wall seconds per population; the command-line launcher uses five wall seconds. Both must retain their mode/duration labels.

## World dimensions, navigation and visibility

| Layer | Existing dimensions | Evidence |
|---|---|---|
| World | 4,800 × 3,200 world units, treated as metres by project convention | `terrain.rs:2–3` |
| Terrain/collision cells | 600 × 400, 8 m spacing | `terrain.rs:4–5`, `Terrain::new` |
| Ground color image | 2,400 × 1,600 RGBA pixels, 2 m spacing | `terrain.rs` RGBA generation; `gpu.rs:310–315` |
| Navigation/spatial bin grid | 300 × 200, 16 m spacing | `terrain.rs:6–7`, `flow()`; `compute.wgsl::nav_cell` |
| Visibility grid | 150 × 100, 32 m spacing | `compute.wgsl::sensor_masks`, `combine_vision` |

World dimensions and grid strides are repeated as constants/literals in CPU generation, camera bounds, GPU params, movement limits, shader cell functions, satellite geometry and diagnostic list reads. Changing only `WW`/`HH` would not produce a correct 20 km map. Whole-world fine RGBA and terrain allocations also violate the intended large-world streaming architecture if merely scaled up.

`Terrain::flow` builds four CPU Dijkstra direction fields at startup for four fixed destinations. Costs use jungle cover, roads, water and object occupancy. There is no slope/height-difference term in this native flow routine. A road can allow a water crossing. `compute.wgsl::movement` follows the shared fields, cycles destinations, checks 8 m cell passability and attempts a small side-step if blocked. It does not read the spatial linked lists, test soldier-versus-soldier avoidance, resolve crowds, acquire enemies or fire weapons. Building bin lists alone is not evidence of those costs being exercised.

`sensor_masks` calculates static emitter obstruction once through `initialize_masks()`, using height and structure alpha. `combine_vision` merges cached video, explored bits, radar coverage and a rotating 30-degree sector. The fixture includes team-0 emitters only; full two-faction intelligence ownership/contact histories are not implemented. Radar turns once per eight game seconds. The circular satellite footprint uses a fixed 1,800 m radius, 18 s sweep and 180 s interval after the first 180 s; full-map coverage/alternating faction schedules on the approved larger map remain work to port.

The `E`/right-click effect is a 440 ms cosmetic flash centered on the camera with local structure shadow checks. It does not excavate terrain, damage agents, create native resource changes or attribute ecological damage. `render.wgsl` uses up to fifteen ray samples during the flash; there is no continuous scene lighting pass.

## Resident GPU payload arithmetic (not measured VRAM)

The explicit source layouts imply these payload sizes for the existing fixed scene. Driver allocations, alignment, pipeline caches, swapchain, staging, host copies and temporary generation memory are additional and are not measured here.

| Payload | Explicit byte count |
|---|---:|
| Frame uniform: six 16-byte vectors | 96 |
| Terrain: 240,000 × 32-byte cells | 7,680,000 |
| Four route fields: 4 × 60,000 × 8-byte directions | 1,920,000 |
| Agent capacity: 100,000 × 32 bytes | 3,200,000 |
| Spatial heads: 60,000 × 4 bytes | 240,000 |
| Fixture objects/sensors: 18 × 48 bytes | 864 |
| Visibility storage: 255,000 × 4 bytes | 1,020,000 |
| Ground texture: 2,400 × 1,600 × RGBA8 | 15,360,000 |
| Embedded material atlas: 1,024 × 1,024 × RGBA8 | 4,194,304 |

These are arithmetic inventories only, not a total process/RAM/VRAM or target-hardware performance result. CPU generation also creates full-world wood/life arrays, cells and RGBA simultaneously. The current upload helper writes complete textures once at startup; the regular per-frame upload is the 96-byte uniform. No local terrain mutation upload path currently exists.

## Controls, diagnostics and persistence

- Window starts at 1280 × 800, seed 78421, 5,000 active agents and `AutoVsync`. WASD/arrows pan; wheel zooms (0.12–6); Space pauses; 1–6 selects 1k/5k/10k/25k/50k/100k; +/- requests 1–32×; B starts visual sweep; F toggles camera-only fog; R toggles radar overlay; E/right-click flashes. Touch panning and a WebAssembly command queue also exist. These benchmark selectors do not define scenario pawn reductions in the final game.
- `logging.rs::log` opens and appends `console.txt` for each native log call and prints to stderr. Session, panic, adapter, ready, action, resize, surface/GPU errors and per-second performance categories exist. Write errors are discarded. This is synchronous per-call I/O; no bounded logging queue/drop accounting exists.
- `main.rs:2–7` changes the working directory to the executable directory. Native console, relative captures and `benchmark-results.json` therefore live beside the executable. The Python command wrapper writes the repository-root console and captures child stdout/stderr, so root build history and packaged runtime history have different physical locations when launched from different executable directories. Log the actual paths to prevent confusion.
- `native/scripts/run.py` captures compiler/stdout/stderr only for commands launched through that wrapper. Native runtime hooks cannot capture every compiler/OS failure on their own.
- Interactive `PERF` reports averaged FPS, achieved/requested simulation speed, mean CPU encode+submit time and maximum frame-start gap once per second. Encode+submit is not measured GPU execution time. It does not record required median/p95/p99 frame times, process-memory high-water marks, resident VRAM or task-queue/backlog costs.
- Headless JSON reports active count, frames, wall duration, average frames/s, p95 synchronized frame duration, adapter/backend/type and a synchronized-headless flag. It lacks median/p99/max, total/achieved ticks, seed/grid/config hash, build identity and full host/OS/thread metadata. Optional timestamp GPU timings are not enabled; descriptors use `timestamp_writes: None`.
- `benchmark-results.json` is overwritten after the whole sweep. If a later phase fails, the function returns before writing it, potentially leaving an older file while the append-only console contains the new failure. Per-run atomic result files and explicit failed/partial status are a minimal useful diagnostic improvement.
- No native campaign save, terrain-edit persistence, migration or native click-to-update client exists. The launcher/package and browser updater are not native campaign save/update parity.

## Existing build/run/package routes (documented, not run here)

Use the existing log wrapper for future checks. The manifest path below makes the working directory explicit:

```sh
python3 native/scripts/run.py cargo test --manifest-path native/Cargo.toml --locked --lib
python3 native/scripts/run.py cargo build --manifest-path native/Cargo.toml --locked --release
python3 native/scripts/run.py cargo run --manifest-path native/Cargo.toml --locked --release -- --headless --counts 1000,5000,10000,25000,50000,100000 --seconds 5 --check
```

On Windows, `Run-Visual-Test.cmd` launches `Last-World.exe`; `Run-Benchmark.cmd` runs that headless six-population sequence for five seconds each. `--check` performs finite-coordinate, observed first-agent movement, and every-active-agent-exactly-once linked-list checks. It does not validate combat, ecology or full navigation rules. `--capture path.png` requests an actual render-target capture, not image generation.

`native/scripts/package-windows.py` packages an already-built `native/target/x86_64-pc-windows-gnu/release/last-world.exe`, launchers, README, existing console, SHA-256 executable/lock metadata and dependency notices into `dist/downloads/Last-World-Bevy-GPU-Windows.zip`. It writes `.zip.part` then renames. It does not build the executable or prove Windows execution. `export-pixels.mjs` reproduces the native PNG atlas from the established browser procedural pixel forms; it is not Python primitive-shape artwork or image generation.

The historical README still advertises the prior phone/WebGPU testing route and calls the Update accumulator a fixed-clock schedule. Current DECISIONS.md supersedes its phone priority and performance implications. Update user-facing docs alongside the new diagnostic commands rather than modifying dependency pins.

## Historical evidence and qualification limits

`evidence/native-verification.md` records two terrain-reference/Naga checks, Windows GNU cross-compilation, WebAssembly packaging, a six-phase Vulkan correctness run and browser regression checks. `evidence/native-software-vulkan-results.json` identifies the adapter as llvmpipe (LLVM 20.1.2, 256 bits), device type Cpu, backend Vulkan. These software-adapter results are correctness/proxy evidence only. They are not a weak-PC floor, real low-end measurement, complete campaign result, or evidence that Tor's integrated 10k-plus workload meets a target speed.

No Windows runtime check on Tor's hardware, real low-end performance qualification, or integrated native ecology/construction/combat measurement is established by those files. They were read, not rerun, for this inventory.

## Minimal Milestone A additions (PROPOSED implementation sequence)

1. Add an explicit workload/config inventory independent of the renderer. Record map type/dimensions, military/non-military/detailed-wildlife/cohort counters, seed, requested speed, output resolution, effect settings and run purpose. Mark integrated fixtures unavailable until real rules exist; never label extra moving dots a complete civilian/ecology workload.
2. Add bounded frame sample accumulation and median/p95/p99/max, actual submitted simulation ticks/achieved speed, selected adapter/backend/driver, OS/build/config identity, available parallelism and visible pending/unimplemented workload fields. Keep CPU submission metrics separate from GPU timings and VSync/headless modes.
3. Make diagnostic result files per-run or atomically replace a current report while preserving history, with explicit failure/partial results. Log exact console/result paths; validate CLI numbers rather than silently accepting invalid/NaN duration or silently clamping requested counts.
4. Record explicit resident buffer/texture bytes and memory high-water evidence where obtainable; do not present source payload arithmetic as real VRAM. Add supported named proxy controls separately from hardware qualification claims.
5. Preserve this renderer/pins and the browser fidelity reference. Make a concrete plan for retained authoritative terrain, sparse edits/streamed pages, slope-aware navigation and integer-clock simulation before integrating those gameplay systems in Milestone B. Do not simply resize the current full-world textures to 20 km.

These are planned improvements, not evidence of implementation or successful execution in this report. The parent agent owns shared records, actual code changes, acceptance budgets and any authorized builds/tests.
