# Milestone A — Windows cross-build audit

Audit date: 2026-10-29, America/Chicago (binary timestamp below is UTC on October 30). This is read-only PE/import and source-timestamp inspection of the freshly cross-built executable. The Windows executable was **not executed**. No build, test, benchmark, browser session or performance measurement was run by this audit.

## Audited executable

| Field | Observed value |
|---|---|
| Path | `native/target/x86_64-pc-windows-gnu/release/last-world.exe` |
| File size | 10,526,543 bytes |
| SHA-256 | `a8b9f619be733a1eb18436e0988f9b611a8b55001aec57aaa923ce29ae609d69` |
| File format from objdump | `pei-x86-64` |
| COFF machine | `0x8664` (AMD64/x86-64) |
| PE optional-header magic | `020b` (PE32+) |
| Subsystem | `00000003` (Windows CUI/console) |
| Executable modification time | `2026-10-30T00:25:25.765071+00:00` |

Tool used:

```text
/workspace/scratch/5f5799dd0333/milestone-a-build-tools/mingw/usr/bin/x86_64-w64-mingw32-objdump -p /workspace/sites/last-world/native/target/x86_64-pc-windows-gnu/release/last-world.exe
```

The tool inspected the file's PE metadata; it did not launch the application. SHA-256 and COFF-machine metadata were read directly from the file bytes.

## Import table and portable package conclusion

The static PE import table contains the following unique names (case-normalized; repeated kernel32/ntdll/ole32 import descriptors are omitted from this list):

```text
api-ms-win-core-synch-l1-2-0.dll
bcryptprimitives.dll
dwmapi.dll
gdi32.dll
imm32.dll
kernel32.dll
msvcrt.dll
ntdll.dll
ole32.dll
oleaut32.dll
opengl32.dll
shell32.dll
user32.dll
uxtheme.dll
```

These are Windows system libraries/API-set names. In particular, `msvcrt.dll` is the Windows system runtime, not MinGW's separate libstdc++ runtime. The API-set name is resolved by Windows and is not a DLL to copy into the package.

**No `libwinpthread-1.dll`, `libgcc_s_seh-1.dll`, `libstdc++-6.dll`, or other non-system runtime DLL appears in the static import table. No MinGW runtime DLL needs to be added to the portable ZIP based on these imports.** Do not copy operating-system DLLs into the ZIP.

For reference only, the workspace toolchain contains these runtime candidates; their presence does not make them dependencies of this executable:

| Candidate, not imported | Exact toolchain location |
|---|---|
| libwinpthread-1.dll | `/workspace/scratch/5f5799dd0333/milestone-a-build-tools/mingw/usr/x86_64-w64-mingw32/lib/libwinpthread-1.dll` |
| libgcc_s_seh-1.dll | `/workspace/scratch/5f5799dd0333/milestone-a-build-tools/mingw/usr/lib/gcc/x86_64-w64-mingw32/13-posix/libgcc_s_seh-1.dll` |
| libstdc++-6.dll | `/workspace/scratch/5f5799dd0333/milestone-a-build-tools/mingw/usr/lib/gcc/x86_64-w64-mingw32/13-posix/libstdc++-6.dll` |

This conclusion covers the PE static imports only. Graphics APIs may load driver/runtime components dynamically; a successful real-PC startup and backend run are still required. The import table does not establish Vulkan/DX12 driver compatibility, Windows runtime execution, game correctness or performance.

## Source freshness

Compared nanosecond modification times of every regular file beneath `native/src`, `native/shaders`, and `native/assets`, plus regular `native/Cargo*` files and any regular immediate files under `native/.cargo`, against the executable timestamp. There were **15 input files** in this scope. **None was newer than the executable.**

The latest input in that scope was:

```text
native/src/runtime.rs
2026-10-30T00:20:33.482390+00:00
```

The executable was therefore newer than all specified current native project inputs at inspection. This is a timestamp freshness check, not a reproducible-build proof or a claim about every dependency/toolchain file. If project inputs change after this audit, rebuild and refresh the executable hash before packaging. The parent owns the successful build log and package verification.

## Evidence boundary

This report supports **cross-build artifact identity and metadata inspection only**. It does not claim the Windows executable was run, that Windows tests passed, that any hardware frame-time target was met, or that the benchmark is an integrated native campaign. Real-PC and integrated-workload performance remain unmeasured here.
