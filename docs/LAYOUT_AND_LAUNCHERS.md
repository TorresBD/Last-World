# Last World folder and launch buttons

Use Run-Game.cmd to play and Run-Checks.cmd for the full automatic test. Both sit directly beside App, Tools, Data, Documentation and Source. Send-Back.zip is created directly beside them after checks; it is the only file to return.

| Item | Purpose |
| --- | --- |
| App | Native game, standalone checks, build identity, benchmark definitions, default UI assets and licenses. Application-owned files. |
| Data | Persistent saves, settings, canonical append-only console.txt, histories, check-results, locks, temporary files and preserved legacy files. |
| Tools | Internal launcher, supervisor, recovery and read-only hardware-logging code. |
| Documentation | Instructions, project records and verification notes. |
| Source | Reviewed Rust/shader/asset/script source snapshot. |

Extract the ZIP, open its Last-World folder, then copy the CONTENTS into the existing Last-World folder. Copying the folder into the old folder creates Last-World/Last-World and leaves the outer launchers unchanged. A missing Windows replacement prompt does not prove an update failed; it can mean the files went into another location. The startup banner prints the actual install path plus source/executable prefixes to identify the copy being used. The supervisor hashes the declared installed payload before checks.

On first launch the initializer verifies App/Tools, closes no processes, checks that this install is idle, and moves recognized old logs/saves/settings/checkpoints into Data. Append histories preserve exact original bytes and retain import receipts/originals. Existing Data saves/settings win collisions; both versions survive under Data/Legacy/Conflicts and a warning names the retained file. Old helpers/default assets are retained under Data/Legacy/Loose-Files. Unknown user files and nested folders stay untouched. Rerunning initialization is idempotent. Windows-specific file locking still requires the PC run; portable filesystem probes are separate evidence.

Data/Locks/Run-Checks.lock coordinates the supervisor and feedback recovery. Its exclusive open handle, not file existence, indicates activity. Empty lock files can remain after a clean exit and are reused; they are not extra test results. The legacy root lock is archived after migration while the new lock is held, or retained with a recorded reason if it cannot be moved safely. Source/native/Cargo.lock instead pins dependencies and remains part of the source.

Root launchers set child-only absolute LAST_WORLD_DATA_DIR and LAST_WORLD_ASSET_DIR. Native startup enters Data before logging. UI resolution is Data/ui custom overrides, App/ui defaults, then embedded assets. Saves retain their existing schema. No install drive is changed.

Long phases print elapsed time and, when native samples are available, ticks/target, percentage, achieved speed, tick debt and sample age every ten seconds. No sample yet may mean world/checkpoint loading or validation; it is not an inferred freeze. The72000-tick season case targets one game hour at requested2x (about30minutes only if2x holds).

Tools/Check-Progress.cmd is an independent, read-only collector for a currently running suite. It takes two samples five seconds apart and publishes Progress.txt plus separate Send-Back-Progress.zip at root, without changing the running child, suite lock, checkpoints or existing Send-Back.zip. For an older flat install, put the collector beside its root Run-Checks.cmd. Missing/unverified ownership and short no-progress intervals are reported without declaring a hang.
