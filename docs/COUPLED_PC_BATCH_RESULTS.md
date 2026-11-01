# Coupled water/ground PC batch — 2026-10-30

Tor returned one `Send-Back.zip`. This review checks the existing PC evidence; it does not run the game again or change its source. The four functional stages passed. The river scene has a measured long frame interval that remains unresolved; functional success is not a performance qualification.

## Identity and workflow

- Run: `20261030T021220123Z-23228`, 2026-10-30 02:12:20–02:13:42 UTC. Returned ZIP entry timestamps are October29 on the PC.
- Source: `26654552b435e2453a1a99bf5935fddcffdaa764`.
- Executable SHA256: `1f917b220e1d096d9f73377f885b325ac8314c061dba8295942f9750450227df`.
- Returned ZIP SHA256: `99194d4386ea3c1e93a5058855715a79aa088b3f62aa61d75306476ccd1298da`, 3,157,993bytes,18entries.
- Returned manifest exactly matches the delivered update manifest. The host runner recorded successful payload verification before starting its children.
- CPU ground report, Vulkan movement, DX12 movement and automatic river renderer each exited0. No timeouts, preparation errors or cleanup uncertainty.
- All three fresh captures and the final renderer report belong to childPID17376. The ZIP contains the named logs, histories, host records, manifest and stage summary. No further manual files are needed.
- The supplied259-line console contains these four current sessions. No FATAL/PANIC/GPU_ERROR/SURFACE_ERROR tags occur in it or the four child stderr records.

The successful normal path of Run-Checks.cmd/PowerShell and Send-Back.zip collection is now tested on Windows. This run did not exercise duplicate-launch locking, timeout termination, parser/startup fallback or failed-stage packaging. Nor does a current-only console prove that Tor merged over a folder containing older logs. Prior local sentinel overlay preservation is separate evidence.

## Exact correctness review

The returned CPU report is JSON-identical to the accepted Linux report at `evidence/ground-water-2026-10-29/cpu-report.json`. Its stdout and river-history entry also agree.

Independent arithmetic checks10CPU checkpoints,44GUI WATER records and the final visual state. At every state, each of these five residuals is exactly zero:

1. Surface water: initial + boundary input + returns − stored − boundary output − extraction.
2. Sediment: initial − bed − suspended − boundary output. Deposited subset is already part of bed.
3. Biomass: initial − live − mobile − deposited − boundary output.
4. Soil water: initial + infiltration − stored − evaporation − returns.
5. Combined water: initial surface + initial soil + boundary input − surface − soil − boundary output − evaporation.

That is275zero-residual identities. Additional cross-owner checks match surface extraction/returns to soil infiltration/returns and ensure deposit-subset stock does not exceed bed stock. These are checks of the reported model stocks, not a proof of physical realism or every cell-level invariant.

The automatic visual sequence reached tick3600, captured all three actual renderer frames and reported conservation. Its timeline differs from the4800tick CPU report: close at20simulated seconds, breach at80seconds, finish at180seconds.

## Named hardware and isolated movement measurements

Ryzen9 3900X,12cores/24threads;32GB RAM; RTX2080SUPER8GB; Windows10Pro19045. Vulkan driver621.01 and DX12 driver32.0.16.2101. Each phase lasts approximately3wall seconds.

| Backend | Moving records | Median ms | p95 ms | p99 ms | Max ms |
|---|---:|---:|---:|---:|---:|
| Vulkan |1,000|0.3048|0.5895|0.8734|3.0382|
| Vulkan |10,000|0.3621|0.6319|0.8854|2.5660|
| Vulkan |100,000|0.5358|0.8373|1.0696|2.4770|
| DX12 |1,000|0.3556|0.5906|1.1975|1.8132|
| DX12 |10,000|0.3905|0.8361|1.2391|1.9651|
| DX12 |100,000|0.5761|1.0942|1.3969|1.7473|

All six GPU correctness checks passed. These timings measure synchronized headless loop throughput, including device.poll(wait_indefinitely()); they are not raw GPU timestamps. The clock uses a minimum synthetic50ms delta, so reported synthetic speed is not achieved real-time game speed. These records have no complete military/civilian/ecology campaign rules. Quantiles retain the latest8192samples; the8798-frame Vulkan1k phase therefore omits606older samples from its quantiles. Max and mean cover all phase samples.

Maximum sampled OS process peak working sets: CPU54.73MiB; Vulkan268.41MiB; DX12267.14MiB; river313.97MiB. These are neither allocator accounting nor VRAM measurements, and they are not per-phase maxima. They apply to this executable/fixture, not the full20km workload.

## River latency issue — open

There are44periodic PERF_DETAIL windows. Most report frame-interval medians around8.33ms. Three windows contain a maximum above the100ms development warning threshold:

| Reported at UTC | Tick | Window max ms | Pending ticks | Context |
|---|---:|---:|---:|---|
|02:12:58.233|79|199.8925|0|Initial capture/startup interval window|
|02:13:13.382|1292|153.0291|0|After dam capture|
|02:13:20.443|1799|909.9335|56|After breach; no matching named capture|

The following window reaches tick1937 with no pending debt. Reported submitted speed temporarily exceeds requested4× during catch-up; this is retained simulation debt being processed, not a new selected speed. The number of slow frames cannot be reconstructed from window maxima alone.

The frame interval is measured between timestamps taken before surface acquisition. It can include previous simulation work, partial terrain uploads, driver/present waits, capture readback/PNG encoding, synchronous logging and event-loop/OS scheduling. The only separate CPU timer surrounds render encoding/submission; it excludes the Bevy update, terrain upload, surface acquisition and capture. That mean cannot identify the long interval's cause. GPU execution/completed-speed fields remain null. The final capture exits before another periodic report, so its cost need not appear in those windows.

Next development batch: add bounded per-stage CPU timing and slow-interval records for simulation, terrain synchronization/upload, surface acquisition, encode/submit, present, capture and reporting/event-loop gaps. Retain maxima and frame/tick identity so a future one-file batch can localize the cause. Avoid attributing it to the solver, GPU, driver or external activity without that evidence. No immediate repeat of this unchanged binary is requested. This local river run is not qualified as hitch-free4×.

## Visual observations and limits

The three distinct1280×800RGB captures decode successfully. They show the procedural colored foliage and ground, a brown channel widening upstream in the dam frame, and a narrower channel after breach. The banks retain changed/bare material in the later frame. This supports execution of the real rendering/update/capture path. Individual screenshots do not establish the complete moisture delay, realistic hydraulics, final art approval, large-world generation or final troop appearance. No image-generation or stamped artwork was used.

Raw input files are preserved under `evidence/pc-coupled-2026-10-30/returned/`; `review-summary.json` records hashes, each balance, exact timing records and limits. No new build or Rust suite rerun was needed because game source and delivered executable are unchanged. Previous50code checks and locked Linux/Windows release builds remain the source verification.
