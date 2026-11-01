@echo off
setlocal EnableExtensions DisableDelayedExpansion
set "LAST_WORLD_INSTALL_ROOT=%~dp0"
set "LAST_WORLD_DATA_DIR=%~dp0Data"
set "LAST_WORLD_ASSET_DIR=%~dp0App"
set "LAST_WORLD_PS=%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe"
if exist "%SystemRoot%\Sysnative\WindowsPowerShell\v1.0\powershell.exe" set "LAST_WORLD_PS=%SystemRoot%\Sysnative\WindowsPowerShell\v1.0\powershell.exe"
if not exist "%~dp0Tools\Initialize-Layout.ps1" (
  echo This is not the complete organized Last World folder.
  echo Open the folder with App and Tools beside these two launch buttons.
  echo Copy the CONTENTS of the downloaded Last-World folder into your existing folder.
  pause
  exit /b 12
)
"%LAST_WORLD_PS%" -NoProfile -ExecutionPolicy Bypass -File "%~dp0Tools\Initialize-Layout.ps1"
if errorlevel 1 (
  echo Folder preparation stopped. Read the message above; existing files are preserved.
  pause
  exit /b 12
)
rem Transfer directly, avoiding CALL's second expansion of folder names.
"%~dp0Tools\Run-Checks-Watchdog.cmd"
