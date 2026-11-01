@echo off
setlocal EnableExtensions DisableDelayedExpansion
if not defined LAST_WORLD_INSTALL_ROOT set "LAST_WORLD_INSTALL_ROOT=%~dp0.."
if not defined LAST_WORLD_DATA_DIR set "LAST_WORLD_DATA_DIR=%LAST_WORLD_INSTALL_ROOT%\Data"
if not defined LAST_WORLD_ASSET_DIR set "LAST_WORLD_ASSET_DIR=%LAST_WORLD_INSTALL_ROOT%\App"
cd /d "%LAST_WORLD_DATA_DIR%"
set "CHECK_PS=%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe"
if exist "%SystemRoot%\Sysnative\WindowsPowerShell\v1.0\powershell.exe" set "CHECK_PS=%SystemRoot%\Sysnative\WindowsPowerShell\v1.0\powershell.exe"
set "CHECK_WATCHDOG_ATTEMPT=0"
set "CHECK_START_FILETIME="
echo Last World - comprehensive resumable diagnostics
echo Close other Last World windows. Individual failures retry once, then the suite continues.
echo One adaptive speed ramp, one 20-minute battle at 4x, and CPU/RAM/GPU telemetry.
echo Long cases may take hours. Same-build reruns resume; finished phases are not repeated.
echo Send back only Send-Back.zip from the main Last-World folder when this window finishes.
rem Run PowerShell directly: FOR /F command substitution reparses the quoted
rem executable/command through CMD and can fail before recording the timestamp.
if not exist "check-results" mkdir "check-results"
"%CHECK_PS%" -NoProfile -Command "[DateTime]::UtcNow.ToFileTimeUtc()" >"check-results\launch-start-filetime.txt" 2>>"check-launcher-errors.txt"
set /p CHECK_START_FILETIME=<"check-results\launch-start-filetime.txt"

:RunSupervisor
"%CHECK_PS%" -NoProfile -ExecutionPolicy Bypass -File "%~dp0Run-Checks.ps1" 2>>"check-launcher-errors.txt"
set "CHECKS_EXIT=%errorlevel%"
if "%CHECKS_EXIT%"=="0" goto CheckFeedback
if "%CHECKS_EXIT%"=="10" goto CheckFeedback
if "%CHECKS_EXIT%"=="11" goto CheckFeedback
if "%CHECKS_EXIT%"=="12" goto CheckFeedback
if "%CHECKS_EXIT%"=="2" goto ActiveBatch
if "%CHECK_WATCHDOG_ATTEMPT%"=="2" goto CheckFeedback
set /a CHECK_WATCHDOG_ATTEMPT+=1
echo Supervisor ended unexpectedly. Restarting from its durable journal, restart %CHECK_WATCHDOG_ATTEMPT% of 2.
goto RunSupervisor

:CheckFeedback
"%CHECK_PS%" -NoProfile -Command "$start=0L; if (-not [Int64]::TryParse($env:CHECK_START_FILETIME,[ref]$start)) {exit 1}; $file=Join-Path $env:LAST_WORLD_INSTALL_ROOT 'Send-Back.zip'; if ((Test-Path -LiteralPath $file -PathType Leaf) -and (Get-Item -LiteralPath $file).LastWriteTimeUtc.ToFileTimeUtc() -ge $start) {exit 0}; exit 1" 2>>"check-launcher-errors.txt"
if not errorlevel 1 goto FeedbackReady
echo Creating current feedback from available owned logs and journal...
"%CHECK_PS%" -NoProfile -ExecutionPolicy Bypass -File "%~dp0Run-Checks-Feedback.ps1" -Fallback 2>>"check-launcher-errors.txt"
set "CHECK_FALLBACK_EXIT=%errorlevel%"
if "%CHECK_FALLBACK_EXIT%"=="2" goto ActiveBatch
if not "%CHECK_FALLBACK_EXIT%"=="0" goto FeedbackUnavailable

:FeedbackReady
if "%CHECKS_EXIT%"=="0" (
  echo Suite completed. Send back Send-Back.zip.
) else (
  echo Some phases failed, were skipped, or stopped safely. They are not counted as passed.
  echo Send back Send-Back.zip; all underlying progress and checkpoint files remain local.
)
goto Finish

:ActiveBatch
set "CHECKS_EXIT=2"
echo A check batch is already active. This launch did not start another suite.
goto Finish

:FeedbackUnavailable
set "CHECKS_EXIT=12"
echo A current Send-Back.zip could not be created. Do not send an older copy.
echo The journal and logs remain under Data\check-results and Data\check-launcher-errors.txt.

:Finish
pause
exit /b %CHECKS_EXIT%
