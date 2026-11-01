# Static UI design gallery

Open `index.html` for the six scenes and three viewport sizes. The gallery displays the rendered PNGs; each source SVG is also available. These are explicitly labelled UI mockups with illustrative data over an existing game capture. They are not native screenshots or evidence of implemented interactions/performance.

The `references/` folder contains byte-identical copies of the existing world capture, existing procedural atlas and the exact bundled font/license files. No new world art, image generation or Python shape-stamp artwork was used. Original source identities and hashes are in `source-manifest.json`.

Rebuild in the repository's Node environment with Sharp available:

```
node docs/ui-mockups/generate.mjs --render
```

The generator writes six1920×1080masters, six adaptive1366×768 variants and six2560×1440 checks, each SVG plus PNG. Rendered previews use Sharp/libvips/librsvg and the bundled fonts through a private fontconfig. Chromium was unavailable, so this is SVG rendering rather than browser or game execution.

`layout-review.json` records panel bounds, body font sizes, minimum44px hit-region geometry and approximate default plate coverage. Internal PNG review corrected tiny atlas crops, a43.5px small-window layer target, wide-button text/icon overlap, the mockup evidence label position, distinct pause/speed targets and food-web producer/consumer arrows. These static geometry checks do not replace native interaction tests.

The calm layout's bounds-summed plate coverage is approximately20.5% at1366,16.4% at1920 and13.3% at2560. Body text is14/16/19.2px respectively; every declared action region is at least44px. The gallery's physical asset sources and world pixels remain unchanged. Style details: `../UI_STYLE_DIRECTION.md`.
