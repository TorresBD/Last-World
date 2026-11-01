# Milestone A — verification and limits

Date: 2026-10-29 (Tor's client timezone). Build/compiler/check output is retained in append-only root console.txt. No image generation or Python shape-stamped artwork was used. Dependency pins, shaders/material assets and browser gameplay were not changed.

## Tested / implemented

- `cargo test --manifest-path native/Cargo.toml --locked`: **7 passed**, 0 failed. Five library checks: terrain matches the Long Front reference, WGSL kernels parse/validate, and three frame-recorder checks for quantiles/spikes, invalid values, bounded-ring overflow/reset. Two binary checks reject invalid counts and nonfinite/nonpositive durations. The suite passed again after reporting-review fixes.
- `cargo build --manifest-path native/Cargo.toml --locked --release`: Linux optimized build passed. Subsequent reporting-label changes compile in the repeated test build; the final Windows build is recorded separately below.
- Native `--describe`: exited 0, reported fixed 4.8 × 3.2 km isolated workload and explicit unimplemented 20 km/10k-military-plus-other-populations targets.
- Native `--headless --counts 0 --seconds 1`: exited 1 with intended rejection, before adapter initialization. This expected error is retained rather than silently substituting a smaller count.
- A separate read-only review found and corrected three issues: CPU-submitted speed mislabeled as GPU-completed achievement, movement-check wording overstating coverage, and potential owned-child leakage on PowerShell errors. PowerShell runtime behavior still needs Windows execution.

## Failed / unavailable checks retained

- `--headless --backend vulkan --counts 256,10000 --seconds 1 --check` failed before creating an adapter: no usable Vulkan graphics driver/ICD is available here. No new GPU movement/correctness/performance result was obtained. The shader unit check is validation, not execution.
- Initial Windows cross-build failed because MinGW dlltool was missing. System package installation failed under container permissions. Workspace-local package extraction was attempted instead; checksum checks detected a truncated GCC download and stopped before executing it. These failures remain in console.txt.
- This environment has no actual Windows desktop/PowerShell runtime or Tor's GPU. Cross-compilation, when successful, does not prove the Windows launcher or application ran. No real-PC, real low-end or integrated native campaign performance is measured.

## Windows artifact status

Implemented: `cargo build --manifest-path native/Cargo.toml --locked --release --target x86_64-pc-windows-gnu` passed using the verified workspace-local MinGW toolchain. The earlier missing-tool/truncated-download failures were resolved without a system install or engine-pin change.

Tested static metadata: the executable is x86-64 PE32+, 10,526,543 bytes, SHA256 `a8b9f619be733a1eb18436e0988f9b611a8b55001aec57aaa923ce29ae609d69`. Its imports indicate Windows system DLLs only; no libgcc/libwinpthread/libstdc++ import was found. All 15 audited native source/shader/asset/Cargo inputs predate the executable. See MILESTONE_A_WINDOWS_BUILD_AUDIT.md. Static import inspection is not Windows execution or proof of all dynamically loaded graphics dependencies.

The packager rejects an executable older than native inputs and records executable/lock/source hashes. Packaging/archive integrity evidence follows the actual package run; actual Windows/PowerShell execution and target-hardware performance remain pending.

## Interpretation

Current code is a benchmark, not the integrated game. Software adapters are proxy checks only. Interactive frame intervals and CPU submission time are not GPU execution timing; CPU-submitted ticks are not completed GPU simulation speed. Headless synthetic time is labeled explicitly. Actual RAM/VRAM qualification remains pending; the Windows wrapper samples OS process working set, while known GPU-buffer bytes exclude textures/driver/staging overhead.

The inventories describe pre-A source behavior. New diagnostics do not implement the missing 20 km generator, editable terrain, economy/combat/ecology, conservation or saves. Proposed fixture counts/budgets and proposed30 list remain unapproved, with all mandatory biology and all160 release mechanisms preserved.

## Package verification

Tested: Last-World-PC-Qualification-Windows.zip is 5,326,492 bytes / 702 entries; ZIP CRC, required entry presence, embedded executable SHA256 and Cargo.lock hash all passed. Evidence: evidence/milestone-a-package-verification.json. Archive SHA256: 47ef7391094cfb766c6498a560b2c0d2ad2a41db2d94d1e72b74c949f9ee8437. Source checkpoint: f654db573e75e6b9986c44ab929e1875c8b7716c. These are archive/static checks, not actual Windows execution.

Next external step: Tor runs Run-PC-Qualification.cmd on the confirmed development PC and supplies console.txt, benchmark-history.jsonl, pc-host-history.jsonl and workload-scope.json. Optional visual run checks presentation/controls. These remain isolated benchmark checks; integrated game qualification and Milestone B authorization remain pending.

## Superseding actual-PC result record — 2026-10-29

Previously pending Windows execution evidence was supplied by Tor. Vulkan and DX12 both completed1k/10k/100k phases, all6 correctness checks passed, both exits0, and current Windows sessions contain no error tags. Successful PowerShell host/process-memory/backend-completion records are present. Full measured figures and limits: MILESTONE_A_PC_RESULTS.md; independent session/error audit: MILESTONE_A_PC_LOG_AUDIT.md. These supersede the earlier pending automated-run gate. Interactive presentation/controls, exceptional cleanup, integrated campaign, long-war and low-end performance remain unqualified. No source code changed and no new build was needed for this review.
