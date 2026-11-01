# Milestone A — actual PC benchmark review

Date: 2026-10-29, America/Chicago. Review of Tor-supplied records; no new game run, compiler invocation or performance experiment was performed by this review.

## Result

Tested on Tor’s actual PC: Vulkan and DirectX12 each passed the 1,000/10,000/100,000 movement-agent phases and their finite-coordinate, first-agent movement and exact spatial-membership checks. Both backend processes exited0. There are no current Windows FATAL/PANIC/GPU_ERROR/SURFACE_ERROR records. Source audit: MILESTONE_A_PC_LOG_AUDIT.md.

Hardware: AMD Ryzen9 3900X (12cores/24threads), approximately32GB RAM, NVIDIA GeForce RTX2080 SUPER, Windows10 Pro build19045. Vulkan reports NVIDIA driver621.01; DX12 reports32.0.16.2101. Native available logical parallelism is24.

The host session ran at approximately19:33 CDT on2026-10-29 (UTC00:33 onOctober30). The scope-only PID21416 has no supplied exit record, so no exit-code claim is inferred for it.

## Measured synchronized headless frame durations

All numbers below are milliseconds for the existing isolated fixture. They include CPU loop/encoding and waiting for GPU completion; they are not isolated GPU timestamps or interactive presentation times.

| Backend | Movement agents | Median ms | p95 ms | p99 ms | Maximum ms | Phase seconds |
|---|---:|---:|---:|---:|---:|---:|
| Vulkan | 1,000 | 0.2663 | 0.5492 | 0.8505 | 4.7583 | 5.0005 |
| Vulkan | 10,000 | 0.2945 | 0.5721 | 0.7964 | 2.5252 | 5.0003 |
| Vulkan | 100,000 | 0.6660 | 1.0102 | 1.2269 | 2.5395 | 5.0002 |
| Dx12 | 1,000 | 0.3515 | 0.8879 | 1.2853 | 1.7501 | 5.0003 |
| Dx12 | 10,000 | 0.3792 | 0.9080 | 1.2747 | 1.5968 | 5.0002 |
| Dx12 | 100,000 | 0.7285 | 1.2615 | 1.5397 | 1.8824 | 5.0001 |

Quantiles use nearest rank over up to the most recent8,192 samples. The 1k/10k phases exceed that count, so their quantiles describe the retained trailing samples; maximum and mean cover all phase samples. The 100k phases fit entirely in the ring. The synthetic minimum50ms frame delta is not proof of 68–170× game speed; no real-time game-speed qualification is inferred from those fields.

## Measured process memory

| Backend | PID | Samples | Highest reported OS peak working set | Exit |
|---|---:|---:|---:|---:|
| vulkan | 21300 | 17 | 257.07 MiB | 0 |
| dx12 | 10456 | 17 | 239.95 MiB | 0 |

These are maxima of the sampled Windows peak-working-set counters across each process, not per-population allocator peaks, total system memory, or VRAM. Both processes use the same preallocated100k-agent capacity. Known GPU buffer payload is14,060,960bytes; textures, driver allocations and staging are excluded, and actual VRAM remains unmeasured.

## Evidence and remaining qualification

The six phase starts pair exactly with six results. GPU_CHECK appears six times. Backend completion/PID records agree with benchmark run IDs and the memory samples. Old software/Linux/build failures precede the Windows segment and are retained, not reclassified as current failures.

Raw structured records and extracted Windows console are archived in evidence/pc-2026-10-29-21300-10456/. review-summary.json records source filenames, Library IDs and SHA256 values; the original full uploaded console remains under its supplied Library identity.

This establishes a passing automated native-PC baseline for the existing4.8×3.2km movement/rendering fixture. The 20×20km map, construction/terrain editing, actual military combat, non-military jobs, procedural ecology, mandatory biology/30 additional mechanisms, conservation, saves and long wars still need implementation and integrated qualification. Real low-end performance is UNMEASURED. No interactive READY/PERF/ACTION/session-end evidence or screenshots were supplied, so visual fidelity/controls are not marked tested.

Vulkan has lower median/p95/p99 timings in these short sequential runs. The worst maximum frame was lower on DX12. This is enough to keep both backends viable for subsequent integration, not a broad backend/hardware guarantee or an automatic minimum-spec/default change.

Milestone A’s automated PC-results review is satisfied by the named checks above. Milestone B still requires Tor’s go-ahead under DECISIONS.md. Exact proposed30 and proposed fixture/budget values remain proposals.
