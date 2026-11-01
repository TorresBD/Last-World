# Coupled ground/water batch

Date:2026-10-29, Tor's client timezone. This batch adds local ground interaction and the single-update/single-feedback workflow. It does not qualify the20km campaign or complete the RTS.

## Implemented mechanisms

`ground.rs` owns finite local bed sediment, suspended sediment, live foliage, transported/deposited debris and soil water. It shares the authoritative20Hz tick with `WaterGraph`; a transfer generation cannot be consumed twice. Flat arrays and reusable scratch hold proposed changes before committing. No terrain/foliage ECS entities or per-cell heap collections are added.

Actual last-tick water transfers advect sediment/debris through connected links and named boundary outlets. Erosion removes finite erodible substrate; lower carrying capacity limits new erosion. Deposition adds existing material to the bed. Local bed elevation feeds back into subsequent water transfers, and the terrain's rendering/query cache receives the same committed geometry. External digging/filling of a managed patch requires a material transaction; the prototype rejects an inconsistent overwrite rather than silently restoring stocks.

The bed has a finite erodible layer over a resistant floor. The first model uses a depth/flow/root-resistance proxy, not calibrated hydrodynamic momentum or full sediment mechanics. New erosion has a default suspension capacity of100permille relative to local water. Excess settles at a bounded rate, including in faster water; dry cells deposit all remaining finite residue. Material remains exact to integer liters while rendered/query elevation is quantized to1mm with submillimeter material volume retained. `current_deposit_subset_liters` is a subset of bed stock and must not be counted again.

Foliage detaches from finite generated patch units at an eligible flow/depth/resistance threshold and becomes mobile/deposited/outflow biomass. Existing noise-grown wood/leaf/grass templates are thinned deterministically; no new physical sprite assets or primitive stamps are introduced. Zero live stock exposes dirt. This is a patch-level engineering primitive, not the final species/roots/food-web biology or automatic regrowth.

Soil water is withdrawn from real surface storage, remains owned by the ground ledger, and later evaporates as a named loss. The default fixture holds moisture for60simulated seconds of continuous air exposure, then dries gradually. Rewetting resets exposure eligibility. Submillimeter films retain their liters but do not block air exposure forever; scouring/detachment require at least10mm of water. Defaults are development tuning, not a claim of real-world calibration or final game balance.

Brown dirt and deeper dark-brown bed colors replace the earlier blue-green treatment. Drained wet/disturbed soil persists through the ground-state cache and partial repaint until actual moisture changes. Drying never restores eroded soil or washed-out vegetation. Full biological recovery will need viable organisms/stages, resources and time.

`WorldSurface` holds the coherent CPU transaction through each accepted frame update; water and ground process every ordinary tick before the final terrain upload. The fixture continues with no military agents, so this is not yet water/combat/path coupling. Hidden or offscreen test regions still progress. The existing renderer keeps partial updates and avoids unused sight-mask work in the water-only view.

## Checks and findings

The combined Rust suite passes50checks:47library and3CLI. Eighteen additions beyond the previous32 cover finite erosion/deposition/transport, source/sink conservation, moisture hold/drying/rewetting, tiny films, shallow-flow resistance, stale tick/snapshot rejection, atomic water exchanges, finite foliage display, persistent wet dirt, carrying capacity and fractional settling. Existing terrain reference, shader validation, clock/speed and bounded frame-statistics checks remain.

Review of the first full seeded run found that exact conservation alone still allowed unrealistic sediment concentration. That run is retained as `evidence/ground-water-2026-10-29/pre-capacity-review.json`, explicitly superseded. The new carrying-capacity and settling checks address that finding. No unsuccessful/historical output is removed from console history.

The deterministic CPU scenario runs4,800ticks/240simulated seconds, closes a barrier after60seconds, and later breaches it. Its final accepted report and independently recomputed balance summary are stored alongside the review case. The report includes the exact ground configuration, source seed and schema version. Frame time is not inferred from this CPU correctness scene.

The automatic native renderer test is a different timeline: close at20seconds, breach at80seconds, finish at180seconds, capturing start/closed/drained frames from the actual GPU target. It writes a unique PID/run report. It requires an actual GPU/PC run and does not run on the current driverless Linux build host.

## Update and feedback workflow

`Last-World-Update-Windows.zip` contains the stable `Last-World/` program folder and no mutable console/history/settings/save files. Payload hashes detect mixed or incomplete copies. `Run-Checks.cmd` runs the requested CPU/backend/renderer checks and produces one `Send-Back.zip`. Details and exact steps: `UPDATE_AND_FEEDBACK.md`.

Static review checked PowerShell5.1/CMD flow, child ownership, bounded timeouts, log ownership, a per-install batch lock, PID-scoped fresh captures, safe ZIP selection and fallback collection. The script has not been executed on Windows here. The returned PC ZIP will establish that path.

Package verification checks CRC, required/hash-matched owned files and a simulated overlay preserving existing console, histories, settings, saves, prior results and feedback. It does not certify Explorer behavior or a Windows driver by itself.

## Remaining scope

Full-world coarse hydrology/streaming and fine edit pages; riverbank mechanics beyond this proxy; biological roots/species and recovery; terrain construction/material transactions; military/naval path/combat coupling; actual seasons/climate/groundwater; burrows; conservation/research; all160traits; campaign saves and native UI/update parity remain required. The current work preserves those interfaces/scope without claiming they are implemented by a local scene. No new particle labels, image generation, Python primitive sprite stamps, engine-pin upgrade or Site deployment.


## Final native correctness/build evidence

- Locked optimized Linux and Windows x86-64 GNU builds passed after the capacity/settling corrections. Windows is cross-compiled; no new Windows execution is claimed here.
- Final CPU report exited0 after4,800ticks. Independent arithmetic recomputed all water/sediment/biomass/soil-water/combined-water identities at all10checkpoints. Configuration records100permille suspension capacity and1,200tick exposure delay.
- End-of-scenario cumulative erosion2,418,368L; deposition921,005L; suspended sediment642,375L; surface water6,859,571L. Live foliage fell by1,431,283finite units; the units are all owned by mobile/deposited/outflow accounts. Soil water6,205,043L; evaporation947,686L. These are exact model stocks, not calibrated nature or performance figures.
- Raw accepted report: evidence/ground-water-2026-10-29/cpu-report.json. Independently recomputed summary: cpu-balance-summary.json in the same folder. The pre-capacity review case is retained separately.
- New PC batch treats current-child native FATAL/PANIC/GPU_ERROR/SURFACE_ERROR tags as a failed stage even if an executable exits0. Fresh PID-scoped captures do not hide errors; one feedback ZIP still collects them.

## Subsequent actual-PC evidence

Tor returned the single Send-Back.zip. All four functional stages passed on the confirmed Windows/RTX2080SUPER system; the CPU report exactly matches this accepted Linux reference, and275independently recomputed stock identities across CPU/GUI/final records balance. All three renderer captures were returned. Windows normal-path automation/rendering is now tested. River latency is not qualified: a909.9335ms frame interval accumulated56pending ticks before catch-up. Cause remains unlocalized with the current encode/submit-only stage timer. Full evidence, measurements and next instrumentation task: `COUPLED_PC_BATCH_RESULTS.md`. No source changes, new builds or immediate PC rerun for this evidence review.
