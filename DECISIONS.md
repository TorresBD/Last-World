# DECISIONS (append-only: never delete or reword old entries)

Status key: APPROVED = Tor decided it. PROPOSED = suggested, not yet approved. Do not treat PROPOSED as approved.

## 2026-10-29

- APPROVED: Last World is its own game, native Rust/Bevy/wgpu, real-time top-down 2D. PC is the primary platform.
- APPROVED: All 160 trait mechanisms stay in scope. Work may be re-ordered but not dropped.
- APPROVED: Confirmed development hardware (the top tier, not the baseline):
  - CPU: AMD Ryzen 9 3900X, 12 cores / 24 threads
  - RAM: 32 GB
  - GPU: NVIDIA GeForce RTX 2080 SUPER, 8 GB VRAM, driver 32.0.16.2101, DirectX 12 (feature level 12.1)
  - OS: Windows 10 Pro, build 19045
- APPROVED: The game must also run on weaker computers than the development PC.
- APPROVED: Development resumes with Milestone A only. Milestone B waits for Tor's go-ahead.

## Open items still needing Tor's decision (all PROPOSED defaults, not approved)

- PROPOSED: Hardware tiers. Low: 4 cores, 8 GB RAM, integrated or GTX 960-class GPU with 2 GB VRAM (about 2,000 pawns, about 10x10 km map). Recommended: 6 cores, 16 GB, GTX 1060-class. High: the development PC (10,000 pawns).
- PROPOSED: First validation map of 20 x 20 km.
- PROPOSED: Speed cap of 4x with the full 10,000-pawn army; slower hardware lowers achieved speed visibly.
- PROPOSED: Phone target is "functional with reduced visuals" for now.
- PROPOSED: Season length and biological time compression: not yet chosen.
- PROPOSED: Vertical slice of about 12 to 15 traits across the families before expanding.

## 2026-10-29 — Governing documents and critique review

- APPROVED: Tor instructed that the supplied RULES.md, STATE.md and DECISIONS.md govern Last World. RULES.md was copied without edits; DECISIONS.md preserves every supplied entry above.
- This request is a planning review, not a request to begin Milestone A or B. Development, builds and tests remain paused for this session. The existing Milestone A-only authorization boundary still applies when development resumes.
- PROPOSED: Treat CPU affinity and explicit RAM/VRAM cache budgets as stress scenarios, not as certification of a weaker PC. Software Vulkan is a correctness/compatibility path, not a guaranteed performance floor.
- PROPOSED: Separate graphics-quality settings from explicitly chosen world configurations. A 2,000-pawn configuration would require Tor's approval; no active-world pawn reduction or biological-rule removal is authorized.
- PROPOSED: Preserve fixed tick duration and authoritative simulation rules under overload; show requested and achieved simulation speed. The 4x cap remains unapproved.
- PROPOSED: Choose map dimensions using travel times and logistics as well as memory costs. The 20 x 20 km suggestion is about 26 times the area of the existing 4.8 x 3.2 km reference and is not an approved replacement.
- PROPOSED: Keep the existing renderer during Milestone A inventory. A shared-device integration must resolve the observed wgpu 27/30 API mismatch before implementation. No dependency change or second renderer is approved by this review.
- PROPOSED: Include player-facing ecology explanations, recon-safe inspectors, controls/tutorials, audio and enemy difficulty in the playable slice. All 160 trait mechanisms remain in scope; the 12–15-trait starting set remains unapproved.
- PROPOSED: During future development, add cross-seed and cross-feature invariant checks and dense-edited-world save scenarios. Agree numerical memory, save-size and load-time budgets before treating any as acceptance criteria.

## 2026-10-29 — Tor supplied revised RULES.md

- APPROVED: Adopt Tor's updated RULES.md verbatim. Limited-core/budget and software-renderer runs must be labeled proxy checks, never low-end performance evidence. Low-end performance remains UNMEASURED until recorded on real low-end hardware named by Tor.
- No new map size, speed cap, hardware tier, season timing, phone target or vertical-slice size was approved in this message. Tor offered to clarify before development starts; this session remains planning only.

## 2026-10-29 — Tor's world, workload, platform and trait clarifications

These approved decisions supersede conflicting earlier proposals; prior entries remain as history.

- APPROVED: Target large-map dimensions are 20 x 20 km. Provide map-generation types Continental, Archipelago, Large Land Mass and Medium Islands, with generation settings including rivers and mountains. Smaller maps and settings for shorter games are also required; their exact dimensions are not chosen yet.
- APPROVED: The target includes 10,000 military pawns PLUS non-military pawns and wildlife. Ten thousand is not a cap on all agents or organisms. Representative performance scenarios must include those other populations, economy/logistics and ecology; their numerical budgets are not yet chosen.
- APPROVED: A typical war on a large map should last approximately 3–4 hours of play. Support shorter-game configurations on smaller maps. This is a gameplay-duration target, not a measured result or a hard-coded simulation cutoff.
- APPROVED: The product is a native PC game. Browser testing is permitted only for very small things; most qualification must use actual PC execution so environment differences do not omit relevant costs. Phone play does not need planning for now, superseding the earlier proposed phone target without requiring removal of existing browser/reference artifacts.
- APPROVED: Start the integrated slice with 30 ADDITIONAL trait mechanisms, over and above all required baseline biology and retained original capabilities. This supersedes the 12–15 proposal. It does not mean 30 traits per species. All 160 additional catalogue mechanisms remain in final scope.
- SOURCE CLARIFICATION: Original prompt sections 4–6 require the species and individual biology, finite resources/life stages and plant/ecosystem fundamentals recorded in PARITY.md. Section 5 explicitly retains nocturnal activity, docility, slow movement and milk production outside the additional catalogue. These capabilities are required where biologically applicable, not universal properties of every organism.
- UNRESOLVED: Fast-forward cap, precise season length/biological compression, smaller-map dimensions, civilian/detailed-wildlife/cohort budgets, exact 30-trait selection, low-end named hardware and save/memory acceptance targets. Do not silently choose or mark these approved.

## 2026-10-29 — Tor's visual-generation approval rules

- APPROVED: Tor explicitly authorized adding these two hard rules to RULES.md.
- APPROVED: Any and all use of the image generation skill or image generation tools requires Tor's approval before use.
- APPROVED: Any use of Python to assemble crude shapes and stamp them as sprites or other game visuals requires Tor's approval before creation or use. Tor expects to decline almost all such proposals; ask when significant resource/time savings can preserve the established look. No image-generation or shape-stamp use was approved by the request to add these rules.

## 2026-10-29 — Tor authorized development to begin

- APPROVED: Begin Milestone A. The prior planning pause is lifted for this milestone; ordinary necessary source inspection, diagnostic code, builds and relevant checks are authorized. Milestone B still waits for Tor's go-ahead.
- APPROVED: Delegate bounded grunt work to sub-agents when doing so saves time; use enough parallel capacity for independent tedious work, avoid delegation that adds delay, and retain primary review/integration.
- APPROVED: Include percentages in progress updates for the current task/milestone. Percentages describe checklist progress, not measured game completeness or an FPS guarantee.
- PROPOSED: Milestone A integrated qualification fixtures of 2,000 civilians, 5,000 exact wildlife and 1,000,000 cohort wildlife plus the approved 10,000 military, using 256 shared species definitions. These are test-fixture proposals, not world caps or approved population balance.
- PROPOSED: Frame/memory/save budgets and requested-speed experiments in native/qualification/milestone-a-scenarios.json. They are not measurements, approved minimum specs or promises.
- PROPOSED: Exact initial30 selection in docs/MILESTONE_A_TRAIT_PROPOSAL.md. Count30 is approved; this particular list is not yet approved. Mandatory biology remains outside it and all160 mechanisms stay in final scope.

## 2026-10-29 — Actual PC results received (evidence, not new balance approval)

- RECORD: Tor supplied four qualification files. Reviewed Vulkan PID21300 and DX12 PID10456: all six1k/10k/100k isolated movement checks passed; both exits0. Named hardware and measured limits are recorded in docs/MILESTONE_A_PC_RESULTS.md.
- RECORD: The Milestone A automated gate is satisfied by the source/build/package checks and these actual-PC results. Uploading the logs did not approve Milestone B, the exact proposed30, proposed fixture/budget values, a speed cap or season timing. Original approved20km/10k military PLUS other populations/all160 requirements remain unchanged.

## 2026-10-29 — Consolidated approval request (not approved)

- RECORD: Tor asked for one direct list of pending approvals and instructed the AI to maintain project files without making file links necessary for the conversation. Current request is docs/MILESTONE_B_APPROVALS.md; no approval was inferred from asking for the list.
- PROPOSED: Begin Milestone B using the existing integration plan and exact proposed30, plus the previously proposed representative population/species fixture, speed cap and performance/save budgets as itemized in the consolidated request.
- PROPOSED: Full seasonal cycle2 real hours at1x, with biome-specific manifestations and an explicit shared-clock biology mapping.
- PROPOSED: Smaller maps5x5km (45–60-minute balance target) and10x10km (90–120-minute target), preserving the approved20x20km large map/3–4-hour target; no forced match cutoff or implicit pawn defaults.
- PROPOSED: Docks/unarmed troop-and-cargo transport boats for larger water crossings and bridges for narrow crossings, without silently adding a dedicated naval-combat roster.
- RECORD: Protected artwork, pin changes, save-breaking behavior and lower-army hardware tiers are not requested by the consolidated list. Existing approval gates remain.


## 2026-10-29 — Tor approved the list with terrain, speed, season and naval changes

- APPROVED: Tor clarified, "things you asked about approval for that are not specifically mentioned in my last message are approved". This authorizes Milestone B and all unchanged items in the consolidated eight-item request. Do not ask again for the same approvals. Implementation/evidence remain separate.
- APPROVED: Prioritize initial map-changing traits now, explicitly101 Dam construction and12 Burrowing. Dams must pool finite water upstream, reduce downstream flow while storing water, and inundate banks into a reservoir. Burrows must support small horizontal/sloped/curved/branching passages, not only vertical holes. Other ecosystem-engineering cases belong in the early common architecture.
- RECORD — selected engineering response to that revision: initial30 IDs11,12,46,61,64,65,67,72,75,84,86,93,96,101,102,103,104,105,106,107,121,125,128,129,132,135,143,156,157,159. This supersedes the earlier family-quota selection; Tor set the criteria rather than naming every replacement. Required biology remains additional, and the other130 are deferred mechanisms in the same full160 scope, not removed requirements. Details/costs/future checks: docs/MILESTONE_A_TRAIT_PROPOSAL.md.
- APPROVED: Selectable1×/2×/4× must be available for testing in the first build. This is not a guarantee that4× can sustain the complete target workload. Preserve fixed ticks/rules/counts and report requested/submitted/achieved speed plus backlog honestly.
- APPROVED: One full seasonal cycle every hour at1×, superseding the proposed2-hour cycle. It uses the shared game clock:72,000 ticks at20Hz,30 wall minutes at achieved2×,15 at4×. Pause and overload affect calendar progression. Plan actual effects on flora/fauna, ground, water and related logistics; jungle wet/dry behavior can differ from cold-region seasons. Species development/aging compression is explicit, not universal one-hour maturation.
- APPROVED: Naval combat is required in addition to bridges, docks and troop/cargo transport vessels. This supersedes the earlier transport-only proposal. Exact vessel roster, weapons and balance remain future design details; transports alone do not satisfy a navy.
- APPROVED: Representative integrated qualification fixture of10,000 military PLUS2,000 nonmilitary pawns,5,000 detailed wildlife and approximately1,000,000 additional cohort wildlife,256 shared species definitions. These are test loads, not world caps, fake population targets or identical recipes for every seed.
- APPROVED: Smaller presets5×5km (45–60-minute balance target) and10×10km (90–120-minute target), plus20×20km/3–4-hour large map. No forced match cutoff or implied smaller-world population caps.
- APPROVED: Development goals:steady60FPS/median16.67ms,p9525ms,p9950ms,max>100msflag; peak process RAM4GiB; GPU payload/cache2GiB on Tor's8GiB card, with adapter-appropriate caches elsewhere; mature heavily edited saves around256MiB/load15s on named storage. Goals are not measurements, approved minimum specs or performance guarantees; driver/texture/staging overhead is separately accounted.
- RECORD: Shared terrain/water/underground/season/naval architecture is planned in docs/TERRAIN_WATER_AND_SEASONS.md. Natural engineering versus faction-induced downstream harm requires causal records; exact score weights remain tuning work. One physical owner per stock, persistent tunnels/dams and no hidden offscreen freezing remain mandatory.
- RECORD: Approval does not extend to image generation, Python shape-stamp artwork, engine version upgrades, save-breaking changes or smaller-army hardware tiers. RULES.md remains unchanged.

- RECORD — B foundation evidence: integer20Hz clock, first-build1×/2×/4× controls/CLI and72,000tick season phase implemented;16code checks passed and locked Windows release cross-build passed. Terrain/water/underground/seasonal-environment/naval systems remain planned. No new actual-PC performance claim or replacement of the delivered A ZIP. Verification: docs/MILESTONE_B_FOUNDATION_VERIFICATION.md.


## 2026-10-29 — Tor clarified gradual river flow and continued B

- APPROVED: River water must flow and accumulate over time. Building a dam must not replace an ordinary river with an immediately completed reservoir. Tor instructed continuing independently if no additional clarification is needed; no further clarification blocks this increment.
- RECORD: Implemented a CPU-owned conservative local water graph and retained coarse editable terrain, plus an explicitly separate native river interaction fixture. Closing/opening a barrier changes links but never directly changes owned liters. Materials, actual animal construction and full trait101 remain planned.
- RECORD: A water-only fixture with explicitly no military/civilian/wildlife agents is separate from the movement benchmark and approved full-workload target. It is not an army reduction or integrated-game qualification. Proper fixed-tick coupling with military/navigation remains required before those modes can be combined.
- RECORD: Native CPU report seed78421/4800ticks conserved water at all10checkpoints. Upstream storage grew from8,586,635L before closure to10,958,366L after120simulated seconds; closure and breach themselves preserved inventory. Numerical-model behavior is not real-world calibration or performance evidence. Raw report and limitations: docs/RIVER_FLOW_VERIFICATION.md.
- RECORD: Existing single renderer, procedural pixel material/growth formulas, engine pins and visual approval rules retained. No image generation or Python stamp artwork; no browser deployment. Future20km streaming, fine pages, biology/conservation/research/saves, burrows, seasons and navy remain required scope.

- RECORD — delivered separately named Last-World-River-Foundation-Windows.zip after32checks, Linux/Windows builds,4800tick CPU conservation report and package identity/CRC verification. Existing A archive unchanged. Source checkpoint a22597b; no actual Windows visual/GPU execution claimed.


## 2026-10-29 — Tor clarified water/soil/foliage interaction

- APPROVED: Water must physically interact with ground elevation/depth, rather than only changing a water mask. Flow must be able to wash away foliage and change the bed/banks through erosion/transport/deposition.
- APPROVED: Riverbed material should read as brown dirt, with darker brown at the deepest part. Preserve the procedural ground style; underwater material/depth coloring does not require continuous lighting. Current blue-green water shading is not acceptance of the requested final riverbed appearance.
- APPROVED: Flooded/disturbed ground retains its changed state and moisture after drainage. No immediate dry-land texture reset or ecological recovery. Require sufficient exposure to air before moisture loss/recovery progresses, with explicit game-clock exposure state and resource/viability conditions.
- IMPLEMENTATION DIRECTION: Flow-driven detachment/soil transport will use finite sediment/biomass stocks, material resistance and water/depth/flow conditions; loss at source transfers to suspended/debris/deposited or explicit boundary-owned stocks. Terrain bed changes feed back into water storage heads/connectivity and routes. Submergence alone is not evidence that every tree is instantly uprooted. Numerical thresholds/exposure duration remain tuning work within approved scope.
- IMPLEMENTATION DIRECTION: Exposed-air eligibility/deadlines derive from authoritative ticks; resubmergence resets/suspends exposure as defined. After the required exposure delay, actual drying follows finite soil-water loss and climate/canopy conditions; no hidden offscreen fast-forward or new lighting engine. Vegetation recovery separately needs surviving roots/seeds/organisms, resources and time; washed-out biomass is not regenerated automatically.
- RECORD: These interactions are planned, not implemented by the delivered River Foundation ZIP. No new source/build/test/artwork change in this clarification task. Existing32checks and prior CPU report are not evidence for erosion, foliage transport or delayed drying.
- RECORD: No additional source files are needed to continue implementation. For actual-PC evidence, console.txt from the new extracted river build is useful; river-history.jsonl exists after Run-Water-Check.cmd, and a screenshot/clip can show shoreline/material behavior. These are evidence requests, not new permission or development gates.


## 2026-10-29 — Visual test clarification and PC evidence

- APPROVED: Tor clarified the speckled appearance is acceptable as a development test only and must not represent final in-game units. No extra particle labels/test UI are requested. Skip that work; preserve the established procedural pixel style with readable unit forms and proper crowd spacing/avoidance in the game, without hiding/reducing individual military counts.
- RECORD: Pending uncommitted HUD/default-launch/label changes created during the initial interpretation were discarded before integration/build/shipping. Existing source executable/launchers/download remain unchanged; no labels or new physical sprites were added.
- RECORD: Uploaded current Windows logs distinguish riverPID23312 (zeroagents, G/J andspeedcontrols), movementPIDs9088/18664/10492 (5000agents), andCPUreportPID16652. All four screenshot filename times fall withinPID10492; visual content matches dense overlapping movement agents. This is temporal/content correlation, not EXIF proof or definitive launcher identification.
- RECORD: Zero FATAL/PANIC/GPU_ERROR/SURFACE_ERROR tags in the current Windows segment starting uploadedline11806;100GUIWATERrecords independently conserve liters. UploadedCPUhistory contains one report identical to the Linux reference with10balanced checkpoints. Older error tags precede this segment. These observations qualify the local river execution path, not full-game/erosion/ecology/naval performance.
- RECORD: Evidence archived at evidence/pc-river-2026-10-29/ with input identities/hashes. Current river visual screenshots were not supplied; the four images show movement mode. No code change, new build, repeat benchmark or new download in this review.


## 2026-10-29 — Tor requested batched work and one-file feedback

- APPROVED: Continue development in larger coherent batches with fewer stops. Batch related tests before requesting PC feedback; do not hand off every small component separately.
- APPROVED: Deliver an overlay update folder that can be copied over the existing game folder, replacing application files as needed while preserving existing logs, settings and saves. Keep the existing user-selected drive/location.
- APPROVED: Provide one CMD to run all requested checks and one file to drag back, without manual selection of individual logs/results. The chosen contract is Run-Checks.cmd and Send-Back.zip.
- IMPLEMENTATION DIRECTION: Stable Last-World folder/update ZIP, immutable payload manifest, mutable runtime files excluded from update; append-only console/history preserved; owned-child sequential batch, automatic river captures, failure bundle, no recursive old ZIPs. Native tests remain scoped and honest about integrated-game/performance limitations.
- RECORD: No extra particle/test labels are reintroduced by this workflow. Image-generation/Python-stamp approval rules and engine pins remain unchanged.

- RECORD — coupled batch: finite ground/water/biomass, delayed soil drying and brown/depth-darkened bed implemented with50passing code checks and a4,800tick accepted CPU report. Initial uncapped-suspension model case preserved and corrected before delivery. Exact model parameters/counts and limitations are in docs/GROUND_WATER_VERIFICATION.md; full campaign/biology/performance is not qualified.
- RECORD — update/feedback policy: exclude all mutable user/session files from overlay; one Run-Checks.cmd and one Send-Back.zip, with automatic renderer captures and failure collection. This is the current development distribution path, not removal of the final click-to-update/native parity requirement.

- RECORD — delivered stable overlay update from source26654552b435e2453a1a99bf5935fddcffdaa764. ZIP/hash/executable/overlay preservation checks passed; old downloads unchanged. One Run-Checks.cmd and one Send-Back.zip are the PC workflow.50code checks andacceptedCPUledgerreport remain the native evidence; actual new Windows batch/captures are pending.

## 2026-10-30 — Single-file coupled PC batch reviewed

- RECORD: Tor returned one Send-Back.zip, run20261030T021220123Z-23228. Four functional stages exit0; six isolated movement GPU checks pass; CPU JSON exactly matches the accepted Linux report;275independent water/sediment/biomass balance identities pass. All three actual renderer captures and final tick3600report returned. Source26654552/executable identity matches delivered payload; no current FATAL/PANIC/GPU_ERROR/SURFACE_ERROR.
- RECORD: Windows normal-path runner/capture/feedback collection is tested by this run. Exceptional timeout/duplicate-lock/fallback branches and actual preservation of a prior PC folder are not established by this successful current-only upload. Existing local sentinel overlay QA remains separate.
- RECORD: River max909.9335ms at tick1799 with56pending ticks; later debt clears. Smaller199.8925/153.0291ms maxima coincide with startup/capture windows. Cause of the largest interval remains unlocalized. Functional success is not hitch-free4× qualification; stage timing belongs in the next coherent development batch, without an immediate unchanged-binary rerun.
- RECORD: Numerical measurements, raw records, input/capture hashes and limitations: docs/COUPLED_PC_BATCH_RESULTS.md and evidence/pc-coupled-2026-10-30/. No game source, new build/archive, engine pins, visual approval rules or scope changed by this evidence review. No new user decision inferred from uploading results.

## 2026-10-30 — Tor requested uninterrupted Milestone B integration

- APPROVED: Continue until the entire MilestoneB connected small game/ecosystem/limited-trait checkpoint is evidenced, or a concrete required user action/decision/PC test blocks further progress. Do not end with routine partial checkpoints or an offer to continue.
- RECORD: Handoff19.2 defines B's connected native ecology/discovery/capture/research/breeding/release/consequences/save path integrated with useful construction/logistics/combat. Initial30 additional mechanisms plus required biology remain required. Production20km/all160 expansion belongs to C without reducing approved release scope. Acceptance checklist: docs/MILESTONE_B_ACCEPTANCE.md.
- RECORD: One parent task covers B integration with six bounded delegated areas and primary integration/review. User-facing percentages refer to that checklist, not an invented finished-game/FPS guarantee. Normal one-folder/one-CMD/one-ZIP workflow remains in force.

## 2026-10-30 — Tor specified shipped RTS UI and waived intermediate stops

- APPROVED: UI requirements in docs/UI_REQUIREMENTS.md apply to the interface; preserve current rendered-world graphics. Replace the temporary text-heavy test HUD with compact industrial RTS controls, deliberate bundled open-licensed fonts, real object icons, minimap, selection/command cards, alerts, economy/ecology/food-web/logistics depth, accessibility and saved adaptive layout. UI owns projection/preferences only and sends ordinary commands.
- APPROVED: Tor subsequently directed making sensible decisions while he is away and continuing through MilestoneB without stopping at each UI step. This supersedes the earlier style/mockup/step approval pauses; perform the design/mockup/review process internally. It does not authorize image generation or primitive Python sprite artwork.
- IMPLEMENTATION DIRECTION: Same wgpu device/renderer, cached custom UI layer; graphite/warm typography, faction amber/blue with noncolor trim; RajdhaniSemiBold headings + IBMPlexSans body and bundled official OFL files. Budget targets1ms CPU/1ms GPU1080p remain unmeasured until appropriate native evidence. Actual running-build screenshots at1080p/1440p/smallwindow must be gathered in the combined PC batch; static mockups are not native proof.

## 2026-10-30 — Tor requested exhaustive resumable stress qualification

- APPROVED: After the mini playable candidate local pass, extend the single-click PC suite to exercise every currently available system together and through focused mechanisms, push beyond intended load targets, and step requested benchmark rate through100x. This does not silently unlock normal playable speeds or change20Hz rules. Report requested/achieved speed, debt, frame quantiles, exact counts and coverage gaps.
- APPROVED: Long-running automatic PC test; each crashing phase is retried once, skipped after two failed attempts, and remaining independent phases continue. Persist progress, resume interrupted same-build work, automatically gather one feedback archive. A skipped/crashed/unreached test is never a pass.
- IMPLEMENTATION: Keep the existing renderer/commands/finite owners. Use isolated native processes and an owned journal/checkpoint directory; supervise only verified owned children. Include individual Windows mechanism tests without requiring Cargo/Python on Tor's PC. Benchmark-generated initial workloads are explicitly named; no rescue spawning, free construction or fictional coverage of unimplemented systems.


## 2026-10-30 — Tor revised speed qualification

- APPROVED: One gradual speed-ramp test rather than separate repeated-load speed tests. Give loading adequate time; do not run higher speeds once a sustained lower-rate limit makes their failure clear. Retain exact clock/debt/rules/counts and report measured/not-attempted honestly.
- IMPLEMENTATION: One windowed same-world/same-device adaptive ramp,15second live initial warm-up,5second settling per stage and repeated10second measurements, with early capacity-stop. Normal playable1/2/4 is unchanged. Independent seasonal/population/system correctness probes remain.
- RECORD: Two subsequent valid .part archives contain the actual current6bac/2c367 PC suite; this supersedes the duplicate-only return observation.251/262phases passed,11skipped (8missing material,1AlreadyPresent,1expectedcapacity,1intentionalrecoverycontrol). Source-level fixes and acceptance evidence are maintained separately; no current measured speedup is inferred.

- IMPLEMENTATION CLARIFICATION: Independent focused smoke/population/seed/recovery probes request1x and seasonal soak2x. They no longer impose100x/32x demand after the adaptive test has found a limit. Only the one ramp requests higher rates. The seasonal workload remains72000actual additional ticks; at achieved2x it takes about30wallminutes, with a retained4-hour safety timeout.


## 2026-10-30 — Hardware utilization during PC tests

- APPROVED: Tor requested GPU utilization/VRAM/3D processing visibility during tests, then added CPU and RAM. Collect read-only owned-process activity and hardware context in the same one-click/single-feedback workflow.
- IMPLEMENTATION: Optional Windows PDH English GPU engine/dedicated/shared allocation counters and native CPU/system-memory sampling,1Hz, per-attempt raw records/summary with overhead and unavailable statuses. GPU3D/Compute/Copy are separate engine kinds; do not sum contexts into an invented total percentage or treat allocation counters as resident VRAM proof. No artificial GPU/memory usage target, engine pin, new renderer or gameplay count changes.

- RECORD: Frozen repairedsource7f903735 passes237nativechecks/24portable/2identity/all14connectedCPUgates; bothgame/harnessbuilds pass and firstoverlayCRC/source/hash/sentinel audit passes. NewWindowsrendering/feedback/PDHutilization/performance need the singlePCrun. No newgamebalanceapproval inferred.


## 2026-10-30 — Folder clarity and active battle test

- APPROVED: Tor requested a better-organized game folder and clarification of the correct Run-Checks launcher and .lock. Tor identified the missing replacement prompt himself: the new Last-World folder had been copied inside the old one. Correct copying merges the contents into the actual existing installation.
- IMPLEMENTATION: Two obvious root launch buttons; App/Tools immutable files, Data persistent state, Documentation and Source. Verified initializer preserves known old data, exact append histories, collision copies and test progress; no automatic nested-folder relocation/drive change. Banner and payload validation identify the active copy. Lock files move under Data; active ownership depends on the open handle.
- APPROVED: One actual visible battle test at requested4x for20minutes of game time with BOTH factions aggressively fighting and constructing while generation/ecology/water/seasons continue. This is a scenario workload, in addition to the one adaptive speed comparison; it does not reinstate separate repeated speed tests.
- IMPLEMENTATION: Strict24000actual ticks with bounded two-team ordinary-command AI, a named pre-tick0 frontline-deployment fixture preserving initial military/workers/identities/stocks, and clearly labeled diagnostic Observer rendering only. Normal commander vision, AI knowledge, rules and populations remain unchanged. Coverage must demonstrate later combat and paid construction on both sides. New scenario's actual results await qualification.

- DIRECTIVE: Tor clarified that sub-agents must handle ONLY easy work, whether tedious or repetitive. Complex architecture, implementation and diagnosis remain with the primary agent. Prior complex delegation was acknowledged; active agent diagnosis was stopped.
- DIAGNOSTIC: Tor observed the old cpu-season-78421 phase at45minutes. That phase requests2x over72000ticks with a14400second deadline; duration alone does not prove a freeze. Standalone Check-Progress.cmd takes read-only samples and publishes separate Send-Back-Progress.zip, preserving the running test and original feedback. New supervisor shows progress every10seconds.

- PC EVIDENCE: Send-Back(2).zip SHAa04adb320a79b6ef96ce1ec8685ea3e753827252d10d809faf6fff36901cb78d, old7f903735/3b09afc2, run22544. Suite completed254/257; season passed42m30.7s,1.41479x achieved versus2requested,72052actualticks. Speed ramp measured1x/2x, stopped higher rates after4xcapacity, then failed Invalid UI evidence prefix because nested directory was passed as a basename. Rendering API now receives directory separately and retains basename traversal rejection. Pure-path regression added. Renderer120000capacity and intentional always-crash probe remain declared nonpasses. No speed guarantee inferred.

- RECORD: source0f1ddec0 frozen host/Windows apps and runners pass;250host tests pass. Full fixed-elapsed battle core passes exact24000ticks,10000military+2000workers,9521shots, paid jobs[9,15], sustained later combat and biology/water. Heavy build concurrency/no GPU make this correctness-only. Current Windows runtime, migration and frame/throughput remain PC gates.

- RECORD: organized candidate audit passes CRC/payload/source/239native inputs,3WindowsPEs,250test catalog,13cases/one ramp/explicit battle,37mutable sentinels. First-launch Windows migration is not implied by ZIP merging. Final delivery preserves one root Run-Checks.cmd and one root Send-Back.zip.

- DELIVERY: Locally verified organized update saved under the existing Last-World-Update-Windows.zip identity. Native source0f1ddec0/Windows9e6bfd2a;250host checks/full24000tick correctness/organized envelope checks pass. This records delivery readiness; actual new Windows migration, rendered battle and throughput require the single Tor PC batch. Final documentation records may change envelope bytes while native source identity remains unchanged.

## 2026-10-30 — Natural flora/fauna palette

- APPROVED: Tor requested a quick natural-color survey and procedural color changes because pink/turquoise plants were unwanted.
- IMPLEMENTATION: forest/fern/olive/yellow-green canopy, amber/cream accents, brown/olive/ochre/charcoal animal coats. Existing seeds, growth/geometry/alpha, biome/soil/water/height/deformation, ecological stocks and faction materials remain unchanged. No new imagegen or primitive stamps. Shared existing world/preview/atlas materials receive the palette.

- RECORD: Palette source837a1f28/Windowse80b77a9 frozen builds and250host checks pass;3328form/actual atlas scope checks pass. Exact shapes/alpha/non-fauna materials preserved. No game clock, populations, terrain/biomass rules, save schema or performance test changes. New visual approval is through normal Run-Game.cmd.

## 2026-10-30 — Correct fixed-base palette limitation

- APPROVED: Tor clarified that the colors themselves must be procedural across greens, browns, reds and yellows, rather than a narrow fixed palette, and explicitly requested the correction.
- IMPLEMENTATION: continuous seeded HSV sampling, cached smooth vegetation gradients, generated accents and species/world-stable animal coats shared with UI/Observer. Pink/turquoise remain excluded. Visual RNG is independent, real tactical factions and gameplay RNG/terrain/biomass/save data remain unchanged. Existing shapes and raster layout retained; no imagegen/Python primitive stamping.

- RECORD: broad-color source3c9147c8/Windows41dbfc9d frozen builds and253native checks pass;3328shape comparisons, actual atlas scope check, cross-language seed vectors and two exact physical stock/terrain hashes pass.4073distinct JS colors observed from4096samples. CPU terrain crop inspected separately from native rendering. No new performance claim or long speed test.
