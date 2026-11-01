# River-flow and retained-terrain increment

Date:2026-10-29, Tor's client timezone. Status is specific to the evidence below; this is not integrated-campaign qualification.

## Required behavior

Tor explicitly requires river flow over time. Building a dam must not turn a river into a fully formed reservoir in one operation. A barrier changes discharge; existing water and continuing inflow gradually raise upstream storage, inundate connected banks and reduce downstream supply. Opening/breaching it releases finite stored water over later ticks.

## Implemented

- `water.rs`: flat CPU-owned finite-volume stores and links,20Hz ordinary ticks, reusable scratch arrays, source/outlet rate remainders, snapshot flow requests, donor allocation and one transactional commit. Checked integer liters and cumulative accounting. Bidirectional head-driven transfers, barrier crest/permeability, finite spill/leak flow, volume-preserving bed/barrier edits. This is a dissipative storage model, without fluid momentum.
- `terrain.rs`: retained integer millimeter elevations plus the existing seeded material/wood/foliage data; signed coarse cut/fill, local deformation and stable-source smoothing APIs; exact coarse displacement reports; row-bounded dirty buffer ranges and local texture repaint rectangles. This fixture is8m terrain sampling, not the future sparse1m campaign terrain.
- `WorldSurface` Bevy Resource: owns the CPU terrain and optional river model; no ECS entity per sample. Solver bed values are a geometry cache refreshed from authoritative terrain revisions. Rendering uses derived heights/depths.
- Existing single wgpu renderer: retained ground texture, partial cell/texture uploads, cached sight invalidation for geometry changes, derived depth/flow shading. Water-only changes do not rebuild static sight masks or repaint the whole ground texture. Dry samples do not gain water merely through visual interpolation.
- `--river`: native local96×128/12,288-cell water interaction fixture over a seeded channel/valley,24,352links. Explicitly zero military/civilian/wildlife agents; it does not reduce an existing army or stand in for a full game stress case. G closes the hydraulic barrier, J opens/breaches, Space pauses, F1/F2/F3 select1×/2×/4×. Water advances independently of camera visibility. No work is repeated on paused/no-tick frames.
- `--water-report`: CPU integration scene, seed78421,4,800ticks with open/closed/breached checkpoints. Append-only `river-history.jsonl` and console records. No GPU requested and no performance claim.

Current initial river water is specified finite stock; the upstream inlet is a named external boundary supply of25,000L/s, and bottom outlets are named sinks. The open initial state is not claimed to be a calibrated hydraulic equilibrium. The fixture perimeter is a test boundary, not the future world's chunk-boundary policy.

## Evidence

-32code checks passed:29library +3CLI. Six new terrain checks cover dirty locality, signed volume, invalid edits, snapshot smoothing, dynamic substrate repaint and effective slopes. Ten new water checks cover conservation/rate fractions, incremental dam fill and connected-bank inundation, blocked ridge connectivity, equal heads, breach, donor overcommit, snapshot ordering, invalid settings/overflow, multi-outlet crest retention and lower open paths. Previous16clock/statistics/terrain/shader/numeric checks remain.
- Locked optimized Linux and Windows x86-64 GNU builds passed. Windows is cross-compiled using the verified workspace toolchain; actual Windows execution/GUI/GPU appearance is not tested by those builds.
- The native CPU interaction report completed all4,800ticks, exited0 and passed independent balance arithmetic at all10checkpoints: initial+external inflow=stored+boundary outflow. Raw report: `evidence/river-foundation-2026-10-29/cpu-interaction-report.json`.

| Checkpoint | Upstream liters | Total stored liters |
|---|---:|---:|
| Open after60 simulated seconds |8,586,635|14,575,660|
| Barrier closed, before next tick |8,586,635|14,575,660|
| Closed after one tick |8,587,871|14,573,415|
| Closed after20seconds |9,039,266|13,681,440|
| Closed after60seconds |9,860,262|11,919,910|
| Closed after120seconds |10,958,366|11,195,460|
| Breached, before next tick |10,958,366|11,195,460|
| Breached after one tick |10,882,036|11,196,375|
| Breached after60seconds |4,594,566|11,025,970|

The upstream gain after closure is2,371,731L over120simulated seconds. Total storage can decline while upstream storage rises because downstream water continues leaving the fixture. Neither closing nor breaching changes water inventory at the operation itself. These numbers describe this deterministic CPU scenario, not real-river calibration, FPS, a20km world or the full RTS.

## Review fixes and limits

Review identified a multi-outlet retention bug before qualification: several individually limited links could jointly spend water below a sealed crest. Each active outgoing path now receives a conservative share of eligible volume before donor allocation; regression checks include both common-crest and lower open paths. Geometry changes preserve liters. Paused/no-tick frames no longer rescan the water graph for display. River telemetry has its own workload label.

The current GPU movement fixture still batches multiple ticks against one cell snapshot. Water mode deliberately excludes coupled troops; future gameplay must interleave water, movement, combat and route changes at fixed tick boundaries. Final-frame water uploads alone do not establish equal coupled outcomes at1×/2×/4×.

Legacy roads still act as any-depth crossings. Real bridge clearance/load, flooding of roads, dynamic shared-route invalidation, vessels/naval combat, wildlife construction/material gathering, ecology/damage attribution, rainfall/groundwater, erosion/sediment, burrow flooding/collapse, seasonal environment, campaign saves and20km streaming remain planned. Trait101 itself is not marked implemented by a hydraulic control alone. Soil displacement API reports do not constitute a finite construction-material economy.

No image generation, new primitive sprite stamps, new renderer, dependency-pin changes or browser deployment. Existing noise/growth material formulas are retained. Shore appearance and actual GPU uploads still need native PC visual review. Old A RAM/performance results apply to its older executable; retained CPU terrain increases the present resident data and no new memory/performance number is claimed.

## PC use

Extract the new separately named Windows ZIP completely. Run `Run-River-Test.cmd`, then use G/J and speed keys. `Run-Water-Check.cmd` produces the deterministic CPU report. Logs append to the extracted folder. The earlier PC-qualification ZIP is retained unchanged; this download is a new foundation build, not the full native campaign.
