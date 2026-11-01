# Milestone A — browser reference inventory

Source inspection: 2026-10-29. Scope: current `dist/*.mjs`, `dist/diagnostics.js`, and named existing test files. Release-tree duplicates were excluded. No build, test, benchmark, browser session, artwork generation, deployment, or game-source change was performed for this report.

This is a static inventory, not new proof that the game compiles or that a check passes. Historical evidence in `STATE.md`/the Handoff names 72 browser gameplay checks and native reference/shader checks. Individual test files below identify existing intended checks; they were not rerun. Real-hardware integrated performance remains unmeasured.

## 1. Current truth and scale

- `dist/config.mjs:1–4`: the browser map is hardcoded at **4,800 × 3,200 world units**, with 8-unit terrain cells, 16-unit navigation cells, 128-unit raster chunks, 20 fixed simulation ticks per game second, and save format 2. It does not implement the newly approved 20 × 20 km production map or the four requested map-type presets.
- `Simulation.constructor/setup` (`dist/sim-core.mjs:185–213`) defaults to `population:240`; that number is split between both factions and then divided into workers, haulers and soldiers. It is **not** a military-only target. The approved 10,000 military plus non-military/wildlife target needs separate declared counts.
- `setup` creates two crashed ships, local depots/mines/mills, early roads, three survey beacons and at most 180 initially attempted fauna placements. There are three simple fauna visual categories (`species:j%3`).
- `Simulation.step` (`478–490`) advances one integer tick and derives time as `tick * DT`; simulation runs in a Web Worker. Rendering is a separate Canvas 2D loop (`Renderer.draw`).

## 2. Ground, colorful foliage and pixel objects

| Reference feature | Static evidence | Native fidelity requirement |
|---|---|---|
| Long Front-derived geography | `LongFrontField.hash/noise/height` in `dist/long-front-field.mjs:7–18` uses explicit float rounding and the ported mixed hash/noise method. | Preserve seeded terrain structure and its small-scale grain. Changing RNG/rounding can alter a seed even if the formula looks similar. |
| Organic ground-color regions | `LongFrontField.groundMix/species`; `paintTerrainChunk` in `dist/terrain-paint.mjs:9–31` mixes soil palettes using the same noise family. | Preserve continuous noisy material regions, grain and height-related shading; do not replace them with displaced geometric decals. |
| Organic clearings | Default generation version 2 uses `LongFrontField.opening/cover`; `Terrain.generate` (`sim-core:24–55`). Version 1 retains legacy ellipse clearings for old saves. | New worlds should retain the version-2 field method, with generator version persisted for old worlds. |
| Dense colorful jungle | `Terrain.growWood` (`56–69`) generates seeded branch/bud random walks into sparse wood/life chunks. `paintTerrainChunk` uses fern/vine/scrub/reed palettes, flowers and vigor variation. | Retain the walk/growth appearance, living greens/teals/purples/yellows, canopy density and organically varying clearings. New biological species will need to influence those parameters. |
| Procedural pixel troops/buildings/fauna | `growPixelForm` and `PixelForms` in `dist/pixel-forms.mjs`; `Renderer.draw/drawBuilding` (`renderer:21–56`). Small forms grow from walks; larger forms use frontier accretion and material variation. | Carry forward the material-cell style and legibility. These are existing procedural source rules, not permission to invoke image generation or create Python shape-stamped assets. |
| Physical artwork caching | `PixelForms.get` uses 512² small-form and 1024² large-form atlas pages, bounded slot reuse, small variants and pose keys. | Preserve batched/cached appearance; avoid individual ECS foliage render objects. |
| Crisp world rendering | `Renderer.draw` disables image smoothing for world drawing, performs screen culling, and interpolates between snapshots. | Match pixel scale, camera zoom/pan, unit readability and interpolated movement in native rendering. |

The method named `Terrain.stamp` is a **local damage/clearing operation**, not a sprite-art stamp. In current generator version 2 it delegates to material-resistance growth (`soilPatch`) instead of drawing a circle. Its name should not be confused with the forbidden crude shape-asset technique.

## 3. Terrain deformation: actual behavior and important gaps

### Excavation exists in source

- `Terrain.height` is a flat `Int16Array` at 8-unit spacing. `cutChunks` is a sparse map of 128² `Uint8Array` pages (`sim-core:13–20`).
- `cutAt` interprets each stored excavation unit as **5 cm**, giving a representable excavation depth of 0–12.75 m. `elevation` is `(base height - cut depth) * 0.01` (`95–97`).
- `soilPatch` (`98–106`) grows a bounded local area through a priority queue, with material/noise resistance and base-height differences. Default `deform` (`107–118`) uses this growth at 2-unit sampling and writes each result into four 1-unit excavation cells. Blasts use irregular fracture/depth variation. Legacy generation version 1 retains radial falloff for saved-world continuity.
- `Simulation.blast` (`273–281`) clears/scars vegetation, excavates a crater, updates nearby navigation regions, damages actual units/buildings/fauna, records faction damage and emits a separately delivered visual burst.
- `finishLinear` (`257`) excavates completed trenches using `deform(...,12,180,'dig')`, marks the actual trench cells and updates navigation. `shoot` applies small local habitat damage; it does not excavate a crater.
- `paintTerrainChunk` colors excavation according to actual cut depth and neighboring bank differences. Excavated pixels suppress the intact wood/leaf rendering (`terrain-paint:14–28`).
- Dirty raster/version sets, camera/radar region versions and local navigation region revisions exist. Changes are not represented merely by a crater sprite.

### Promised terrain features that are not yet present

1. **Building foundations do not physically level the heightfield.** `addBuilding`/construction completion call `occupy` plus vegetation/scar damage; `occupy` writes an axis-aligned obstruction footprint and navigation/sight revisions. No foundation plane, cut/fill volume or terrain-height flattening call exists (`sim-core:217–223`, `workUnit:360–368`).
2. **No general smoothing, backfilling or terrain-raising tool exists.** `deform` only increases unsigned excavation depth. `restore` resets vegetation/scar toward the initial state and cannot fill a crater or smooth elevation (`88–94`). There is no such command in `Simulation.command` (`451–477`). These behaviors remain requested work, not already verified browser parity.
3. **Crater cuts are not fully reflected in movement gradients.** `speedAt` calculates slope from the coarse base `height`; `updateNavCell` averages base height and terrain occupancy/cover (`119–133`). Both omit `cutAt`. A blast invokes local route invalidation, but fine crater slope/edge traversal is not thereby included in the cost formula. Trenches do have a coarse trench speed modifier.
4. **The height overlay is initialized from base height, not updated excavation.** `Renderer.setTerrain` builds an elevation canvas once from the worker's initial height array (`renderer:10–13`). Live raster patches update the natural view, not that cached elevation canvas.
5. **Current soil is excavation-only; displaced soil is not a conserved stock.** No deposition berm, spoil inventory or fill source is represented. Do not describe those as existing behavior. Any stronger physical cut/fill model needs explicit scope and gameplay choices.

By contrast, `Simulation.canSee` (`373`) and recon `buildMask` use `Terrain.elevation`, so physical cut depth does already affect their height queries. Native code should use one authoritative effective-elevation query consistently across ground visuals, movement, LOS, construction and saves.

## 4. Construction, resources and logistics

- `TYPES` (`config:5–24`) defines ship, depot, mine, mill, farm, solar, barracks, turret, mortar, road, trench, wall, habitat restoration, camera pole/tower, mounted camera and radar. Building costs/work/health/ranges are data, not native ECS behavior yet.
- `canPlace/place/line` (`sim-core:234–256`) validate terrain/spacing/resources, snap ordinary construction to 16-unit centers, reserve available materials at a source, and queue real worker-built orders. Linear paths bridge diagonal samples to avoid gaps.
- `assignWorker/workUnit` (`289–372`) select orders, move to source, remove/reserve materials, carry physical cargo, deliver it, then accumulate construction work. Workers also harvest/deposit. Haulers rebalance stock between actual ships/depots.
- `harvest` (`314–324`) removes finite ore or vegetation from specific terrain cells; exhausted extraction buildings disable themselves. Farms produce provisions using workers and grid power. Soldier provisions are consumed periodically (`updateBuildings`).
- Roads provide speed 2.4 in terrain movement, trenches 0.8, dense jungle reduces ordinary speed substantially (`speedAt`). A completed road over water acts as a passable bridge with a plank texture. A wall/occupied structure obstructs movement.
- `FlowRouter.get/touch/advance` (`146–181`) shares destination fields, budgets expansions, caches up to 40 fields, tracks which region revisions the used corridor depends on, retains a previous partial field during replacement, and avoids unconditional per-troop A* rebuilds.
- Soldiers, workers, haulers and camera operators have separate roles in compact typed arrays (`UNIT_FIELDS`, `config:25`). Ordinary military troops do not become construction workers in `step` dispatch.
- Current resource values include fractions and code has defensive clamps. They are not yet the new exact integer ownership ledger architecture. Porting needs explicit transfer semantics, failure recovery, quantities and event-order checks, not direct transcription of every float/clamp.

## 5. Combat, enemy behavior and outcomes

- Soldiers acquire nearby enemies through a flat spatial grid (`rebuildSpatial/nearestEnemy`), use physical LOS before shooting, and receive trench damage reduction (`soldier/shoot`). Turrets and mortars require power/local metal and use video or radar contact reports (`updateBuildings`). Mortars have actual travel deadlines and blast consequences.
- Enemy behavior is a deterministic rule queue at hardcoded locations with a five-second strategic cadence (`updateAI:432–442`). It builds infrastructure/roads and advances to beacons; it is not trained machine learning or map-generic strategy.
- Existing military victory includes destruction of the command ship (`hitBuilding`) and holding a majority of three survey beacons for 180 game seconds (`updateBeacons`). These timings and hardcoded geography do not establish the newly approved 3–4-hour large-war balance.
- `charge` records faction damage by cause. `integrity` combines vegetation/scar loss and killed fauna. At the configured collapse threshold (default 80), `checkEcology` ends in ecological stalemate: the **higher-damage faction loses**, using `lastActor` for an exact tie (`515–517`). Restoring integrity does not erase the cumulative damage ledger.
- Current fauna only wander and can die; there is no generated food web, feeding/drinking/reproduction, conservation facility, species research, viable stock, causal extinction accounting or 160-mechanism biology. Habitat `restore` is an immediate reset toward initial cover, not the new finite biological recovery process.

## 6. Intelligence and presentation

- `Reconnaissance.sourcesNow` (`recon:30–40`) grants commander video to enabled/powered ship/building cameras, mounted cameras and battery-equipped camera operators. Ordinary combat troops do not grant the command map video, although they still use local physical LOS for their own fire.
- Mounted cameras snap to completed friendly walls/structures, face outward and depend on their host remaining alive (`mountPoint`, `sourcesNow`, `hitBuilding`). Towers have greater source height/range.
- `cameraTrooper` (`sim-core:374–385`) is unarmed through role dispatch, has a 240-game-second battery, estimates return travel, recharges at home over about 20 seconds, and resumes its assignment. Camera operators do not count as soldiers for beacon capture.
- `buildMask` distinguishes camera horizon/canopy occlusion from radar's taller-solid blocking. Geometry masks are cached by source pose and region revisions, with at most two ordinary rebuilds per recon update. Recon updates at every other simulation tick (`recon:44–79`).
- Radar rotates every eight game seconds through a 30-degree sector. Full-turn coverage uses the same occluded geometry and is a faint filled overlay, while the sweep is a thin outline (`config:4`, `recon-renderer:9–33`). Coverage grants no video. Contacts outside video are anonymous approximate dots, can persist stale for 9.5 seconds, and are aimed at as reports rather than hidden live positions.
- `satelliteState` uses a circular radius 1,800 footprint crossing the map over 18 seconds every 180 game seconds, with factions offset by 90 seconds. The current footprint covers the current map cumulatively; its dimensions must be rederived for 20 km square maps.
- `prepareSatelliteTerrain` schedules modified terrain ahead of reveal, with a five-game-second lead, a bounded cache (192 tiles) and per-call budget; `getPatches` rejects stale versions, limits cold painting and alpha-redacts nonvisible pixels. `flushTerrainPatches` coalesces uploads by chunk with a 2 ms/four-patch frame budget. Visual prewarming does not itself reveal objects.
- `redact` filters live enemy units/structures, fauna positions, effects/projectiles, enemy stock and events, with last-seen building ghosts (`recon:111–120`). It still carries some global counters from the raw snapshot, and initial terrain/height/ore survey data is sent to the browser. The new species-information UI must derive observations/confidence deliberately rather than treating every internal counter as player knowledge.

## 7. Explosion-only lighting

- `ExplosionEffects` (`explosion-fx:13–36`) consumes visible burst events, culls to the camera, merges nearby bursts, limits simultaneous lights, uses a **440 real-millisecond** lifetime, and creates cached 256² light/shadow textures only for those effects.
- `collectCasters/shadowQuad` use nearby structures and spatially deduplicated troops, bounded at 96 PC or 56 narrow-screen casters. Roads, trenches, restoration markers, mounted cameras and stale building ghosts do not cast these shadows. These are bounded approximate local object shadows, not exact terrain/canopy shadows or a continuously running lighting simulation.
- Effect clipping follows current video visibility. Explosion light does not grant commander vision. A setting can disable the cosmetic effect without removing actual blast damage.

## 8. Saves, update and diagnostics

- `Simulation.save/restoreSave` (`501–514`) persists generator version/options, tick/RNG, units/cargo/orders, buildings/stocks/reservations, fauna, terrain edits, base64 excavation pages, damage history, beacons, projectiles and recon memories. It regenerates baseline terrain and reapplies edits; format 1 and 2 load paths exist.
- `app.mjs:13–15`, `78–103`, `124` store browser campaigns in IndexedDB, export/import JSON and manage save failures. The browser import guard is 35 MiB and 40,000 unit slots; these are current guards, not approved native large-world save budgets.
- `createUpdater/fetchLatestBuild` (`updater.mjs`) and `prepareUpdate` (`app:80–90`) check the current build, save/flush campaign and console history, and reload the new content-hashed release. `scripts/prepare-release.mjs` owns module-graph release retention. Native update parity remains separate work.
- `dist/diagnostics.js` stores append-only browser sessions in IndexedDB, batches log writes, records UI/gameplay events, errors, rejected promises, resource failures, long tasks and performance samples, retains build diagnostics and exports one `console.txt`. This is browser persistence/export, not a browser app directly appending to a Windows filesystem text file.
- The worker records requested versus achieved speed, average step and maximum step; render diagnostics report FPS/render time. These do not yet provide all required per-system median/p95/p99/max real-PC metrics.
- The worker's bank is bounded to 80 ticks and has a 12 ms/60-step callback budget (`worker:21–29`). This controls responsiveness, but is not proof of target-speed capacity, a deterministic background-job commit policy or integrated native behavior.

## 9. Existing check sources (not run by this inventory)

- `tests/simulation.test.mjs`: construction reservations/cancellation, local excavation, route cache invalidation, restoration/history, save continuity, actual worker trench completion and economy delivery.
- `tests/recon.test.mjs`: ordinary soldier visibility exclusion, physical occlusion, camera mounting, battery persistence, stale ghosts, snapshot redaction and radar-report targeting.
- `tests/satellite-streaming.test.mjs`: circular coverage including map corners, reveal lifetime, prewarming without vision, stale preparation rejection and bounded painting/uploads.
- `tests/radar-coverage.test.mjs`: occluded coverage overlay without granting early contacts/video.
- `tests/explosion.test.mjs`: shadow filtering, hidden explosions, burst delivery and mortar ammunition/damage.
- `tests/updater.test.mjs`: save-before-update, update failure retention and content-hashed release retention.

## 10. Transfer priorities

1. Freeze the current procedural formulas/palettes/material growth as reference data and preserve generator-version migration semantics.
2. Build effective terrain height/material queries that cover excavation, requested foundation cut/fill and requested smoothing consistently; do not call those latter features existing parity.
3. Preserve real workers/cargo/roads and exact ownership while scaling counts independently from military.
4. Keep simulation visibility and player knowledge separate from render preparation, including satellite prewarming and future ecology inspectors.
5. Replace hardcoded world coordinates/array dimensions with approved world configuration and chunked storage, without allocating a full 20 km fine raster.
6. Retain the current ecological-damage verdict while replacing the simple fauna/habitat approximation with the required finite ecosystem; do not import immediate restoration as hidden organism spawning.

Only this report file was written by the inventory assignment. No shared state/rule files were edited.
