#requires -Version 5.1
[CmdletBinding()]
param(
    [string]$Version = '2.0.0',
    [string]$Prefix = (Join-Path $env:LOCALAPPDATA 'QinAegis'),
    [string]$Archive,
    [string]$Checksum,
    [switch]$NoStart,
    [switch]$Rollback,
    [switch]$Uninstall,
    [switch]$DiscardPrevious
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:OS -ne 'Windows_NT') { throw 'This installer requires Windows.' }
if ($Version -notmatch '^2\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$') { throw 'Invalid 2.x version.' }
if (-not [IO.Path]::IsPathRooted($Prefix)) { throw 'Prefix must be absolute.' }
$Prefix = [IO.Path]::GetFullPath($Prefix).TrimEnd('\')
if ($Prefix -eq [IO.Path]::GetPathRoot($Prefix).TrimEnd('\')) { throw 'Drive root cannot be an installation directory.' }
if ($Rollback -and $Uninstall) { throw 'Select one operation.' }
function Assert-Plain([string]$Path) {
    $item = Get-Item -LiteralPath $Path -Force
    if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'Linked installation path refused.' }
}
$ancestor = $Prefix
while ($ancestor) {
    if (Test-Path -LiteralPath $ancestor) { Assert-Plain $ancestor }
    $ancestor = [IO.Path]::GetDirectoryName($ancestor)
}
[void][IO.Directory]::CreateDirectory($Prefix)
$lock = $null; $stage = $null; $download = $null; $stageIsBackup = $false
$binary = Join-Path $Prefix 'qinaegis.exe'
$previous = Join-Path $Prefix 'qinaegis.previous.exe'
$receipt = Join-Path $Prefix 'installation.json'
function Hash([string]$Path) { (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() }
function Assert-Stopped {
    # Inspect OS process paths; never execute an unverified pre-existing binary.
    foreach ($process in Get-Process -Name qinaegis,qinaegis.previous -ErrorAction SilentlyContinue) {
        try { $path = $process.Path } catch { throw 'Cannot inspect a running QinAegis process; stop it before maintenance.' }
        if (-not $path -or $path -eq $binary -or $path -eq $previous) { throw 'Existing service is running; stop it before installation maintenance.' }
    }
}
# Finite HTTPS transport, with an explicit allowlist for GitHub release redirects.
function Download-Release([uri]$Uri, [string]$Destination, [long]$Limit, [int]$Seconds) {
    $clock = [Diagnostics.Stopwatch]::StartNew()
    for ($redirect = 0; $redirect -le 5; $redirect++) {
        if ($Uri.Scheme -ne 'https' -or $Uri.Port -ne 443 -or $Uri.UserInfo -or
            $Uri.Host -notin @('github.com','release-assets.githubusercontent.com','objects.githubusercontent.com','github-releases.githubusercontent.com')) {
            throw 'Release redirect is outside the permitted HTTPS hosts.'
        }
        $remaining = $Seconds * 1000 - [int]$clock.ElapsedMilliseconds
        if ($remaining -le 0) { throw 'Release download timed out.' }
        $request = [Net.HttpWebRequest]::Create($Uri)
        $request.AllowAutoRedirect = $false
        $request.Timeout = $remaining
        $request.ReadWriteTimeout = [Math]::Min(30000,$remaining)
        $response = $request.GetResponse()
        try {
            if ([int]$response.StatusCode -in @(301,302,303,307,308)) {
                if ($redirect -eq 5 -or -not $response.Headers['Location']) { throw 'Release redirect limit exceeded.' }
                $Uri = [uri]::new($Uri,$response.Headers['Location'])
                continue
            }
            if ([int]$response.StatusCode -ne 200 -or $response.ContentLength -gt $Limit) { throw 'Release response failed or exceeds limit.' }
            $networkStream = $response.GetResponseStream()
            $output = [IO.File]::Open($Destination,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
            try {
                $buffer = New-Object byte[] 65536
                $total = 0L
                while ($true) {
                    $readBudget = $Seconds * 1000 - [int]$clock.ElapsedMilliseconds
                    if ($readBudget -le 0) { throw 'Release download timed out.' }
                    $networkStream.ReadTimeout = [Math]::Min(30000,$readBudget)
                    $count = $networkStream.Read($buffer,0,$buffer.Length)
                    if ($count -eq 0) { break }
                    $total += $count
                    if ($total -gt $Limit) { throw 'Release download exceeds limit.' }
                    if ($clock.Elapsed.TotalSeconds -ge $Seconds) { throw 'Release download timed out.' }
                    $output.Write($buffer,0,$count)
                }
                $output.Flush($true)
            } finally { $output.Dispose(); $networkStream.Dispose() }
            return
        } finally { $response.Dispose() }
    }
}
function Save-Receipt($Hashes) {
    $temp = Join-Path $Prefix ('.receipt-' + [Guid]::NewGuid().ToString('N'))
    try {
        [IO.File]::WriteAllText($temp, (@{schema_version=1; hashes=@($Hashes | Select-Object -Unique)} | ConvertTo-Json -Compress))
        if (Test-Path -LiteralPath $receipt) { [IO.File]::Replace($temp,$receipt,$null) }
        else { [IO.File]::Move($temp,$receipt) }
    } finally { if (Test-Path -LiteralPath $temp) { Remove-Item -LiteralPath $temp } }
}
try {
    if (Test-Path -LiteralPath (Join-Path $Prefix '.install.lock')) { Assert-Plain (Join-Path $Prefix '.install.lock') }
    $lock = [IO.File]::Open((Join-Path $Prefix '.install.lock'),[IO.FileMode]::OpenOrCreate,[IO.FileAccess]::ReadWrite,[IO.FileShare]::None)
    $known = @('qinaegis.exe','qinaegis.previous.exe','installation.json','.install.lock')
    foreach ($item in Get-ChildItem -LiteralPath $Prefix -Force) {
        Assert-Plain $item.FullName
        if ($item.Name -notin $known -or $item.PSIsContainer) { throw 'Unknown installation contents; preserve and inspect manually.' }
    }
    $hashes = @()
    if (Test-Path -LiteralPath $receipt) {
        if ((Get-Item -LiteralPath $receipt).Length -gt 4096) { throw 'Installation receipt exceeds limit.' }
        $record = Get-Content -LiteralPath $receipt -Raw | ConvertFrom-Json
        if ($record.schema_version -ne 1 -or @($record.hashes).Count -gt 3) { throw 'Invalid installation receipt.' }
        $hashes = @($record.hashes)
        foreach ($hash in $hashes) { if ($hash -notmatch '^[0-9a-f]{64}$') { throw 'Invalid installation checksum.' } }
    }
    foreach ($file in @($binary,$previous)) {
        if ((Test-Path -LiteralPath $file) -and (Hash $file) -notin $hashes) { throw 'Existing executable is not verified by this installer; preserve it manually.' }
    }
    $hashes = @(@($binary,$previous) | Where-Object { Test-Path -LiteralPath $_ } | ForEach-Object { Hash $_ })
    Assert-Stopped
    if ($Uninstall) {
        foreach ($file in @($binary,$previous,$receipt)) { if (Test-Path -LiteralPath $file) { Remove-Item -LiteralPath $file } }
        Write-Output 'Removed verified service executables. Project, model, runtime and report data are preserved.'
        return
    }
    if ($Rollback) {
        if (-not (Test-Path -LiteralPath $previous) -or -not (Test-Path -LiteralPath $binary)) { throw 'Verified previous installation is unavailable.' }
        $stage = Join-Path $Prefix ('.swap-' + [Guid]::NewGuid().ToString('N'))
        # Atomic replacement retains the former current binary at a known temp.
        [IO.File]::Replace($previous,$binary,$stage)
        $stageIsBackup = $true
        [IO.File]::Move($stage,$previous); $stage = $null; $stageIsBackup = $false
        Write-Output 'Rolled back the service. Data is unchanged.'
    } else {
        if ((Test-Path -LiteralPath $previous) -and -not $DiscardPrevious) { throw 'Previous backup exists; explicitly use -DiscardPrevious after inspection.' }
        if (-not $Archive) {
            $download = Join-Path ([IO.Path]::GetTempPath()) ('qinaegis-download-' + [Guid]::NewGuid().ToString('N'))
            [void][IO.Directory]::CreateDirectory($download)
            $name = 'qinaegis-x86_64-pc-windows-msvc.zip'
            $base = "https://github.com/mbpz/qinAegis/releases/download/v$Version"
            [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
            $Archive = Join-Path $download $name
            Download-Release ([uri]"$base/$name") $Archive 134217728 120
            $checksumFile = Join-Path $download 'SHA256SUMS'
            Download-Release ([uri]"$base/SHA256SUMS") $checksumFile 131072 30
            $sums = [IO.File]::ReadAllText($checksumFile)
            $matches = @($sums -split "`n" | Where-Object { $_.TrimEnd("`r") -match ('^[0-9a-fA-F]{64}  ' + [regex]::Escape($name) + '$') })
            if ($matches.Count -ne 1) { throw 'Missing or ambiguous release checksum.' }
            $Checksum = $matches[0].Substring(0,64)
        }
        if ($Checksum -notmatch '^[0-9a-fA-F]{64}$') { throw 'Archive requires an exact SHA256 checksum.' }
        Add-Type -AssemblyName System.IO.Compression.FileSystem
        $archiveStream = [IO.File]::Open([IO.Path]::GetFullPath($Archive),[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
        $zip = $null
        try {
            if ($archiveStream.Length -gt 134217728) { throw 'Archive exceeds limit; installation preserved.' }
            $sha = [Security.Cryptography.SHA256]::Create()
            try { $actual = ([BitConverter]::ToString($sha.ComputeHash($archiveStream))).Replace('-','').ToLowerInvariant() } finally { $sha.Dispose() }
            if ($actual -ne $Checksum.ToLowerInvariant()) { throw 'Checksum mismatch or archive exceeds limit; installation preserved.' }
            $archiveStream.Position = 0
            $zip = [IO.Compression.ZipArchive]::new($archiveStream,[IO.Compression.ZipArchiveMode]::Read,$true)
            if ($zip.Entries.Count -ne 3) { throw 'Archive must have exactly three regular entries.' }
            $names = @('qinaegis.exe','LICENSE','release.json')
            foreach ($entry in $zip.Entries) {
                $unixType = (($entry.ExternalAttributes -shr 16) -band 61440)
                if ($entry.FullName -cnotin $names -or $entry.Length -le 0 -or $entry.Length -gt 67108864 -or $unixType -notin @(0,32768) -or ($entry.ExternalAttributes -band 16) -ne 0) { throw 'Unsafe ZIP entry.' }
                $names = @($names | Where-Object { $_ -cne $entry.FullName })
            }
            if ($names.Count -ne 0) { throw 'Duplicate or missing ZIP entry.' }
            $manifestEntry = $zip.GetEntry('release.json')
            if ($manifestEntry.Length -gt 8192) { throw 'Release manifest exceeds limit.' }
            $reader = [IO.StreamReader]::new($manifestEntry.Open())
            try {
                $characters = New-Object char[] 8193
                $length = $reader.ReadBlock($characters,0,$characters.Length)
                if ($length -gt 8192) { throw 'Inflated release manifest exceeds limit.' }
                $manifest = [string]::new($characters,0,$length) | ConvertFrom-Json
            } finally { $reader.Dispose() }
            if ($manifest.schema_version -ne 1 -or $manifest.product -ne 'qinaegis' -or $manifest.version -ne $Version -or $manifest.target -ne 'x86_64-pc-windows-msvc' -or $manifest.entry -ne 'qinaegis.exe' -or $manifest.interface -ne 'local-web' -or $manifest.engine_included -ne $false -or $manifest.commit -notmatch '^[0-9a-f]{40}$') { throw 'Archive release metadata mismatch.' }
            $stage = Join-Path $Prefix ('.stage-' + [Guid]::NewGuid().ToString('N'))
            $executableEntry = $zip.GetEntry('qinaegis.exe')
            $source = $executableEntry.Open()
            $target = [IO.File]::Open($stage,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
            try {
                $buffer = New-Object byte[] 65536
                $written = 0L
                while (($count = $source.Read($buffer,0,$buffer.Length)) -gt 0) {
                    $written += $count
                    if ($written -gt 67108864 -or $written -gt $executableEntry.Length) { throw 'Inflated executable exceeds limit.' }
                    $target.Write($buffer,0,$count)
                }
                if ($written -ne $executableEntry.Length) { throw 'Executable length mismatch.' }
                $target.Flush($true)
            } finally { $target.Dispose(); $source.Dispose() }
        } finally { if ($zip) { $zip.Dispose() }; $archiveStream.Dispose() }
        Assert-Stopped
        $newHash = Hash $stage
        $oldHash = if (Test-Path -LiteralPath $binary) { Hash $binary } else { $null }
        # Write the allowed identities before atomic replacement. After an
        # interrupted replacement either verified binary remains recognizable.
        Save-Receipt @(@($hashes) + @($newHash) | Select-Object -Unique)
        if (Test-Path -LiteralPath $previous) { Remove-Item -LiteralPath $previous }
        if ($oldHash) { [IO.File]::Replace($stage,$binary,$previous) } else { [IO.File]::Move($stage,$binary) }
        $stage = $null
        Save-Receipt @(@($newHash,$oldHash) | Where-Object { $_ })
        Write-Output "Installed $binary. Start it to open the local Web workbench."
    }
    if (-not $NoStart) {
        $launch = [Diagnostics.ProcessStartInfo]::new()
        $launch.FileName = $binary; $launch.Arguments = 'start'; $launch.UseShellExecute = $false
        $process = [Diagnostics.Process]::Start($launch)
        try {
            if (-not $process.WaitForExit(20000)) {
                $process.Kill()
                throw 'Service launcher timed out. Installation is retained; inspect service status before retrying.'
            }
            if ($process.ExitCode -ne 0) { throw 'Service start failed; verified installation and backup are retained.' }
        } finally { $process.Dispose() }
    }
} finally {
    if ($stage -and (Test-Path -LiteralPath $stage)) {
        if ($stageIsBackup) { Write-Warning "Retained verified rollback backup at $stage; inspect before further maintenance." }
        else { Remove-Item -LiteralPath $stage }
    }
    if ($download -and (Test-Path -LiteralPath $download)) { Remove-Item -LiteralPath $download -Recurse }
    if ($lock) { $lock.Dispose() }
}
