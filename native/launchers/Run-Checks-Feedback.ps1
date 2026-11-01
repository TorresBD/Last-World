param([switch]$Fallback)
# Shared normal/fallback bundler. Only this run's owned results are traversed.
function Add-LastWorldSnapshot($Archive, [string]$Path, [string]$EntryName) {
    $source = $null; $output = $null
    try {
        $source = [System.IO.FileStream]::new($Path,[System.IO.FileMode]::Open,[System.IO.FileAccess]::Read,[System.IO.FileShare]::ReadWrite)
        $remaining = $source.Length
        $entry = $Archive.CreateEntry($EntryName,[System.IO.Compression.CompressionLevel]::Optimal)
        $output = $entry.Open(); $buffer = New-Object byte[] 65536
        while ($remaining -gt 0) {
            $count = $source.Read($buffer,0,[int][Math]::Min([long]$buffer.Length,$remaining))
            if ($count -le 0) { break }
            $output.Write($buffer,0,$count); $remaining -= $count
        }
    } finally { if ($null -ne $output) {$output.Dispose()}; if ($null -ne $source) {$source.Dispose()} }
}

function Make-LastWorldFeedback([string]$InstallRoot,[string]$RunFolder,$CaptureOwners,[string]$Reason) {
    $InstallRoot=[System.IO.Path]::GetFullPath($InstallRoot)
    $appRoot=Join-Path $InstallRoot 'App'
    $dataRoot=if ([string]::IsNullOrWhiteSpace($env:LAST_WORLD_DATA_DIR)) {Join-Path $InstallRoot 'Data'} else {[System.IO.Path]::GetFullPath($env:LAST_WORLD_DATA_DIR)}
    try {
    Add-Type -AssemblyName System.IO.Compression
    $utf8 = [System.Text.UTF8Encoding]::new($false)
    $temporaryRoot=Join-Path $dataRoot 'Temporary';[void][System.IO.Directory]::CreateDirectory($temporaryRoot)
    $temporary = Join-Path $temporaryRoot ('Send-Back-' + $PID + '-' + [DateTime]::UtcNow.Ticks + '.zip.part')
    $stream=$null; $archive=$null
    $warnings=New-Object 'System.Collections.Generic.List[string]'
    $excluded=New-Object 'System.Collections.Generic.List[object]'
    $entries=New-Object 'System.Collections.Generic.HashSet[string]'
    try {
        $stream=[System.IO.FileStream]::new($temporary,[System.IO.FileMode]::CreateNew,[System.IO.FileAccess]::Write,[System.IO.FileShare]::None)
        $archive=[System.IO.Compression.ZipArchive]::new($stream,[System.IO.Compression.ZipArchiveMode]::Create,$true)
        foreach ($group in @(
            @{root=$dataRoot;prefix='Data';names=@('console.txt','campaign-history.jsonl','river-history.jsonl','ground-history.jsonl','benchmark-history.jsonl','pc-host-history.jsonl','check-launcher-errors.txt','check-feedback-errors.txt')},
            @{root=$appRoot;prefix='App';names=@('workload-scope.json','build-manifest.json','native-build-stamp.json','benchmark-plan.json','benchmark-unit-tests.json')}
        )) {
            foreach ($name in $group.names) {
                $path=Join-Path $group.root $name;$entryName=$group.prefix+'/'+$name
                if (Test-Path -LiteralPath $path -PathType Leaf) {
                    try { Add-LastWorldSnapshot $archive $path $entryName; [void]$entries.Add($entryName) } catch {$warnings.Add($entryName+': '+$_.Exception.Message)}
                }
            }
        }
        $pointer=Join-Path $dataRoot 'check-results/current-suite.json'
        if (Test-Path -LiteralPath $pointer -PathType Leaf) { Add-LastWorldSnapshot $archive $pointer 'Data/check-results/current-suite.json' }
        if (-not [string]::IsNullOrWhiteSpace($RunFolder) -and (Test-Path -LiteralPath $RunFolder -PathType Container)) {
            $resultDirectory=[System.IO.Path]::GetFullPath((Join-Path $dataRoot 'check-results'))
            $resultRoot=$resultDirectory+[System.IO.Path]::DirectorySeparatorChar
            $fullRun=[System.IO.Path]::GetFullPath($RunFolder)
            if (-not $fullRun.StartsWith($resultRoot,[StringComparison]::OrdinalIgnoreCase)) {throw 'Feedback run folder escaped Data/check-results.'}
            if (((Get-Item -LiteralPath $resultDirectory -Force).Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {throw 'Feedback check-results folder is a reparse point.'}
            if (((Get-Item -LiteralPath $fullRun -Force).Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {throw 'Feedback run folder is a reparse point.'}
            $directories=New-Object 'System.Collections.Generic.Stack[string]'; $directories.Push($fullRun)
            while ($directories.Count -gt 0) {
                $directory=$directories.Pop()
                foreach ($item in @(Get-ChildItem -LiteralPath $directory -Force)) {
                    if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {continue}
                    $relative='Data/'+$item.FullName.Substring($dataRoot.TrimEnd([char[]]@(92,47)).Length+1).Replace([char]92,[char]47)
                    if ($item.PSIsContainer) {
                        if ($item.Name -eq 'checkpoints') {$excluded.Add(@{path=$relative;reason='checkpoint_state_retained_locally_for_resume'})}
                        else {$directories.Push($item.FullName)}
                    } elseif ($item.Extension -in @('.json','.jsonl','.txt','.log','.csv','.png','.dmp')) {
                        if ($item.Length -gt 64MB) {$excluded.Add(@{path=$relative;bytes=$item.Length;reason='owned_file_over64MiB_retained_locally'}) ; continue}
                        try { if ($entries.Add($relative)) {Add-LastWorldSnapshot $archive $item.FullName $relative} } catch {$warnings.Add($relative+': '+$_.Exception.Message)}
                    } else {$excluded.Add(@{path=$relative;bytes=$item.Length;reason='not_feedback_type_or_large_local_checkpoint'})}
                }
            }
        }
        foreach ($owner in @($CaptureOwners)) {
            if ($null -eq $owner) {continue}
            $prefix=[string]$owner.prefix+'-'+[int]$owner.process_id+'-'
            $started=[DateTime]::Parse([string]$owner.started_utc).ToUniversalTime()
            foreach ($suffix in @('*.png','*.json','*.jsonl')) {
                foreach ($item in @(Get-ChildItem -LiteralPath $dataRoot -Filter ($prefix+$suffix) -File | Where-Object {$_.LastWriteTimeUtc -ge $started.AddSeconds(-2)})) {
                    if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {continue}
                    $entryName='Data/'+$item.Name
                    try {if ($entries.Add($entryName)) {Add-LastWorldSnapshot $archive $item.FullName $entryName}} catch {$warnings.Add($entryName+': '+$_.Exception.Message)}
                }
            }
        }
        $entry=$archive.CreateEntry('feedback-bundle-notes.json'); $output=$entry.Open()
        try {$bytes=$utf8.GetBytes((@{schema_version=1;reason=$Reason;layout='App/Tools/Data';created_utc=[DateTime]::UtcNow.ToString('o');warnings=@($warnings.ToArray());excluded_local_files=@($excluded.ToArray());checkpoint_saves_included=$false;old_feedback_zip_included=$false} | ConvertTo-Json -Depth 12));$output.Write($bytes,0,$bytes.Length)} finally {$output.Dispose()}
    } finally {
        try {if ($null -ne $archive) {$archive.Dispose()}} finally {
            if ($null -ne $stream) {try {$stream.Flush($true)} finally {$stream.Dispose()}}
        }
    }
    $feedback=Join-Path $InstallRoot 'Send-Back.zip'
    # Windows PowerShell converts $null to an empty string for a .NET string
    # argument. Replace requires a real null to mean "no backup path".
    if (Test-Path -LiteralPath $feedback) {[System.IO.File]::Replace($temporary,$feedback,[NullString]::Value)} else {[System.IO.File]::Move($temporary,$feedback)}
    return $feedback
    } catch {
        $failure=$_
        # Keep the closed .part and the previous feedback on failure. Record the
        # actual exception before fallback, whose ZIP can include this log.
        try {
            $detail='['+[DateTime]::UtcNow.ToString('o')+'] Feedback finalization failed'+[Environment]::NewLine+$failure.Exception.ToString()+[Environment]::NewLine+$failure.InvocationInfo.PositionMessage+[Environment]::NewLine+$failure.ScriptStackTrace+[Environment]::NewLine
            [System.IO.File]::AppendAllText((Join-Path $dataRoot 'check-feedback-errors.txt'),$detail,[System.Text.UTF8Encoding]::new($false))
        } catch {}
        throw $failure
    }
}

if ($Fallback) {
    $ErrorActionPreference='Stop'; $lock=$null
    $installRoot=if ([string]::IsNullOrWhiteSpace($env:LAST_WORLD_INSTALL_ROOT)) {[System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))} else {[System.IO.Path]::GetFullPath($env:LAST_WORLD_INSTALL_ROOT)}
    $dataRoot=if ([string]::IsNullOrWhiteSpace($env:LAST_WORLD_DATA_DIR)) {Join-Path $installRoot 'Data'} else {[System.IO.Path]::GetFullPath($env:LAST_WORLD_DATA_DIR)}
    $locksRoot=Join-Path $dataRoot 'Locks';[void][System.IO.Directory]::CreateDirectory($locksRoot)
    try {$lock=[System.IO.FileStream]::new((Join-Path $locksRoot 'Run-Checks.lock'),[System.IO.FileMode]::OpenOrCreate,[System.IO.FileAccess]::ReadWrite,[System.IO.FileShare]::None)} catch {exit 2}
    try {
        $folder=$null; $owners=@(); $pointer=Join-Path $dataRoot 'check-results/current-suite.json'
        $recoveryNote=''
        try {
            if (Test-Path -LiteralPath $pointer) {
                $p=Get-Content -LiteralPath $pointer -Raw | ConvertFrom-Json
                if ([string]$p.run_id -match '^[A-Za-z0-9_-]+$') {
                    $folder=Join-Path (Join-Path $dataRoot 'check-results') ([string]$p.run_id)
                    $journal=Join-Path $folder 'suite-journal.json'
                    if (Test-Path -LiteralPath $journal) {$j=Get-Content -LiteralPath $journal -Raw | ConvertFrom-Json;$owners=@($j.capture_owners)}
                }
            }
        } catch {$recoveryNote='; corrupt pointer/journal metadata: '+$_.Exception.Message}
        $reason='feedback_recovery; supervisor_exit='+$env:CHECKS_EXIT+'; watchdog_restarts='+$env:CHECK_WATCHDOG_ATTEMPT+'; no suite pass inferred'
        [void](Make-LastWorldFeedback $installRoot $folder $owners ($reason+$recoveryNote))
        exit 0
    } catch {Write-Error -ErrorAction Continue $_;exit 1} finally {if ($null -ne $lock) {$lock.Dispose()}}
}
