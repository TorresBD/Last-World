# Isolated Windows PowerShell 5.1 regression probe; never launches the game.
# -Portable permits real ZIP/filesystem checks in PowerShell Core elsewhere;
# Windows destination-delete sharing semantics remain explicitly unverified.
# All inputs/results are retained in a unique TEMP folder.
param([switch]$Portable)
$ErrorActionPreference='Stop'
$runningOnWindows=$env:OS -eq 'Windows_NT'
if ($PSVersionTable.PSVersion -lt [version]'5.1' -or (-not $runningOnWindows -and -not $Portable)) {throw 'This probe requires Windows PowerShell 5.1 or newer on Windows; use -Portable for limited filesystem QA in PowerShell Core elsewhere.'}
$helperPath=Join-Path (Split-Path $PSScriptRoot -Parent) 'launchers/Run-Checks-Feedback.ps1'
. $helperPath
$root=Join-Path ([System.IO.Path]::GetTempPath()) ("Last World Feedback ' & ["+$PID+'-'+[Guid]::NewGuid().ToString('N')+']')
[void][System.IO.Directory]::CreateDirectory($root)
$app=Join-Path $root 'App';[void][System.IO.Directory]::CreateDirectory($app)
$data=Join-Path $root 'Data';[void][System.IO.Directory]::CreateDirectory($data)
$temporary=Join-Path $data 'Temporary'
$lockRoot=Join-Path $data 'Locks';[void][System.IO.Directory]::CreateDirectory($lockRoot)
$lockPath=Join-Path $lockRoot 'Run-Checks.lock'
$run=Join-Path $data 'check-results/probe-run'
[void][System.IO.Directory]::CreateDirectory((Join-Path $run 'benchmark/checkpoints'))
$utf8=[System.Text.UTF8Encoding]::new($false)
[System.IO.File]::WriteAllText((Join-Path $data 'console.txt'),'owned diagnostic',$utf8)
[System.IO.File]::WriteAllText((Join-Path $data 'settings.json'),'untouched user sentinel',$utf8)
[System.IO.File]::WriteAllText((Join-Path $app 'build-manifest.json'),'{"probe_metadata":true}',$utf8)
[System.IO.File]::WriteAllText((Join-Path $data 'check-results/current-suite.json'),'{"schema_version":1,"run_id":"probe-run"}',$utf8)
[System.IO.File]::WriteAllText((Join-Path $run 'suite-journal.json'),'{"schema_version":2,"run_id":"probe-run","capture_owners":[]}',$utf8)
[System.IO.File]::WriteAllText((Join-Path $run 'run-summary.json'),'{"completed":true,"passed":false,"terminal_exit":10}',$utf8)
[System.IO.File]::WriteAllText((Join-Path $run 'benchmark/checkpoints/sentinel.json'),'retained checkpoint sentinel',$utf8)
$savedEnvironment=@{}
foreach ($name in @('LAST_WORLD_INSTALL_ROOT','LAST_WORLD_DATA_DIR','CHECKS_EXIT','CHECK_WATCHDOG_ATTEMPT')) {$savedEnvironment[$name]=[Environment]::GetEnvironmentVariable($name,'Process')}
$env:LAST_WORLD_INSTALL_ROOT=$root;$env:LAST_WORLD_DATA_DIR=$data;$env:CHECKS_EXIT='10';$env:CHECK_WATCHDOG_ATTEMPT='2'
$hostName=if (-not $runningOnWindows) {'pwsh'} elseif ($PSVersionTable.PSEdition -eq 'Core') {'pwsh.exe'} else {'powershell.exe'}
$hostExecutable=Join-Path $PSHOME $hostName
function Require([bool]$Condition,[string]$Message) {if (-not $Condition) {throw $Message}}
function Read-ZipNotes([string]$Path) {
    $file=[System.IO.FileStream]::new($Path,[System.IO.FileMode]::Open,[System.IO.FileAccess]::Read,[System.IO.FileShare]::Read)
    $zip=$null;$reader=$null
    try {
        $zip=[System.IO.Compression.ZipArchive]::new($file,[System.IO.Compression.ZipArchiveMode]::Read,$true)
        $names=@($zip.Entries|ForEach-Object {$_.FullName})
        Require ('Data/settings.json' -notin $names) 'User settings entered feedback.'
        Require ('Data/console.txt' -in $names) 'Data diagnostic is missing.'
        Require ('Data/check-results/current-suite.json' -in $names) 'Data suite pointer is missing.'
        Require ('Data/check-results/probe-run/run-summary.json' -in $names) 'Data run summary is missing.'
        Require (@($names|Where-Object {$_ -like '*/checkpoints/*'}).Count -eq 0) 'Checkpoint entered feedback.'
        Require (@($names|Where-Object {$_ -like '*.zip' -or $_ -like '*.zip.part'}).Count -eq 0) 'Previous feedback or retained candidate entered feedback.'
        $reader=[System.IO.StreamReader]::new($zip.GetEntry('feedback-bundle-notes.json').Open(),$utf8)
        return @{notes=($reader.ReadToEnd()|ConvertFrom-Json);entries=$names}
    } finally {if ($null -ne $reader) {$reader.Dispose()};if ($null -ne $zip) {$zip.Dispose()};$file.Dispose()}
}
$checks=New-Object 'System.Collections.Generic.List[string]'
try {
    Require (Test-Path -LiteralPath $hostExecutable -PathType Leaf) 'Cannot locate the current PowerShell executable beside PSHOME.'
    $feedback=Make-LastWorldFeedback $root $run @() 'first-no-existing-feedback'
    $first=Read-ZipNotes $feedback
    Require ($feedback -eq (Join-Path $root 'Send-Back.zip')) 'Feedback was not published at the install root.'
    Require ($first.notes.reason -eq 'first-no-existing-feedback') 'First publication failed.'
    Require ('App/build-manifest.json' -in $first.entries) 'App build identity metadata is missing.'
    $checks.Add('first_publication')
    [void](Make-LastWorldFeedback $root $run @() 'replace-existing-feedback')
    Require ((Read-ZipNotes $feedback).notes.reason -eq 'replace-existing-feedback') 'Existing feedback was not atomically replaced.'
    [void](Make-LastWorldFeedback $root $run @() 'second-existing-feedback-overwrite')
    Require ((Read-ZipNotes $feedback).notes.reason -eq 'second-existing-feedback-overwrite') 'Second existing-feedback overwrite failed.'
    Require (@(Get-ChildItem -LiteralPath $temporary -Filter '*.zip.part').Count -eq 0) 'Successful publication left a .part.'
    Require (@(Get-ChildItem -LiteralPath $root -Filter '*.zip.part').Count -eq 0) 'Candidate was written into the install root.'
    $checks.Add('two_existing_feedback_overwrites_with_true_null_backup')
    $before=(Get-FileHash -LiteralPath $feedback -Algorithm SHA256).Hash
    $failed=$false
    if ($runningOnWindows) {
        $failureReason='locked-destination'
        $locked=[System.IO.FileStream]::new($feedback,[System.IO.FileMode]::Open,[System.IO.FileAccess]::Read,[System.IO.FileShare]::None)
        try {[void](Make-LastWorldFeedback $root $run @() $failureReason)} catch {$failed=$true} finally {$locked.Dispose()}
        Require $failed 'An exclusively locked destination unexpectedly published.'
    } else {
        # Unix rename can replace an open destination despite FileShare.None.
        # Fail publication only after a complete closed candidate was created.
        $failureReason='portable-missing-publication-parent'
        $missingRoot=Join-Path $root 'intentionally-missing-publication-parent'
        try {[void](Make-LastWorldFeedback $missingRoot $run @() $failureReason)} catch {$failed=$true}
        Require $failed 'Publication into a missing parent unexpectedly succeeded.'
    }
    Require ((Get-FileHash -LiteralPath $feedback -Algorithm SHA256).Hash -eq $before) 'Failed publication changed old feedback.'
    $parts=@(Get-ChildItem -LiteralPath $temporary -Filter '*.zip.part')
    Require ($parts.Count -eq 1) 'Failed publication did not retain its candidate.'
    Require ((Read-ZipNotes $parts[0].FullName).notes.reason -eq $failureReason) 'Retained candidate was not a closed ZIP.'
    Require ((Get-Content -LiteralPath (Join-Path $data 'check-feedback-errors.txt') -Raw) -match 'Feedback finalization failed') 'Failure diagnostic was not persisted.'
    $checks.Add('failed_publication_preserves_old_zip_closed_candidate_and_error')
    $lock=[System.IO.FileStream]::new($lockPath,[System.IO.FileMode]::OpenOrCreate,[System.IO.FileAccess]::ReadWrite,[System.IO.FileShare]::None)
    try {
        & $hostExecutable -NoProfile -ExecutionPolicy Bypass -File $helperPath -Fallback
        Require ($LASTEXITCODE -eq 2) 'Fallback did not honor the persistent Data/Locks supervisor lock.'
        Require ((Get-FileHash -LiteralPath $feedback -Algorithm SHA256).Hash -eq $before) 'Locked fallback changed old feedback.'
    } finally {$lock.Dispose()}
    Require (Test-Path -LiteralPath $lockPath -PathType Leaf) 'Fallback removed the persistent lock path.'
    $checks.Add('fallback_honors_persistent_data_lock')
    & $hostExecutable -NoProfile -ExecutionPolicy Bypass -File $helperPath -Fallback
    Require ($LASTEXITCODE -eq 0) 'Fallback could not publish after the supervisor lock was released.'
    $recovered=Read-ZipNotes $feedback
    Require ($recovered.notes.reason -eq 'feedback_recovery; supervisor_exit=10; watchdog_restarts=2; no suite pass inferred') 'Recovery did not publish the actual fallback reason.'
    Require ('Data/check-feedback-errors.txt' -in $recovered.entries) 'Recovery omitted the original finalization error.'
    Require ([System.IO.File]::ReadAllText((Join-Path $data 'settings.json')) -eq 'untouched user sentinel') 'User settings changed.'
    Require ([System.IO.File]::ReadAllText((Join-Path $run 'benchmark/checkpoints/sentinel.json')) -eq 'retained checkpoint sentinel') 'Checkpoint changed.'
    Require (Test-Path -LiteralPath $parts[0].FullName) 'Recovery removed the retained failed candidate.'
    $checks.Add('recovery_includes_error_and_preserves_local_state')
    $summary=@{schema_version=2;passed=$true;checks=$checks.ToArray();powershell_version=$PSVersionTable.PSVersion.ToString();running_on_windows=$runningOnWindows;windows_locked_destination_replacement_verified=$runningOnWindows;portable_requested=[bool]$Portable;retained_directory=$root}
    [System.IO.File]::WriteAllText((Join-Path $root 'probe-summary.json'),($summary|ConvertTo-Json -Depth 6),$utf8)
    $probeKind=if ($runningOnWindows) {'Windows'} else {'portable (Windows locked-destination replacement not verified)'}
    Write-Host ('Feedback '+$probeKind+' probe passed; retained evidence: '+$root)
} catch {
    [System.IO.File]::WriteAllText((Join-Path $root 'probe-failure.txt'),($_|Out-String),$utf8)
    throw
} finally {
    foreach ($name in $savedEnvironment.Keys) {[Environment]::SetEnvironmentVariable($name,$savedEnvironment[$name],'Process')}
}
