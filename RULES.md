# LAST WORLD: HARD RULES

These rules override your own preferences, shortcuts and anything you "remember" from earlier in a conversation. Conversation memory degrades. These files do not. When in doubt, re-read them.

Owner: Tor. Project: Last World (Rust, Bevy, wgpu). Main design source: "Last World Corrected Architecture and Ecology Handoff" (the Handoff).

---

## 0. FILES YOU MUST KEEP (in the repo root)

| File | Purpose | Rule |
|---|---|---|
| `RULES.md` | This file | Never edit without Tor's written approval |
| `STATE.md` | Current truth: what exists, what works, what is next | Rewrite at the end of EVERY work session |
| `DECISIONS.md` | Every decision Tor made, with date | Append-only. Never delete or reword old entries |
| `BLOCKERS.md` | Open questions and known failures | Remove an item only when resolved, noting how |
| `PARITY.md` | Checklist of features from the browser reference and the 160 traits | Mark each: planned / implemented / tested / measured |
| `console.txt` | Append-only game log | Never truncate or rotate away |

If any of these is missing, create it before doing anything else.

## 1. SESSION PROTOCOL

**Start of every session (and after any long pause or when the conversation gets long):**
1. Read `RULES.md`, `STATE.md`, `DECISIONS.md`, `BLOCKERS.md`, in that order.
2. Reply with a 5-line summary: current milestone, last completed task, next task, open blockers, any rule you are at risk of breaking.
3. Do not start work until you have done this.

**During a session:**
- Work on ONE task at a time. State the task in one sentence before starting.
- Re-read `RULES.md` before each new milestone and at least every 10 tasks.
- Commit to Git after every task that builds. Message format: `[milestone] what changed`.
- If the conversation feels long, or you notice you are unsure about an earlier decision, STOP and re-read the files. Do not guess from memory.

**End of every session (or when asked for a handoff):**
1. Update `STATE.md`: what exists, what is tested, what is measured, what is broken, exact next step.
2. Append any new decisions to `DECISIONS.md`.
3. Make sure the project builds, or record exactly why it does not.
4. Give Tor a handoff message of at most 15 lines that a new AI with zero context could act on, pointing to the files above.

## 2. TRUTHFUL REPORTING

Use only these four status words, and use them precisely:

- **planned**: written down, no code
- **implemented**: code exists and compiles
- **tested**: a check ran and passed; name the check
- **measured**: numbers recorded on named hardware; give the numbers

Rules:
- Never say "done", "complete" or "works" without naming the evidence.
- Never claim an FPS, pawn count, speed or memory result you did not measure on this project's real workload.
- Software-renderer (llvmpipe) results are not performance evidence for real hardware.
- A benchmark, demo or isolated simulator is not the game.
- Report failures as plainly as successes. Keep failed runs.

## 3. SCOPE PROTECTION

- Do not remove, shrink or silently postpone any requirement in the Handoff. All 160 trait mechanisms stay in scope.
- Re-ordering work is allowed (vertical slice first). Dropping work is not.
- If something is too hard or too slow, tell Tor, explain the cost, and offer options. Tor decides.
- Never reduce pawn counts, fake populations, add hidden prey spawning, set population floors, or regenerate extinct species.
- Do not invent requirements. Do not turn "proposed" values in the Handoff into approved ones.
- Proposals (hardware tiers, map sizes, speed caps, cadences) stay marked PROPOSED in `DECISIONS.md` until Tor approves them.

## 4. ENGINEERING INVARIANTS (never break these)

1. **One owner per quantity.** Each animal, stock, resource or item is owned by exactly one place. No duplicated populations, cargo or biomass.
2. **Integer counts are exact.** Births, deaths, transfers and migrations conserve totals. Use checked arithmetic. No negative counts, no silent clamping.
3. **One game clock.** A single integer tick counter drives everything. Frame rate must never change game outcomes.
4. **Unseen does not mean frozen.** Offscreen ecology and combat keep progressing at coarse resolution.
5. **Information filtering.** Cameras, radar and satellite limit what the player sees. No UI, inspector or tooltip may leak hidden positions, species or health.
6. **CPU is authoritative for gameplay rules first.** GPU is for rendering and bulk work, and only becomes authoritative after end-to-end validation.
7. **No full-world fine-resolution allocations.** Use chunks, pages and sparse data. Track memory high-water marks.
8. **Stale jobs are rejected.** Background results must carry revision stamps and be validated before commit.
9. **Saves are sacred.** Never break an existing save without a migration. Use atomic writes and keep the previous save.
10. **Extinction is permanent** unless real surviving organisms or viable stored stages exist.
11. **Every trait has a real effect, cost and test.** No label-only traits.
12. **Determinism.** Same generator version, seed and settings must give the same accepted world.

## 5. HARDWARE AND PERFORMANCE RULES

Tor's confirmed development PC:
- CPU: AMD Ryzen 9 3900X (12 cores, 24 threads)
- RAM: 32 GB
- GPU: NVIDIA GeForce RTX 2080 SUPER, 8 GB VRAM, driver 32.0.16.2101, DirectX 12 (FL 12.1)
- OS: Windows 10 Pro build 19045

Rules:
- The game must also run on weaker computers. Tor's PC is NOT the baseline. It is the top tier.
- Size worker threads from available parallelism, never from a hardcoded number.
- Size GPU caches and textures from the adapter's reported limits.
- Provide a low-detail path that keeps gameplay semantics identical (fewer visual effects, smaller caches, lower resolution), never fewer rules.
- Hardware tiers are PROPOSED until Tor approves them. See `DECISIONS.md`.
- Limiting CPU cores, memory or VRAM budgets on Tor's PC, and running on a software (llvmpipe) renderer, are only rough checks for crashes, budget overruns and scaling behavior. They are NOT evidence of how the game performs on a weaker PC. Weaker real hardware differs in per-core speed, cache, memory bandwidth, drivers and GPU architecture.
- Never describe a core-limited run or a software-renderer run as a "floor", "minimum spec result" or "low-end result". Label it "proxy check" and name exactly what was limited.
- Claims about low-end performance require a measurement on real low-end hardware that Tor names. Until then, report low-end performance as UNMEASURED.
- Pawn counts, map sizes and speed caps for each tier are PROPOSED numbers until Tor approves them and they are measured on real hardware.
- Report frame time as median, p95, p99 and max, not average FPS alone.
- Never run heavy builds while recording a performance measurement.

## 6. WORKING STYLE

- Build on the existing Last World project. Do not start a new one or create a second renderer.
- Prefer simple, concrete models that deliver the promised effect. No speculative frameworks.
- Keep the game playable at the end of every milestone, even if small.
- Do not ask Tor for permission on routine, reversible work. DO ask when a decision changes scope, hardware targets, game balance or saves.
- When blocked, write the blocker to `BLOCKERS.md`, then continue with independent work.
- Do not change pinned Bevy or wgpu versions unless a concrete need is shown and Tor approves.
- Never copy secrets, tokens or credentials into any file.
- Do not create a duplicate Site or change its audience.

## 7. SELF-CHECK BEFORE EVERY REPLY TO TOR

Ask yourself:
1. Did I read the state files this session?
2. Did I claim anything without evidence?
3. Did I shrink, skip or rename a requirement?
4. Did I break an invariant in section 4?
5. Is `STATE.md` current?
6. Would a new AI with no memory understand where we are from the files alone?

If any answer is wrong, fix it before replying.

## 8. VISUAL GENERATION: TOR'S APPROVAL REQUIRED

1. **Any and all use of the image generation skill or image generation tools requires Tor's explicit approval before use.** This includes generating or editing images, sprites, textures, concept art, mockups and marketing images. General authorization to develop the game does not authorize image generation.
2. **Any and all use of Python to assemble crude primitive shapes and stamp them as sprites or other game visuals requires Tor's explicit approval before creating or using them.** Expect these proposals to be declined in almost all cases. Ask only when the approach would save significant resources and time while preserving the established look; explain the intended asset, technique, expected savings and visual tradeoff, then wait for approval. Do not generate a stamped example before approval. Continue to follow the existing no-shape-stamps requirement unless Tor explicitly approves the proposed exception.
