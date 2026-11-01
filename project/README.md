# Project records

These files live in the game source directory in every current and future version.

- `MASTER.md`: required current project scope, rules, state and next steps.
- `LEDGER.md`: required append-only history; search IDs/tags in bounded excerpts.
- `archive/`: frozen earlier handoffs, searched by LW code only.
- `snapshots/`: exact historical revisions.

Keep both live records inside `Source/project/` and include them in all complete Source zips and update packages. Library copies/checkpoints supplement these working files. The packager and package audit reject missing or empty records. Old root RULES/STATE/DECISIONS/BLOCKERS/PARITY files are retained historical material.
