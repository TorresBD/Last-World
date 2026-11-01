@echo off
cd /d "%~dp0"
echo Last World - Bevy / GPU benchmark gate
echo This checks GPU movement and spatial links at each population.
echo Hardware and software adapters are identified in console.txt.
Last-World.exe --headless --counts 1000,5000,10000,25000,50000,100000 --seconds 5 --check
if errorlevel 1 (
  echo Test failed. Please keep console.txt for diagnosis.
) else (
  echo Finished. Results are in benchmark-results.json and console.txt.
)
pause
