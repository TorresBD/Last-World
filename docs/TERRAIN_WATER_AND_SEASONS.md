# Milestone B — terrain, water, underground habitat and seasonal foundations

Date: 2026-10-29, Tor's client timezone. Status: **planned architecture**. This document is not a report of implemented hydrology, burrows, seasons or naval combat. Read current root records before using older handoff proposals.

## Current decisions and first dependency

Tor has authorized Milestone B and approved the remaining numbered proposals in MILESTONE_B_APPROVALS.md. His explicit changes take precedence: prioritize map-changing traits in the initial30; make1×/2×/4× available for the first build; use a full seasonal cycle of one hour at1×; include a combat navy as well as bridges, docks and transport vessels. Full160-trait scope and mandatory biology remain unchanged.

Build the common physical substrate before implementing species-specific shortcuts. A wildlife dam, military obstruction, crater, trench, foundation, smoothing operation and shallow tunnel breach must meet the same terrain/water/path interfaces. Species call these mechanisms through finite jobs; they do not each own a second water or terrain simulation.

The selected30 and effects/costs are in MILESTONE_A_TRAIT_PROPOSAL.md. Detailed resolutions, physical coefficients, discharge laws, weather amplitudes, vessel roster and exact score weights are implementation/tuning work, not measured promises or already approved numerical balance.

## Ownership and representations

| Quantity | Authoritative owner | Derived consumers |
|---|---|---|
| Effective surface elevation and soil material | CPU generated base plus persistent sparse edit pages | Land slopes, foundations, cover, water boundaries, tunnel cover and GPU appearance |
| Surface/channel/reservoir water | CPU nonoverlapping storage compartments with finite volume | Surface level, flooded footprint, current/depth queries, habitats, boats and rendering |
| Groundwater / soil water | Explicit CPU stocks and exchange links | Roots, infiltration, seepage and drying; water cannot be withdrawn twice |
| Tunnel water | CPU tunnel compartments connected through real openings or defined seepage | Wet passage capacity, escape, exposure and drowning |
| Excavated soil/wood/stone, litter and pollutants | Material/resource ledgers and location owners | Spoil, dam construction, nutrient return, waterborne pollution and loss accounting |
| Underground geometry | Sparse CPU chambers, passages and portals with stable IDs | Underground routes, occupants, collapse, surface entrances and recon filtering |
| Animals | Exactly one individual/cohort/captive/transit owner and location | Ecology, research, rendering and damage accounting |
| Time | One integer game-tick authority | Combat, jobs, seasons, ecology, hydrology, saves and visual interpolation |

Neither a river graph nor a rendered flood page owns a duplicate of the same water. Refined surface compartments replace/partition a coarse owner's stock through a conservative handoff. Coarsening performs the reverse handoff. Underground compartments receive water subtracted from their upstream owner. Cohort-to-individual transitions follow the same ownership rule for animals.

Keep dense resident fields as flat arrays, with sparse pages, indexed records and flat adjacency ranges where appropriate. A sparse graph for actual rivers or tunnels is useful; it is not a quadtree/octree for dense tile-neighbor lookups. No tile, leaf, tunnel voxel or offscreen population becomes an ECS entity by default.

The current benchmark uses artistic height/wetness values. They are not yet calibrated physical meters or conserved water volumes. Establish explicit physical elevation, depth, distance, volume and time units before hydrology; preserve the existing procedural pixel color/growth transfer functions for appearance. Do not interpret the benchmark's arbitrary `water=100` as100 meters of depth.

## Surface deformation and drainage

Retain CPU effective terrain and persistent edits. Surface queries include base height plus signed cut/fill; use the identical query for slopes, water connections and tunnel roof thickness. Foundations and smoothing are actual jobs with soil/material handling, not a cosmetic reset. Smoothing must read a stable source neighborhood before applying its outputs, so iteration order cannot change the result. Cut and fill expose/remove/bury real roots or structures according to their depth.

Each edit transaction identifies affected terrain bounds, material changes, structural consequences and causal source. Update per-region terrain revisions and reverse dependencies for water connections, entrances, roads, navigation and sight. A small edit invalidates its affected routes/regions rather than every cached path in the world. Do not merge two distant edits into a huge world-spanning upload rectangle: use per-page bounds and coalesce nearby ranges only.

GPU uploads cover dirty texture rectangles/buffer ranges. The GPU derives the image from the authoritative terrain/material inputs; no CPU terrain remesh or global fine texture rebuild is required. Keep upload queues bounded and report bytes/backlog. Tactical consumers cannot continue indefinitely with outdated effective heights while rendering shows a different crater.

World generation must create drainage connected to effective geography: catchments, river beds, tributaries, basin spill points and outflow boundaries. Preserve the noise-grown visual language, but do not retain a water stripe that crosses slopes independently of topography. The20km world uses resident coarse structure and streamed/refined pages, not a resized fine raster of the entire map.

## Dams, reservoirs and water redistribution

Use a global catchment/reach/basin network with terrain-derived stage/storage relationships. Refine changing flood fronts and strategically important crossings locally. A narrow river can be represented by a channel cross-section below the coarse grid's width; a coarse64m cell must not erase a10m river. Grid size and refinement policy remain engineering choices to qualify.

A dam changes a water connection's crest, opening/permeability, discharge capacity and durability. Building it requires a suitable connected site, animal work, real material collection and maintenance. A wood dam does not create an infinite impermeable wall across a whole map region.

At each scheduled water interval:

1. Apply explicit rainfall/catchment inflow and boundary inflows; record evaporation, withdrawals and boundary outflows as named sources/sinks. Infiltration is a transfer when groundwater is modeled, not disappearing water.
2. Determine requested flows from the same pre-transfer state. Allocate scarce donor volume across outgoing links in stable order or a deterministic proportional scheme; the donor cannot promise the same water to two destinations.
3. Commit buffered transfers once and derive water stages from remaining storage. Store fractional transfer remainders if quantized units are used. Use checked wide accumulation; do not clamp a negative result to hide an accounting error.
4. Derive inundation from hydraulic connectivity and effective terrain. Water may spread onto connected low banks as upstream storage rises. Disconnected hollows behind higher ridges do not flood solely because they share an elevation.
5. Update moisture, habitats, pollution transport, land/boat route revisions and visual page bounds where meaningful thresholds change. Cosmetic ripples do not invalidate paths.

The reservoir therefore widens over time as it inundates the river banks. Downstream flow falls while storage fills; leakage, overflow, tributaries and groundwater contributions may keep it above zero. Once a reservoir reaches a steady state, downstream discharge may approach inflow again. Destroying the dam releases the finite stored water through bounded connections, with a flood pulse and downstream consequences, not an instantaneous deletion or teleport of the lake.

Initially, bank widening means **inundation**. Permanent erosion and sediment deposition are separate soil transfers with explicit costs and conservation; water does not secretly lower terrain to make a wider-looking lake. Planned soil stability, vegetation cover and disturbance hooks must permit that process without adding a second ground owner.

Reservoirs and flood networks may span many chunks. Chunk borders cannot become invisible levees. Stable channels use cheap storage/discharge updates; active local fronts use refined work. Unseen basins still receive rainfall, flow and ecological consequences. The method targets readable RTS consequences, not full-world fluid dynamics or per-droplet simulation.

Topology rebuilds carry terrain/topology/storage revisions and deterministic due ticks. Apply connectivity changes and their conservative storage remapping atomically. Pure geometry jobs need not snapshot changing volumes unnecessarily; transfer current owned volume at commit. Obsolete results are rejected. If a mandatory due result is late, finish required work or visibly slow the authoritative clock; do not let thread completion order choose a different world or freeze just the unseen region.

## Burrows with sideways, curved and branching passages

A surface heightmap cannot represent an intact roof over a horizontal tunnel. Keep ordinary ground as a surface; represent real underground voids in a separate sparse network:

- A **portal** joins the surface to a passage/chamber and has a real entrance elevation and clearance.
- A **passage** has a3D centerline polyline, width/height clearance, measured travel length, soil/roof properties, current damage/blockage and endpoint adjacency.
- A **chamber** has finite usable volume/capacity, connected passage IDs, shelter conditions, stored materials/life stages and a water-storage compartment if flooding occurs.
- Vertically overlapping tunnels remain distinct unless an excavated connection joins them. A nearby endpoint never implies passage through intact soil.

Curved, horizontal, sloping and multi-entrance burrows use the same graph. Individuals move by distance along a passage; turns and slopes count toward length. Body size, turning/clearance limits and congestion determine passage eligibility. Bounded local avoidance can be simpler in a narrow tunnel than on open ground. Small holes do not require a whole-world voxel engine, general3D rigid-body physics or an entity per soil cell.

An animal's location distinguishes surface, passage, chamber, captivity or transport. Entering a burrow changes ownership/location once, so it cannot remain in both a surface cohort and an underground cohort. Common unseen occupants can be cohorts attached to an actual chamber with capacity/age/resource records; identity-sensitive individuals retain their finite state. Rendering visibility never determines whether their biology continues.

Digging removes real substrate and deposits/moves spoil. Ordinary digging leaves the roof surface intact; only entrance edits, spoil, subsidence or collapse alter surface height. Groundwater and loose/wet soil affect eligible sites and work. A shallow crater can break through a roof; foundations can load weak shallow passages; collapse can obstruct exits, bury organisms or create an exposed surface depression. Use a bounded cover/strength model first, rather than finite-element geotechnics.

Floodwater enters through a connected low entrance, crater breach or explicit seepage link. It transfers from surface/groundwater storage into tunnel storage. Finite chamber levels and spills determine which passages flood, leaving real escape opportunities and species-dependent exposure. A distant low chamber does not fill through solid rock. A breached or collapsed edge remains persistent after the animal leaves or dies.

Surface cameras reveal exposed entrances and emerging animals when in sight. Intact ground hides internal occupants from ordinary visual/radar payloads. Player inspectors, map markers and render preparation must not leak the underground graph or hidden animals.

Illustrative schema direction, not compiled code or the final save format:

```rust
enum HabitatLocation {
    Surface { page: PageId, xy_mm: [i32; 2] },
    Passage { id: PassageId, distance_mm: u32 },
    Chamber(ChamberId),
    Captive(FacilityId),
    Transport(VesselId),
}
struct Passage {
    endpoints: [NodeId; 2],
    centerline: Range<u32>, // range into one flat xyz-point vector
    length_mm: u32,
    clearance_mm: [u16; 2],
    revision: u64,
}
struct WaterTransfer {
    from: WaterStoreId,
    to: WaterStoreId,
    volume_liters: u64,
    cause: CauseId,
}
// Resolve donor budgets from a stable snapshot, then commit once.
// All IDs, memberships and pending transfers persist through save/load.
```

## Other early map-changing traits

The initial selection exposes different kinds of persistent change, not merely different visual labels:

| Catalogue mechanism | Physical/ecological dependency |
|---|---|
| 102 Soil mixing | Moves existing soil/organic material, changing local fertility/infiltration without creating nutrients. |
| 103 Tree thinning | Removes real woody biomass, opens canopy/routes and changes cover, roots, habitat and material availability. |
| 104 Cavity excavation | Persistent void in an actual tree/structure, with finite substrate, capacity and later reuse; separate from soil tunnels. |
| 105 Reef building | Slow finite biological structure growth/death, aquatic shelter and local bed/clearance effects relevant to vessels. |
| 46 Trap construction | Real webs or excavated pits where anatomy permits; pits reuse surface excavation and finite spoil. |
| 67 Wallowing | Repeated disturbance/compaction can create mud and small depressions using real soil/water; not an infinite water source. |
| 65 Nest construction | Persistent material-built shelter attached to a valid site; its destruction/removal leaves accounted material/loss. |
| 121 Deep roots /125 Flood tolerance | Root access to finite subsurface water and survival differences after reservoir flooding; tolerance is not immunity. |
| 128 Pioneer establishment /129 Root resprouting /132 Persistent seed bank | Disturbed ground recovers only through viable survivors/stages, resources and time. |
| 143 Buoyant seeds | Real propagules follow connected changed waterways, potentially behind a dam, with viability and loss. |
| 156 Dense ground cover /157 Broad shading canopy /159 Nutrient-rich litter | Changes soil protection, water/heat conditions, light and nutrient return through real biomass. |

Unselected catalogue mechanisms stay in final scope. For example110 Communal latrines,131 Fire-triggered germination,133 Clonal runners,145 Nitrogen-fixing partnership,146 Mycorrhizal association,155 Climbing growth and158 Allelopathy require their own effects/costs later. Their shared soil/resource/partner/disturbance interfaces should fit the substrate; they are not implemented by mentioning them. Ordinary seasonal output, basic soil stabilization and decomposition remain mandatory biology outside the30.

## One-hour seasonal cycle and its consequences

One cycle is72,000 authoritative20Hz ticks:60 minutes at achieved1×,30 at2×,15 at4×. Pausing pauses seasons. Slower achieved speed slows calendar progress; a separate wall-clock timer never jumps weather ahead of ecology. This interprets Tor's hour as one hour at1×, consistent with the proposal he revised. A3–4-hour large war at1× spans approximately3–4 cycles.

Use saved climate/biome curves rather than imposing four identical15-minute seasons everywhere. Tropical jungle may have wet/dry transitions; high/cold regions can have relevant cold periods. Climate inputs, weather events and starting phase are deterministic world state. Species development/aging rates are explicit profile parameters; a one-hour calendar does not make every long-lived animal mature or every damaged tree regrow in an hour.

| System | Seasonal consequences to model |
|---|---|
| Water and ground | Rainfall/runoff, evaporation, infiltration, groundwater availability, river stage/current, reservoir fill/spill, flood extent, saturation/mud, drainage/drying, and soil-stability/erosion susceptibility. |
| Cold regions where applicable | Snow/ice/freeze-thaw state, finite meltwater, traversability/load limits and boat obstruction. Do not apply tropical freezing to every map. Detailed coefficients and the first-build coverage are not yet implemented. |
| Plants | Water/thermal stress, growth/dormancy, flowering overlap, finite fruit/nectar/seed production, pollination success, flood/root survival, germination and resource-limited recovery. |
| Animals | Food/water access, thermal/metabolic costs, activity windows, migration/connectivity, seasonal dormancy where the selected trait applies, mating, incubation, juvenile care/survival and shelter demand. |
| Soil and decomposers | Moisture/temperature-dependent breakdown and nutrient release from finite litter/carcasses; no repeated return of the same nutrients. |
| Disturbance | Dry biomass/fuel moisture and fire susceptibility; wet soil/roots affect collapse and erosion. Fire consumes real fuel and causes attributed damage through the shared disturbance system. |
| Construction/logistics | Flooded roads, mud and bearing/ground conditions affect travel/building; bridges and foundations query changing banks/soil/water; stock production and demand follow real ecological supply. |
| Naval activity | Seasonal depth/current, usable docks, draft/grounding and applicable ice change routes, supply and combat approaches. |

Calendar inputs interpolate smoothly. Reproduction, hatching, dormancy, migration and other discontinuities have explicit due ticks. Coarse offscreen catch-up splits at meaningful events/thresholds; one giant linear formula across drought, flooding and breeding boundaries is not valid. Offscreen outcomes may be coarser, but not frozen or replaced with rescue spawning.

Ground temperature, canopy shade and animal sensing are ecological scalars. They do not require restoring the removed continuous lighting/shadow engine. Explosion-only cosmetic lighting retains its separate bounded visual budget.

## Naval combat and shared routes

Naval combat is approved scope. Bridges, docks and transport vessels are approved too; implementing transports alone does not satisfy a combat navy. Exact vessel roster, weapons, prices and balance will be designed in a later playable increment within that scope.

Water routing queries connected channels/bodies with current depth, vessel draft/beam/clearance, bridges and relevant region revisions. A bridge supplies land connectivity and has finite underpass clearance for boats. Docks require both an accessible land approach and suitable water depth; falling reservoir/river levels can strand a dock or vessel. Land, amphibious where applicable, swimming, diving, burrowing and surface-vessel movement are capability classes rather than independent unversioned route systems.

Transports own loaded troops and cargo exactly once until unloading, rescue, destruction or transfer. Crew, hulls and carried military/civilian agents must be counted separately in diagnostics; no double counting and no silent absorption of wildlife/civilians into the approved military target. Exact naval composition of the10k-military qualification fixture remains a fixture-design detail to record before a naval performance claim.

Combat uses the shared authoritative clock, damage/target and recon rules, with water-aware reachability and shore effects. Ship wakes are cosmetic; gameplay does not require3D ocean physics. Rendering must preserve the existing procedural pixel style and visual-approval rules.

## Performance, persistence and ecological responsibility

- At2×/4×, schedule more of the same fixed ticks. Do not enlarge timestep, omit rules, change counts or advance the season alone. Bound per-frame work; retain/report owed work, requested speed and achieved speed. If simulation cannot keep up, slow visibly rather than silently jumping ticks.
- Separate terrain, water, structural and navigation revisions. Batch event-driven invalidations at meaningful thresholds and amortize shared routes, while closing an actually destroyed bridge promptly. Avoid one global revision invalidating every route for a cosmetic change.
- Refine only where needed; common unseen populations and stable water remain cheap, real numerical state. Capture/research and essential identity can require exact records even offscreen. No hidden minimum counts or regenerated species.
- Save water volumes/stages as appropriate, channel/basin links, dam materials/health, edited terrain, portals/passages/chambers, occupants, viable stages, soil/material stocks, pending transfers/jobs, deadlines, season/weather/random state, revisions and causal history. Load must restore one ownership ledger, not regenerate edited hydrology from the old seed. Dense long-war edits need bounded page compaction.
- Natural dam building is not automatically faction ecological damage. Faction dam destruction, toxic runoff or habitat clearance can cause delayed downstream deaths. Preserve causal event/source records across water/material transfers; direct versus indirect scoring and natural-baseline rules need explicit tuning, no double-charge. Actual waterborne pollutants move/settle through finite stocks where modeled.
- Log resident/dirty/refined pages, basin/tunnel counts, transfer balances, topology/nav backlog, catch-up lag, upload bytes, memory high-water, frame tails and requested/achieved speed in the existing append-only records. Unknown world state must stay filtered from player-facing diagnostics.

## Implementation sequence and future acceptance evidence

1. Integer clock and explicit1×/2×/4× controls; retained CPU terrain and common signed edit transactions/dirty ranges; world settings/units and save-schema boundaries.
2. Terrain-connected drainage/storage, local flooding and a dam build/fill/overflow/breach interaction. Establish cross-chunk conservation and route updates before decorating river banks.
3. Sparse nonvertical burrows, entrances and ownership; digging/spoil, roof breach/collapse and water transfers through actual openings. Integrate map-engineering traits through these mechanisms.
4. Seasonal climate inputs and mandatory flora/fauna lifecycles; regional coarse progression and chosen30; real war jobs, logistics and ecology consequence attribution in playable increments.
5. Bridges/docks/transports and naval combat using the established water/land interfaces; full recon-safe UI, research/conservation, saves and broader160-trait rollout.

Dependency order does not postpone approved release scope or promise that all these systems are in the next executable. Future checks must cover water/material balance; upstream fill/downstream change/overflow/breach; connected versus disconnected low ground; multi-chunk reservoirs; curved/branching/multi-entrance and overlapping burrows; crater roof exposure and evacuation; no duplicated occupants; vessel/bridge clearance and changing dock depth; seasonal event-boundary catch-up;1×/2×/4× state equivalence at equal ticks; and save/load continuity. No such integrated checks or full-game performance results are claimed by this plan.


## Implementation checkpoint — 2026-10-29

A bounded first foundation now exists: retained coarse CPU terrain/edit APIs, local dirty uploads and CPU-owned river stores/links with gradual barrier fill/breach, plus an opt-in native river fixture.32code checks and a4800tick CPU interaction sequence passed; Linux/Windows releases compile. See RIVER_FLOW_VERIFICATION.md for exact numbers and boundaries. The overall architecture above remains the direction: no20km hydrology, wildlife dam construction, burrow system, seasonal environment, navy or full-game qualification is claimed. The fixture excludes coupled troops until per-tick interactions are implemented.


## Required soil, vegetation and exposed-ground behavior — 2026-10-29

Tor clarified that fluid movement must affect the ground itself. These requirements are **planned**; the River Foundation download has local conservative water transfer, not this coupled soil/ecology system.

- Erosion/scouring removes finite bed/bank substrate according to flow/depth, soil cohesion and vegetation/root reinforcement. Suspended sediment travels through buffered water links and deposits where transport capacity falls. Terrain elevation and substrate layer ownership change together; displaced material cannot remain in the source while also appearing downstream. Water storage geometry then refreshes from the effective terrain before later transfers. Channel deepening, bank retreat and deposits affect subsequent water depth, connectivity and path eligibility.
- Flow can detach and transport foliage/organic debris. Apply the species/material resistance and rootedness model: loose litter and ground cover differ from established trunks, while root exposure/undermining can weaken trees. Detach once from real plant biomass; retain transported/deposited debris and causal damage records. Neither a cosmetic tint nor instantaneous deletion of every submerged plant satisfies washing away vegetation.
- Riverbed substrate is brown dirt; greater water depth produces darker brown bed appearance. Persistent wet/disturbed material state belongs to the CPU world, with derived GPU coloring and partial uploads. Depth-based palette changes are not the removed continuous-lighting system, and an opaque blue-green overlay must not hide the requested dirt bed.
- A drained cell stays wet/disturbed. Store saturation/soil-water stock and a last-submerged/exposed-since tick. Require sufficient continuous exposure to air before drying can proceed; resubmergence resets/suspends that eligibility under the defined rule. Subsequent moisture loss is finite and climate/soil/canopy-dependent. Exact exposure delay and rates are tuning parameters, not fixed here.
- Ground recovery and plant recovery are distinct. Drying does not rebuild eroded soil or restore washed-away foliage. New growth requires viable remaining roots/seeds/organisms or real migration, suitable substrate/resources and time. Persistence, offscreen evolution and saves retain the same state. Natural deposition can rebuild ground only from real transported sediment.

Implement these through the common material/water/terrain ownership and fixed tick boundary. Limit work to affected resident/refined regions; dormant/coarse regions continue equivalent bounded stock evolution. Future acceptance cases must cover actual bed/bank elevation changes, finite sediment/debris conservation, different foliage resistance, brown/depth-darkened bed visibility, drainage with no immediate drying/regrowth, sufficient-air-exposure delay, rewetting resets, and equal-tick/save continuity. No such checks ran for this clarification.
