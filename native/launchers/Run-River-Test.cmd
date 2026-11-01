@echo off
cd /d "%~dp0"
echo Last World - river-flow foundation
echo This is the isolated water fixture with zero army or wildlife agents.
echo G closes the dam. J breaches/opens it. Space pauses.
echo F1/F2/F3 select 1x/2x/4x. WASD/arrows pan and the mouse wheel zooms.
echo Water fills and drains over simulation ticks; no instant reservoir is assigned.
Last-World.exe --river --speed 1
set "RIVER_EXIT=%errorlevel%"
if not "%RIVER_EXIT%"=="0" (
  echo River test failed. Keep console.txt for diagnosis.
) else (
  echo River window closed. Logs remain in console.txt.
)
pause
exit /b %RIVER_EXIT%
