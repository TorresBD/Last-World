# Last World — native UI direction

Status: UI design reference for the Milestone B shell. Static mockups are labelled design examples, never native execution evidence. Tor's current UI requirements and approval-pause waiver are recorded in UI_REQUIREMENTS.md. Existing world rendering, procedural ground/foliage and physical sprites remain unchanged by this work.

## Identity and material

An improvised faction field terminal frames a living alien jungle. The map stays the main surface. Small metal plates at the edges carry essential controls; a selected subject or deliberate button opens detail. Avoid a central dashboard, log wall, permanent encyclopedia or settings form.

Frames use seeded, subtle metal grain, a lighter upper edge, a dark inset, a few asymmetrical scratches and clipped/riveted joints. Small hazard strips mark active faction controls. Texture must remain quiet behind body text. Corners have a consistent 6–8px chamfer and 1px highlight; press state moves the inset by 1px, with a separate focus outline. This is UI material only, not new game artwork. Do not apply blur or grain filters over the world.

| Token | Value | Use |
|---|---|---|
| Frame / inset | #171D1F / #20282B | Frame recess and panel body |
| Raised / border | #263034 / #65716D | Buttons, edge highlights and separators |
| Text / secondary | #EEE9D8 / #ADB8AD | Main body and explanatory text |
| Faction0 | #E9B35A | Amber plus double-notch/hazard trim |
| Faction1 | #67B7E1 | Blue plus stepped/ribbed trim |
| Biology / damage | #83C991 / #E57D70 | Leaf/shield or warning/strike cues plus text |
| Pending / disabled | #B9A66D / #7D8983 | Clock/queue or dimmed asset with explicit reason |

Faction identity and alerts must remain readable in monochrome: faction0 repeats two diagonal cuts; faction1 repeats three perpendicular ribs. Warning indicators have an angular warning glyph, text and priority ordering; a color change alone is insufficient.

## Fonts and hierarchy

Bundle native/assets/ui/Rajdhani-SemiBold.ttf for headings and numerals (Rajdhani,600), and IBMPlexSans-Regular.ttf for body text (IBM Plex Sans,400). Their actual OFL files are alongside them: Rajdhani-OFL.txt and IBMPlex-OFL.txt. These are the exact files used in the mockup rendering configuration and native glyph work; no system-font substitution is intended.

At1920×1080: headings22–26px, body16px, secondary14px, topbar numbers22px. At1366×768: headings20–22px, body14px minimum and compact detail layouts. At2560×1440 the reference uses1.2× sizes. Numeric fields use tabular spacing. Hotkeys sit beside or below the actual icon, never replace it with a letter in a box. Keep all primary targets at least44px at100%; UI scale75–200% remains the native implementation contract.

## Real icon and portrait sources

Use existing native/assets/pixel-atlas.png and its pixel-atlas.json crop rectangles. Depot, camera, road, trench, wall, mine, solar, barracks and radar buttons depict those grown assets. Unit and fauna portraits reuse the existing grown pixel materials. Scale with nearest-neighbor sampling. Unknown subjects use a dark silhouette of the same existing material, a textual “Unidentified” title and hidden fields. Do not invent a new animal or building illustration for the UI.

Static mockups reuse the actual start capture at evidence/pc-coupled-2026-10-30/returned/river-check-17376-1793326377221-start.png as the unmodified world backdrop. Values, graphs, selected subjects and alerts are illustrative layout data and are labelled as such. Minimap elements and selection/focus/graph glyphs are UI geometry, not physical sprites.

## Default layout and interaction depth

- Top-left: compact resources with stock, rate and shortage cue. Clicking opens Logistics directly.
- Top-right: faction, elapsed season and1×/2×/4×/pause controls, then menu/help. Keep the default status terse.
- Bottom-left: minimap, camera box and small layer strip. A portrait/icon-led Ecology control stays visible with its hotkey.
- Bottom-center: compact selection portrait, key condition, current order and queue. No unit labels on the world.
- Bottom-right: large icon command card. Road, camera and select/order are discoverable in the first glance. Build category opens one icon grid; item hover supplies costs. Shift queue and hotkeys are visible hints.
- Right edge: at most a few prioritized merged alerts. A click centers the camera; a detail action opens the relevant compact panel. The battle example expands alerts, not a scrolling log.

Full detail is always deliberate, closable and pinnable. A glance control opens its relevant panel in one click; individual facts/relationships stay within one more click. Movable/resizable panels and saved user layout are implementation requirements. At1366px, bottom selection compresses before the map is covered by full-height permanent sidebars. Deep panels use tabs/scrolling inside the panel; body text is never scaled below14px to force a fit.

## Six static scenes and evidence boundary

1. Calm HUD: default map-dominant edge layout.
2. Battle alerts: merged actionable warnings with a camera-jump affordance.
3. Build grid: icon-led construction categories, real object icons and hover costs.
4. Expanded species: portrait, known status, research, traits, habitat and conservation actions; unobserved population remains unknown.
5. Food web: pan/zoom-style known portrait nodes and known directional edges. Hidden species/edges remain absent, not guessed.
6. Logistics: depot/supply portraits, current bottleneck, flow and actual-stock layout; no spreadsheet wall.

The source generator writes1920×1080 masters, adaptive1366×768 and2560×1440 variants. The HTML gallery can switch scene/size and exposes source SVGs. Rendered PNGs are design previews generated from those SVGs; they do not prove native controls, renderer speed or gameplay. Actual native screenshots and measurements must come from the combined PC batch.

## Internal acceptance before native shell work

Check the calm layout for road/select/camera discoverability,44px target sizes, small default coverage, text contrast and strong map area. Check each deliberate panel for legible body text, one clear primary action, a close/pin affordance and no hidden-state leak. Compare every preview at1366/1920/2560 rather than shrinking a single fixed design. Confirm that every physical-looking icon comes from the existing atlas. Accessibility and reduce-motion keep noncolor status/focus cues.

Mockup material filters are a design reference; native implementation should bake/cache quiet material panels and glyphs, not rerun expensive procedural filters per frame. The provisional1ms CPU and1ms GPU UI budgets are targets, not measurements. Unchanged HUD data/layout should reuse cached geometry/rasterization.

## Internal review result

Six scenes now have18SVG/PNG variants and a portable HTML gallery at ui-mockups/index.html. Sharp/libvips/librsvg rendered the previews using the exact bundled fonts; Chromium is unavailable here. Manual PNG inspection and source geometry checks found and corrected small atlas footprints, narrow layer targets, primary-button text overlap, an obscured evidence label, and an incorrect food-web relationship/arrow endpoint. Existing icon crops were tightened to their original alpha bounds, without editing the physical art.

The final layout records have no out-of-bounds panels and no declared action region below44px. Calm bounds-summed panel coverage is20.5%/16.4%/13.3% at1366/1920/2560. Body text is14/16/19.2px. The small layout keeps the persistent Ecology command while hiding its duplicate glance plate. These are static design checks, not native runtime measurements or proof of working buttons. Source/font hashes and contrast ratios are recorded in ui-mockups/source-manifest.json.

## Native implementation record

The connected native shell now includes visible-friendly box/double-click selection, saved authoritative Shift order queues, control groups, camera bookmarks, contextual transport commands, movable/pinned panels, minimap navigation/zoom/resize, masked layers, merged known-location alerts, searchable help and separate UI preferences. Ecology uses readable researched traits and existing grown fauna art; flora previews are actual currently observed terrain crops. The automated PC campaign check requests fifteen actual native UI captures: calm/build/ecology/food-web/logistics at1366×768,1920×1080 and2560×1440. Capture/readback costs remain in diagnostic timing. These hooks are implementation, not a claim that those PC captures have already been returned or that the UI timing budget is qualified.

UI-only recovery preserves a corrupt `ui-layout.json` byte-for-byte under a unique PID/timestamp `.corrupt` name after validating its previous snapshot. It then creates a fresh current through the existing atomic save helper, leaving the verified `.previous` intact. Later preference changes can save normally. A fixture checks recovery, exact quarantined bytes, retained previous bytes and the subsequent save. Campaign saves and the generic refusal to overwrite corrupt current saves retain their existing policy.
