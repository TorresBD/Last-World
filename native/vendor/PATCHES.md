# wgpu 27.0.1 WebGPU surface compatibility backport

Source: official wgpu 27.0.1 crates.io package, with its original licenses retained.
One change in `src/backend/webgpu.rs`: use the WebGPU canvas context guaranteed by a successful `getContext("webgpu")`, rather than a redundant JS prototype-chain check that can reject a valid context.

The same upstream fix is documented in the current source:
https://docs.rs/wgpu/latest/wasm32-unknown-unknown/src/wgpu/backend/webgpu.rs.html (create_surface_from_context).

Observed during browser QA: `canvas context is not a GPUCanvasContext` despite a returned GPUCanvasContext. Native backends are unchanged. GPU feature, limit, shader, and resource validation remain enabled.
