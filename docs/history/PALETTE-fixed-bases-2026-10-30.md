# Superseded fixed-base palette; see current PALETTE_DIRECTION.md

# Living jungle palette —2026-10-30

Tor requested a quick natural-color survey and replacement of pink/turquoise procedural flora. This is an artistic palette choice informed by references, not measured species reflectance or a new ecosystem rule.

| Material | Selected base colors |
|---|---|
| Forest/fern/olive/sunlit foliage | #327834, #47893B, #538033, #849941 |
| Sparse warm plant accents | #BA994D amber, #CDBD7F cream |
| Wildlife coat families | #76583A bark brown, #5F713F forest olive, #9B713D ochre, #4C4F41 charcoal |

Foliage retains its seeded per-pixel growth, existing brightness/grain, canopy variation and accent frequencies. All hills, soil, riverbed/mud, wetness/drying, erosion, biomass, life arrays and world seeds stay unchanged. Wildlife retains the same growth walks, anatomy, alpha, poses, IDs and atlas slots. Troop and building pixels remain byte-identical. Species habitat previews sample the updated terrain directly; fauna icons use the same updated atlas.

The natural reference range includes evergreen greens and warm yellow/rust/gold accents, forest-green camouflage with cream/yellow details, charcoal coats with pale markings, and reddish-gold fur. Pink/magenta/cyan/turquoise were deliberately excluded from these material families. No whole-spectrum hue randomization was introduced. These are shared visual families, not a claim of complete species-specific fine appearance.

References consulted2026-10-30:
- US Forest Service, Fall Colors: https://www.fs.usda.gov/visit/fall-colors — evergreen contrast and warm leaf-color families. This informs accent choices, not a claim that tropical jungle becomes a temperate autumn forest.
- Smithsonian, Green tree python: https://nationalzoo.si.edu/animals/green-tree-python — green adult camouflage, pale/yellow markings, and yellow/red-brown juvenile variation.
- Smithsonian, Sloth bear: https://nationalzoo.si.edu/animals/sloth-bear — dusty black coats and cream markings.
- Smithsonian, Golden lion tamarin: https://nationalzoo.si.edu/animals/golden-lion-tamarin — reddish-gold fur in humid forest habitat.

Verification:3328existing forms compared across seed/team/role/pose. Geometry and alpha match; only fauna colors differ. Actual1024x1024atlas comparison changes1232RGB pixels exclusively inside fauna slots; metadata/all non-fauna RGBA are identical. No imagegen, Python drawing or new shape/sprite generation algorithm was used. Frozen builds and existing native checks are the release gates; no speed benchmark is needed to assess a palette change. Actual new native screenshots need the PC renderer.
