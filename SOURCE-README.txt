LAST WORLD - SOURCE SNAPSHOT

This source is inside the same Last-World update folder as the native executable.
It is an application-owned reference snapshot, not a second game install or an update service.

Snapshot parent commit: 1177730b5808dfebb3a2806ce8323a18aa287416
Working tree has changes relative to that commit: True
Native compilation-input SHA-256: 3c9147c8a52073d64788d521c8325c57a614a1f337ada31d0d706522f8686e1e
Executable SHA-256: 41dbfc9dea9553e6e681aa9a2ac951fa70b412362dc9f74d4aedebc55807e17f

The commit identifies ancestry; it does not claim that all snapshot files were committed.
The outer ../App/build-manifest.json contains a SHA-256/size for every Source file and an aggregate
source_snapshot hash. Its native hash identifies the build-stamped compilation inputs;
the complete snapshot hash additionally covers scripts, vendor sources, records and docs.

Included: native Rust, shaders, baked pixel/font assets and licenses, Cargo manifest/lock,
the patched wgpu vendor source/PATCHES.md, scripts, examples, launchers, qualification
inputs, project records, docs/design references and current B text/JSON evidence.
Only dist/config.mjs and dist/pixel-forms.mjs are included for export-pixels.mjs.
No old distribution ZIPs, browser runtime, build targets, caches, installed logs, saves,
settings or check-results are copied. Old large PC screenshot trees are omitted.
Docs/mockups retain one existing world reference and their labelled design previews;
these are not new native execution evidence.

Build from Source with a suitable Rust/Cargo toolchain (see native/Cargo.toml), using
cargo build --manifest-path native/Cargo.toml --locked --release. Other Cargo dependencies
still require a registry/network or a populated local Cargo cache: this is not a full
offline toolchain/registry bundle. Python3.11+ runs the build/package scripts. Windows
cross compilation also needs its target/linker. Baked assets are included; optional
regeneration uses Node for export-pixels and Pillow/fontTools for licensed glyph baking.
The native build identity covers Cargo inputs, Rust/shaders/assets and the patched
vendor tree. Scripts/docs are covered by the separate complete snapshot hash.
Overlay QA independently recomputes the native identity from packaged Source bytes.
The build-release.py helper can record null Git ancestry when repository metadata is
absent; this snapshot intentionally has no .git directory. Direct Cargo builds are
independent of Git metadata.

Scope: the current small native Milestone B development slice. Read project/MASTER.md;
selectively search project/LEDGER.md and consult docs/MILESTONE_B_VERIFICATION.md for evidence.
Master and Ledger are required source files in every future version; old root records are historical.
Including source does not qualify production20km streaming, all160 traits, complete
mandatory biology, native-PC visuals, throughput, memory or release gameplay.
