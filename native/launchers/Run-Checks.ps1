# Windows PowerShell 5.1. One resumable comprehensive suite; no PC Python/Cargo.
$ErrorActionPreference='Stop'
$toolsRoot=[System.IO.Path]::GetFullPath($PSScriptRoot)
$installRoot=if ([string]::IsNullOrWhiteSpace($env:LAST_WORLD_INSTALL_ROOT)) {[System.IO.Path]::GetFullPath((Join-Path $toolsRoot '..'))} else {[System.IO.Path]::GetFullPath($env:LAST_WORLD_INSTALL_ROOT)}
$appRoot=Join-Path $installRoot 'App'
$dataRoot=if ([string]::IsNullOrWhiteSpace($env:LAST_WORLD_DATA_DIR)) {Join-Path $installRoot 'Data'} else {[System.IO.Path]::GetFullPath($env:LAST_WORLD_DATA_DIR)}
[void][System.IO.Directory]::CreateDirectory($dataRoot)
$locksRoot=Join-Path $dataRoot 'Locks';[void][System.IO.Directory]::CreateDirectory($locksRoot)
$env:LAST_WORLD_INSTALL_ROOT=$installRoot;$env:LAST_WORLD_DATA_DIR=$dataRoot
Set-Location -LiteralPath $dataRoot
$utf8=[System.Text.UTF8Encoding]::new($false)
$nl=[Environment]::NewLine
$batchLock=$null
try {$batchLock=[System.IO.FileStream]::new((Join-Path $locksRoot 'Run-Checks.lock'),[System.IO.FileMode]::OpenOrCreate,[System.IO.FileAccess]::ReadWrite,[System.IO.FileShare]::None)} catch {Write-Host 'Another check supervisor is active, or its lock is unavailable.';exit 2}
$script:journal=$null; $script:runFolder=$null; $script:runId='preparation-'+$PID
$script:ownedChild=$null; $script:unsafeCleanup=$false; $script:memoryGuard=2GB
$script:manifest=$null; $script:catalog=$null; $script:exeHashes=@{}
$script:benchmarkSourceIdentity=$null
$script:pendingConsole=New-Object 'System.Collections.Generic.List[string]'
$script:terminalExit=12; $script:bundleCreated=$false
$workRoot=Join-Path $dataRoot 'check-results'
[void][System.IO.Directory]::CreateDirectory($workRoot)
$pointerPath=Join-Path $workRoot 'current-suite.json'
$hostHistory=Join-Path $dataRoot 'pc-host-history.jsonl'

function Unix-Milliseconds {return [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()}
function Hash-Text([string]$Text) {$h=[System.Security.Cryptography.SHA256]::Create();try{return ([BitConverter]::ToString($h.ComputeHash($utf8.GetBytes($Text)))).Replace('-','').ToLowerInvariant()}finally{$h.Dispose()}}
function Record-Event($Event) {
    $Event.run_id=$script:runId;$Event.timestamp_utc=[DateTime]::UtcNow.ToString('o')
    $line=$Event|ConvertTo-Json -Compress -Depth 16
    [System.IO.File]::AppendAllText($hostHistory,$line+$nl,$utf8)
    $script:pendingConsole.Add('[CHECK_HOST] '+$line)
}
function Flush-HostConsole {
    if ($script:unsafeCleanup -or $null -ne $script:ownedChild) {return}
    Check-UnownedPrograms
    if ($script:pendingConsole.Count -gt 0) {[System.IO.File]::AppendAllText((Join-Path $dataRoot 'console.txt'),($script:pendingConsole -join $nl)+$nl,$utf8);$script:pendingConsole.Clear()}
}
function Write-AtomicJson([string]$Path,$Value) {
    $temporary=$Path+'.pending-'+$PID+'-'+[DateTime]::UtcNow.Ticks
    $stream=$null
    try {
        $bytes=$utf8.GetBytes(($Value|ConvertTo-Json -Depth 40)+$nl)
        $stream=[System.IO.FileStream]::new($temporary,[System.IO.FileMode]::CreateNew,[System.IO.FileAccess]::Write,[System.IO.FileShare]::None)
        $stream.Write($bytes,0,$bytes.Length);$stream.Flush($true);$stream.Dispose();$stream=$null
        if (Test-Path -LiteralPath $Path) {[System.IO.File]::Replace($temporary,$Path,$Path+'.previous')} else {[System.IO.File]::Move($temporary,$Path)}
    } finally {if ($null -ne $stream) {$stream.Dispose()};if (Test-Path -LiteralPath $temporary) {Remove-Item -LiteralPath $temporary -Force}}
}
function Read-RecoverableJson([string]$Path) {
    try {return Get-Content -LiteralPath $Path -Raw|ConvertFrom-Json} catch {
        $previous=$Path+'.previous'
        if (-not (Test-Path -LiteralPath $previous)) {throw}
        $value=Get-Content -LiteralPath $previous -Raw|ConvertFrom-Json
        if (Test-Path -LiteralPath $Path) {Move-Item -LiteralPath $Path -Destination ($Path+'.corrupt-'+$PID+'-'+[DateTime]::UtcNow.Ticks)}
        Write-AtomicJson $Path $value
        return $value
    }
}
function Save-Journal {
    if ($null -eq $script:journal) {return}
    $script:journal.revision=[long]$script:journal.revision+1
    $script:journal.updated_utc=[DateTime]::UtcNow.ToString('o')
    Write-AtomicJson (Join-Path $script:runFolder 'suite-journal.json') $script:journal
}
function Verify-Payload {
    $path=Join-Path $appRoot 'build-manifest.json'
    $script:manifest=Get-Content -LiteralPath $path -Raw|ConvertFrom-Json
    if ($null -eq $script:manifest.payload_files) {throw 'Copy the complete current Last-World update folder; an owned-payload manifest is required.'}
    foreach ($entry in $script:manifest.payload_files.PSObject.Properties) {
        $relative=[string]$entry.Name
        if ([System.IO.Path]::IsPathRooted($relative) -or $relative -match '(^|[\\/])\.\.([\\/]|$)') {throw 'Unsafe manifest path.'}
        $file=Join-Path $installRoot $relative
        if (-not (Test-Path -LiteralPath $file -PathType Leaf)) {throw ('Missing update file: '+$relative)}
        if ((Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant() -ne [string]$entry.Value.sha256) {throw ('Mixed or modified update file: '+$relative)}
    }
    foreach ($name in @('Last-World.exe','Last-World-Mechanisms.exe','Last-World-Cli-Checks.exe')) {
        $entry=$script:manifest.payload_files.PSObject.Properties['App/'+$name]
        if ($null -eq $entry) {throw ('Executable missing from owned payload: '+$name)}
        $script:exeHashes[$name]=[string]$entry.Value.sha256
    }
    if ($script:exeHashes['Last-World.exe'] -ne [string]$script:manifest.executable_sha256) {throw 'Main executable identity disagrees with manifest.'}
    if ($null -eq $script:manifest.payload_files.PSObject.Properties['App/native-build-stamp.json']) {throw 'Adjacent native build stamp is missing from the owned payload.'}
    $stamp=Get-Content -LiteralPath (Join-Path $appRoot 'native-build-stamp.json') -Raw|ConvertFrom-Json
    if ($stamp.native_source_sha256 -ne $script:manifest.native_source_sha256 -or $stamp.executable_sha256 -ne $script:exeHashes['Last-World.exe'] -or $stamp.source_frozen_through_build -ne $true) {throw 'Adjacent native build stamp disagrees with the exact packaged executable/source.'}
    $script:benchmarkSourceIdentity=[pscustomobject]@{native_source_sha256=[string]$stamp.native_source_sha256;executable_sha256=[string]$stamp.executable_sha256}
}
function New-Phase([string]$Id,[string]$Kind,[string]$Executable,[string[]]$Arguments,[int]$Timeout) {
    if ($Id -notmatch '^[A-Za-z0-9_-]+$' -or $Timeout -lt 5 -or $Timeout -gt 86400) {throw 'Invalid phase ID/timeout.'}
    return [pscustomobject][ordered]@{id=$Id;sequence=0;kind=$Kind;executable=$Executable;arguments=@($Arguments);timeout_seconds=$Timeout;status='pending';attempts=@();skip_reason=$null;native_case_id=$null;unit_test_name=$null;capture_kind=$null;capture_labels=@();minimum_captures=0;inventory_id=$null}
}
function Add-Phase($Phase) {
    if (@($script:journal.phases|Where-Object {$_.id -eq $Phase.id}).Count -eq 0) {$Phase.sequence=@($script:journal.phases).Count;$script:journal.phases=@($script:journal.phases)+@($Phase)}
}
function Baseline-Definitions {
    $definitions=New-Object 'System.Collections.Generic.List[object]'
    $definitions.Add((New-Phase 'baseline-campaign-cpu' 'baseline' 'Last-World.exe' @('--campaign-report') 600))
    $definitions.Add((New-Phase 'baseline-ground-cpu' 'baseline' 'Last-World.exe' @('--ground-report') 180))
    foreach ($backend in @('vulkan','dx12')) {$definitions.Add((New-Phase ('baseline-movement-'+$backend) 'baseline' 'Last-World.exe' @('--headless','--backend',$backend,'--counts','1000,10000,100000','--seconds','3','--check') 120))}
    $river=New-Phase 'baseline-river-render' 'baseline' 'Last-World.exe' @('--river-check','--speed','4') 180;$river.capture_kind='river-check';$river.capture_labels=@('start','dam','drained');$river.minimum_captures=3;$definitions.Add($river)
    $campaign=New-Phase 'baseline-campaign-render' 'baseline' 'Last-World.exe' @('--campaign-check','--speed','4') 240;$campaign.capture_kind='campaign-check';$campaign.capture_labels=@('start','built','ecology');$campaign.minimum_captures=18;$definitions.Add($campaign)
    return $definitions.ToArray()
}
function Resolve-CanonicalPhase($Phase) {
    # Journal progress is mutable state, never authority to choose another test
    # or command. Rebuild arguments from the current verified definitions.
    if ($Phase.kind -eq 'unit') {
        $inventory=@($script:journal.unit_inventories|Where-Object {$_.executable -eq $Phase.executable -and $_.source -eq 'actual_windows_owned_list'}|Select-Object -First 1)
        if ($inventory.Count -ne 1 -or $Phase.unit_test_name -cnotin @($inventory[0].actual_names)) {throw 'Saved unit case is not in its actual Windows inventory.'}
        $entry=@($script:catalog.tests|Where-Object {$_.executable -eq $Phase.executable -and $_.test_name -ceq $Phase.unit_test_name}|Select-Object -First 1)
        $id='unit-'+(Hash-Text ($Phase.executable+'|'+$Phase.unit_test_name)).Substring(0,20)
        if ($entry.Count -eq 1 -and [string]$entry[0].id -match '^[A-Za-z0-9_-]+$') {$id=[string]$entry[0].id}
        if ($Phase.id -cne $id) {throw 'Saved unit ID does not identify its exact test name.'}
        $Phase.arguments=@('--exact',[string]$Phase.unit_test_name,'--test-threads','1','--nocapture')
        $Phase.timeout_seconds=180;if ($entry.Count -eq 1 -and $null -ne $entry[0].timeout_seconds) {$Phase.timeout_seconds=[int]$entry[0].timeout_seconds}
    } elseif ($Phase.kind -eq 'discovery') {if ($Phase.executable -notin @('Last-World-Mechanisms.exe','Last-World-Cli-Checks.exe') -or $Phase.id -cne ('discover-'+[System.IO.Path]::GetFileNameWithoutExtension([string]$Phase.executable))) {throw 'Saved discovery identity changed.'};$Phase.arguments=@('--list');$Phase.timeout_seconds=120}
    elseif ($Phase.kind -eq 'baseline') {
        $definition=@(Baseline-Definitions|Where-Object {$_.id -ceq $Phase.id})
        if ($definition.Count -ne 1 -or $Phase.executable -ne 'Last-World.exe') {throw 'Saved baseline case is not a canonical baseline.'}
        foreach ($field in @('arguments','timeout_seconds','capture_kind','capture_labels','minimum_captures')) {$Phase.$field=$definition[0].$field}
    } elseif ($Phase.kind -eq 'benchmark') {
        $case=@($script:plan.cases|Where-Object {$_.id -ceq $Phase.native_case_id})
        if ($case.Count -ne 1 -or $Phase.id -cne ('benchmark-'+$case[0].id) -or $Phase.executable -ne 'Last-World.exe') {throw 'Saved native benchmark definition changed.'}
        $Phase.arguments=@('--benchmark-case',[string]$case[0].id,'--benchmark-output',('check-results/'+$script:runId+'/benchmark'))
        if ($null -ne $case[0].backend) {$Phase.arguments+=@('--backend',[string]$case[0].backend)}
        $Phase.timeout_seconds=[int]$case[0].timeout_seconds
    } else {throw 'Unknown phase kind.'}
    if ([int]$Phase.timeout_seconds -lt 5 -or [int]$Phase.timeout_seconds -gt 86400) {throw 'Canonical phase timeout outside allowed bounds.'}
}
function Validate-JournalState {
    if ($script:journal.schema_version -ne 2 -or [string]$script:journal.run_id -notmatch '^[A-Za-z0-9_-]+$') {throw 'Invalid saved suite schema/run ID.'}
    $seen=New-Object 'System.Collections.Generic.HashSet[string]'
    $sequence=0
    foreach ($phase in @($script:journal.phases)) {
        if ([int]$phase.sequence -ne $sequence) {throw 'Saved phase ordering differs from its durable sequence.'};$sequence++
        if ([string]$phase.id -notmatch '^[A-Za-z0-9_-]+$' -or -not $seen.Add([string]$phase.id)) {throw 'Unsafe or duplicate saved phase ID.'}
        if ([string]$phase.executable -notin @('Last-World.exe','Last-World-Mechanisms.exe','Last-World-Cli-Checks.exe') -or $phase.kind -notin @('discovery','unit','baseline','benchmark')) {throw 'Saved phase executable/kind is not allowed.'}
        if (@($phase.attempts).Count -gt 2 -or [int]$phase.timeout_seconds -lt 5 -or [int]$phase.timeout_seconds -gt 86400) {throw 'Saved phase attempt/timeout bounds invalid.'}
        foreach ($attempt in @($phase.attempts)) {
            if ([int]$attempt.number -lt 1 -or [int]$attempt.number -gt 2) {throw 'Saved attempt number invalid.'}
            $stem='cases/'+$phase.id+'/attempt-'+$attempt.number
            if ($attempt.stdout -ne ($stem+'-stdout.txt') -or $attempt.stderr -ne ($stem+'-stderr.txt')) {throw 'Saved log paths escaped their owned phase.'}
        }
        if ($phase.kind -eq 'unit' -and [string]$phase.unit_test_name -notmatch '^[A-Za-z0-9_:]+$') {throw 'Unsafe saved unit test name.'}
        if ($phase.kind -eq 'benchmark' -and @($script:plan.cases|Where-Object {$_.id -eq $phase.native_case_id}).Count -ne 1) {throw 'Saved native case is absent from the current plan.'}
    }
}
function Synchronize-RejectedAttempts {
    if ($null -eq $script:journal) {return}
    $rows=New-Object 'System.Collections.Generic.List[object]'
    foreach ($phase in @($script:journal.phases|Where-Object {$_.kind -eq 'benchmark'})) {
        foreach ($attempt in @($phase.attempts|Where-Object {$_.status -notin @('passed','launching','running')})) {
            $rows.Add([pscustomobject]@{case_id=[string]$phase.native_case_id;attempt_index=[int]$attempt.number;source_identity=$script:benchmarkSourceIdentity;reason=([string]$attempt.status+': '+[string]$attempt.error);process_id=if ($null -ne $attempt.owner) {$attempt.owner.process_id} else {$null};session_id=$attempt.native_session_id;started_utc_ms=$attempt.started_unix_ms;recorded_utc=$attempt.finished_utc})
        }
    }
    $folder=Join-Path $script:runFolder 'benchmark';[void][System.IO.Directory]::CreateDirectory($folder)
    Write-AtomicJson (Join-Path $folder 'rejected-attempts.json') @{schema_version=1;rejections=@($rows.ToArray())}
}
function Process-Identity($Owner) {
    if ($null -eq $Owner) {return @{state='uncertain';reason='launch was not journaled with a PID'}}
    $process=Get-Process -Id ([int]$Owner.process_id) -ErrorAction SilentlyContinue
    if ($null -eq $process) {return @{state='absent'}}
    try {
        $start=$process.StartTime.ToUniversalTime().Ticks.ToString()
        if ($start -ne [string]$Owner.start_utc_ticks) {$process.Dispose();return @{state='absent';reason='PID reused; never killed'}}
        $path=$process.Path
        if ([string]::IsNullOrWhiteSpace($path) -or -not [string]::Equals([System.IO.Path]::GetFullPath($path),[string]$Owner.exe_path,[StringComparison]::OrdinalIgnoreCase)) {$process.Dispose();return @{state='uncertain';reason='exact executable path unavailable/different'}}
        $hash=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($hash -ne [string]$Owner.exe_sha256) {$process.Dispose();return @{state='uncertain';reason='executable bytes no longer match journal'}}
        return @{state='match';process=$process}
    } catch {$process.Dispose();return @{state='uncertain';reason=$_.Exception.Message}}
}
function Recover-InterruptedAttempts {
    foreach ($phase in @($script:journal.phases)) {
        foreach ($attempt in @($phase.attempts|Where-Object {$_.status -in @('launching','running')})) {
            if ($null -ne $attempt.owner) {
                $identity=Process-Identity $attempt.owner
                if ($identity.state -eq 'uncertain') {$script:unsafeCleanup=$true;throw ('Owned orphan identity uncertain: '+$identity.reason)}
                if ($identity.state -eq 'match') {
                    try {$identity.process.Kill();if (-not $identity.process.WaitForExit(10000)) {throw 'Verified orphan did not exit.'}}
                    finally {$identity.process.Dispose()}
                }
            }
            $attempt.status='supervisor_interrupted';$attempt.finished_utc=[DateTime]::UtcNow.ToString('o');$attempt.error='Supervisor ended before a verified result; no pass inferred.'
            $phase.status='pending'
            if (@($phase.attempts).Count -ge 2) {$phase.status='skipped_after_failures';$phase.skip_reason='two attempts ended without verified success'}
            Record-Event @{event='interrupted_attempt_recovered';phase=$phase.id;attempt=$attempt.number}
        }
    }
    Synchronize-RejectedAttempts;Save-Journal
}
function Check-UnownedPrograms {
    foreach ($name in $script:exeHashes.Keys) {
        foreach ($process in @(Get-Process -Name ([System.IO.Path]::GetFileNameWithoutExtension($name)) -ErrorAction SilentlyContinue)) {
            try {
                $path=$process.Path
                if ([string]::IsNullOrWhiteSpace($path)) {$script:unsafeCleanup=$true;throw 'A matching program is active but its path cannot be verified.'}
                if ([string]::Equals([System.IO.Path]::GetFullPath($path),(Join-Path $appRoot $name),[StringComparison]::OrdinalIgnoreCase)) {$script:unsafeCleanup=$true;throw 'Another program from this folder is active without a recoverable owned identity. It was not killed.'}
            } finally {$process.Dispose()}
        }
    }
}
function Discover-UnitCases($Phase,[string]$Stdout) {
    $names=New-Object 'System.Collections.Generic.List[string]'
    foreach ($line in [System.IO.File]::ReadLines($Stdout)) {
        if ($line -match '^(.+): test$') {
            $name=$Matches[1]
            if ($name -notmatch '^[A-Za-z0-9_:]+$') {throw ('Unsupported actual Rust test name: '+$name)}
            $names.Add($name)
        }
    }
    if ($names.Count -eq 0 -or @($names|Select-Object -Unique).Count -ne $names.Count) {throw 'Windows test discovery returned zero or duplicate test names.'}
    $expected=@($script:catalog.tests|Where-Object {$_.executable -eq $Phase.executable}|ForEach-Object {$_.test_name})
    $actual=@($names.ToArray()|Sort-Object)
    $missing=@($expected|Where-Object {$_ -cnotin $actual});$extra=@($actual|Where-Object {$_ -cnotin $expected})
    $inventory=[pscustomobject]@{executable=$Phase.executable;source='actual_windows_owned_list';actual_names=$actual;expected_host_names=$expected;missing_from_windows=$missing;additional_windows_names=$extra;actual_count=$actual.Count;inventory_drift=($missing.Count -gt 0 -or $extra.Count -gt 0)}
    $script:journal.unit_inventories=@($script:journal.unit_inventories|Where-Object {$_.executable -ne $Phase.executable})+@($inventory)
    foreach ($name in $actual) {
        $id='unit-'+(Hash-Text ($Phase.executable+'|'+$name)).Substring(0,20)
        $timeout=180
        $entry=@($script:catalog.tests|Where-Object {$_.executable -eq $Phase.executable -and $_.test_name -ceq $name}|Select-Object -First 1)
        if ($entry.Count -gt 0) {if ($null -ne $entry[0].timeout_seconds) {$timeout=[int]$entry[0].timeout_seconds};if ([string]$entry[0].id -match '^[A-Za-z0-9_-]+$') {$id=[string]$entry[0].id}}
        $unit=New-Phase $id 'unit' $Phase.executable @('--exact',$name,'--test-threads','1','--nocapture') $timeout
        $unit.unit_test_name=$name;Add-Phase $unit
    }
    Write-AtomicJson (Join-Path $script:runFolder ($Phase.id+'-actual-windows-inventory.json')) $inventory
}
function Skip-UnverifiedBinaryCases($Phase) {
    $reason='Windows discovery failed twice; actual Windows inventory remains unverified. Host-expected names are not passed.'
    foreach ($entry in @($script:catalog.tests|Where-Object {$_.executable -eq $Phase.executable})) {
        $name=[string]$entry.test_name
        if ($name -notmatch '^[A-Za-z0-9_:]+$') {continue}
        $id='unit-'+(Hash-Text ($Phase.executable+'|'+$name)).Substring(0,20)
        if ([string]$entry.id -match '^[A-Za-z0-9_-]+$') {$id=[string]$entry.id}
        $unit=New-Phase $id 'unit' $Phase.executable @('--exact',$name,'--test-threads','1','--nocapture') 180
        $unit.unit_test_name=$name;$unit.status='skipped_after_failures';$unit.skip_reason=$reason;Add-Phase $unit
    }
    foreach ($unit in @($script:journal.phases|Where-Object {$_.kind -eq 'unit' -and $_.executable -eq $Phase.executable -and $_.status -ne 'passed'})) {$unit.status='skipped_after_failures';$unit.skip_reason=$reason}
    $script:journal.unit_inventories=@($script:journal.unit_inventories|Where-Object {$_.executable -ne $Phase.executable})+@([pscustomobject]@{executable=$Phase.executable;source='discovery_failed';actual_names=@();expected_host_names=@($script:catalog.tests|Where-Object {$_.executable -eq $Phase.executable}|ForEach-Object {$_.test_name});actual_count=0;inventory_drift=$null;reason=$reason})
}
function Verify-AttemptArtifacts($Phase,$Attempt) {
    $stderr=Join-Path $script:runFolder $Attempt.stderr
    if (@(Select-String -LiteralPath $stderr -Pattern '\[(FATAL|PANIC|GPU_ERROR|SURFACE_ERROR)\]').Count -gt 0) {throw 'Native fatal/panic/GPU/surface error tags; no pass inferred.'}
    $stdout=Join-Path $script:runFolder $Attempt.stdout
    if ($Phase.kind -eq 'discovery') {Discover-UnitCases $Phase $stdout;return}
    if ($Phase.kind -eq 'unit') {
        $summary=@(Select-String -LiteralPath $stdout -Pattern '^test result: ok\. 1 passed; 0 failed; 0 ignored;')
        $named=@(Select-String -LiteralPath $stdout -CaseSensitive -Pattern ('^test '+[Regex]::Escape([string]$Phase.unit_test_name)+' \.\.\.'))
        if ($summary.Count -ne 1 -or $named.Count -ne 1) {throw 'Exact Rust invocation did not prove its named single passed test (zero/other-test exits never pass).'}
        return
    }
    if ($Phase.kind -eq 'benchmark') {
        $folder=Join-Path $script:runFolder 'benchmark'
        $files=@(Get-ChildItem -LiteralPath $folder -Filter ($Phase.native_case_id+'-'+$Attempt.owner.process_id+'-*-report.json') -File)
        $valid=New-Object 'System.Collections.Generic.List[object]'
        foreach ($file in $files) {
            $report=Get-Content -LiteralPath $file.FullName -Raw|ConvertFrom-Json
            if ($report.case_id -cne $Phase.native_case_id -or [long]$report.process_id -ne [long]$Attempt.owner.process_id) {continue}
            if ([long]$report.started_utc_ms -lt ([long]$Attempt.started_unix_ms-2000)) {continue}
            if ([long]$report.started_utc_ms -gt ((Unix-Milliseconds)+2000)) {continue}
            if ([string]::IsNullOrWhiteSpace([string]$report.session_id) -or $file.Name -ne ([string]$report.session_id+'-report.json')) {continue}
            $valid.Add(@{file=$file;report=$report})
        }
        if ($valid.Count -ne 1) {throw 'Missing, ambiguous, stale or mismatched owned native benchmark report.'}
        $Attempt.native_report=$valid[0].file.Name;$Attempt.native_session_id=[string]$valid[0].report.session_id
        if ([int]$valid[0].report.attempt -ne [int]$Attempt.number) {throw 'Native report did not honor the durable benchmark attempt index.'}
        $nativeHash=[string]$valid[0].report.source_identity.native_source_sha256;$executableHash=[string]$valid[0].report.source_identity.executable_sha256
        if ($nativeHash -notmatch '^[0-9a-fA-F]{64}$' -or $executableHash -notmatch '^[0-9a-fA-F]{64}$' -or -not [string]::Equals($nativeHash,$script:benchmarkSourceIdentity.native_source_sha256,[StringComparison]::OrdinalIgnoreCase) -or -not [string]::Equals($executableHash,$script:benchmarkSourceIdentity.executable_sha256,[StringComparison]::OrdinalIgnoreCase)) {throw 'Native benchmark report has a missing, malformed or foreign source/executable hash pair.'}
        if ([int]$Attempt.number -eq 2 -and $null -ne $valid[0].report.recovered_previous_success) {throw 'Retry2 returned a prior receipt instead of replaying and validating this phase.'}
        if ($valid[0].report.status -ne 'passed' -or $valid[0].report.passed -isnot [bool] -or $valid[0].report.passed -ne $true -or $valid[0].report.checkpoint.committed -isnot [bool] -or $valid[0].report.checkpoint.committed -ne $true) {throw ('Native benchmark did not commit a fully validated successful phase: '+$valid[0].report.status)}
        return
    }
    if ($null -ne $Phase.capture_kind) {
        $prefix=$Phase.capture_kind+'-'+$Attempt.owner.process_id+'-'
        $started=[DateTime]::Parse($Attempt.started_utc).ToUniversalTime()
        $png=@(Get-ChildItem -LiteralPath $dataRoot -Filter ($prefix+'*.png') -File|Where-Object {$_.LastWriteTimeUtc -ge $started.AddSeconds(-2)})
        $reports=@(Get-ChildItem -LiteralPath $dataRoot -Filter ($prefix+'*-report.json') -File|Where-Object {$_.LastWriteTimeUtc -ge $started.AddSeconds(-2)})
        if ($png.Count -lt $Phase.minimum_captures -or $reports.Count -lt 1) {throw 'Owned visual window did not leave its required fresh captures/report.'}
        foreach ($label in $Phase.capture_labels) {if (@($png|Where-Object {$_.Name -like ('*-'+$label+'.png')}).Count -lt 1) {throw ('Missing owned capture: '+$label)}}
        foreach ($file in $reports) {$report=Get-Content -LiteralPath $file.FullName -Raw|ConvertFrom-Json;if ($report.status -ne 'passed' -or $report.conserved -ne $true) {throw 'Visual correctness report failed.'}}
    }
}
function Show-OwnedProgress($Phase,$Attempt,[double]$ElapsedSeconds) {
    $prefix='  '+$Phase.id+' | elapsed '+[Math]::Round($ElapsedSeconds,0)+'s'
    if ($Phase.kind -ne 'benchmark' -or $null -eq $Attempt.owner) {Write-Host $prefix;return}
    try {
        $folder=Join-Path $script:runFolder 'benchmark'
        $files=@(Get-ChildItem -LiteralPath $folder -Filter ($Phase.native_case_id+'-'+$Attempt.owner.process_id+'-*-progress.json') -File -ErrorAction SilentlyContinue|Sort-Object LastWriteTimeUtc -Descending|Select-Object -First 8)
        foreach ($file in $files) {
            if ($file.Length -gt 1MB -or ($file.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {continue}
            $value=Get-Content -LiteralPath $file.FullName -Raw|ConvertFrom-Json
            if ($value.case_id -cne $Phase.native_case_id -or [long]$value.process_id -ne [long]$Attempt.owner.process_id -or $file.Name -cne ([string]$value.session_id+'-progress.json') -or [long]$value.started_utc_ms -lt ([long]$Attempt.started_unix_ms-2000)) {continue}
            $definition=@($script:plan.cases|Where-Object {$_.id -ceq $Phase.native_case_id}|Select-Object -First 1)
            $target=if ($definition.Count -eq 1) {[long]$definition[0].target_ticks} else {0}
            $ticks=[long]$value.progress.simulated_ticks
            $percent=if ($target -gt 0) {([Math]::Round(100.0*$ticks/$target,1)).ToString()+'%'} else {'ramp'}
            $age=[Math]::Round(([DateTime]::UtcNow-$file.LastWriteTimeUtc).TotalSeconds,0)
            Write-Host ($prefix+' | ticks '+$ticks+'/'+$target+' ('+$percent+') | achieved '+[Math]::Round([double]$value.progress.achieved_simulation_rate,2)+'x | debt '+$value.progress.pending_ticks+' | sample age '+$age+'s')
            return
        }
        Write-Host ($prefix+' | waiting for first native progress sample (loading/validation may still be active)')
    } catch {Write-Host ($prefix+' | progress display temporarily unavailable; owned test continues')}
}
function Run-Attempt($Phase) {
    Check-UnownedPrograms
    if (@($Phase.attempts).Count -ge 2) {$Phase.status='skipped_after_failures';$Phase.skip_reason='Two attempts already recorded; no third child is allowed.';if ($Phase.kind -eq 'discovery') {Skip-UnverifiedBinaryCases $Phase};Save-Journal;return}
    Resolve-CanonicalPhase $Phase
    $exe=Join-Path $appRoot $Phase.executable
    if ((Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant() -ne $script:exeHashes[$Phase.executable]) {throw 'Executable changed during the suite; no child was started.'}
    $number=@($Phase.attempts).Count+1
    $benchmarkAttempt=0;if ($Phase.kind -eq 'benchmark') {$benchmarkAttempt=$number}
    $relative='cases/'+$Phase.id+'/attempt-'+$number
    $folder=Join-Path $script:runFolder ('cases/'+$Phase.id)
    [void][System.IO.Directory]::CreateDirectory($folder)
    $attempt=[pscustomobject][ordered]@{number=$number;benchmark_attempt_environment=if ($benchmarkAttempt -eq 0) {$null} else {$benchmarkAttempt.ToString()};temporary_directory=('cases/'+$Phase.id+'/temp-'+$number);status='launching';started_utc=[DateTime]::UtcNow.ToString('o');started_unix_ms=(Unix-Milliseconds);finished_utc=$null;elapsed_seconds=$null;owner=$null;exit_code=$null;error=$null;timed_out=$false;guard_stop=$false;native_crash=$false;stdout=$relative+'-stdout.txt';stderr=$relative+'-stderr.txt';native_report=$null;native_session_id=$null;peak_working_set_bytes=0;peak_private_bytes=0;memory_guard_bytes=$script:memoryGuard;hard_process_commit_guard=$true;hardware_samples=$relative+'-hardware.jsonl';hardware_summary=$relative+'-hardware-summary.json';hardware_telemetry=$null}
    $phaseWatch=$null;$hardwareState=$null
    $Phase.attempts=@($Phase.attempts)+@($attempt);$Phase.status='running';Save-Journal
    Record-Event @{event='attempt_launch_prepared';phase=$Phase.id;attempt=$number;arguments=@($Phase.arguments);memory_guard_bytes=$script:memoryGuard;timeout_seconds=$Phase.timeout_seconds}
    Write-Host ('['+(@($script:journal.phases|Where-Object {$_.status -in @('passed','skipped_after_failures')}).Count)+'/'+@($script:journal.phases).Count+'] '+$Phase.id+' attempt '+$number+'/2')
    try {
        $cwd=$dataRoot
        if ($Phase.kind -in @('unit','discovery')) {$cwd=Join-Path $folder ('cwd-'+$number);[void][System.IO.Directory]::CreateDirectory($cwd)}
        $temporaryDirectory=Join-Path $script:runFolder $attempt.temporary_directory;[void][System.IO.Directory]::CreateDirectory($temporaryDirectory)
        $script:ownedChild=[LastWorldCheckChild]::StartSuspended($exe,[string[]]$Phase.arguments,$cwd,(Join-Path $script:runFolder $attempt.stdout),(Join-Path $script:runFolder $attempt.stderr),[uint64]$script:memoryGuard,[int]$benchmarkAttempt,$temporaryDirectory)
        $process=$script:ownedChild.Process
        $attempt.owner=[pscustomobject]@{process_id=$process.Id;start_utc_ticks=$process.StartTime.ToUniversalTime().Ticks.ToString();exe_path=[System.IO.Path]::GetFullPath($exe);exe_sha256=$script:exeHashes[$Phase.executable]}
        if ($null -ne $Phase.capture_kind) {$script:journal.capture_owners=@($script:journal.capture_owners)+@([pscustomobject]@{prefix=$Phase.capture_kind;process_id=$process.Id;started_utc=$attempt.started_utc})}
        Save-Journal # suspended child cannot execute any test before ownership is durable
        if ($Phase.executable -eq 'Last-World.exe') {
            try {
                if ($null -ne $script:hardwareTelemetryLoadError) {throw $script:hardwareTelemetryLoadError}
                $includeGpu=($Phase.kind -eq 'baseline' -and $Phase.id -notin @('baseline-campaign-cpu','baseline-ground-cpu'))
                if ($Phase.kind -eq 'benchmark') {$definition=@($script:plan.cases|Where-Object {$_.id -ceq $Phase.native_case_id}|Select-Object -First 1);$includeGpu=($definition.Count -eq 1 -and $definition[0].mode -eq 'window')}
                $hardwareState=Start-LastWorldHardwareTelemetry $process $attempt.owner $includeGpu (Join-Path $script:runFolder $attempt.hardware_samples)
                $attempt.hardware_telemetry=@{status=$hardwareState.status;gpu_requested=$includeGpu;setup_ms=$hardwareState.setup_ms}
            } catch {$attempt.hardware_telemetry=@{schema_version=1;status='unavailable_setup';error=$_.Exception.Message;performance_qualification=$false}}
        }
        $phaseWatch=[System.Diagnostics.Stopwatch]::StartNew()
        $script:ownedChild.Resume();$attempt.status='running';Save-Journal
        $nextSampleSeconds=0.0;$nextProgressSeconds=10.0
        while (-not $process.WaitForExit(500)) {
            if ($phaseWatch.Elapsed.TotalSeconds -ge $nextProgressSeconds) {Show-OwnedProgress $Phase $attempt $phaseWatch.Elapsed.TotalSeconds;$nextProgressSeconds=$phaseWatch.Elapsed.TotalSeconds+10.0}
            if ($phaseWatch.Elapsed.TotalSeconds -ge $Phase.timeout_seconds) {$attempt.timed_out=$true;$attempt.status='timeout';throw 'Owned phase exceeded its bounded monotonic wall deadline.'}
            if ($phaseWatch.Elapsed.TotalSeconds -ge $nextSampleSeconds) {
                $nextSampleSeconds=$phaseWatch.Elapsed.TotalSeconds+1.0
                if ($null -ne $hardwareState) {try {Sample-LastWorldHardwareTelemetry $hardwareState $process $phaseWatch.Elapsed.TotalSeconds} catch {$hardwareState.error=$_.Exception.Message;$hardwareState.status='unavailable_after_sample_error'}}
                $working=$null;$private=$null
                try {$process.Refresh();if (-not $process.HasExited) {$working=[long]$process.WorkingSet64;$private=[long]$process.PrivateMemorySize64;$attempt.peak_working_set_bytes=[Math]::Max([long]$attempt.peak_working_set_bytes,[long]$process.PeakWorkingSet64);$attempt.peak_private_bytes=[Math]::Max([long]$attempt.peak_private_bytes,$private)}}
                catch {if (-not $process.HasExited) {Record-Event @{event='memory_sample_unavailable';phase=$Phase.id;attempt=$number;error=$_.Exception.Message;hard_process_commit_guard_still_enforced=$true}}}
                if ($null -ne $working) {Record-Event @{event='owned_process_memory';phase=$Phase.id;attempt=$number;process_id=$process.Id;working_set_bytes=$working;private_bytes=$private;guard_bytes=$script:memoryGuard;note='OS counters, not allocator-only RAM or total VRAM'};if ($working -ge $script:memoryGuard -or $private -ge $script:memoryGuard) {$attempt.guard_stop=$true;$attempt.status='guard_stop';throw 'Explicit owned-process memory guard reached; this is not a measured hardware capacity or intentional machine OOM.'}}
            }
        }
        $process.WaitForExit();$attempt.exit_code=$process.ExitCode
        if ($null -eq $attempt.exit_code) {throw 'Child exit code unavailable.'}
        if ($attempt.exit_code -ne 0) {$attempt.native_crash=([long]$attempt.exit_code -lt 0 -or $attempt.exit_code -eq 101);$attempt.status=if ($attempt.native_crash) {'native_crash'} else {'native_nonzero'};throw ('Child exit '+$attempt.exit_code)}
        Verify-AttemptArtifacts $Phase $attempt
        $attempt.status='passed';$Phase.status='passed'
    } catch {
        if ($attempt.status -in @('launching','running')) {$attempt.status='invalid_result_or_launch_failure'}
        $attempt.error=$_.Exception.Message
    } finally {
        if ($null -ne $script:ownedChild) {
            try {
                $process=$script:ownedChild.Process
                if (-not $process.HasExited) {
                    $identity=Process-Identity $attempt.owner
                    if ($identity.state -eq 'match') {$identity.process.Dispose();$script:ownedChild.Stop();if (-not $process.WaitForExit(10000)) {throw 'Owned job did not exit after stop.'}}
                    elseif ($identity.state -ne 'absent' -or -not $process.WaitForExit(500)) {throw 'Cleanup ownership could not be verified.'}
                }
            } catch {$script:unsafeCleanup=$true;$attempt.status='ownership_uncertain';$attempt.error=$_.Exception.Message}
            finally {try {if ($script:ownedChild.Process.HasExited -and $null -eq $attempt.exit_code) {$attempt.exit_code=$script:ownedChild.Process.ExitCode}}catch{};$script:ownedChild.Dispose();$script:ownedChild=$null}
        }
        if ($Phase.executable -eq 'Last-World.exe') {
            try {
                if ($null -ne $hardwareState) {$attempt.hardware_telemetry=Stop-LastWorldHardwareTelemetry $hardwareState}
                Write-AtomicJson (Join-Path $script:runFolder $attempt.hardware_summary) $attempt.hardware_telemetry
            } catch {try {Record-Event @{event='hardware_telemetry_finalization_unavailable';phase=$Phase.id;attempt=$number;error=$_.Exception.Message}}catch{}}
        }
        $attempt.finished_utc=[DateTime]::UtcNow.ToString('o')
        if ($null -ne $phaseWatch) {$phaseWatch.Stop();$attempt.elapsed_seconds=$phaseWatch.Elapsed.TotalSeconds}
        if ($Phase.status -ne 'passed') {
            if (@($Phase.attempts).Count -ge 2) {$Phase.status='skipped_after_failures';$Phase.skip_reason='Two failed attempts; final reason: '+$attempt.status+' / '+$attempt.error}
            else {$Phase.status='pending'}
        }
        if ($Phase.kind -eq 'discovery' -and $Phase.status -eq 'skipped_after_failures') {Skip-UnverifiedBinaryCases $Phase}
        if ($Phase.kind -eq 'benchmark' -and $attempt.status -ne 'passed') {Synchronize-RejectedAttempts}
        Save-Journal;Record-Event @{event='attempt_finished';phase=$Phase.id;result=$attempt;phase_status=$Phase.status};Flush-HostConsole
    }
}

try {
    Verify-Payload
    $supervisorHash=(Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash.ToLowerInvariant()
    Write-Host ('Last-World checks | organized App/Tools/Data | source '+([string]$script:manifest.native_source_sha256).Substring(0,12)+' | supervisor '+$supervisorHash.Substring(0,12)+' | adaptive telemetry: one collector')
    Record-Event @{event='supervisor_identity';layout='App/Tools/Data';supervisor_sha256=$supervisorHash;native_source_sha256=$script:manifest.native_source_sha256;executable_sha256=$script:manifest.executable_sha256;telemetry_policy='adaptive_one_collector';install_root=$installRoot;data_root=$dataRoot}
    . (Join-Path $toolsRoot 'Run-Checks-OwnedProcess.ps1')
    . (Join-Path $toolsRoot 'Run-Checks-Feedback.ps1')
    $script:hardwareTelemetryLoadError=$null
    $hardwareLoadWatch=[System.Diagnostics.Stopwatch]::StartNew()
    try {. (Join-Path $toolsRoot 'Run-Checks-GpuTelemetry.ps1')} catch {$script:hardwareTelemetryLoadError=$_.Exception.Message}
    finally {$hardwareLoadWatch.Stop()}
    Record-Event @{event='hardware_telemetry_preparation';setup_ms=$hardwareLoadWatch.Elapsed.TotalMilliseconds;error=$script:hardwareTelemetryLoadError;optional_read_only=$true;native_frame_timing_includes_this_setup=$false}
    $planPath=Join-Path $appRoot 'benchmark-plan.json';$catalogPath=Join-Path $appRoot 'benchmark-unit-tests.json'
    $script:plan=Get-Content -LiteralPath $planPath -Raw|ConvertFrom-Json
    $script:catalog=Get-Content -LiteralPath $catalogPath -Raw|ConvertFrom-Json
    if ($script:plan.schema_version -ne 1 -or $script:catalog.schema_version -ne 1 -or @($script:plan.cases).Count -eq 0) {throw 'Unsupported or empty benchmark plan/test catalogue.'}
    if ([string]$script:catalog.native_source_sha256 -ne [string]$script:manifest.native_source_sha256) {throw 'Test catalogue/native executable compilation identity mismatch.'}
    $unitExecutables=@($script:catalog.executables|ForEach-Object {$_.executable})
    if ($unitExecutables.Count -ne 2 -or @($unitExecutables|Select-Object -Unique).Count -ne 2 -or 'Last-World-Mechanisms.exe' -notin $unitExecutables -or 'Last-World-Cli-Checks.exe' -notin $unitExecutables) {throw 'Both distinct Windows test executables must be catalogued for discovery.'}
    foreach ($entry in @($script:catalog.tests)) {if ([string]$entry.test_name -notmatch '^[A-Za-z0-9_:]+$' -or [string]$entry.executable -notin $unitExecutables) {throw 'Invalid host-expected test catalogue entry.'}}
    $planHash=(Get-FileHash -LiteralPath $planPath -Algorithm SHA256).Hash.ToLowerInvariant();$catalogHash=(Get-FileHash -LiteralPath $catalogPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $buildKey=Hash-Text ($script:manifest.executable_sha256+'|'+$script:manifest.native_source_sha256+'|'+$planHash+'|'+$catalogHash+'|supervisor3-hardware1')
    $os=Get-CimInstance Win32_OperatingSystem
    $totalRam=[uint64]$os.TotalVisibleMemorySize*1024
    $script:memoryGuard=[long][Math]::Min([double]6GB,[Math]::Floor([double]$totalRam*0.25))
    if ($script:memoryGuard -lt 256MB) {throw 'Insufficient reported physical memory for the explicit safety guard.'}
    if (Test-Path -LiteralPath $pointerPath) {
        $pointer=Read-RecoverableJson $pointerPath
        if ([string]$pointer.run_id -notmatch '^[A-Za-z0-9_-]+$') {throw 'Invalid saved suite pointer.'}
        $previousFolder=Join-Path $workRoot ([string]$pointer.run_id)
        $previous=Read-RecoverableJson (Join-Path $previousFolder 'suite-journal.json')
        if ($previous.build_key -eq $buildKey -and $previous.schema_version -eq 2) {$script:journal=$previous;$script:runFolder=$previousFolder;$script:runId=[string]$previous.run_id}
        else {
            # An older build may still own an orphan. Never adopt its work or
            # kill by program name; verify each recorded PID/start/path/hash.
            foreach ($phase in @($previous.phases)) {foreach ($attempt in @($phase.attempts|Where-Object {$_.status -in @('launching','running')})) {
                if ($null -ne $attempt.owner) {$identity=Process-Identity $attempt.owner;if ($identity.state -eq 'uncertain') {$script:unsafeCleanup=$true;throw 'Older-build orphan identity uncertain.'};if ($identity.state -eq 'match') {try {$identity.process.Kill();if (-not $identity.process.WaitForExit(10000)) {throw 'Older owned child did not stop.'}}finally{$identity.process.Dispose()}}}
            }}
        }
    }
    if ($null -eq $script:journal) {
        $script:runId=[DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffZ')+'-'+$PID
        $script:runFolder=Join-Path $workRoot $script:runId
        [void][System.IO.Directory]::CreateDirectory($script:runFolder)
        [void][System.IO.Directory]::CreateDirectory((Join-Path $script:runFolder 'benchmark'))
        $script:journal=[pscustomobject][ordered]@{schema_version=2;revision=0;run_id=$script:runId;build_key=$buildKey;created_utc=[DateTime]::UtcNow.ToString('o');updated_utc=$null;status='running';executable_sha256=$script:manifest.executable_sha256;native_source_sha256=$script:manifest.native_source_sha256;plan_sha256=$planHash;unit_catalogue_sha256=$catalogHash;source_commit=$script:manifest.source_commit;memory_guard_bytes=$script:memoryGuard;memory_guard_policy='min(6GiB,25percent_physical_RAM);hard_process_commit_cap_and1second_OS_counter_poll';phases=@();unit_inventories=@();capture_owners=@();terminal_reason=$null}
        foreach ($entry in @($script:catalog.executables)) {
            if ([string]$entry.executable -notin @('Last-World-Mechanisms.exe','Last-World-Cli-Checks.exe') -or [string]$entry.sha256 -ne $script:exeHashes[[string]$entry.executable]) {throw 'Unit executable catalogue identity is invalid.'}
            Add-Phase (New-Phase ('discover-'+[System.IO.Path]::GetFileNameWithoutExtension([string]$entry.executable)) 'discovery' ([string]$entry.executable) @('--list') 120)
        }
        foreach ($baseline in @(Baseline-Definitions)) {Add-Phase $baseline}
        $caseIds=New-Object 'System.Collections.Generic.HashSet[string]'
        foreach ($case in @($script:plan.cases)) {
            if ([string]$case.id -notmatch '^[A-Za-z0-9_-]+$' -or -not $caseIds.Add([string]$case.id)) {throw 'Invalid/duplicate native benchmark case ID.'}
            $arguments=@('--benchmark-case',[string]$case.id,'--benchmark-output',('check-results/'+$script:runId+'/benchmark'))
            if ($null -ne $case.backend) {if ([string]$case.backend -notin @('vulkan','dx12')) {throw 'Unsupported planned backend.'};$arguments+=@('--backend',[string]$case.backend)}
            $phase=New-Phase ('benchmark-'+$case.id) 'benchmark' 'Last-World.exe' $arguments ([int]$case.timeout_seconds);$phase.native_case_id=[string]$case.id;Add-Phase $phase
        }
        Save-Journal;Write-AtomicJson $pointerPath @{schema_version=1;run_id=$script:runId;build_key=$buildKey}
    }
    Validate-JournalState
    $script:journal.memory_guard_bytes=$script:memoryGuard
    Record-Event @{event='suite_opened';status=$script:journal.status;build_key=$buildKey;supervisor_process_id=$PID;watchdog_restart=$env:CHECK_WATCHDOG_ATTEMPT;total_phases=@($script:journal.phases).Count;physical_RAM_bytes=$totalRam;memory_guard_bytes=$script:memoryGuard;requested_rate_cap='plan benchmark only; playable1x2x4x unchanged'}
    try {
        $cpu=@(Get-CimInstance Win32_Processor|Select-Object Name,NumberOfCores,NumberOfLogicalProcessors)
        $gpu=@(Get-CimInstance Win32_VideoController|Select-Object Name,DriverVersion,AdapterRAM)
        Record-Event @{event='host';os=$os.Caption;os_build=$os.BuildNumber;visible_physical_RAM_bytes=$totalRam;cpu=$cpu;gpu=$gpu;gpu_WMI_AdapterRAM_note='32bit WMI field may truncate; not a verified total VRAM measurement'}
    } catch {Record-Event @{event='optional_host_detail_unavailable';error=$_.Exception.Message}}
    Recover-InterruptedAttempts;Check-UnownedPrograms
    if ($script:journal.status -notin @('completed','completed_with_failures')) {
        $script:journal.status='running';Save-Journal
        while ($true) {
            $phase=@($script:journal.phases|Where-Object {$_.status -notin @('passed','skipped_after_failures')}|Sort-Object @{Expression={switch ($_.kind) {'discovery' {0};'unit' {1};'baseline' {2};default {3}}}},@{Expression={[int]$_.sequence}}|Select-Object -First 1)
            if ($phase.Count -eq 0) {break}
            Run-Attempt $phase[0]
            if ($script:unsafeCleanup) {throw 'Further launches stopped because owned-child cleanup is uncertain.'}
        }
        $failed=@($script:journal.phases|Where-Object {$_.status -ne 'passed'}).Count
        $script:journal.status=if ($failed -eq 0) {'completed'} else {'completed_with_failures'}
        Save-Journal
    }
    $script:terminalExit=if ($script:journal.status -eq 'completed') {0} else {10}
} catch {
    $script:terminalExit=if ($script:unsafeCleanup) {11} else {12}
    if ($null -ne $script:journal) {$script:journal.status=if ($script:unsafeCleanup) {'unsafe_cleanup'} else {'preparation_failed'};$script:journal.terminal_reason=$_.Exception.Message;try {Save-Journal}catch{}}
    try {Record-Event @{event='supervisor_stopped';exit_code=$script:terminalExit;error=$_.Exception.Message;unsafe_cleanup=$script:unsafeCleanup}}catch{}
    Write-Host ('Supervisor stopped: '+$_.Exception.Message)
} finally {
    try {
        if ($null -ne $script:journal) {
            $summary=@{schema_version=2;run_id=$script:runId;status=$script:journal.status;passed=($script:journal.status -eq 'completed');completed=($script:journal.status -in @('completed','completed_with_failures'));unsafe_cleanup=$script:unsafeCleanup;phase_count=@($script:journal.phases).Count;passed_phases=@($script:journal.phases|Where-Object {$_.status -eq 'passed'}).Count;skipped_after_failures=@($script:journal.phases|Where-Object {$_.status -eq 'skipped_after_failures'}).Count;build_key=$script:journal.build_key;memory_guard_bytes=$script:memoryGuard;terminal_exit=$script:terminalExit;feedback_file='Send-Back.zip';performance_qualification=$false;completion_semantics='Passed phases completed their correctness protocol; requested-rate sustained/FPS/latency are separate native report metrics, not implied by suite completion.'}
            Write-AtomicJson (Join-Path $script:runFolder 'run-summary.json') $summary
        }
        Record-Event @{event='supervisor_finished';exit_code=$script:terminalExit;unsafe_cleanup=$script:unsafeCleanup};Flush-HostConsole
    } catch {Write-Host ('Summary/log finalization: '+$_.Exception.Message)}
    try {
        if (Get-Command Make-LastWorldFeedback -ErrorAction SilentlyContinue) {
            $owners=@();if ($null -ne $script:journal) {$owners=@($script:journal.capture_owners)}
            $feedback=Make-LastWorldFeedback $installRoot $script:runFolder $owners ('supervisor terminal exit '+$script:terminalExit)
            $script:bundleCreated=$true;Write-Host ('Send only: '+$feedback)
        }
    } catch {Write-Error -ErrorAction Continue ('Feedback finalization failed: '+$_.Exception.Message);if ($script:terminalExit -eq 0) {$script:terminalExit=12}}
    if ($null -ne $batchLock) {$batchLock.Dispose()}
}
exit $script:terminalExit
