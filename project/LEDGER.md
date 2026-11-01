# LAST WORLD: LEDGER (append-only)

**TOKEN RULE: never read, print or parse this whole file without Tor's explicit approval.** Search by ID or tag and read about 50 lines at most per lookup. Example: `grep -n "#ui" project/LEDGER.md project/archive/LEDGER-*.md`. **The latest line mentioning an ID is its current status.**

Active file: `project/LEDGER.md` (Milestone B). Frozen ledgers: `project/archive/LEDGER-<milestone>.md`.

## TAGS (the index; add a tag only when a new topic appears)

| Tag | Covers |
|---|---|
| #process | rules, workflow, Library/repo, handoff and ledger policy |
| #scope | 1.0 list, maps, workload, traits, naval, victory |
| #time | game speed, seasons, clock |
| #world | terrain, water, erosion, map generation |
| #ecology | biology, food webs, research, conservation, extinction |
| #opening | crash-landing opening, crew, briefing, pause flow |
| #ui | HUD, icons, selection, cameras, palette, fonts |
| #perf | speed tests, frame time, hardware, weaker-PC work |
| #risk | risks and blockers |
| #build | build log |

Last entry: L-0018. Last approval: A-036. Last build: B-002.

## Formats

Approval or decision: `A-nnn | date | status | text | #tags | source`
Statuses: APPROVED, PROVISIONAL, PROPOSED, SUPERSEDED (by A-nnn), REJECTED, ASSUMPTION.
Event: `L-nnnn | date | type | summary | #tags`
Build: `B-nnn | date | commit | exe hash | change | tests | verified on Tor's PC Y/N | #build`

---

## Approvals (A-001 to A-022 consolidated from the Oct 29-31 decisions; dated history in ARCHIVE `[LW045]`-`[LW069]`)

A-001 | 2026-10-29 | APPROVED | Native PC game; actual-PC testing priority; browser only for tiny checks; phone deferred. | #scope | [LW050]
A-002 | 2026-10-29 | APPROVED | Large map 20x20 km; Continental/Archipelago/Large Land Mass/Medium Islands with river/mountain controls; presets 5x5 km (45-60 min) and 10x10 km (90-120 min); targets, not cutoffs. | #scope #world | [LW050] [LW055]
A-003 | 2026-10-29 | APPROVED | 10,000 military PLUS non-military pawns and wildlife; large war about 3-4 hours. | #scope | [LW050]
A-004 | 2026-10-29 | APPROVED | Qualification fixture: 10,000 military + 2,000 non-military + 5,000 detailed wildlife + about 1,000,000 cohort wildlife + 256 species. Test load, not a cap. | #scope #perf | [LW055]
A-005 | 2026-10-29 | APPROVED | Initial 30 mechanisms (11,12,46,61,64,65,67,72,75,84,86,93,96,101-107,121,125,128,129,132,135,143,156,157,159); all 160 plus mandatory biology in final scope. | #scope #ecology | [LW055]
A-006 | 2026-10-29 | APPROVED | Selectable 1x/2x/4x from the first build; report requested vs achieved speed; no 4x guarantee. | #time | [LW055]
A-007 | 2026-10-29 | APPROVED | One season per game hour at 1x (72,000 ticks at 20 Hz); biome-specific effects; species-specific aging compression. | #time #ecology | [LW055]
A-008 | 2026-10-29 | APPROVED | Combat navy plus bridges, docks and troop/cargo transports; roster TBD. | #scope | [LW055]
A-009 | 2026-10-29 | APPROVED | Development goals: median 16.67 ms, p95 25, p99 50, flag max >100 ms; RAM 4 GiB; GPU payload 2 GiB on 8 GiB card; save about 256 MiB, load 15 s. Goals, not measurements. | #perf | [LW055]
A-010 | 2026-10-29 | APPROVED | Water flows/accumulates; dams pool gradually; erosion, deposition, foliage wash-out; brown bed darker at depth; delayed drying after air exposure. | #world | [LW056] [LW057]
A-011 | 2026-10-29 | APPROVED | Ecological stalemate: more damaging faction loses; natural erosion is not faction harm. | #scope | [LW055]
A-012 | 2026-10-29 | APPROVED | Image generation and Python primitive-shape stamping each need Tor's explicit approval. | #process #ui | [LW051]
A-013 | 2026-10-29 | APPROVED | Overlay update folder merged into the existing install preserving saves/settings/logs; one Run-Checks.cmd and one Send-Back.zip; batch work. | #process | [LW059] [LW066]
A-014 | 2026-10-30 | APPROVED | Shipped RTS UI per docs/UI_REQUIREMENTS.md; same renderer; Rajdhani SemiBold + IBM Plex Sans; UI owns projection/preferences only. | #ui | [LW062]
A-015 | 2026-10-30 | APPROVED | Continuous seeded natural palette (greens/browns/reds/yellows); stable animal coats. | #ui | [LW067] [LW068]
A-016 | 2026-10-30 | APPROVED | Sub-agents only for easy, tedious or repetitive work. | #process | [LW066]
A-017 | 2026-10-29 | APPROVED | Progress percentages refer to the current task only. | #process | [LW052]
A-018 | 2026-10-31 | APPROVED | Do not ask to use Tor's computer. | #process | [LW069]
A-019 | 2026-10-30 | APPROVED | Hardware tests collect read-only CPU/RAM/GPU activity; no artificial VRAM filling. | #perf | [LW065]
A-020 | 2026-10-30 | APPROVED | One gradual adaptive speed ramp plus one 20-minute 4x visible battle. | #perf | [LW064] [LW066]
A-021 | 2026-10-29 | APPROVED | Milestone B authorized and active. | #process | [LW055] [LW061]
A-022 | 2026-10-31 | APPROVED | Opening/HUD/recon requirements 1-7 (Master section 8). | #opening #ui | [LW069]
A-023 | 2026-10-31 | APPROVED | Master file replaces RULES/STATE/DECISIONS/BLOCKERS/PARITY as the working instructions. | #process | chat
A-024 | 2026-10-31 | SUPERSEDED by A-031 | Source retained in Git in a neat hierarchy. | #process | chat
A-025 | 2026-10-31 | PROVISIONAL | Opening crew 6 builders + 4 defenders + 1 camera trooper per faction, as a test; revisit after playtesting. | #opening | chat
A-026 | 2026-10-31 | APPROVED | UI icons are hand-authored pixel maps stored as data in the source. | #ui | chat
A-027 | 2026-10-31 | APPROVED | Weaker-PC optimization and testing deferred until Tor chooses; still in scope; low-end stays UNMEASURED. | #perf | chat
A-028 | 2026-10-31 | APPROVED | 1.0 means the Master section 11 list; it may grow. | #scope | chat
A-029 | 2026-10-31 | APPROVED | Handoff (Master, Ledger, archive) lives inside the source under project/ and ships with every Source zip. | #process | chat
A-030 | 2026-10-31 | APPROVED | Whole Ledger/Archive is never read or parsed without Tor's approval; use tag/ID search. | #process | chat
A-031 | 2026-10-31 | APPROVED | ChatGPT Library is the source of truth; Git is a secondary mirror. SUPERSEDES A-024. | #process | chat
A-032 | 2026-10-31 | APPROVED | Clarifies A-026: pixel maps = pixel plotting and hexadecimal mapping; hex rare in the game. | #ui | chat
A-033 | 2026-10-31 | APPROVED | Source-tree layout approved; moves only as one dedicated task, by default after the opening/HUD task. | #process | chat
A-034 | 2026-10-31 | APPROVED | Preview-only tool that enlarges the AI's own pixel data into a PNG; creates no art. Scope limited by A-036. | #ui | chat
A-035 | 2026-10-31 | APPROVED | Default autonomy: a request or notes produce an updated game; ask only the genuine questions defined in Master rule 5.0. | #process | chat
A-036 | 2026-10-31 | APPROVED | Clarifies A-034: the preview tool is used only to fix issues Tor calls out. Otherwise the AI runs the icon protocol on its own and puts icons in the game without an approval gate. | #ui | chat

## Events

L-0001 | 2026-10-31 | DECISION | Earlier approvals consolidated as A-001 to A-022. | #process
L-0003 | 2026-10-31 | RISK | Interrupted session's HUD/opening edits not recoverable from head 1177730b. Rule 5.6 added. Performance risk: 1x/2x sustained, 4x not. | #risk #perf
L-0018 | 2026-10-31 | DECISION | Master revised to M5: approvals moved from Master to this Ledger; Master holds core scope only; tag index replaces ID lists; ledger rollover per milestone; rule 5.0 and rewritten 5.12. Earlier interim entries L-0002, L-0004 to L-0017 are folded into A-023 to A-034 above. | #process

## Builds (historical, from ARCHIVE)

B-001 | 2026-10-30 | (see ARCHIVE) | native 7f903735 / exe 3b09afc2 | repaired mini candidate | 237 checks | Y (PC run 22544: 254/257 phases) | #build
B-002 | 2026-10-30 | parent of 1177730b | native 3c9147c8 / exe 41dbfc9d | broad natural procedural palette | 253 checks | N (visual review pending) | #build


## Events continued (organization pass)

L-0019 | 2026-10-31 | ORGANIZATION | Reviewed the supplied M5 Master and adopted its Master/Ledger/frozen-archive structure for project records; M6 adds compact navigation and updates the session/current-record status. Original M5 inputs and M6 outputs retained in snapshots; indexed main and one-time handoffs frozen byte-for-byte in archive. Assumption: this request covers project-record organization; game-source path moves remain a separate task after opening/HUD per section 6. No game code, dependency, approval, save schema or build changed. | #process
L-0020 | 2026-10-31 | RISK | Standalone Library read/write tools are unavailable in this Codex session. Organization files and ZIP are staged local deliverables only, not Library-saved and not a complete Source/game update. O-1 remains unresolved; merge project/ into the newest verified complete Source zip and save matching standalone Master/Ledger before treating this revision as durably saved under rule 5.6. | #process #risk

Last entry: L-0020. Last approval: A-036. Last build: B-002.


## Source-local records requirement

A-037 | 2026-10-31 | APPROVED | Tor explicitly requires Master and Ledger in the actual game source directory, retained in every future version; amend contradictory Master wording. Canonical paths are Source/project/MASTER.md and Source/project/LEDGER.md. Clarifies A-029/A-031: Library checkpoint copies do not replace the source-local working records; Library unavailability does not block preserving them in source. | #process | current chat
L-0021 | 2026-10-31 | ORGANIZATION | M7 prepared for source-local installation; update packager whitelists the project tree and requires nonempty Master/Ledger; package audit enforces the same. Placement pending filesystem write grant/read-back; focused checks pending. Previous M5/M6 snapshots and frozen handoffs retained. No gameplay code or native compilation input changed; no new build or Library save. | #process

Last entry: L-0021. Last approval: A-037. Last build: B-002.


L-0022 | 2026-10-31 | VERIFIED | Installed and byte-read-back verified project/MASTER.md (M7), project/LEDGER.md, archive and snapshots in E:\Games I am Making\Last-World\Source; installed package-windows.py, check-overlay-package.py and test-overlay-package.py updates. Thirteen focused package/identity/governance checks and five actual source_snapshot fixture cases passed; temporary-directory ACL errors from initial attempts retained, then runner used retained workspace fixtures. Native compilation-input SHA remains 3c9147c8a52073d64788d521c8325c57a614a1f337ada31d0d706522f8686e1e. Resolves the local-placement portion of L-0020/L-0021; Library synchronization/O-1 remain open. No game rebuild or performance measurement. | #process

Last entry: L-0022. Last approval: A-037. Last build: B-002.


## Superseded governance cleanup

A-038 | 2026-10-31 | APPROVED | Tor requested removal of unneeded files covered by Master/Ledger. Remove superseded root RULES.md, STATE.md, DECISIONS.md, BLOCKERS.md and PARITY.md after verifying their full contents remain in the indexed archive; retain unique designs/evidence/source/history. Master and Ledger remain source-local in project/. | #process | current chat
L-0023 | 2026-10-31 | CLEANUP | M8 cleanup staged; all five old root records verified in archived handoff, with hashes/LW locations recorded. Prepared compact root README, frozen original browser README, updated current pointers and packaging/audit rules. No native compilation inputs or gameplay changes. Source installation and focused checks pending. | #process

Last entry: L-0023. Last approval: A-038. Last build: B-002.


L-0024 | 2026-10-31 | PACKAGE | Prepared complete cleaned Source snapshot M8 in the writable workspace, removing five root governance files whose entire text is already verified in the indexed archived handoff. New compact root README preserves its prior unique text in archive. Fourteen focused checks plus five source-snapshot cases passed; packaging/audit use Master/Ledger and reject obsolete root records. The original E:\Games I am Making\Last-World\Source was not modified because the requested filesystem write grant was not granted; M8 cleanup there remains unapplied. Library synchronization remains unavailable. Same-day records-only Source ZIP uses -M8 suffix; game build remains B-002 with native inputs unchanged. | #process #risk

Last entry: L-0024. Last approval: A-038. Last build: B-002.

L-0025 | 2026-10-31 | SOURCE_DRAFT | Resumed all seven opening/HUD/recon requirements from saved mirror 9b89dc3; missing edits recreated on codex/opening-hud-recon. Draft includes separate 6/4/1 crew opening, paused notices, unit task/loadout/history/role commands, legacy-compatible camera variants, authored pixel-map icons and saved optical terrain/object memory. Source-local records retained; pinned Windows CI and focused regressions/capture example added. Local icon checks passed; native compilation, executable/package and visual checks pending. Assumptions: keep legacy camera as pole; preserve existing compressed biological day (12h normalized daylight/24h, 60s/120s at 1x). Library archive download unavailable; no installed-game access requested. | #opening #ui #process #risk

Last entry: L-0025. Last approval: A-038. Last build: B-002.

L-0027 | 2026-10-31 | CHECK_FAILURE | First feature build 69365df compiled on Windows; cargo library run passed 249 and failed one old 19-kind material-registry assertion after the three appended camera variants (22 kinds). Updated the check to cover all 22 and explicitly preserve the old camera identity/order. Failure log retained in CI run 41270005009. The newer sparse-memory candidate still requires its own checks. | #ui #build #risk

L-0026 | 2026-10-31 | CI_CANDIDATE | Locked Windows compilation and library/CLI checks passed ({'cli': 5, 'library': 253}); authored icon data checked; 12 production-renderer frames captured at 1366/1920/2560 widths with paused tick0. First overlay identity/preservation audit passed; matching records resealed before final audit/upload. Visual inspection and Tor-PC performance remain pending/unmeasured. CI run 41271604718; commit 7296753d274452e003e4bc11dde6154952e2875f. | #opening #ui #build
B-003 | 2026-10-31 | 7296753d274452e003e4bc11dde6154952e2875f | 234c03baf9c5ad10b8dec0a6e18e0e49218a79e0c53083216bbcbb69938f97d3 | opening/HUD/recon candidate | CI library/CLI + icon-data + production capture + first overlay audit; final sealing audit required | N | #build

Last entry: L-0026. Last approval: A-038. Last build: B-003.

A-039 | 2026-11-01 | APPROVED | Tor: the game must be intuitive; restore lost road-planning feedback (see the road shape while drawing, know whether it was planned) and stop features disappearing between versions (camera variants had to be re-added before). Implement placement outlines/feedback and add rule 5.13 (never drop a working feature; regression tests; work from newest main; merge branches back). | #ui #process | chat
L-0028 | 2026-11-01 | SOURCE_CHANGE | Root cause: road preview existed only in the retired browser build ("R draws roads"); the native port kept the drag but drew nothing and planned orders stayed invisible until built (status alert only, priority 0). Lost camera variants: work done on codex/opening-hud-recon was never merged; Tor's manual upload to main (4c6794c) carried code but dropped .github/ CI, so sessions resumed from mixed baselines. Changes: live line outline with length/cost/refusal reason, cursor site footprints, persistent planned outlines (dashed, solid built portion, red when blocked), white/red accept/refuse flash, builder alert (priority 1), line tools stay armed (Esc/right-click ends), HUD refresh on camera change, GPU quad budget guard; status text now '<Kind> planned: N m (order id)'. Regression tests named in Master section 7. CI workflow restored on main. Local compile impossible (crates.io blocked in session); evidence is the Windows CI run on the pushed commit. Assumption: keeping line tools armed after release is wanted. | #ui #process #risk

Last entry: L-0028. Last approval: A-039. Last build: B-003.
