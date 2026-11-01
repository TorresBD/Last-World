# LAST WORLD: MASTER FILE

**Revision:** M10 | **Date:** 2026-10-31 | **Owner:** Tor
**Where it lives:** Required working files in every game source directory: `project/MASTER.md` and `project/LEDGER.md`. This checkout: `E:\Games I am Making\Last-World\Source\project\`. Keep these paths in every future version, complete Source zip and update package. The Library holds matching versioned source checkpoints and standalone copies; `project/archive/` holds frozen history.
**Git mirror (secondary):** https://github.com/TorresBD/Last-World, branch `main` (work branches merge back into it; rule 5.13), last inspected head `d57bbf1efa63ec40982df8d3adfa3678a1544f75` (A-040 merge follows).
**Voids:** every earlier handoff and the old RULES/STATE/DECISIONS/BLOCKERS/PARITY files as instructions. The superseded source-root copies are removed; their full text remains in the indexed archive, located by `project/archive/legacy-governance-location-map.json` (A-038).
**Authority order:** Tor's latest chat message > platform rules > this file > LEDGER > ARCHIVE.

---

## 0. HOW TO USE THIS FILE

- Read this whole file once at the start of a session. It is the only file you must read.
- **The Ledger is the record of every approval, decision and build.** This file holds only core scope, rules and current state. Never read the whole Ledger or Archive without Tor's approval (rule 5.11).
- Hard cap: 30,000 characters (about 7,500 tokens). Compact when near it (section 12).

### Navigation

For Ctrl+F, type two hash signs, a space, the section number and a period. For a specific rule under section 5, use three hash signs and its rule number. The archive keeps its existing bracketed LW codes unchanged.

| Section | Topic |
|---|---|
| 0 | How to use this file |
| 1 | Session summary |
| 2 | Top risks |
| 3 | Vision |
| 4 | Core scope |
| 5 | Hard rules (5.0-5.13) |
| 6 | Source tree map |
| 7 | Current state |
| 8 | Opening, HUD and recon task |
| 9 | Next step |
| 10 | Open decisions |
| 11 | 1.0 gate |
| 12 | Compaction and ledger rollover |


---

## 1. SESSION SUMMARY (overwrite every session; hard limit 1,500 characters)

**This session (2026-11-01):** Two batches for Tor's "not intuitive" notes. (1) A-039: live road/trench/bridge outline with length/cost/refusal reason, planned orders kept on the map, accept/refuse flash and alert (B-004). (2) A-040: Field orders checklist driven by real state; Elevation/Water/Movement overlays; legend for every colour overlay; all 12 overlay buttons with hover help; hover info for units and planned orders; camera range ring; selected units' route lines; camera kept on the map (no black void); sentence-case labels (B-005). Rule 5.13 added; CI restored on main and publishes review captures to `ci-evidence/<branch>`.
**Evidence:** Windows CI green (L-0029); renderer captures reviewed. Tor-PC play and performance UNMEASURED.
**Next:** Tor plays and notes what is still unclear. **Look up:** A-039, A-040, L-0028, L-0029; #ui.

---

## 2. TOP RISKS (max 5; keep current)

1. **Performance is unproven.** On Tor's PC only 1x and 2x were sustained. A 4x attempt reached about 3.42x with growing debt and about 1 presented frame/second. The earlier full baseline showed frame p99 about 120 ms and max about 1,187 ms against goals of 50 ms and 100 ms. These belong to older builds.
2. **Opening verification.** The lost HUD/opening changes have been recreated and saved on codex/opening-hud-recon. Native compilation and visual checks remain pending; resume this source, not the older checkpoint.
3. **Scope vs. reality.** The resident map is 4.8 x 3.2 km, not 20 x 20 km. 130 of 160 traits, complete mandatory biology, full causal scoring and fuller naval/mounted-camera parity remain.
4. **Information leaks.** The new HUD, terrain memory and radar must not reveal hidden positions, species, health or biology.
5. **No weaker-hardware evidence.** All measurements are from one PC. Weaker-PC work is deferred, not dropped.

---

## 3. VISION (retain)

Last World is a native Rust/Bevy/wgpu, real-time, top-down 2D ecological RTS for Windows PC. Two factions crash-land on opposite sides of a valuable living jungle world and crudely industrialize it while fighting for control. Roads, mines, depots, trenches, defenses, cameras and supply lines cut through thick, colorful, procedurally grown jungle with real terrain, rivers, hills and clearings. The commander sees only through cameras, radar and periodic satellite passes, never through ordinary soldiers. The land is the prize: ecological harm can decide a stalemate, and the faction responsible for more harm loses. The world has procedural species and food webs, all 160 biological trait mechanisms, finite resources, reproduction, research, conservation, and permanent extinction. Unexplored regions evolve before discovery. Water flows, erodes and floods; dams, burrows and engineering change the map. Losses persist.

**Visual identity.** Procedural pixel materials, noise/branching growth, layered height fields. No geometric shape stamps, lettered boxes or generic stand-in icons. Lighting is explosion-only. Natural procedural colors (greens, browns, reds, yellows; no pink or turquoise). Shipped-RTS UI: compact industrial HUD, real object icons, deliberate bundled fonts.

**Product principles.** Last World is its own game (The Long Front is only a style and engineering-lessons source). Performance may not silently remove features, pawn counts, consequences or populations. Big map, big wildlife total and many detailed agents are three separate costs. Looking away never pauses consequences. Build a connected, playable whole first, then deepen it. The game is in very early development.

---

## 4. CORE SCOPE (stable facts; the approval history is in the Ledger)

- **Platform:** native Windows PC; actual-PC testing is the priority; browser only for tiny checks; phone deferred. (A-001)
- **Maps:** large 20 x 20 km; types Continental, Archipelago, Large Land Mass, Medium Islands, with river/mountain controls; presets 5 x 5 km (45-60 min) and 10 x 10 km (90-120 min). Targets, not cutoffs. (A-002)
- **Workload:** 10,000 military PLUS non-military pawns and wildlife; a large war lasts about 3-4 hours. Qualification fixture (a test load, not a cap): 10,000 military + 2,000 non-military + 5,000 detailed wildlife + about 1,000,000 cohort wildlife + 256 species. (A-003, A-004)
- **Biology:** initial 30 mechanisms 11, 12, 46, 61, 64, 65, 67, 72, 75, 84, 86, 93, 96, 101-107, 121, 125, 128, 129, 132, 135, 143, 156, 157, 159. All 160 plus mandatory biology stay in final scope. (A-005)
- **Time:** selectable 1x/2x/4x, with requested vs. achieved speed always shown. One season per game hour at 1x (72,000 ticks at 20 Hz); aging compression is species-specific. (A-006, A-007)
- **Naval:** combat navy plus bridges, docks and transports; roster TBD. (A-008)
- **Development goals (not measurements):** frame median 16.67 ms, p95 25 ms, p99 50 ms, flag max > 100 ms; peak RAM 4 GiB; GPU payload/cache 2 GiB on the 8 GiB card; mature save about 256 MiB, load 15 s. (A-009)
- **Water:** flows and accumulates; dams pool gradually; flow erodes, deposits and washes foliage; brown bed, darker at depth; wet ground dries only after enough air exposure. (A-010)
- **Victory:** in an ecological stalemate the more damaging faction loses; natural erosion is not faction harm. (A-011)
- **UI and palette:** shipped RTS UI per `docs/UI_REQUIREMENTS.md` on the same renderer, Rajdhani SemiBold + IBM Plex Sans; continuous natural palette with stable animal coats. (A-014, A-015)
- **Opening crew:** 6 builders + 4 defenders + 1 camera trooper per faction. **PROVISIONAL test value**, revisit after playtesting. (A-025)
- **1.0** means everything in section 11; the list may grow. (A-028)
- **Not approved:** hardware tiers, smaller-army fallbacks, per-tier pawn counts, map sizes or speed caps.

---

## 5. HARD RULES (these override your preferences and your memory of the conversation)

### 5.0 Default autonomy: one request in, an updated game out (A-035)
- A request or a set of notes from Tor means: implement, build, run checks, package and save. The reply delivers the updated game, a short change summary and only what truly needs Tor.
- **Ask only when:** (a) the request contradicts core scope, a rule or a recorded approval; (b) the change is irreversible, breaks saves or deletes data; (c) it needs a new kind of approval (image generation, pin changes, smaller armies, moving the source of truth); or (d) two readings would give substantially different results and a wrong guess would waste a whole batch.
- Otherwise pick the most sensible reading that fits the vision, do it, and list it under "Assumptions" in the reply and the Ledger so Tor can correct it.
- Before asking anything, search the Ledger by tag. Never ask what is already recorded.
- If a question is needed: one message, at most 3 questions, each with a default. Keep doing all unaffected work. Unanswered questions use the default.
- Treat every note as an instruction. Implement all notes in one batch. Never ask "should I proceed?" and never offer to do work instead of doing it.

### 5.1 Truthful reporting
Use only: **planned** (written, no code) / **implemented** (code compiles) / **tested** (a named check passed) / **measured** (numbers on named hardware).
- Never say done/complete/works without naming evidence. Never claim FPS, capacity, speed or memory you did not measure on this project's real workload.
- Software-renderer, core-limited and memory-limited runs are *proxy checks*, never "floor", "minimum spec" or "low-end results". Low-end performance is UNMEASURED until measured on real low-end hardware Tor names.
- A benchmark, demo or isolated simulator is not the game. Headless/CPU correctness is not Windows visual, GPU or throughput proof. Keep failed runs; report failures plainly.
- Report frame times as median, p95, p99 and max. Never overlap heavy builds with a performance run.

### 5.2 Scope protection
- Never remove, shrink or silently postpone a requirement. Re-ordering is allowed; dropping is not. If something is too hard or slow, say so with cost and options; Tor decides.
- Never reduce pawn counts, fake populations, add hidden prey, set population floors or regenerate extinct species.
- Do not invent requirements. Do not promote PROPOSED or PROVISIONAL items to final.

### 5.3 Engineering invariants
1. One owner per quantity. No duplication.
2. Integer counts are exact; checked arithmetic; no negative counts or silent clamping.
3. One integer game clock (20 Hz). Frame rate never changes outcomes.
4. Unseen does not mean frozen.
5. Information filtering: no UI, inspector or tooltip may leak hidden positions, species, health or biology.
6. CPU is authoritative for gameplay rules; GPU for rendering and bulk work until validated end to end.
7. No full-world fine-resolution allocations; use chunks, pages and sparse data; track memory high-water marks.
8. Stale background results are rejected via revision stamps.
9. Saves are sacred: migrate on schema change; atomic writes; keep the previous save.
10. Extinction is permanent unless real surviving organisms or viable stored stages exist.
11. Every trait has a real effect, cost and test.
12. Determinism: same generator version, seed and settings give the same accepted world.

Also: one renderer; pinned Bevy 0.20.1 and wgpu 27.0.1 (no change without need and approval); `console.txt` is append-only.

### 5.4 Hardware and performance
- Dev PC (top tier, not the baseline): Ryzen 9 3900X (12c/24t), 32 GB RAM, RTX 2080 SUPER 8 GB (driver 32.0.16.2101, DirectX 12 FL 12.1), Windows 10 Pro 19045.
- The game must also run on weaker computers. Size threads from available parallelism and GPU caches from adapter limits. A low-detail path keeps gameplay rules identical. Weaker-PC testing is deferred until Tor chooses (A-027). Windows 11 is also a target.
- Hardware tests collect read-only CPU/RAM/GPU activity; no artificial VRAM filling. (A-019)
- Speed testing is one gradual adaptive ramp that stops early when a lower rate fails, plus one 20-minute 4x visible battle. No long speed suite for a small UI change. (A-020)

### 5.5 Visual generation approval (A-012)
1. No image-generation tools (images, sprites, textures, concept art, mockups) without Tor's explicit prior approval.
2. No Python assembly/stamping of crude primitive shapes as visuals without explicit prior approval. Expect refusal; explain asset, technique, savings and tradeoff before asking; no sample before approval.
3. Approved exceptions only: hand-authored pixel-map icons and the fix-only preview tool, both per rule 5.12.

### 5.6 Source safety (A-031)
- **Keep the working records inside the game source:** `project/MASTER.md` and `project/LEDGER.md` are required in every checkout and future source version (A-037). Chat attachments, Downloads and an organization-only ZIP are not substitutes.
- The Library remains the durable archive/source of truth for saved versioned Source packages; the Git mirror is an optional backup. Keep matching standalone Library copies when tools permit. A Library outage does not prevent source-local work or justify omitting these files; report unsynchronized changes plainly.
- Start: read the working source's `project/MASTER.md` and selectively consult its Ledger; reconcile newer Library checkpoints when available. End: update the source-local records in the same session, include `project/` in the complete Source zip/update, and save the checkpoint plus matching standalone copies to the Library when possible. If Library saving is unavailable, preserve the actual source files and supply the package to Tor with that limitation recorded.
- Name zips `LastWorld-Source-B<nnn>-<YYYY-MM-DD>.zip`; add a Master revision suffix (for example `-M8`) for a same-day records-only revision of the same game build. Never overwrite or delete an older zip without approval; keep at least the latest 3 plus every milestone snapshot.
- After saving, list the zip's contents and confirm `project/` and `native/` are present; name the saved file in the handoff. If saving fails, hand Tor the zip and record the failure.
- Never claim to continue edits you cannot see in a saved zip or the Git mirror. State the real baseline.

### 5.7 Working style
- Build on the existing project. Prefer simple concrete models. Keep the game playable at the end of every milestone.
- Distribution: an overlay update folder merged (not nested) into the existing install, preserving saves, settings and logs; one `Run-Checks.cmd` and one `Send-Back.zip`. (A-013)
- Sub-agents only for easy, tedious or repetitive work; complex work stays with the primary agent. (A-016)
- Progress percentages refer to the current task only. (A-017)
- Never ask to use Tor's computer; use Library files, the Git mirror or supplied archives. (A-018)
- Never copy secrets into files. Do not create a duplicate Site or change its audience.

### 5.8 Session protocol
- **Start:** read this file. Begin your first reply with 5 lines (current task, last completed, next step, blockers, rule at risk), then do the work in the same turn.
- **During:** one task at a time. When unsure, re-read sections 1 and 5 only. Re-read the whole file at each milestone.
- **End:** overwrite section 1; update sections 2 and 7-11 only if they changed; append to the source-local Ledger; verify both records remain under `project/`, then package/synchronize per rule 5.6. The handoff is at most 15 lines and cites Ledger IDs instead of copying text.

### 5.9 Self-check before every reply
Did I read this file? Claim anything without evidence? Shrink a requirement? Break an invariant? Leak hidden info? Leave work unsaved? Ask something rule 5.0 says not to ask?

### 5.10 The handoff lives in the source (A-029)
- **Every current and future game source directory must retain `project/MASTER.md` and `project/LEDGER.md` (A-037).** Keep `project/archive/` and snapshots with them. These are maintained source files, not temporary delivery attachments.
- Every complete Source zip and update package must include the same current records under its Source `project/` path. Packaging and its audit must reject missing or empty Master/Ledger files. A package omitting them is incomplete.
- Standalone delivery/Library copies must match the source-local records and the matching Source zip. Reconcile differing revisions or later Ledger entries explicitly; preserve older records before replacing them.
- Update both in the source directory in the same session as the change they describe. Never move them out of source or omit them in a later version. Unrelated source-tree reorganization may be deferred; maintaining these records may not.

### 5.11 Ledger and Archive token discipline (A-030)
- **Never read, print or parse a whole Ledger or Archive file without Tor's explicit approval.**
- Search by ID or tag (for example `grep -n "#ui" project/LEDGER.md project/archive/LEDGER-*.md`) and read only matching lines, about 50 lines per lookup at most.
- To check whether an approval is current, search its ID; **the latest line mentioning it wins.**
- Look up the Archive by `[LWnnn]` code only.

### 5.12 Pixel-map icons (A-026, A-032, A-034, A-036)
**The AI makes icons on its own and puts them in the game in the same build. There is no approval gate.** Tor reviews them in the running game and leaves notes.

**Format.** An icon is a character grid: one character per pixel, each mapped to a named palette entry. Hexadecimal appears only in the palette (rarely elsewhere). Sizes 16x16 (HUD buttons) and 32x32 (hover/detail). One shared palette of at most 12 colors; `.` is transparent; faction accents reserved. Style: 1 px dark outline, light from the top-left, 3 tones per material, no anti-aliasing, 1 px padding, matching the game's pixel style. Icons live in `assets/icons/` with `STYLE.md` and a manifest (ID, size, label, hotkey, version, note IDs). IDs are stable and never reused.

**Process.**
1. Make a 6-icon style anchor first (road, trench, depot, camera tower, builder, mine) and match every later icon to it.
2. Work in batches of 4-6 by type. For each icon: describe the silhouette and proportions in words, plot the outline, add shading, run the checks, revise. At most 4 revision rounds per batch.
3. Checks, all automated on the data (no preview image): rows equal length and only palette symbols; no two icons identical or over 90% the same pixels; filled silhouettes of icons in a group under 80% overlap; key colors at least 3:1 contrast against the HUD background; camera tower, pole, camera and mounted camera differ by silhouette; troop roles share a base figure with a distinct tool.
4. Wire icons into the HUD and ship. Record check results in the build notes. Real in-game screenshots from the normal check run are the visual record.

**Preview tool (A-034, A-036): only for fixing what Tor calls out.** It draws exactly the pixels in the icon data, enlarged, as a before/after PNG. It never creates, completes, smooths, mirrors or alters art. Do not use it for routine icon creation.

**Tor's notes.** Tor cites an icon ID (shown when a dev "Show IDs" toggle is on) or the screen and position, in chat or `assets/icons/NOTES.md`. For each note, without asking first: restate it in one line, change the pixels, bump the version, render before/after with the preview tool, ship the fix, and record it in the manifest. For a vague note, ship the best variant and include two alternatives (B, C) in the preview so Tor can swap by letter later. Only after 2 rounds on the same icon fail does the AI ask Tor for a coordinate hint or ASCII sketch.

### 5.13 Never lose a working feature (A-039)
- Never remove or silently drop a player-facing feature (controls, previews, feedback, camera options). Replacing one needs Tor's approval and an equal or better version in the same build.
- Every restored or Tor-requested feature gets a named regression test; keep those tests passing.
- Start from the newest `main` (or a newer Library checkpoint), never an older copy. Merge finished branches into `main` in the same session, `.github/` included.

---

## 6. SOURCE TREE MAP (approved, A-033; same layout in the Library zip and Git mirror)

```
project/      MASTER.md, LEDGER.md, archive/ (frozen handoffs, old ledgers), snapshots/
native/       Rust game source (src/, shaders/, Cargo.toml, Cargo.lock)
assets/       baked pixel maps, icons/ (data, STYLE.md, manifest, NOTES.md), fonts + licenses
docs/         design/, ui/, qualification/
evidence/     PC results, benchmarks, screenshots (dated folders)
scripts/      build, package, check and preview tools
dist/         legacy browser reference (read-only)
README.md     one screen: what this is, pointer to project/MASTER.md
```
- Each folder has a short `README.md` index. Governance files live only in `project/`. Do not recreate old root governance records or package their full standalone contents; use Master/Ledger and the frozen archive. An older native diagnostic path may have a generated current-record pointer only.
- Moving existing gameplay/assets/scripts is one dedicated task (never mixed into feature work), by default after the current opening/HUD task. Builds and checks pass before and after; manifests are updated; the old-to-new path map goes in the Ledger. This deferral does not apply to installing and maintaining `project/MASTER.md` and `project/LEDGER.md`, which Tor has requested now (A-037).

---

## 7. CURRENT STATE (rewrite each session; current truth only)

**B-003 Windows CI candidate:** Section 8 source compiles; named library/CLI and pixel-map data checks passed in the recorded CI run. A small paused landing, both notices, crew identity/tasks/loadout/real bounded history and role commands, four camera choices, hand-plotted icons and sparse optical terrain/object memory are present. Legacy tool/save identities and full-count constructors remain.
**Placement feedback (A-039):** road/trench/bridge drafting outline with live length/cost/refusal reason, site footprints, persistent planned-order outlines with progress, accept/refuse flash and builder alert are implemented with named regressions (`road_drag_shows_shape_then_confirms_and_keeps_planned_outline`, `refused_road_is_marked_invalid_while_dragging_and_flashes_on_release`, `road_preview_and_planned_roads_draw_along_their_shape`, `many_long_planned_roads_stay_inside_the_gpu_quad_budget`). Tor-PC visual check pending.
**Readability pass (A-040):** Field orders checklist, Elevation (Y)/Water (M)/Movement (Z) overlays, legends, full overlay grid with hover help, unit/order hover info, camera range ring, route lines and on-map camera clamp are tested (Windows CI, L-0029) and visible in renderer captures. Tor-PC check pending.
**Visual evidence:** 12 production-renderer captures were written at small/1080p/1440p sizes. They need human/assistant inspection; creating them alone is not visual approval. Tor-PC playtesting and performance remain UNMEASURED for this candidate.
**Delivery evidence:** First overlay/source identity and user-data preservation audit passed. The matching records are included in the resealed overlay and dated complete Source zip; upload is conditional on the final audit passing. Source-native inputs and executable identities are in the build stamp. Historical evidence folders absent from the mirror are listed explicitly in the package manifest.
**Remaining scope:** The resident fixture remains 4.8x3.2km. Production20km streaming, complete mandatory biology, remaining130 mechanisms and all other open 1.0/performance gates are unchanged.

---

## 8. ACTIVE TASK: OPENING / HUD / RECON CORRECTION (A-022)

"Tested" needs the check to pass; UI also needs real in-game screenshots.

| # | Requirement | Pass/fail check |
|---|---|---|
| 1 | **Icons:** clear depictions of real objects/actions (roads, trenches, buildings, resources, troop roles, camera variants), hover labels, hotkeys, readable at HUD size. Made per rule 5.12. | Every icon passes the 5.12 checks; no two actions share a picture. |
| 2 | **Selection and commands:** unit type, stats, task, role buttons, loadout and event history, multi-select role counts, idle/blocked/job info, queues. Real state only; bounded real history. | Single and mixed selections show correct data; unavailable actions disabled; enemy panels leak nothing hidden. |
| 3 | **Terrain memory:** initial survey visible everywhere; outside current camera/satellite coverage show last-known visuals (world, foliage, water, buildings, minimap, overlays); memory stored apart from simulation; radar shows anonymous contacts only. | A hidden change appears only once optically observed; simulation keeps advancing; radar reveals no species, unit or health. |
| 4 | **Camera choices:** separate tower, post/pole, camera and mounted camera with normal cost, placement, LOS/height, ownership and saves. | Each option places, costs and behaves differently; old saves load. |
| 5 | **Small crash landing:** ship plus the provisional crew (6/4/1 per faction); no idle army or prebuilt base. Stress fixtures unchanged. | New games start with exactly that crew; fixtures still use full counts. |
| 6 | **Planet briefing:** pre-landing knowledge from real generation data (biomes, seasons, daylight, broad features); unknowns stay unknown; satellite stated overhead and matching real state. | Every figure matches generation metadata; satellite agrees with recon. |
| 7 | **Paused start:** new games start paused; after the first notice show exactly "What is the build plan immediately after landing?" with a Play reminder and Pause/1x/2x/4x explanation. Closing notices or changing speed never unpauses. Planning works while paused. No replay on load; modals send no world commands. | After both notices the clock reads tick 0; speed change stays paused; modal input issues no commands. |

---

## 9. NEXT STEP

1. Work from `main`; it already contains the codex/opening-hud-recon code (4c6794c) plus A-039. Do not restart from older checkpoints (rule 5.13).
2. Finish the pinned Windows CI compilation/regressions. Inspect production-renderer captures at 1366/1920/2560 widths and resolve actual failures.
3. Run the overlay/source identity and user-data preservation audits, seal matching Master/Ledger records, and deliver the update plus dated complete Source zip. Synchronize Library copies when available.
4. Keep all unmeasured 1.0/performance scope open; do not run a long speed suite for this UI correction.

---

## 10. OPEN DECISIONS FOR TOR

| ID | Decision needed | Default if unanswered |
|---|---|---|
| O-1 | Library read-back test: can a brand-new chat open the saved files, and is an old file kept when one is replaced? | Dated, never-overwritten names (rule 5.6). |
| O-4 | Additions to the 1.0 list (section 11). | List stays as is. |
| O-5 | Naval roster, climate curves, scoring weights, smaller-world population defaults. | Not blocking; decide when reached. |
| O-6 | How notes reach the AI: chat, a notes file in `Send-Back.zip`, or a dev-only in-game Note button. | Chat plus `assets/icons/NOTES.md`; the AI may build the dev Note button as a reversible dev tool. |

---

## 11. 1.0 GATE (living list; A-028)

Tick an item only with evidence (rule 5.1). Everything here stays in scope even when deferred.

| Item | Status |
|---|---|
| 20 x 20 km world with streaming and fine terrain pages | planned |
| Map types, river/mountain controls, 5/10/20 km presets | planned |
| 10,000 military + others at the A-009 goals | unmeasured |
| Complete mandatory biology | partial fixtures |
| All 160 trait mechanisms (30 fixture-tested, 130 planned) | partial |
| Ecology, food webs, research, conservation, breeding, release, extinction, Ecology tab | components tested on a small fixture |
| Causal ecological damage scoring and victory rule | partial |
| Naval combat, bridges, docks, transports | initial rules only |
| Cameras, radar, satellite | partial; section 8 in progress |
| Terrain, water, erosion, seasons | fixtures tested |
| Shipped RTS UI, tutorials/onboarding, audio | partial / planned |
| Long-war saves and balance | unqualified |
| Native click-to-update | planned |
| Weaker-hardware optimization and measurement (deferred) | unmeasured |

---

## 12. KEEPING THIS FILE SMALL OVER HUNDREDS OF UPDATES

- **Every approval and decision goes to the Ledger only**, as one line with an ID and tags. Change this file only when core scope (section 4), a rule (section 5), the active task, open decisions or the 1.0 list actually change.
- Section 1 is overwritten, never appended, within 1,500 characters.
- **Ledger rollover:** at each milestone, freeze `project/LEDGER.md` as `project/archive/LEDGER-<milestone>.md` and start a fresh `LEDGER.md` with the same index and format. Searches cover both.
- **Compaction** at each milestone or every 10 builds: remove resolved items, rewrite section 7 to current truth, refresh section 2. Bump the revision (M6, M7...) and save a copy with the Ledger and commit hash into `project/snapshots/`.
- Do not log minor fixes or refactors beyond one build-log line.
