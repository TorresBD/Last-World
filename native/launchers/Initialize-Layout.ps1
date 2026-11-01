# Windows PowerShell 5.1. Dot-source for the isolated filesystem probe.
param([string]$InstallRoot)

function Get-LastWorldFullPath([string]$Path) {
    return [System.IO.Path]::GetFullPath($Path).TrimEnd([char[]]@([System.IO.Path]::DirectorySeparatorChar,[System.IO.Path]::AltDirectorySeparatorChar))
}
function Test-LastWorldPathExists([string]$Path) {
    # File.Exists/Directory.Exists intentionally hide some dangling links.
    return $null -ne (Get-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue)
}
function Assert-LastWorldSafePath([string]$Root,[string]$Path) {
    $rootPath=Get-LastWorldFullPath $Root;$full=Get-LastWorldFullPath $Path
    $comparison=[StringComparison]::OrdinalIgnoreCase
    if ([System.IO.Path]::DirectorySeparatorChar -ne '\') {$comparison=[StringComparison]::Ordinal}
    if (-not $full.Equals($rootPath,$comparison) -and -not $full.StartsWith($rootPath+[System.IO.Path]::DirectorySeparatorChar,$comparison)) {throw ('Path leaves the selected install folder: '+$Path)}
    # Inspect ancestors too: a normal-looking install directory below a junction
    # is not an acceptable destination for automatic player-data moves.
    $part=$full
    while (-not [string]::IsNullOrEmpty($part)) {
        $item=Get-Item -LiteralPath $part -Force -ErrorAction SilentlyContinue
        if ($null -ne $item -and ($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {throw ('Linked/reparse path is not allowed during layout migration: '+$part)}
        $parent=[System.IO.Path]::GetDirectoryName($part)
        if ($parent -eq $part) {break};$part=$parent
    }
    return $full
}
function Assert-LastWorldSafeTree([string]$Root,[string]$Path) {
    [void](Assert-LastWorldSafePath $Root $Path)
    if ([System.IO.Directory]::Exists($Path)) {
        foreach ($child in [System.IO.Directory]::GetFileSystemEntries($Path)) {Assert-LastWorldSafeTree $Root $child}
    }
}
function Get-LastWorldRelativePath([string]$Root,[string]$Path) {
    $full=Assert-LastWorldSafePath $Root $Path
    return $full.Substring((Get-LastWorldFullPath $Root).Length).TrimStart([char[]]@('\','/')).Replace('\','/')
}
function Get-LastWorldFileHash([string]$Path) {return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()}
function Get-LastWorldTextHash([string]$Text) {
    $hash=[System.Security.Cryptography.SHA256]::Create()
    try {return ([BitConverter]::ToString($hash.ComputeHash([Text.Encoding]::UTF8.GetBytes($Text)))).Replace('-','').ToLowerInvariant()} finally {$hash.Dispose()}
}
function Get-LastWorldManifestPath([string]$Root,[string]$Relative) {
    if ([string]::IsNullOrWhiteSpace($Relative) -or [System.IO.Path]::IsPathRooted($Relative) -or $Relative -match '(^|[\\/])\.\.([\\/]|$)' -or $Relative -match '[:\x00]') {throw ('Unsafe owned-payload path: '+$Relative)}
    return Assert-LastWorldSafePath $Root (Join-Path $Root $Relative)
}
function Assert-LastWorldOrganizedPayload([string]$Root) {
    foreach ($name in @('App','Tools','Data','Source','Documentation')) {[void](Assert-LastWorldSafePath $Root (Join-Path $Root $name))}
    $manifestPath=Join-Path $Root 'App/build-manifest.json'
    [void](Assert-LastWorldSafePath $Root $manifestPath)
    if (-not [System.IO.File]::Exists($manifestPath)) {throw 'The organized App/build-manifest.json is missing. Copy the complete update into this folder before launching.'}
    $manifest=[System.IO.File]::ReadAllText($manifestPath)|ConvertFrom-Json
    if ($manifest.application -ne 'Last World' -or $null -eq $manifest.payload_files -or [string]$manifest.native_source_sha256 -notmatch '^[0-9a-fA-F]{64}$') {throw 'The organized application manifest is incomplete or belongs to another application.'}
    if ($null -ne $manifest.layout_version -and [int]$manifest.layout_version -ne 1) {throw 'This initializer does not recognize the application layout version.'}
    $required=@('App/Last-World.exe','Tools/Initialize-Layout.ps1','Tools/Run-Checks.ps1','Tools/Run-Checks-OwnedProcess.ps1','Tools/Run-Checks-GpuTelemetry.ps1','Tools/Run-Checks-Feedback.ps1','Tools/Run-Checks-Watchdog.cmd')
    foreach ($name in $required) {if ($null -eq $manifest.payload_files.PSObject.Properties[$name]) {throw ('The complete organized update is required; missing manifest entry: '+$name)}}
    foreach ($entry in $manifest.payload_files.PSObject.Properties) {
        $relative=[string]$entry.Name
        # Validate path syntax even for the source snapshot, but bootstrap only
        # hashes executable/helper roles. The supervisor verifies the full set.
        $path=Get-LastWorldManifestPath $Root $relative
        if ($relative -notmatch '^(App|Tools)/') {continue}
        if (-not [System.IO.File]::Exists($path) -or [string]$entry.Value.sha256 -notmatch '^[0-9a-fA-F]{64}$' -or (Get-LastWorldFileHash $path) -ne ([string]$entry.Value.sha256).ToLowerInvariant()) {throw ('Mixed, missing or modified update file; recopy the complete update: '+$relative)}
    }
    $mainHash=Get-LastWorldFileHash (Join-Path $Root 'App/Last-World.exe')
    if ($mainHash -ne ([string]$manifest.executable_sha256).ToLowerInvariant() -or $mainHash -ne ([string]$manifest.payload_files.PSObject.Properties['App/Last-World.exe'].Value.sha256).ToLowerInvariant()) {throw 'The native executable and organized manifest disagree. No player data was migrated.'}
    return $manifest
}
function Assert-LastWorldNoActiveProgram([string]$Root,[scriptblock]$ProcessProvider) {
    $names=@('Last-World.exe','Last-World-Mechanisms.exe','Last-World-Cli-Checks.exe')
    $paths=@();foreach ($name in $names) {$paths+=(Join-Path $Root $name);$paths+=(Join-Path (Join-Path $Root 'App') $name)}
    foreach ($process in @(& $ProcessProvider)) {
        $path=$null
        try {
            if ($process -is [string]) {$path=[string]$process} elseif ($null -ne $process.Path) {$path=[string]$process.Path} else {$path=[string]$process.MainModule.FileName}
        } catch {continue}
        if ([string]::IsNullOrWhiteSpace($path)) {continue}
        $full=[System.IO.Path]::GetFullPath($path)
        foreach ($owned in $paths) {if ($full.Equals($owned,[StringComparison]::OrdinalIgnoreCase)) {throw ('Close the Last World game and checking windows, then run the launcher again. An installed native program is still active: '+$full)}}
    }
}
function New-LastWorldArchivePath($State,[string]$Category,[string]$Relative) {
    $path=Join-Path (Join-Path $State.Data ('Legacy/'+$Category)) $Relative
    [void](Assert-LastWorldSafePath $State.Root $path)
    if (Test-LastWorldPathExists $path) {$path+='.retained-'+[DateTime]::UtcNow.Ticks+'-'+[Guid]::NewGuid().ToString('N')}
    [void][System.IO.Directory]::CreateDirectory([System.IO.Path]::GetDirectoryName($path))
    return $path
}
function Write-LastWorldMigrationRecord($State,[string]$Action,[string]$Source,[string]$Destination,[string]$Detail,[string]$Hash='') {
    $record=[ordered]@{schema_version=1;timestamp_utc=[DateTime]::UtcNow.ToString('o');action=$Action;source=(Get-LastWorldRelativePath $State.Root $Source);destination=(Get-LastWorldRelativePath $State.Root $Destination);detail=$Detail;source_sha256=$Hash}
    $line=$record|ConvertTo-Json -Depth 5 -Compress
    [System.IO.File]::AppendAllText($State.Journal,$line+[Environment]::NewLine,$State.Utf8)
    [System.IO.File]::AppendAllText($State.Console,'[LAYOUT] '+$line+[Environment]::NewLine,$State.Utf8)
    $State.Operations=[int]$State.Operations+1
}
function Move-LastWorldExactItem($State,[string]$Source,[string]$Destination,[string]$Action,[string]$Detail) {
    [void](Assert-LastWorldSafePath $State.Root $Source);[void](Assert-LastWorldSafePath $State.Root $Destination)
    if (Test-LastWorldPathExists $Destination) {throw ('Refusing to overwrite a retained file: '+$Destination)}
    [void][System.IO.Directory]::CreateDirectory([System.IO.Path]::GetDirectoryName($Destination))
    $hash='';if ([System.IO.File]::Exists($Source)) {$hash=Get-LastWorldFileHash $Source}
    # Same-volume Rename/Move is atomic. Do not fall back to copy-and-delete.
    if ([System.IO.Directory]::Exists($Source)) {[System.IO.Directory]::Move($Source,$Destination)} else {[System.IO.File]::Move($Source,$Destination)}
    Write-LastWorldMigrationRecord $State $Action $Source $Destination $Detail $hash
}
function Move-LastWorldDataItem($State,[string]$Source,[string]$Destination) {
    [void](Assert-LastWorldSafePath $State.Root $Source);[void](Assert-LastWorldSafePath $State.Root $Destination)
    if (-not (Test-LastWorldPathExists $Destination)) {Move-LastWorldExactItem $State $Source $Destination 'move_user_data' 'Moved without replacing an existing Data file.';return}
    if ([System.IO.Directory]::Exists($Source) -and [System.IO.Directory]::Exists($Destination)) {
        foreach ($child in [System.IO.Directory]::GetFileSystemEntries($Source)) {Move-LastWorldDataItem $State $child (Join-Path $Destination ([System.IO.Path]::GetFileName($child)))}
        # Only the now-empty directory is removed; no file is deleted, even if
        # equal by hash. Every colliding original receives a retained path.
        [System.IO.Directory]::Delete($Source,$false)
        return
    }
    $relative=Get-LastWorldRelativePath $State.Root $Source
    $archive=New-LastWorldArchivePath $State 'Conflicts' $relative
    Move-LastWorldExactItem $State $Source $archive 'preserved_conflict' ('Existing '+(Get-LastWorldRelativePath $State.Root $Destination)+' remains authoritative; inspect both files before choosing a save/settings/UI version.')
    $State.Conflicts=[int]$State.Conflicts+1
    Write-Warning ('Preserved a migration conflict: '+$relative+' -> '+(Get-LastWorldRelativePath $State.Root $archive)+'. Existing Data file kept.')
}
function Test-LastWorldImportMarker([string]$Path,[string]$Marker) {
    $stream=[System.IO.FileStream]::new($Path,[System.IO.FileMode]::Open,[System.IO.FileAccess]::Read,[System.IO.FileShare]::Read)
    $buffer=New-Object byte[] 65536;$tail='';$keep=$Marker.Length+4
    try {
        while (($count=$stream.Read($buffer,0,$buffer.Length)) -gt 0) {
            # Marker is ASCII; bounded decoding also handles a log with a single
            # enormous line. Retain only enough overlap for a split marker.
            $chunk=$tail+[Text.Encoding]::ASCII.GetString($buffer,0,$count)
            if ($chunk.Contains("`n"+$Marker+"`n") -or $chunk.Contains("`n"+$Marker+"`r`n")) {return $true}
            $tail=$chunk.Substring([Math]::Max(0,$chunk.Length-$keep))
        }
        return $false
    } finally {$stream.Dispose()}
}
function Merge-LastWorldAppendHistory($State,[string]$Source,[string]$Destination) {
    if (-not (Test-LastWorldPathExists $Destination)) {Move-LastWorldExactItem $State $Source $Destination 'move_append_history' 'Original bytes moved intact to the canonical Data path.';return}
    if (-not [System.IO.File]::Exists($Destination)) {Move-LastWorldDataItem $State $Source $Destination;return}
    $relative=Get-LastWorldRelativePath $State.Root $Source;$hash=Get-LastWorldFileHash $Source
    $id=Get-LastWorldTextHash ($relative+[char]0+$hash)
    $begin=([ordered]@{layout_migration='begin_exact_history_import';migration_id=$id;source=$relative;source_sha256=$hash}|ConvertTo-Json -Compress)
    $end=([ordered]@{layout_migration='end_exact_history_import';migration_id=$id}|ConvertTo-Json -Compress)
    $already=(Get-LastWorldFileHash $Destination) -eq $hash -or (Test-LastWorldImportMarker $Destination $end)
    if (-not $already) {
        $temporary=Join-Path (Join-Path $State.Data 'Temporary') ('history-import-'+[Guid]::NewGuid().ToString('N')+'.pending')
        $candidate=$null;$current=$null;$old=$null
        try {
            $candidate=[System.IO.FileStream]::new($temporary,[System.IO.FileMode]::CreateNew,[System.IO.FileAccess]::Write,[System.IO.FileShare]::None)
            $current=[System.IO.FileStream]::new($Destination,[System.IO.FileMode]::Open,[System.IO.FileAccess]::Read,[System.IO.FileShare]::Read)
            $old=[System.IO.FileStream]::new($Source,[System.IO.FileMode]::Open,[System.IO.FileAccess]::Read,[System.IO.FileShare]::Read)
            $current.CopyTo($candidate,65536)
            $bytes=$State.Utf8.GetBytes([Environment]::NewLine+$begin+[Environment]::NewLine);$candidate.Write($bytes,0,$bytes.Length)
            $old.CopyTo($candidate,65536)
            $bytes=$State.Utf8.GetBytes([Environment]::NewLine+$end+[Environment]::NewLine);$candidate.Write($bytes,0,$bytes.Length)
            $candidate.Flush($true);$candidate.Dispose();$candidate=$null;$current.Dispose();$current=$null;$old.Dispose();$old=$null
            $backup=New-LastWorldArchivePath $State 'Loose-Files' ('Before-History-Import/'+$relative)
            # Backup retains the exact pre-import Data bytes. Failed replace
            # leaves both original files and a closed .pending candidate intact.
            [System.IO.File]::Replace($temporary,$Destination,$backup)
            Write-LastWorldMigrationRecord $State 'append_exact_history' $Source $Destination ('Imported original bytes once; migration_id='+$id+'; prior Data bytes retained at '+(Get-LastWorldRelativePath $State.Root $backup)) $hash
        } finally {foreach ($stream in @($old,$current,$candidate)) {if ($null -ne $stream) {$stream.Dispose()}}}
    }
    $archive=New-LastWorldArchivePath $State 'Loose-Files' ('Imported-History/'+$relative)
    Move-LastWorldExactItem $State $Source $archive 'retain_imported_history' ('Original history retained; canonical import already committed; migration_id='+$id)
}
function Get-LastWorldLegacySources([string]$Root) {
    $found=New-Object 'System.Collections.Generic.List[object]'
    $dataNames=@('console.txt','settings.json','settings','saves','check-results','workload-scope.json','check-launcher-errors.txt','check-feedback-errors.txt')
    $ownedNames=@('Last-World.exe','Last-World-Mechanisms.exe','Last-World-Cli-Checks.exe','native-build-stamp.json','benchmark-plan.json','benchmark-unit-tests.json','build-manifest.json','README.md','THIRD-PARTY-NOTICES.txt','Run-PC-Qualification.cmd','Run-PC-Qualification.ps1','Run-Visual-Test.cmd','Run-Water-Check.cmd','Run-River-Test.cmd','Run-Benchmark.cmd','Run-Checks.ps1','Run-Checks-OwnedProcess.ps1','Run-Checks-GpuTelemetry.ps1','Run-Checks-Feedback.ps1','Run-Checks-Watchdog.cmd','Initialize-Layout.ps1','Check-Progress.cmd','Progress.txt','Send-Back-Progress.zip','docs','qualification','licenses','project-records')
    foreach ($item in Get-ChildItem -LiteralPath $Root -Force) {
        $kind=$null;$name=$item.Name
        if ($name -in $dataNames -or (-not $item.PSIsContainer -and ($name -like '*-history.jsonl' -or $name -like 'benchmark-results.json*' -or $name -match '^(river|campaign)-check-\d+-.*\.(png|json|jsonl)$'))) {$kind='data'}
        elseif (-not $item.PSIsContainer -and $name -match '^Send-?Back-.*\.zip\.part$') {$kind='feedback'}
        elseif ($name -in $ownedNames) {$kind='owned'}
        elseif ($name -eq 'ui' -and $item.PSIsContainer) {$kind='ui'}
        if ($null -ne $kind) {$found.Add([pscustomobject]@{Path=$item.FullName;Name=$name;Kind=$kind})}
    }
    return $found.ToArray()
}
function Move-LastWorldLegacyUi($State,[string]$Folder,$OldManifest) {
    foreach ($child in [System.IO.Directory]::GetFileSystemEntries($Folder)) {
        if ([System.IO.Directory]::Exists($child)) {Move-LastWorldLegacyUi $State $child $OldManifest;continue}
        $relative=Get-LastWorldRelativePath $State.Root $child;$entry=$null
        if ($null -ne $OldManifest -and $null -ne $OldManifest.payload_files) {$entry=$OldManifest.payload_files.PSObject.Properties[$relative]}
        if ($null -ne $entry -and [string]$entry.Value.sha256 -match '^[0-9a-fA-F]{64}$' -and (Get-LastWorldFileHash $child) -eq ([string]$entry.Value.sha256).ToLowerInvariant()) {
            Move-LastWorldExactItem $State $child (New-LastWorldArchivePath $State 'Loose-Files' $relative) 'archive_legacy_default_ui' 'Exact old-manifest default retained as reference; App/ui supplies this build default.'
        } else {Move-LastWorldDataItem $State $child (Join-Path $State.Data $relative)}
    }
    [System.IO.Directory]::Delete($Folder,$false)
}
function Invoke-LastWorldLayout {
    [CmdletBinding()]
    param([Parameter(Mandatory=$true)][string]$InstallRoot,[scriptblock]$ProcessProvider={Get-Process})
    $root=Get-LastWorldFullPath $InstallRoot
    [void](Assert-LastWorldSafePath $root $root)
    if (-not [System.IO.Directory]::Exists($root)) {throw ('Install folder not found: '+$root)}
    $manifest=Assert-LastWorldOrganizedPayload $root
    Write-Host ('Last World | organized layout 1 | native '+([string]$manifest.native_source_sha256).Substring(0,12)+' | exe '+([string]$manifest.executable_sha256).Substring(0,12))
    Write-Host ('Install root: '+$root)
    Write-Host 'One adaptive ramp; 20-minute 4x battle; CPU/RAM/GPU logging.'
    Assert-LastWorldNoActiveProgram $root $ProcessProvider
    $sources=@(Get-LastWorldLegacySources $root)
    foreach ($source in $sources) {Assert-LastWorldSafeTree $root $source.Path}
    $data=Join-Path $root 'Data'
    foreach ($path in @($data,(Join-Path $data 'Locks'),(Join-Path $data 'Temporary'),(Join-Path $data 'Legacy'),(Join-Path $data 'layout-migration.jsonl'),(Join-Path $data 'console.txt'))) {[void](Assert-LastWorldSafePath $root $path)}
    foreach ($path in @($data,(Join-Path $data 'Locks'),(Join-Path $data 'Temporary'),(Join-Path $data 'Legacy'))) {if ((Test-LastWorldPathExists $path) -and -not [System.IO.Directory]::Exists($path)) {throw ('A required layout directory is occupied by a file: '+$path)}}
    foreach ($path in @((Join-Path $data 'layout-migration.jsonl'),(Join-Path $data 'console.txt'))) {if ((Test-LastWorldPathExists $path) -and -not [System.IO.File]::Exists($path)) {throw ('A canonical migration log path is occupied by a directory: '+$path)}}
    # Validate entire destinations that may be recursively merged before moving
    # even the first log. A nested junction cannot redirect saved-player data.
    foreach ($source in $sources) {if ($source.Kind -in @('data','ui')) {Assert-LastWorldSafeTree $root (Join-Path $data $source.Name)}}
    $oldManifest=$null;$oldManifestPath=Join-Path $root 'build-manifest.json'
    if ([System.IO.File]::Exists($oldManifestPath)) {try {$oldManifest=[System.IO.File]::ReadAllText($oldManifestPath)|ConvertFrom-Json} catch {Write-Warning 'The old root manifest is unreadable; all legacy UI files will be preserved as user overrides.'}}
    [void][System.IO.Directory]::CreateDirectory($data)
    [void][System.IO.Directory]::CreateDirectory((Join-Path $data 'Locks'))
    $oldLock=$null;$newLock=$null
    try {
        $oldPath=Join-Path $root 'Run-Checks.lock';[void](Assert-LastWorldSafePath $root $oldPath)
        $options=[System.IO.FileOptions]::None
        $oldLockExisted=Test-LastWorldPathExists $oldPath
        if (-not $oldLockExisted) {$options=[System.IO.FileOptions]::DeleteOnClose}
        try {$oldLock=[System.IO.FileStream]::new($oldPath,[System.IO.FileMode]::OpenOrCreate,[System.IO.FileAccess]::ReadWrite,[System.IO.FileShare]::None,4096,$options)} catch {throw 'Close the existing checking window, then run this launcher again. The old Run-Checks.lock is busy or unavailable; nothing was migrated.'}
        try {$newLock=[System.IO.FileStream]::new((Join-Path $data 'Locks/Run-Checks.lock'),[System.IO.FileMode]::OpenOrCreate,[System.IO.FileAccess]::ReadWrite,[System.IO.FileShare]::None)} catch {throw 'Close the existing checking window, then run this launcher again. Data/Locks/Run-Checks.lock is busy or unavailable; nothing was migrated.'}
        Assert-LastWorldNoActiveProgram $root $ProcessProvider
        [void][System.IO.Directory]::CreateDirectory((Join-Path $data 'Temporary'))
        $state=@{Root=$root;Data=$data;Journal=(Join-Path $data 'layout-migration.jsonl');Console=(Join-Path $data 'console.txt');Utf8=[Text.UTF8Encoding]::new($false);Operations=0;Conflicts=0}
        # Preserve/merge the old canonical log before adding any migration rows.
        $ordered=@($sources|Sort-Object @{Expression={if ($_.Name -eq 'console.txt') {0} elseif ($_.Kind -eq 'data') {1} elseif ($_.Kind -eq 'ui') {2} else {3}}},Name)
        foreach ($source in $ordered) {
            if ($source.Kind -eq 'data') {
                $destination=Join-Path $data $source.Name
                if (-not [System.IO.Directory]::Exists($source.Path) -and ($source.Name -eq 'console.txt' -or $source.Name -like '*-history.jsonl')) {Merge-LastWorldAppendHistory $state $source.Path $destination} else {Move-LastWorldDataItem $state $source.Path $destination}
            } elseif ($source.Kind -eq 'ui') {Move-LastWorldLegacyUi $state $source.Path $oldManifest}
            elseif ($source.Kind -eq 'feedback') {Move-LastWorldExactItem $state $source.Path (New-LastWorldArchivePath $state 'Feedback' $source.Name) 'retain_feedback_candidate' 'Closed or incomplete candidate preserved; final feedback ZIP in root was left in place.'}
            else {Move-LastWorldExactItem $state $source.Path (New-LastWorldArchivePath $state 'Loose-Files' $source.Name) 'archive_legacy_owned' 'Known legacy application file/tree retained; organized App/Tools/Documentation is authoritative.'}
        }
        # The new lock remains held while a stale compatibility lock is archived.
        # Windows cannot rename the old lock while its FileShare.None handle is
        # open; all player-data moves have finished before releasing that handle.
        if ($oldLockExisted) {
            $oldLock.Dispose();$oldLock=$null
            try {
                [void](Assert-LastWorldSafePath $root $oldPath)
                Move-LastWorldExactItem $state $oldPath (New-LastWorldArchivePath $state 'Loose-Files' 'Run-Checks.lock') 'archive_legacy_lock' 'Retained old compatibility lock after all data moves; Data/Locks/Run-Checks.lock remained held.'
            } catch {
                Write-LastWorldMigrationRecord $state 'retain_busy_legacy_lock' $oldPath $oldPath ('All data moves finished; old lock left intact because it could not be safely archived: '+$_.Exception.Message)
                Write-Warning 'The old root Run-Checks.lock remains intact; the organized Data lock is authoritative.'
            }
        }
        Write-Host ('Data: '+$data+' | migration operations: '+$state.Operations+' | retained conflicts: '+$state.Conflicts)
        return [pscustomobject]@{InstallRoot=$root;DataRoot=$data;Operations=$state.Operations;Conflicts=$state.Conflicts;Journal=$state.Journal}
    } finally {if ($null -ne $newLock) {$newLock.Dispose()};if ($null -ne $oldLock) {$oldLock.Dispose()}}
}

if ($MyInvocation.InvocationName -ne '.') {
    try {
        if ([string]::IsNullOrWhiteSpace($InstallRoot)) {$InstallRoot=$env:LAST_WORLD_INSTALL_ROOT}
        if ([string]::IsNullOrWhiteSpace($InstallRoot)) {$InstallRoot=Join-Path $PSScriptRoot '..'}
        [void](Invoke-LastWorldLayout -InstallRoot $InstallRoot)
        exit 0
    } catch {Write-Host ('Layout initialization stopped: '+$_.Exception.Message);Write-Host 'No process was terminated. Close Last World/checking windows and keep the complete update together, then retry.';exit 2}
}
