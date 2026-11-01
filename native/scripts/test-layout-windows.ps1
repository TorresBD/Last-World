# Portable filesystem regression probe; compatible with Windows PowerShell 5.1.
# No game/native test process is launched or scanned. Every fixture is retained.
$ErrorActionPreference='Stop'
if ($PSVersionTable.PSVersion -lt [version]'5.1') {throw 'PowerShell 5.1 or newer is required.'}
. (Join-Path (Split-Path $PSScriptRoot -Parent) 'launchers/Initialize-Layout.ps1')
$utf8=[Text.UTF8Encoding]::new($false)
$probeRoot=Join-Path ([IO.Path]::GetTempPath()) ("Last World Layout ' & ["+$PID+'-'+[Guid]::NewGuid().ToString('N')+']')
[void][IO.Directory]::CreateDirectory($probeRoot)
$checks=New-Object 'System.Collections.Generic.List[string]'
$skips=New-Object 'System.Collections.Generic.List[string]'
function Require([bool]$Condition,[string]$Message) {if (-not $Condition) {throw $Message}}
function Write-ProbeText([string]$Path,[string]$Text) {[void][IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($Path));[IO.File]::WriteAllText($Path,$Text,$utf8)}
function Read-ProbeText([string]$Path) {return [IO.File]::ReadAllText($Path)}
function New-LayoutFixture([string]$Name) {
    $fixture=Join-Path $probeRoot $Name
    $files=[ordered]@{}
    foreach ($relative in @('App/Last-World.exe','Tools/Initialize-Layout.ps1','Tools/Run-Checks.ps1','Tools/Run-Checks-OwnedProcess.ps1','Tools/Run-Checks-GpuTelemetry.ps1','Tools/Run-Checks-Feedback.ps1','Tools/Run-Checks-Watchdog.cmd','App/ui/default.json')) {
        $path=Join-Path $fixture $relative;Write-ProbeText $path ('fixture bytes for '+$relative)
        $files[$relative]=@{sha256=(Get-LastWorldFileHash $path);bytes=([IO.FileInfo]$path).Length}
    }
    $manifest=@{application='Last World';layout_version=1;native_source_sha256=('a'*64);executable_sha256=$files['App/Last-World.exe'].sha256;payload_files=$files}
    Write-ProbeText (Join-Path $fixture 'App/build-manifest.json') ($manifest|ConvertTo-Json -Depth 8)
    foreach ($name in @('Run-Game.cmd','Run-Checks.cmd','README.txt')) {Write-ProbeText (Join-Path $fixture $name) ('new public '+$name)}
    Write-ProbeText (Join-Path $fixture 'Source/player-owned-source.txt') 'source reference untouched'
    Write-ProbeText (Join-Path $fixture 'Documentation/current.txt') 'current documentation untouched'
    return $fixture
}
function Invoke-Fixture([string]$Fixture) {return Invoke-LastWorldLayout -InstallRoot $Fixture -ProcessProvider {@()}}
function Require-Throws([scriptblock]$Action,[string]$Pattern,[string]$Message) {
    $failed=$false
    try {& $Action|Out-Null} catch {$failed=$true;Require ($_.Exception.Message -match $Pattern) ($Message+'; unexpected error: '+$_.Exception.Message)}
    Require $failed $Message
}
try {
    $root=New-LayoutFixture 'preservation'
    $legacyConsole="old console first`r`nold console final without newline"
    $currentConsole="existing Data console`n"
    $legacyHistory="{`"old`":1}`r`n{`"old`":2}"
    Write-ProbeText (Join-Path $root 'console.txt') $legacyConsole
    Write-ProbeText (Join-Path $root 'Data/console.txt') $currentConsole
    Write-ProbeText (Join-Path $root 'river-history.jsonl') $legacyHistory
    Write-ProbeText (Join-Path $root 'Data/river-history.jsonl') "{`"new`":3}`n"
    Write-ProbeText (Join-Path $root 'campaign-history.jsonl') "{`"campaign`":1}`n"
    Write-ProbeText (Join-Path $root 'Data/campaign-history.jsonl') "{`"campaign`":1}`n"
    Write-ProbeText (Join-Path $root 'settings.json') 'old settings'
    Write-ProbeText (Join-Path $root 'Data/settings.json') 'current Data settings'
    Write-ProbeText (Join-Path $root 'saves/player.json') 'old player save'
    Write-ProbeText (Join-Path $root 'Data/saves/player.json') 'current Data player save'
    Write-ProbeText (Join-Path $root 'saves/second-player.json') 'noncolliding save'
    Write-ProbeText (Join-Path $root 'settings/keyboard.json') 'old keyboard settings'
    Write-ProbeText (Join-Path $root 'check-results/old-run/report.json') 'old report'
    Write-ProbeText (Join-Path $root 'benchmark-results.json.part') 'old partial report'
    Write-ProbeText (Join-Path $root 'workload-scope.json') 'old workload'
    Write-ProbeText (Join-Path $root 'check-launcher-errors.txt') 'old launcher failure'
    Write-ProbeText (Join-Path $root 'check-feedback-errors.txt') 'old feedback failure'
    Write-ProbeText (Join-Path $root 'river-check-123-old.png') 'old river image'
    Write-ProbeText (Join-Path $root 'campaign-check-123-old.jsonl') 'old campaign report'
    Write-ProbeText (Join-Path $root 'Send-Back.zip') 'convenient final feedback'
    Write-ProbeText (Join-Path $root 'Send-Back-123-old.zip.part') 'failed candidate'
    Write-ProbeText (Join-Path $root 'SendBack-123-old.zip.part') 'other failed candidate'
    Write-ProbeText (Join-Path $root 'Last-World.exe') 'old native executable'
    Write-ProbeText (Join-Path $root 'Run-Benchmark.cmd') 'old benchmark launcher'
    Write-ProbeText (Join-Path $root 'Run-Checks.ps1') 'old checks helper'
    Write-ProbeText (Join-Path $root 'README.md') 'old readme'
    Write-ProbeText (Join-Path $root 'docs/old.md') 'old documentation'
    Write-ProbeText (Join-Path $root 'qualification/old.json') 'old qualification'
    Write-ProbeText (Join-Path $root 'THIRD-PARTY-NOTICES.txt') 'old notices'
    Write-ProbeText (Join-Path $root 'unknown-user-file.txt') 'unknown file untouched'
    Write-ProbeText (Join-Path $root 'My-Launcher.cmd') 'unknown launcher untouched'
    Write-ProbeText (Join-Path $root 'Last-World/nested-player-file.txt') 'nested folder untouched'
    Write-ProbeText (Join-Path $root 'ui/default.json') 'old default ui'
    Write-ProbeText (Join-Path $root 'ui/custom.json') 'old custom ui'
    Write-ProbeText (Join-Path $root 'ui/collision.json') 'old custom conflict'
    Write-ProbeText (Join-Path $root 'Data/ui/collision.json') 'current Data custom ui'
    $oldManifest=@{payload_files=@{'ui/default.json'=@{sha256=(Get-LastWorldFileHash (Join-Path $root 'ui/default.json'))};'unknown-user-file.txt'=@{sha256=(Get-LastWorldFileHash (Join-Path $root 'unknown-user-file.txt'))}}}
    Write-ProbeText (Join-Path $root 'build-manifest.json') ($oldManifest|ConvertTo-Json -Depth 8)
    Write-ProbeText (Join-Path $root 'Run-Checks.lock') 'old lock bytes retained'
    $result=Invoke-Fixture $root
    Require ($result.Conflicts -eq 3) 'Save, settings and custom UI collisions were not reported.'
    $console=Read-ProbeText (Join-Path $root 'Data/console.txt')
    Require ($console.StartsWith($currentConsole) -and $console.Contains($legacyConsole)) 'Console history bytes/order were not preserved.'
    Require ((Read-ProbeText (Join-Path $root 'Data/Legacy/Loose-Files/Imported-History/console.txt')) -ceq $legacyConsole) 'Exact old console was not retained.'
    Require ((Read-ProbeText (Join-Path $root 'Data/Legacy/Loose-Files/Before-History-Import/console.txt')) -ceq $currentConsole) 'Exact previous Data console was not retained.'
    Require ((Read-ProbeText (Join-Path $root 'Data/river-history.jsonl')).Contains($legacyHistory)) 'Original JSONL bytes were altered during import.'
    Require ((Read-ProbeText (Join-Path $root 'Data/campaign-history.jsonl')) -ceq "{`"campaign`":1}`n") 'Byte-identical history was appended a second time.'
    foreach ($line in (Read-ProbeText (Join-Path $root 'Data/river-history.jsonl')) -split '\r?\n') {if ($line.Length -gt 0) {[void]($line|ConvertFrom-Json)}}
    Require ((Read-ProbeText (Join-Path $root 'Data/settings.json')) -ceq 'current Data settings') 'Existing Data settings were replaced.'
    Require ((Read-ProbeText (Join-Path $root 'Data/saves/player.json')) -ceq 'current Data player save') 'Existing Data player save was replaced.'
    Require ((Read-ProbeText (Join-Path $root 'Data/Legacy/Conflicts/saves/player.json')) -ceq 'old player save') 'Conflicting old player save was lost.'
    Require ((Read-ProbeText (Join-Path $root 'Data/saves/second-player.json')) -ceq 'noncolliding save') 'Noncolliding save was not migrated.'
    Require ((Read-ProbeText (Join-Path $root 'Data/ui/custom.json')) -ceq 'old custom ui') 'Legacy custom UI did not become a Data override.'
    Require ((Read-ProbeText (Join-Path $root 'Data/ui/collision.json')) -ceq 'current Data custom ui') 'Current Data UI override was replaced.'
    Require ((Read-ProbeText (Join-Path $root 'Data/Legacy/Loose-Files/ui/default.json')) -ceq 'old default ui') 'Verified old default UI was not retained as a reference.'
    Require (-not (Test-LastWorldPathExists (Join-Path $root 'ui'))) 'Migrated root UI directory was left behind.'
    foreach ($name in @('benchmark-results.json.part','workload-scope.json','check-launcher-errors.txt','check-feedback-errors.txt','river-check-123-old.png','campaign-check-123-old.jsonl')) {Require ([IO.File]::Exists((Join-Path $root ('Data/'+$name)))) ('Legacy runtime file missing: '+$name)}
    Require ((Read-ProbeText (Join-Path $root 'Send-Back.zip')) -ceq 'convenient final feedback') 'Final root feedback ZIP was moved or modified.'
    Require ((Read-ProbeText (Join-Path $root 'Data/Legacy/Feedback/Send-Back-123-old.zip.part')) -ceq 'failed candidate') 'Failed feedback candidate was discarded.'
    Require ((Read-ProbeText (Join-Path $root 'Data/Legacy/Loose-Files/Run-Checks.lock')) -ceq 'old lock bytes retained') 'Old compatibility lock bytes were not retained.'
    foreach ($name in @('Last-World.exe','Run-Benchmark.cmd','Run-Checks.ps1','README.md','build-manifest.json','THIRD-PARTY-NOTICES.txt')) {Require ([IO.File]::Exists((Join-Path $root ('Data/Legacy/Loose-Files/'+$name)))) ('Legacy owned file missing: '+$name)}
    foreach ($name in @('Run-Game.cmd','Run-Checks.cmd','README.txt','unknown-user-file.txt','My-Launcher.cmd','Last-World/nested-player-file.txt','Source/player-owned-source.txt','Documentation/current.txt')) {Require ([IO.File]::Exists((Join-Path $root $name))) ('Unowned/new public file moved: '+$name)}
    $journalBefore=Read-ProbeText $result.Journal;$consoleBefore=Read-ProbeText (Join-Path $root 'Data/console.txt')
    $journalLines=@($journalBefore -split '\r?\n'|Where-Object {$_.Length -gt 0})
    $records=@($journalLines|ForEach-Object {$_|ConvertFrom-Json})
    Require (@($records|Where-Object {$_.action -eq 'preserved_conflict' -and $_.detail -match 'remains authoritative'}).Count -eq 3) 'Migration journal did not explain Data conflict precedence.'
    # Compare emitted bytes directly. PowerShell versions can deserialize an ISO
    # timestamp as DateTime and normalize it when serializing the parsed object.
    foreach ($line in $journalLines) {Require ($consoleBefore.Contains('[LAYOUT] '+$line)) 'Canonical console omitted a migration record.'}
    $again=Invoke-Fixture $root
    Require ($again.Operations -eq 0) 'Idempotent second launch performed more migration operations.'
    Require ((Read-ProbeText $again.Journal) -ceq $journalBefore) 'Idempotent second launch changed the migration journal.'
    Require ((Read-ProbeText (Join-Path $root 'Data/console.txt')) -ceq $consoleBefore) 'Idempotent second launch repeated log/history imports.'
    $checks.Add('exact_history_and_previous_data_preserved_collision_archive_default_and_custom_ui_unknown_files_idempotence')

    # Simulate interruption after an atomic import but before archiving its old
    # root file. The durable end marker must suppress reimport after restart.
    $resume=New-LayoutFixture 'resume-marker'
    Write-ProbeText (Join-Path $resume 'river-history.jsonl') "{`"old`":9}`n"
    Write-ProbeText (Join-Path $resume 'Data/river-history.jsonl') "{`"new`":8}`n"
    [void](Invoke-Fixture $resume)
    $resumeData=Read-ProbeText (Join-Path $resume 'Data/river-history.jsonl')
    [IO.File]::Move((Join-Path $resume 'Data/Legacy/Loose-Files/Imported-History/river-history.jsonl'),(Join-Path $resume 'river-history.jsonl'))
    [void](Invoke-Fixture $resume)
    Require ((Read-ProbeText (Join-Path $resume 'Data/river-history.jsonl')) -ceq $resumeData) 'Restart appended an already committed exact history import.'
    $checks.Add('interrupted_import_restart_uses_durable_marker')

    foreach ($kind in @('old','new')) {
        $lockedRoot=New-LayoutFixture ('locked-'+$kind);Write-ProbeText (Join-Path $lockedRoot 'console.txt') 'locked migration sentinel'
        $lockPath=Join-Path $lockedRoot 'Run-Checks.lock'
        if ($kind -eq 'new') {$lockPath=Join-Path $lockedRoot 'Data/Locks/Run-Checks.lock'}
        Write-ProbeText $lockPath 'lock sentinel'
        $held=[IO.FileStream]::new($lockPath,[IO.FileMode]::Open,[IO.FileAccess]::ReadWrite,[IO.FileShare]::None)
        try {Require-Throws {Invoke-Fixture $lockedRoot} 'checking window' ('Exclusively held '+$kind+' lock did not stop migration.')} finally {$held.Dispose()}
        Require ((Read-ProbeText (Join-Path $lockedRoot 'console.txt')) -ceq 'locked migration sentinel') 'Lock-blocked migration changed old console.'
        Require (-not [IO.File]::Exists((Join-Path $lockedRoot 'Data/layout-migration.jsonl'))) 'Lock-blocked migration recorded data moves.'
        $checks.Add(($kind+'_exclusive_lock_blocks_without_data_moves'))
    }
    $both=New-LayoutFixture 'locks-held-through-data-moves';Write-ProbeText (Join-Path $both 'console.txt') 'both-locks sentinel'
    $observation=@{Calls=0;Observed=$false}
    $provider={
        $observation.Calls++
        if ($observation.Calls -eq 2) {
            foreach ($relative in @('Run-Checks.lock','Data/Locks/Run-Checks.lock')) {
                $unexpected=$null;$blocked=$false
                try {$unexpected=[IO.FileStream]::new((Join-Path $both $relative),[IO.FileMode]::Open,[IO.FileAccess]::ReadWrite,[IO.FileShare]::None)} catch {$blocked=$true} finally {if ($null -ne $unexpected) {$unexpected.Dispose()}}
                Require $blocked ('Migration did not hold its '+$relative+' handle.')
            }
            $observation.Observed=$true
        }
        return @()
    }.GetNewClosure()
    [void](Invoke-LastWorldLayout -InstallRoot $both -ProcessProvider $provider)
    Require $observation.Observed 'Both-lock acquisition observation did not run.'
    $checks.Add('both_compatibility_and_data_locks_held_before_moves')

    foreach ($relative in @('Last-World.exe','App/Last-World.exe')) {
        $activeRoot=New-LayoutFixture ('active-'+$relative.Replace('/','-'));Write-ProbeText (Join-Path $activeRoot 'console.txt') 'active sentinel'
        $activePath=Join-Path $activeRoot $relative
        Require-Throws {Invoke-LastWorldLayout -InstallRoot $activeRoot -ProcessProvider ({$activePath}.GetNewClosure())} 'still active' 'Injected active native executable did not block migration.'
        Require ((Read-ProbeText (Join-Path $activeRoot 'console.txt')) -ceq 'active sentinel') 'Active-process blocked migration changed old data.'
    }
    $checks.Add('injected_root_and_app_process_paths_block_without_scanning_processes')

    $mixed=New-LayoutFixture 'mixed-update';Write-ProbeText (Join-Path $mixed 'console.txt') 'mixed sentinel'
    Write-ProbeText (Join-Path $mixed 'Tools/Run-Checks.ps1') 'modified helper'
    Require-Throws {Invoke-Fixture $mixed} 'modified update file' 'Mixed helper payload did not stop migration.'
    Require ((Read-ProbeText (Join-Path $mixed 'console.txt')) -ceq 'mixed sentinel') 'Mixed update changed player data before validation.'
    $checks.Add('organized_app_and_tools_hashes_verified_before_moves')

    # A malicious old manifest cannot turn an arbitrary root file into owned
    # payload. A reparse fixture uses a junction on Windows (no symlink privilege).
    $reparse=New-LayoutFixture 'reparse';$outside=Join-Path $probeRoot 'outside-user-install'
    Write-ProbeText (Join-Path $outside 'sentinel.txt') 'outside sentinel'
    $linkPath=Join-Path $reparse 'saves';$link=$null
    try {
        if ($env:OS -eq 'Windows_NT') {
            # Junctions do not require Windows symlink privilege. The provider
            # resolves Value with wildcards, so escape the bracketed fixture.
            $link=New-Item -ItemType Junction -Path $linkPath -Value ([WildcardPattern]::Escape($outside)) -ErrorAction Stop
        } else {
            # .NET receives literal paths; New-Item's target resolution can treat
            # brackets in this deliberately awkward fixture as wildcard syntax.
            $link=[IO.Directory]::CreateSymbolicLink($linkPath,$outside)
        }
    } catch {$skips.Add(('reparse_fixture_creation_unavailable: '+$_.Exception.Message))}
    if ($null -ne $link) {
        Require-Throws {Invoke-Fixture $reparse} 'reparse' 'Source reparse directory was traversed.'
        Require ((Read-ProbeText (Join-Path $outside 'sentinel.txt')) -ceq 'outside sentinel') 'Reparse migration changed outside data.'
        $checks.Add('reparse_source_rejected_outside_install_untouched')
    }
    $traversal=New-LayoutFixture 'manifest-traversal'
    $bad=[IO.File]::ReadAllText((Join-Path $traversal 'App/build-manifest.json'))|ConvertFrom-Json
    $bad.payload_files|Add-Member -MemberType NoteProperty -Name '../outside-user-install/sentinel.txt' -Value @{sha256=('0'*64)}
    Write-ProbeText (Join-Path $traversal 'App/build-manifest.json') ($bad|ConvertTo-Json -Depth 8)
    Require-Throws {Invoke-Fixture $traversal} 'Unsafe owned-payload path' 'Manifest parent traversal was accepted.'
    $checks.Add('manifest_parent_traversal_rejected')

    $summary=@{schema_version=1;passed=$true;checks=$checks.ToArray();skips=$skips.ToArray();powershell_version=$PSVersionTable.PSVersion.ToString();platform=$env:OS;retained_directory=$probeRoot;windows_qualification=($env:OS -eq 'Windows_NT')}
    Write-ProbeText (Join-Path $probeRoot 'probe-summary.json') ($summary|ConvertTo-Json -Depth 6)
    Write-Host ('Layout filesystem probe passed; retained evidence: '+$probeRoot)
} catch {Write-ProbeText (Join-Path $probeRoot 'probe-failure.txt') ($_|Out-String);throw}
