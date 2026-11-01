# Last World UI requirements — Tor, 2026-10-30

Tor's latest steering overrides the earlier step-by-step approval stops: make sensible decisions, review style/mockups internally, continue toward Milestone B while he is away. No approval stop after each UI step. Image-generation and Python primitive-stamp artwork approval rules remain protected. Ignore suggestions that would change the rendered world graphics; this document governs UI only.

## Product identity

Shipped RTS feel; no developer-console, stacked dropdowns, default system fonts, text-heavy settings panels, admin dashboard or spreadsheet appearance. Rugged improvised faction field terminal: worn metal, cables, hazard markings, riveted plates, restrained scanlines/status lights. Industrial frames contrast with lush biology. One coherent pixel-material frame/button/icon treatment, procedural grain/wear/edge highlights. Distinct faction accents/trim readable without color: icon/shape/pattern cues. No plain-circle icons or letters in boxes. Icons depict the actual depot/camera/etc, using existing procedural assets where applicable. Bundle deliberate open-licensed heading and body font families and licenses; tabular legible numbers.

## RTS behavior

Map dominates, default panels hug edges and cover a small fraction. Three detail levels: glance icons/bars/numbers, hover/select compact facts, click/hotkey full closable/pinnable/movable panel. Every full panel one click from its glance control, detail at most two clicks. Command card large icon/hotkey grid, categorized build grids, costs on hover; dropdowns rare settings only.

Mouse/keyboard first: common hotkeys displayed; box select, Shift queue, control groups, camera bookmarks, edge scroll, double-click similar; context right orders and contextual/radial object actions where useful. No unit text labels. Clear selection outlines; health/status bars selected/hovered only; group badges; portrait/key stats/order/queue selection panel.

Compact resource topbar with real stock, rate/arrow/warning; click logistics flows/stockpiles/bottlenecks. Object construction progress and icon queue strip; shortage icon plus reason on hover. Prioritized clickable camera-jump alerts, merged spam/decay, never log wall. Contextual once-only disableable hints and searchable in-game help.

Corner minimap: click navigation, faction colors, camera box, alerts, anonymous radar contacts, satellite footprint; zoomable/resizable. Small toggle layer bar with one-key layers: radar, cameras, roads, supply, mines/resources, ecological health, faction damage, known fauna and territory. Semitransparent; do not obscure map/units or leak hidden info.

## Ecology

Persistent Ecology panel/hotkey, filtered species list and procedural previews; unidentified silhouette and question fields progressively researched. Glance name/status/trend; expanded traits/habitat/relationships/research/threats/captive stock/protection. Food-web pan/zoom/click portrait graph with known edges/nodes only. Conservation icons use shape plus color. Damage scoreboard both factions with breakdown; filter everything through observed/researched knowledge. Never expose unseen locations, hidden species, exact unobserved counts, or live enemy details. UI is read-only projection and sends ordinary game commands, never owns gameplay quantities.

## Layout/accessibility/performance

Clean at1366x768,1920x1080,2560x1440,4K; UI scale75–200%, saved user layout, collapsible/movable/pinnable panels. Body>=14px1080p; high contrast.44px hit targets at100%; no-hover-compatible architecture, but no phone UI implementation now. Text labels on hover, noncolor cues, visual/subtitle sound alerts, reduce-motion (skip purposeful tiny press/slide/alert animation). All palette/fonts/spacing/frames/icon refs in one theme file. Cached layout/drawing for unchanged panels, bounded animation, no per-unit text. State and measure UI budget.

## Process/evidence

Style direction, then six static mockups, then native shell, then evidence/screenshots and panel iteration. Approval pauses waived by Tor's later instruction. Required mockups: calm HUD, battle alerts, build grid, expanded species, food web, logistics. Produce SVG/HTML or images; no imagegen without approval. Real native screenshots at1080p/1440p/smallwindow after UI work; if unavailable, say so, never substitute mockups as actual build evidence.

Quality review: road/select/camera discoverable in10seconds, essentials only bydefault, no console/form aesthetic, coherent material/icons, depth≤2clicks, filtered knowledge, text/contrast/noncolor readability. Current legacy bitmap test HUD is not final UI acceptance.

## Chosen implementation direction

Custom cached wgpu UI on the existing device/renderer, preserving current engine pins and world assets. Graphite frame/panel colors #171D1F/#263034, edge #65716D, warm text #EEE9D8, muted text #ADB8AD; faction0 amber #E9B35A with double-notch/hazard trim, faction1 blue #67B7E1 with stepped/ribbed trim. Damage #E57D70, biology #83C991; all states also use icons/pattern/text. Heading Rajdhani SemiBold; body IBM Plex Sans Regular/Medium; bundle official OFL files. Font sources/license verification: official google/fonts ofl/rajdhani and IBM/plex repositories.

Provisional UI budget at1080p: CPU projection+layout/drawing<=1ms steady and GPU<=1ms; unchanged HUD does not relayout/rasterize. These are targets, not measurements. Real device evidence will be collected with the combined PC batch, no new renderer.
