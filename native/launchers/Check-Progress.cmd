@echo off
setlocal EnableExtensions DisableDelayedExpansion
set "LW_PROGRESS_SELF=%~f0"
set "LW_PROGRESS_ROOT=%~dp0"
set "LW_PROGRESS_PS=%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe"
if exist "%SystemRoot%\Sysnative\WindowsPowerShell\v1.0\powershell.exe" set "LW_PROGRESS_PS=%SystemRoot%\Sysnative\WindowsPowerShell\v1.0\powershell.exe"
"%LW_PROGRESS_PS%" -NoProfile -ExecutionPolicy Bypass -Command "$all=[IO.File]::ReadAllText($env:LW_PROGRESS_SELF); $marker='# LAST_WORLD_PROGRESS_POWERSHELL'; & ([scriptblock]::Create($all.Substring($all.LastIndexOf($marker)+$marker.Length)))"
set "LW_PROGRESS_EXIT=%errorlevel%"
echo.
echo This collector does not stop or restart your checks. Send Send-Back-Progress.zip.
pause
exit /b %LW_PROGRESS_EXIT%
# LAST_WORLD_PROGRESS_POWERSHELL
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath($env:LW_PROGRESS_ROOT).TrimEnd([char[]]@(92,47))
if ((Split-Path -Leaf $root) -eq 'Tools' -and (Test-Path -LiteralPath (Join-Path (Split-Path -Parent $root) 'App') -PathType Container)) {$root=Split-Path -Parent $root}
$utf8=[Text.UTF8Encoding]::new($false)
$snapshots=New-Object 'System.Collections.Generic.Dictionary[string,byte[]]'
$notes=New-Object 'System.Collections.Generic.List[string]'
$lines=New-Object 'System.Collections.Generic.List[string]'
function Say([string]$Line) {$lines.Add($Line);Write-Host $Line}
function Owned-File([string]$Path) {
    $full=[IO.Path]::GetFullPath($Path)
    if (-not $full.StartsWith($root+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) {throw 'A diagnostic path escaped this installation.'}
    $item=Get-Item -LiteralPath $full -Force -ErrorAction SilentlyContinue
    if ($null -eq $item -or $item.PSIsContainer) {return $null}
    $ancestor=$item
    while ($null -ne $ancestor -and $ancestor.FullName.TrimEnd([char[]]@(92,47)) -ne $root) {
        if (($ancestor.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {throw 'A diagnostic path contains a reparse point.'}
        if ($ancestor.PSIsContainer) {$ancestor=$ancestor.Parent} else {$ancestor=$ancestor.Directory}
    }
    return $item
}
function Read-Snapshot([string]$Path,[long]$Limit,[switch]$Tail) {
    $item=Owned-File $Path;if ($null -eq $item) {return $null}
    $stream=$null
    try {
        $stream=[IO.FileStream]::new($item.FullName,[IO.FileMode]::Open,[IO.FileAccess]::Read,([IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete))
        if (-not $Tail -and $stream.Length -gt $Limit) {throw ('Diagnostic exceeds '+$Limit+' bytes: '+$item.Name)}
        $length=[int][Math]::Min($stream.Length,$Limit)
        if ($Tail -and $stream.Length -gt $length) {[void]$stream.Seek(-$length,[IO.SeekOrigin]::End)}
        $buffer=New-Object byte[] $length;$read=0
        while ($read -lt $length) {$count=$stream.Read($buffer,$read,$length-$read);if ($count -le 0) {break};$read+=$count}
        if ($read -ne $length) {throw 'Diagnostic changed length during its bounded snapshot.'}
        return @{path=$item.FullName;bytes=$buffer;modified_utc=$item.LastWriteTimeUtc;tail=[bool]$Tail}
    } finally {if ($null -ne $stream) {$stream.Dispose()}}
}
function Keep-Snapshot($Snapshot,[string]$Suffix='') {
    if ($null -eq $Snapshot) {return}
    $relative=$Snapshot.path.Substring($root.Length+1).Replace([char]92,[char]47)+$Suffix
    $snapshots[$relative]=$Snapshot.bytes
}
function Read-Json([string]$Path,[long]$Limit) {
    $snapshot=Read-Snapshot $Path $Limit
    if ($null -eq $snapshot) {return $null}
    return @{snapshot=$snapshot;value=($utf8.GetString($snapshot.bytes)|ConvertFrom-Json)}
}
function Owner-Sample($Owner,[string]$Executable) {
    $result=@{state='unavailable';alive=$null;verified=$false;cpu_seconds=$null;working_set_bytes=$null;private_bytes=$null;observed_utc=[DateTime]::UtcNow.ToString('o')}
    if ($null -eq $Owner) {$result.reason='No durable child owner was recorded.';return $result}
    $result.process_id=[int]$Owner.process_id
    $process=Get-Process -Id ([int]$Owner.process_id) -ErrorAction SilentlyContinue
    if ($null -eq $process) {$result.state='absent';$result.alive=$false;return $result}
    try {
        $result.alive=$true
        if ($process.StartTime.ToUniversalTime().Ticks.ToString() -ne [string]$Owner.start_utc_ticks) {$result.state='pid_reused';$result.reason='Observed PID belongs to another start time.';return $result}
        $expected=[IO.Path]::GetFullPath([string]$Owner.exe_path)
        $allowed=@((Join-Path $root $Executable),(Join-Path (Join-Path $root 'App') $Executable))
        if ($expected -notin $allowed -or [string]::IsNullOrWhiteSpace($process.Path) -or -not [string]::Equals([IO.Path]::GetFullPath($process.Path),$expected,[StringComparison]::OrdinalIgnoreCase)) {throw 'Executable path does not match this installation and journal.'}
        $file=Owned-File $expected;if ($null -eq $file) {throw 'Owned executable is unavailable.'}
        if ((Get-FileHash -LiteralPath $expected -Algorithm SHA256).Hash.ToLowerInvariant() -ne [string]$Owner.exe_sha256) {throw 'Executable hash differs from the durable owner.'}
        $process.Refresh();$result.state='verified_alive';$result.verified=$true
        $result.cpu_seconds=$process.TotalProcessorTime.TotalSeconds;$result.working_set_bytes=$process.WorkingSet64;$result.private_bytes=$process.PrivateMemorySize64
    } catch {$result.state='identity_uncertain';$result.reason=$_.Exception.Message} finally {$process.Dispose()}
    return $result
}
function Progress-Sample($Context,$Phase,$Attempt) {
    $sample=@{observed_utc=[DateTime]::UtcNow.ToString('o');owner=(Owner-Sample $Attempt.owner ([string]$Phase.executable));progress=$null;age_seconds=$null;file=$null}
    if ([string]$Phase.native_case_id -notmatch '^[A-Za-z0-9_-]+$' -or $null -eq $Attempt.owner) {return $sample}
    $folder=Join-Path $Context.run_folder 'benchmark'
    if (-not (Test-Path -LiteralPath $folder -PathType Container)) {return $sample}
    $prefix=[string]$Phase.native_case_id+'-'+[int]$Attempt.owner.process_id+'-'
    foreach ($item in @(Get-ChildItem -LiteralPath $folder -Filter ($prefix+'*-progress.json') -File|Sort-Object LastWriteTimeUtc -Descending|Select-Object -First 8)) {
        try {
            $parsed=Read-Json $item.FullName 1MB;$value=$parsed.value
            if ($value.case_id -cne $Phase.native_case_id -or [long]$value.process_id -ne [long]$Attempt.owner.process_id -or $item.Name -cne ([string]$value.session_id+'-progress.json') -or [long]$value.started_utc_ms -lt ([long]$Attempt.started_unix_ms-2000) -or [long]$value.started_utc_ms -gt ([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()+2000)) {continue}
            $sample.progress=$value.progress;$sample.native_status=$value.status;$sample.session_id=$value.session_id;$sample.requested_rate=$value.requested_rate
            $sample.age_seconds=[Math]::Round(([DateTime]::UtcNow-$parsed.snapshot.modified_utc).TotalSeconds,1);$sample.file=$item.Name
            Keep-Snapshot $parsed.snapshot
            return $sample
        } catch {$notes.Add('Progress snapshot: '+$_.Exception.Message)}
    }
    return $sample
}
try {
    Say 'Last-World read-only live progress check (two samples, five seconds apart).'
    Say 'Your running checks continue. Existing Send-Back.zip is unchanged.'
    $contexts=@()
    foreach ($data in @((Join-Path $root 'Data'),$root)) {
        try {
            $pointer=Read-Json (Join-Path $data 'check-results/current-suite.json') 1MB
            if ($null -eq $pointer -or [string]$pointer.value.run_id -notmatch '^[A-Za-z0-9_-]+$') {continue}
            $runFolder=Join-Path (Join-Path $data 'check-results') ([string]$pointer.value.run_id)
            $journal=Read-Json (Join-Path $runFolder 'suite-journal.json') 16MB
            if ($null -eq $journal) {continue}
            $active=@($journal.value.phases|ForEach-Object {$phase=$_;foreach ($attempt in @($phase.attempts|Where-Object {$_.status -in @('launching','running')})) {[pscustomobject]@{phase=$phase;attempt=$attempt}}}|Sort-Object @{Expression={[long]$_.attempt.started_unix_ms}} -Descending|Select-Object -First 1)
            $contexts+=@{data=$data;run_folder=$runFolder;pointer=$pointer;journal=$journal;active=$active;active_started=if ($active.Count) {[long]$active[0].attempt.started_unix_ms} else {0}}
        } catch {$notes.Add('Suite metadata: '+$_.Exception.Message)}
    }
    $context=@($contexts|Sort-Object @{Expression={$_.active_started}},@{Expression={$_.journal.snapshot.modified_utc}} -Descending|Select-Object -First 1)
    if ($context.Count -ne 1) {throw 'No readable suite journal found beside this collector in check-results or Data/check-results.'}
    $context=$context[0];Keep-Snapshot $context.pointer.snapshot;Keep-Snapshot $context.journal.snapshot
    $samples=@();$classification='No active attempt is recorded; inspect the journal status.';$target=$null;$phase=$null;$attempt=$null
    Say ('Run: '+$context.journal.value.run_id+' | suite status: '+$context.journal.value.status)
    if ($context.active.Count -gt 0) {
        $phase=$context.active[0].phase;$attempt=$context.active[0].attempt
        Say ('Phase: '+$phase.id+' | attempt '+$attempt.number+'/2 | PID '+$attempt.owner.process_id)
        $planPath=if ($context.data -eq $root) {Join-Path $root 'benchmark-plan.json'} else {Join-Path (Join-Path $root 'App') 'benchmark-plan.json'}
        try {$plan=Read-Json $planPath 1MB;if ($null -ne $plan) {$case=@($plan.value.cases|Where-Object {$_.id -ceq $phase.native_case_id}|Select-Object -First 1);if ($case.Count -eq 1) {$target=$case[0].target_ticks};Keep-Snapshot $plan.snapshot}} catch {$notes.Add($_.Exception.Message)}
        if ($null -eq $target -and $phase.native_case_id -ceq 'cpu-season-78421') {$target=72000}
        $watch=[Diagnostics.Stopwatch]::StartNew()
        $samples+=Progress-Sample $context $phase $attempt
        Start-Sleep -Seconds 5
        $samples+=Progress-Sample $context $phase $attempt
        $watch.Stop();$first=$samples[0];$last=$samples[1]
        foreach ($sample in $samples) {
            $tick=if ($null -ne $sample.progress) {$sample.progress.simulated_ticks} else {'unavailable'}
            $percent=if ($null -ne $sample.progress -and $null -ne $target -and [double]$target -gt 0) {([Math]::Round(100.0*[double]$tick/[double]$target,2)).ToString()+'%'} else {'n/a'}
            Say ('Sample '+$sample.observed_utc+' | ticks '+$tick+'/'+$target+' ('+$percent+') | actual rate '+$sample.progress.achieved_simulation_rate+'x at 20 Hz | debt '+$sample.progress.pending_ticks+' ticks | progress age '+$sample.age_seconds+' s | owner '+$sample.owner.state)
        }
        $delta=$null;if ($null -ne $first.progress -and $null -ne $last.progress) {$delta=[long]$last.progress.simulated_ticks-[long]$first.progress.simulated_ticks}
        $cpu=$null;if ($first.owner.verified -and $last.owner.verified) {$cpu=[Math]::Round(100.0*([double]$last.owner.cpu_seconds-[double]$first.owner.cpu_seconds)/$watch.Elapsed.TotalSeconds,1)}
        $cpuText=if ($null -eq $cpu) {'unavailable'} else {$cpu.ToString()+'% of one logical CPU'}
        $ramText=if ($null -eq $last.owner.working_set_bytes) {'unavailable'} else {[Math]::Round([double]$last.owner.working_set_bytes/1MB,1).ToString()+' MiB'}
        $privateText=if ($null -eq $last.owner.private_bytes) {'unavailable'} else {[Math]::Round([double]$last.owner.private_bytes/1MB,1).ToString()+' MiB'}
        Say ('Observed '+[Math]::Round($watch.Elapsed.TotalSeconds,1)+' s | tick advance '+$delta+' | process CPU '+$cpuText+' | RAM '+$ramText+' | private '+$privateText)
        if (-not $last.owner.verified) {$classification='Child is absent or its ownership cannot be verified; no running/stuck conclusion is safe.'}
        elseif ($null -ne $last.progress -and $null -ne $target -and [double]$target -gt 0 -and [long]$last.progress.simulated_ticks -ge [long]$target) {$classification='Target ticks reached. The verified child may be validating, saving a checkpoint, writing its report, or finishing I/O; this is not proof of a hang.'}
        elseif ($null -ne $delta -and $delta -gt 0) {$classification='ADVANCING: native processed ticks increased during this observation.'}
        elseif ($null -ne $delta) {$classification='NO TICK ADVANCE OBSERVED in this short window. This alone does not prove a hang; CPU and progress age are recorded for review.'}
        else {$classification='No matching native progress snapshot was readable. The verified process counters are recorded; no hang conclusion is inferred.'}
        if ([string]$phase.id -match '^[A-Za-z0-9_-]+$' -and [int]$attempt.number -in @(1,2)) {
            $stem=Join-Path $context.run_folder ('cases/'+$phase.id+'/attempt-'+[int]$attempt.number)
            foreach ($suffix in @('-stdout.txt','-stderr.txt','-hardware.jsonl','-hardware-summary.json')) {try {Keep-Snapshot (Read-Snapshot ($stem+$suffix) 64KB -Tail) '.tail.txt'} catch {$notes.Add($_.Exception.Message)}}
        }
        if ($null -ne $last.session_id -and [string]$last.session_id -match '^[A-Za-z0-9_-]+$') {try {Keep-Snapshot (Read-Snapshot (Join-Path $context.run_folder ('benchmark/'+$last.session_id+'-report.json')) 1MB)} catch {$notes.Add($_.Exception.Message)}}
    }
    Say $classification
    foreach ($name in @('console.txt','pc-host-history.jsonl','benchmark-history.jsonl','check-launcher-errors.txt','check-feedback-errors.txt')) {try {Keep-Snapshot (Read-Snapshot (Join-Path $context.data $name) 64KB -Tail) '.tail.txt'} catch {$notes.Add($_.Exception.Message)}}
    foreach ($name in @('build-manifest.json','native-build-stamp.json')) {foreach ($base in @((Join-Path $root 'App'),$root)) {try {Keep-Snapshot (Read-Snapshot (Join-Path $base $name) 1MB)} catch {$notes.Add($_.Exception.Message)}}}
    $status=@{schema_version=1;read_only_observation=$true;created_utc=[DateTime]::UtcNow.ToString('o');run_id=$context.journal.value.run_id;phase_id=if ($null -ne $phase) {$phase.id} else {$null};target_ticks=$target;classification=$classification;samples=$samples;warnings=@($notes.ToArray());existing_send_back_zip_unchanged=$true;checkpoint_state_included=$false}
    $snapshots['progress-status.json']=$utf8.GetBytes(($status|ConvertTo-Json -Depth 16))
    [void](Owned-File (Join-Path $root 'Progress.txt'))
    [IO.File]::WriteAllText((Join-Path $root 'Progress.txt'),($lines -join [Environment]::NewLine)+[Environment]::NewLine,$utf8)
    $snapshots['Progress.txt']=$utf8.GetBytes(($lines -join [Environment]::NewLine)+[Environment]::NewLine)
    Add-Type -AssemblyName System.IO.Compression
    $temporary=Join-Path $root ('Send-Back-Progress-'+$PID+'.zip.part');$destination=Join-Path $root 'Send-Back-Progress.zip'
    [void](Owned-File $temporary);[void](Owned-File $destination)
    $stream=$null;$archive=$null
    try {
        $stream=[IO.FileStream]::new($temporary,[IO.FileMode]::Create,[IO.FileAccess]::Write,[IO.FileShare]::None)
        $archive=[IO.Compression.ZipArchive]::new($stream,[IO.Compression.ZipArchiveMode]::Create,$true)
        foreach ($name in $snapshots.Keys) {$entry=$archive.CreateEntry($name,[IO.Compression.CompressionLevel]::Optimal);$output=$entry.Open();try {$bytes=$snapshots[$name];$output.Write($bytes,0,$bytes.Length)} finally {$output.Dispose()}}
    } finally {if ($null -ne $archive) {$archive.Dispose()};if ($null -ne $stream) {try {$stream.Flush($true)} finally {$stream.Dispose()}}}
    if (Test-Path -LiteralPath $destination) {[IO.File]::Replace($temporary,$destination,[NullString]::Value)} else {[IO.File]::Move($temporary,$destination)}
    Say ('Saved Progress.txt and '+$destination)
    exit 0
} catch {
    $message='Progress collector could not complete: '+$_.Exception.Message
    Say $message
    try {[IO.File]::WriteAllText((Join-Path $root 'Progress.txt'),($lines -join [Environment]::NewLine)+[Environment]::NewLine+($_|Out-String),$utf8)} catch {}
    exit 1
}
