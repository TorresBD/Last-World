@echo off
cd /d "%~dp0"
echo Last World - Milestone A native PC qualification setup
echo This is the isolated movement/renderer benchmark, not the integrated game.
echo Your actual adapter and failures will be recorded. Nothing is silently scaled down.
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0Run-PC-Qualification.ps1"
if errorlevel 1 (
  echo At least one backend failed. Keep console.txt and benchmark-history.jsonl.
) else (
  echo Backend checks finished. These results do not qualify the full campaign.
)
echo Share console.txt, benchmark-history.jsonl, pc-host-history.jsonl and workload-scope.json.
pause
