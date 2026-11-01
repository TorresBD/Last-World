# Procedurally generated natural colors

Tor clarified that colors themselves must be generated across the natural spectrum. The earlier four fixed green bases were too narrow. The current implementation samples continuous hue/saturation/value ranges from a separate deterministic visual RNG, rather than selecting from a static RGB list.

Vegetation: generated forest/fresh/deep/olive/yellow greens dominate, with additional earthy browns, deep warm reds, rusts, golds and yellows. Warm colors belong to normal healthy visual families; they are not an automatic autumn/death signal. Hue is constrained to0–134degrees with bounded saturation/value, excluding pink/magenta, turquoise/cyan and fluorescent extremes.

A world derives eight color anchors for each of the four existing growth-form families. Cached interpolated gradients reuse the existing smooth terrain/growth noise, giving coherent stands and soft variation without new geometry, grid-shaped patches or per-pixel random-color confetti. Existing leaf brightness/grain and growth/shape streams remain intact. Accent colors are generated too.

Animals: a small world-seeded cache supplies continuous natural green/brown/red/yellow and low-saturation gray/charcoal/cream ranges. Species IDs select and interpolate cache entries deterministically, so different worlds and species yield different coats. Main commander rendering, diagnostic Observer and known-species UI icons use exactly the same packed appearance. This improves color identity; all existing four growth-form templates remain, and complete species-specific fine morphology is still future scope.

Architecture: approximately13KiB visual cache per resident world, independent of map and animal count. HSV generation runs during world/material initialization, not per pixel per frame. The32-byte GPU Agent layout is retained: role4's existing visual-family word becomes a tagged RGB+atlas-family draw packet; real tactical factions are untouched. The vertex shader unpacks that coat, and the fragment shader retains atlas luminance/alpha. CPU UI uses the same modulation. No new buffer, render pass, tree entity, population record, GPU readback, gameplay RNG call or save-schema field is added.

Verification gates: cross-language JS/Rust seed vectors; natural-family diversity/exclusion/determinism; stable per-species packing/alpha; actual shader validation; shape/alpha/non-fauna atlas byte preservation; independent pre/post physical terrain/stock hashes for two seeds; frozen host/Windows builds and existing native checks. These are functional checks, not new performance or real-PC screenshot evidence. No long speed/battle benchmark is needed for this appearance correction.

References retained from the initial quick survey (consulted2026-10-30). Ranges are artistic choices, not measured species reflectance:
- https://www.fs.usda.gov/visit/fall-colors — greens and warm natural leaf-color families, not a tropical-autumn rule.
- https://nationalzoo.si.edu/animals/green-tree-python — green/pale/yellow/red-brown natural coloration.
- https://nationalzoo.si.edu/animals/sloth-bear — charcoal coats and cream markings.
- https://nationalzoo.si.edu/animals/golden-lion-tamarin — reddish-gold humid-forest coats.
