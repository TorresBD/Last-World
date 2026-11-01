# Last World

A playable 2D RTS browser reference, with a native Bevy/wgpu migration underway. The working title is Last World. Physical objects and jungle grow from seeded pixel materials; no imported artwork or geometric object stand-ins. Explosions produce brief local flashes and projected object shadows.

## Native GPU build

Campaign → **Bevy GPU test** opens the same Rust engine compiled to WebAssembly/WebGPU, with touch controls and a Windows portable download. The Windows build runs natively with Bevy and wgpu. Both are the first **benchmark gate**, not the complete RTS port: GPU-owned troop movement, spatial indexing, cached sensor masks, and instanced pixel rendering. Construction, combat, economy, ecological attribution and the full campaign still run in the browser reference until ported and verified. See [native/README.md](native/README.md) for controls, reproducible builds, and the exact boundary.

Hardware performance must be measured on target machines. The build host's software Vulkan adapter is used only to validate kernels, readbacks and captures.

## Play

Open the published Site. Begin with 240 total pawns, or select 1,000 / 5,000 / 10,000 for a population stress test. Those are actual living simulated pawns across both factions, not rendered stand-ins.

PC: WASD/arrows pan, scroll zooms, left-drag selects infantry, right-click moves selected infantry (all infantry if nothing is selected). B toggles construction; R draws roads; T draws trenches (also under Defense); Space pauses; Home returns to base. Phone: drag to pan, pinch to zoom, tap structures, and use the Orders panel for movement. Draw linear construction with one finger; two fingers pan/zoom. Workers and haulers are autonomous.

Draw roads between the ship, finite ore mines, timber camps, and depots. Workers fetch reserved materials, carry them to a site, and work before it completes. Haulers rebalance stock at depots. Hydroponics generate provisions through workers and consume power. Solar adds power; barracks consume local metal/provisions to recruit. Defensive weapons require ammunition from a depot or ship within 480 world units. Infantry consume provisions. The opponent builds its own infrastructure, cameras and radar, expands roads and depots, and captures survey territory using fixed rules, not a trained model.

Hold at least two of three survey beacons for three game minutes, or destroy the enemy command ship. At 80% world integrity the war ends in an ecological stalemate: **the faction that caused more ecological damage loses**. Both sides' damage records include habitat destruction, soil scarring, and native deaths, attributed to the actor. Damage is historical; restoration repairs shared world integrity without removing responsibility. An exact impact tie is broken against the faction whose damage caused closure. These are initial tuning choices, not user-mandated balance values.

## Cameras, radar and satellites

Your command view is fed by crash-ship cameras, camera poles, elevated towers, mounted cameras, and unarmed camera troopers. Ordinary infantry fight locally without revealing the commander map. Friendly radio markers remain available for orders. Use the INTEL tab to build sensors; mounted cameras snap to a completed friendly wall or building and face outward.

Camera troopers cost 140 metal and 25 provisions, carry a four-game-minute battery, return to the crash ship on reserve charge, recharge for 20 seconds, and resume their assigned destination. Their inspector also has an early recall button. They cannot fight or capture beacons.

Each faction gets an 18-second satellite sweep every three game minutes, with a 90-second offset. The moving circular footprint is 3.6 km across and covers the entire map over a pass, including the corners. Altered terrain is prepared five seconds ahead (scaled with requested simulation speed), with bounded preparation and upload work; untouched terrain reuses the existing map raster. Player passes travel west to east; enemy passes travel east to west. The Command Feed panel shows the next pass and live progress.

Radar is a custom rotating sweep with an eight-second turn and a 30-degree active sector. Solid obstacles above the antenna block it; foliage does not. A faint filled footprint shows the reachable area across a full turn, leaving terrain/structure shadows unshaded. The thin wedge marks the current sweep; the footprint itself grants no vision or early contacts. Outside camera/satellite coverage, returns are approximate anonymous dots, retained for up to 9.5 seconds between scans. They expose no unit type or health. Remote weapons use live video or last radar reports; blind radar shots are less accurate. Select a mortar and choose Bombard point for manual fire, including against a radar return.

## Explosion effects

A camera-observed explosion creates a cached 256 × 256 flash/shadow stencil lasting 440 real milliseconds. Nearby structures and sampled groups of pawns cast projected shadows. Geometry is captured at detonation and fades with the burst. No persistent light simulation runs. Offscreen flashes skip stencil generation, overlapping bursts coalesce, and simultaneous stencils are limited to four on desktop or two in the compact layout. Performance includes an effects toggle, drawing cost, and preparation peak. The setting is saved per browser.

Saves are browser-local in IndexedDB, with autosave every 20 real seconds, manual save, and JSON import/export. Separate browsers/devices do not share a save automatically. The simulation pauses while the tab is hidden. The native benchmark is a portable executable, without an installer or APK; it does not import campaign saves yet.

## Source and verification

Serve locally with `python3 -m http.server 8000 --directory dist`, then open http://localhost:8000. Modules and workers require HTTP(S), rather than opening index.html as a file. A Node runtime is only required for the headless checks:

```
node tests/simulation.test.mjs
node tests/recon.test.mjs
node tests/explosion.test.mjs
node tests/recon-benchmark.mjs
node tests/benchmark.mjs 240 1000 5000 10000
```

The Site's external Git repository stores the full source, tests, and architecture notes. `dist/` is the complete static runtime. See `ARCHITECTURE.md` for data layout, budgets, intentional approximations, baseline terrain references, and verification limits.

Terrain fidelity: new expeditions use The Long Front’s original noise/hash, fine vegetation blend, branching growth walks and unlit ground-color calculation, adapted to denser living jungle and the larger map. Natural clearings, soil colors/types and ore veins are continuous seeded fields. Four living plant types—fern thickets, blueleaf vines, flowering scrub and golden reedgrass—grow through branching walks, with distinct palettes. Earthworks and blast scars propagate through soil resistance. No tree stamps or circular clearing templates are used. Fine appearance stays cached at 2 m; gameplay terrain remains 8 m, with sparse 1 m excavation. Old saves retain their original terrain generator.

## Console history

Campaign → Download console.txt exports all retained sessions on the current device. Logs include JavaScript/module/resource/worker errors, warnings, commands and UI actions, gameplay events, 10-second performance samples, and build/syntax/test output. Errors are captured before game modules start; a recovery download button appears if startup fails. Identical error storms are counted. Runtime history is appended to IndexedDB, with a small shutdown tail; it is not automatically pruned. Build checks append to the repository console.txt and are bundled into the runtime export. Run `npm run check` for syntax checks or `npm test` for checks plus regression tests. Browser history is per device/browser; clearing site data removes it. Exported downloads contain the whole history each time.

## Click-to-update

Campaign → **Update game** checks for a new build only when clicked. If one is available, the game commits a fresh campaign save and console history before reloading, then resumes the campaign with its camera, speed and pause setting. Offline checks and failed saves leave the current game open. No browser data is cleared.

`npm run check` (also run by `npm test`) generates `dist/version.json` and a content-addressed copy of the entire script/style graph. HTML and worker imports therefore select one consistent release. The immediately preceding release is kept for older cached pages. Run this check before every publication; capture fixtures continue to use the editable root modules.
