# Milestone B — clock and architecture increment

Date: 2026-10-29, Tor's client timezone. Approval is not implementation or performance evidence.

## Changes and scope

Tor authorized B and the remaining eight-list proposals, revising the initial30 toward terrain-engineering traits, requiring first-build1×/2×/4× testing, a one-hour seasonal cycle at1×, and naval combat plus bridges/docks/transports. Records, selected30 and the future qualification contract now reflect these decisions.

TERRAIN_WATER_AND_SEASONS.md specifies CPU terrain/water owners, conservative dam/reservoir flow, sparse nonvertical burrow graphs, seasonal effects and land/naval integration. These physical/ecological systems are **planned**, not implemented by writing their schema.

Implemented: native integer20Hz clock, retained debt/bounded16tick batches, pause/speed transitions,1×/2×/4× controls/CLI,72,000tick season phase, scope and backlog telemetry. The current scene remains the isolated4.8×3.2km GPU movement/render fixture.

## Named checks

- `cargo test --manifest-path native/Cargo.toml --locked`:13library +3CLI tests passed. Nine additions exercise timing partition invariance at each speed, retained backlog/pause, old-speed debt, exact cycle boundary, checked overflow, supported speed values, elapsed-input single use,4×Control batching/pause and CLI parsing. Seven previous statistics/terrain/shader/numeric checks remain.
- Windows x86-64 GNU `cargo build --locked --release --target x86_64-pc-windows-gnu`: passed using the previously verified workspace-local MinGW toolchain. Bevy/wgpu/Cargo pins unchanged.
- Linux debug `cargo run --locked -- --describe --speed 4`: exited0 without GPU creation; scope identifies integer authority, selectable1/2/4,72,000tick cycle and absent campaign/environment effects.
- Documentation contract check: exactly30unique selected IDs agree with the trait table;12/101 present; cycle72,000ticks; all future integrated scenarios still marked planned.
- `cargo fmt --check` and `git diff --check`: passed.

One initial test compile failed on serde_json macro recursion after metadata growth. Resolved by assigning a separate structured clock object; no global recursion-limit change. Failure/success output remains append-only in root console.txt.

Bounded review caught two temporal handoff problems and the primary agent fixed them: consume elapsed input once; accrue elapsed time at the old state before a pause/speed transition. Clock wall-time accounting retains surface-failure gaps; frame metrics use a separate timestamp. The current CPU tests do not constitute GUI/driver qualification.

## Unmeasured / not tested by this increment

No GPU device run, native GUI event test, Windows execution, real hardware throughput,20km world, dam/flood solver, burrow collapse/flood, actual seasonal ecology, naval combat, campaign save, browser build/deployment or marketing screenshot was produced. The supplied Milestone A actual-PC evidence applies to its older binary only. No updated downloadable package was created; the existing A ZIP is unchanged.

No image generation or Python primitive shape-stamp artwork was used. RULES.md, dependency pins, shaders/material assets, existing saves and deployment audience are unchanged. Future implementation and integrated acceptance cases are in TERRAIN_WATER_AND_SEASONS.md and the planned qualification contract.

New Windows cross-built executable SHA256: `80998d36148a225e0c961f4f2e3f9e3e4374bf4dd427e5a4f16dee93266662e9`. This is an artifact identity, not proof of Windows execution.
