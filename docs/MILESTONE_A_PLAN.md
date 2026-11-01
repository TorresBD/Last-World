# Milestone A — current truth, workloads and implementation sequence

Date: 2026-10-29, Tor's client timezone. Read RULES.md, STATE.md, DECISIONS.md and BLOCKERS.md first. Milestone B is now authorized by Tor; this file preserves the A findings and updates the integration direction. Inventory reports describe the pre-A source snapshot; the diagnostic changes below are subsequent work.

## Current truth

| Area | Evidence and boundary |
|---|---|
| Approved product | Native PC RTS, 20 × 20 km large map, four approved topologies and configurable rivers/mountains; smaller/shorter modes too. |
| Approved workload | 10,000 military PLUS non-military and wildlife. Civilians, exact animals, cohort records and total wildlife are separate costs. |
| Approved biology | Mandatory animal/plant/ecosystem biology plus 30 additional mechanisms in the first integrated slice; all 160 in final scope. The revised initial30 prioritize map-changing species; see MILESTONE_A_TRAIT_PROPOSAL.md. |
| Duration | Typical large war3–4 hours. One-hour full seasonal cycle at1×;1×/2×/4× testing options required in first build. |
| Browser reference | Economy/construction/combat/recon and procedural pixel visuals, local crater/trench excavation, simple habitat/fauna damage, save/update. It is not the required procedural ecosystem. |
| Native reference | 4.8×3.2km movement/render fixture plus an opt-in local river scene. Coarse CPU editable ground/partial uploads now exist; full military/civilian/ecology/campaign saves remain planned. |
| Important ground gap | Coarse cut/fill/smoothing APIs now exist; real construction/material jobs, fine pages and deformation-aware movement-route integration remain required campaign work. |
| A diagnostic additions | Explicit scope/backend/host reports, bounded frame quantiles, strict CLI validation, retained phase history, Windows process-memory wrapper and safer surface-failure clock behavior. Named verification evidence is separate. |
| Renderer | Existing single wgpu 27 renderer retained; optional Bevy rendering disabled. No pins changed, no second renderer. The optional Bevy renderer's wgpu 30 handles cannot be mixed directly. |
| Qualification | Actual-PC isolated benchmark measured on Ryzen9/RTX2080 SUPER;6 phase checks passed. Integrated game and real low-end performance remain UNMEASURED; no visual-session evidence. Software runs remain proxy checks. |

## Milestone A deliverables

1. Browser/native inventories with concrete file/function evidence in the two MILESTONE_A inventory reports.
2. Current executable scope and diagnostic improvements without claiming new campaign systems.
3. Native PC qualification package: explicit Vulkan/DX12, working-set sampling and append-only results. A cross-build is not a Windows runtime check.
4. Future integrated scenarios and development targets in `native/qualification/milestone-a-scenarios.json`.
5. Exactly30 selected additional mechanisms, revised under Tor's terrain-engineering direction and separate from mandatory biology, in `MILESTONE_A_TRAIT_PROPOSAL.md`.
6. Updated state/parity/blocker/decision records and named verification evidence.

## Approved workload and development goals — not measurements

The JSON contract describes future integrated distributed-logistics, combat-chokepoint, satellite/recon, terrain-burst, ecology/conservation and long-war save/reload scenes. It is not executable game content. Every approved map topology needs coverage, including island access and river crossings.

APPROVED qualification fixture values, not population caps or every-world balance: the approved 10,000 military plus 2,000 civilians, 5,000 exact wildlife and 1,000,000 additional cohort wildlife, using 256 shared species definitions. Counts of organisms, exact individuals and resident cohort records must be reported independently. Captive/transit/viable-stage stocks remain real; no aggregation may erase identity/state needed by gameplay.

APPROVED development goals, UNMEASURED on the integrated workload: steady frame median 16.67 ms, p95 25 ms, p99 50 ms; flag maximum over 100 ms; peak process RAM 4 GiB; GPU payload/cache budget 2 GiB; mature edited-world saves 256 MiB and load within 15 seconds on named storage. These are development targets, not minimum specs or measured guarantees. Driver/texture/staging/generation overhead and OS working set need separate accounting/headroom.

Characterize1× first; selectable1×/2×/4× are approved first-build testing options. This is not a measured4× full-workload guarantee. Overload must visibly lower achieved speed while preserving ticks/rules and individual military units. No hidden count reduction, omitted ecology or larger timesteps.

Record source/build identity, actual hardware/OS/driver/backend, proxy limits, seed/generator/settings, dimensions/topology, all population categories, active/queued chunks, resolution/effects, requested/achieved speed, frame tails, CPU submission versus GPU execution, memory high-water, upload bytes/backlog, route reuse/invalidations and save/load bytes/time.

## Dependency sequence for authorized Milestone B

### 1. Authoritative world, clock and editable terrain

Introduce saved world settings and one integer game-tick authority. Fixed tactical intervals, staggered decisions and coarse ecology use that clock. Define deterministic event order and revision/due-tick acceptance for background work; rejecting stale results alone does not guarantee equal accepted worlds if commit time depends on host speed.

Retain CPU authoritative base terrain/edits. All consumers use effective height for slopes, construction, camera/radar blockers and cover/projectile checks. Implement crater excavation, trenches, foundation cut/fill and smoothing/refilling as local edit jobs with labor/resource policies. Numerical terrain balance remains to be chosen. Preserve existing procedural ground/material formulas, colorful foliage and organic disturbed edges.

Track dirty page bounds and stream partial GPU updates. Do not resize the current whole-map fine raster to 20 km. At 2 m RGBA, a 20 km square is 100 million pixels / 400 million bytes for one color copy alone, before all other fields and temporaries.

Candidate layout (PROPOSED): 256 m chunks with resident 8 m terrain samples and sparse 1 m edits; global ecological fields at 64 m cells with 32-cell regions. A 20 km coarse layer is 313² cells: five f32 arrays are about 1.96 MB before metadata. These resolutions are role-specific proposals, not immutable approved settings. Regenerate unedited CPU pages from saved generator parameters, retain persistent edit pages, and compact dense long-war pages rather than assuming edits remain sparse.

### 2. Large-world topology, routes and real native jobs

Generate the approved topology families with continuous fields/hydrology and river/mountain controls. Validate opposite-side landing starts, resources, reachable habitat and strategic routes. Tor approved bridges, docks, troop/cargo transports and a combat navy. Build changing-water depth, channel connectivity, shore access and bridge-clearance interfaces early; exact roster and balance remain to design.

Share corridors/flow fields keyed by destination, movement class and relevant region revisions. Individuals still need immediate avoidance/spacing and real attack recipients. Terrain/road/bridge edits invalidate affected routes promptly with bounded reroute work. A populated world spread over many regions is not a stress test of a local chokepoint.

Port actual workers, cargo/reservations, construction, roads/defenses, production/consumption and combat into the CPU-authoritative game. Keep the existing renderer through compact page/instance uploads first. GPU movement in the benchmark is not permission for CPU combat to use an independent stale location table. Reconsider GPU authority only with an explicit bounded synchronization model and measured end-to-end benefit.

### 3. Required ecology plus the terrain-prioritized30 mechanisms

Keep mandatory biology and original nocturnality/docility/slow movement/milk production outside the count of 30. Species share immutable anatomy/parameters/food-web edges. Individuals retain compact changing state. Ordinary unseen populations use sparse non-ECS cohort classes, real finite resources and pending reproductive/viable-stage records, not a second simulation owning the same animals.

Generation order: terrain/resources → regional roles and food web → compatible species/partners → ranges and viable population structure. Initialize coarse evolving ecological state across the whole world, including unseen areas. Birth/death/consumption have explicit sources/sinks; representation and location transfers conserve ownership exactly once. Identity-sensitive/captive/reproductive records are not discarded into an average.

The revised30 exercise dams, branching burrows, soil/vegetation engineering, aquatic structures, migration/dormancy, finite life stages, plant partners and recovery. Shared water storage, underground geometry and seasonal architecture are specified in TERRAIN_WATER_AND_SEASONS.md; species must use those owners rather than duplicate simulations. Research reveals existing definitions, capture/release transfers real organisms, and complete extinction cannot regenerate from the original seed. All 160 mechanisms remain planned release scope.

### 4. Recon, player understanding, saves and qualification

Scale satellite coverage to 20 km, prewarm bounded visual work ahead of the circular reveal and filter all player payloads. Observation does not activate detailed AI for every animal. Radar, cameras and ecology knowledge remain separate from cosmetic explosion shadows and biological senses.

Add readable known/estimated/stale ecology information, food-web links, shortage causes, damage attribution, protection/research controls, tutorials, audio and enemy difficulty. Teach roads/logistics/ecology in a short playable scenario with an outcome. Hidden inspectors must not leak positions/species/health.

Preserve versioned atomic saves with the previous save, generator/species definitions, terrain pages, exact/cohort/captive/transit/viable stocks, pending deadlines, research/knowledge, random state and extinction/damage history. Future cross-seed/cross-feature invariant checks and 3–4-hour heavily edited save/reload scenarios must exercise the actual integrated game.

## Remaining decisions

Named real weaker hardware and measurement access; exact climate/biological and naval roster/balance coefficients; smaller-world population defaults and generator-control ranges. B, revised30 direction, fixture/goals, smaller-map presets,1×/2×/4×, one-hour cycle, bridges/docks/transports and naval combat are now approved. Milestone A diagnostics do not prove integrated-game performance.

Actual-PC automated gate update: see MILESTONE_A_PC_RESULTS.md. Subsequent Tor approval authorizes B and the development goals. His terrain-trait, one-hour season, first-build speed and navy corrections supersede earlier proposals; see MILESTONE_B_APPROVALS.md and TERRAIN_WATER_AND_SEASONS.md.
