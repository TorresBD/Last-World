@echo off
cd /d "%~dp0"
echo Last World - CPU water correctness report
echo Fixed seed and 4800 action ticks. No GPU device or performance result.
echo This checks the isolated water foundation, not the integrated campaign.
Last-World.exe --water-report
set "WATER_CHECK_EXIT=%errorlevel%"
if not "%WATER_CHECK_EXIT%"=="0" (
  echo Water report failed. Keep console.txt and river-history.jsonl if present.
) else (
  echo Water report recorded. Share river-history.jsonl and console.txt.
)
pause
exit /b %WATER_CHECK_EXIT%
