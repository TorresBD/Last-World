$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath $PSScriptRoot
$consolePath = Join-Path $PSScriptRoot 'console.txt'
$historyPath = Join-Path $PSScriptRoot 'pc-host-history.jsonl'
$pendingConsole = New-Object 'System.Collections.Generic.List[string]'
function Record-HostEvent($Event) {
    $Event.timestamp_utc = [DateTime]::UtcNow.ToString('o')
    $line = $Event | ConvertTo-Json -Compress -Depth 6
    Add-Content -LiteralPath $historyPath -Value $line -Encoding UTF8
    # Avoid two processes appending to console.txt concurrently. Native logs
    # during the run; these host samples join the console after children exit.
    $script:pendingConsole.Add('[PC_HOST] ' + $line)
}
$failed = $false
$unsafeConsoleFlush = $false
try {
    $os = Get-CimInstance Win32_OperatingSystem
    $cpu = Get-CimInstance Win32_Processor | Select-Object Name, NumberOfCores, NumberOfLogicalProcessors
    Record-HostEvent @{ event='host'; os=$os.Caption; os_build=$os.BuildNumber; total_visible_ram_bytes=([uint64]$os.TotalVisibleMemorySize * 1024); cpu=@($cpu); qualification='isolated_benchmark_only' }
    & (Join-Path $PSScriptRoot 'Last-World.exe') --describe | Set-Content -LiteralPath (Join-Path $PSScriptRoot 'workload-scope.json') -Encoding UTF8
    if ($LASTEXITCODE -ne 0) { throw 'Executable scope report failed' }
    foreach ($backend in @('vulkan', 'dx12')) {
        Record-HostEvent @{ event='backend_start'; backend=$backend }
        $process = $null
        try {
            $process = Start-Process -FilePath (Join-Path $PSScriptRoot 'Last-World.exe') -ArgumentList @('--headless','--backend',$backend,'--counts','1000,10000,100000','--seconds','5','--check') -PassThru -NoNewWindow
            [void]$process.Handle
            while (-not $process.HasExited) {
                $process.Refresh()
                if (-not $process.HasExited) {
                    try {
                        Record-HostEvent @{ event='process_memory'; backend=$backend; process_id=$process.Id; working_set_bytes=$process.WorkingSet64; peak_working_set_bytes=$process.PeakWorkingSet64; note='OS process working set; not VRAM or allocator-only bytes' }
                    } catch {
                        if (-not $process.HasExited) { throw }
                    }
                }
                Start-Sleep -Milliseconds 1000
            }
            $process.WaitForExit()
            $exitCode = $process.ExitCode
            if ($null -eq $exitCode) { throw 'Child exit code unavailable; no success inferred' }
            Record-HostEvent @{ event='backend_end'; backend=$backend; process_id=$process.Id; exit_code=$exitCode }
            if ($exitCode -ne 0) { $failed = $true }
        } catch {
            $failed = $true
            Record-HostEvent @{ event='backend_error'; backend=$backend; error=$_.Exception.Message }
        } finally {
            if ($null -ne $process) {
                try {
                    if (-not $process.HasExited) {
                        $process.Kill()
                        if (-not $process.WaitForExit(10000)) {
                            $unsafeConsoleFlush = $true
                            throw 'Owned benchmark child did not terminate; remaining backends aborted'
                        }
                        Record-HostEvent @{ event='owned_child_cleanup'; backend=$backend; process_id=$process.Id }
                    }
                } catch {
                    $unsafeConsoleFlush = $true
                    throw
                } finally {
                    $process.Dispose()
                }
            }
        }
    }
} catch {
    $failed = $true
    Record-HostEvent @{ event='launcher_error'; error=$_.Exception.Message }
}
if (-not $unsafeConsoleFlush) {
    Add-Content -LiteralPath $consolePath -Value $pendingConsole -Encoding UTF8
} else {
    Write-Warning 'Host records remain in pc-host-history.jsonl; console merge skipped while child termination is uncertain.'
}
if ($failed) { exit 1 }
exit 0
