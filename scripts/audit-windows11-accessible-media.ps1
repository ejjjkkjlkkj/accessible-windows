# Windows 11 accessible media audit — read-only
# PowerShell 7 / Windows PowerShell 5.1
# No downloads, ISO mounts, file writes outside the report, firmware/NVRAM changes,
# USB formatting, or reboots. Results are evidence labels, not assumed PASS values.

[CmdletBinding()]
param(
    [string]$IsoPath = "$env:USERPROFILE\Downloads\Windows11_Client_x64_fr-fr_26300_9457.iso",
    [string]$ReportPath = "$env:USERPROFILE\Desktop\WINDOWS11-ACCESSIBLE-MEDIA-AUDIT.txt",
    [string[]]$SearchRoots = @(
        "$env:USERPROFILE\Downloads",
        "$env:USERPROFILE\Desktop",
        "$env:USERPROFILE\Documents",
        "$env:TEMP",
        'C:\UUP',
        'C:\ISO',
        'C:\Windows11',
        'C:\Firmware-Audit'
    )
)

$ErrorActionPreference = 'Continue'
$lines = [System.Collections.Generic.List[string]]::new()
function Add-Line([string]$Text) { $lines.Add($Text) }

Add-Line 'WINDOWS 11 ACCESSIBLE MEDIA AUDIT'
Add-Line "Timestamp: $(Get-Date -Format 'yyyy-MM-ddTHH:mm:ssK')"
Add-Line "Computer: $env:COMPUTERNAME"
Add-Line "PowerShell: $($PSVersionTable.PSVersion)"
Add-Line 'Mode: READ-ONLY INVENTORY'
Add-Line 'Actions prohibited/performed: no download, no mount, no media write, no firmware/NVRAM write, no USB format, no reboot.'
Add-Line ''

Add-Line '=== WINDOWS BASELINE ==='
try {
    $os = Get-CimInstance Win32_OperatingSystem -ErrorAction Stop
    Add-Line "Caption: $($os.Caption)"
    Add-Line "Version: $($os.Version)"
    Add-Line "Build: $($os.BuildNumber)"
    Add-Line "Architecture: $($os.OSArchitecture)"
} catch {
    Add-Line "OS inventory: BLOCKED — $($_.Exception.Message)"
}
Add-Line ''

Add-Line '=== TOOLS ==='
foreach ($name in @('pwsh','git','aria2c','dism','oscdimg','7z','7zz','ollama')) {
    $cmd = Get-Command $name -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($cmd) { Add-Line "$name : FOUND ($($cmd.Source))" }
    else { Add-Line "$name : NOT_FOUND" }
}
Add-Line ''

Add-Line '=== TARGET ISO ==='
if (Test-Path -LiteralPath $IsoPath -PathType Leaf) {
    $iso = Get-Item -LiteralPath $IsoPath -ErrorAction SilentlyContinue
    Add-Line 'ISO: FOUND'
    Add-Line "Path: $($iso.FullName)"
    Add-Line "SizeBytes: $($iso.Length)"
    Add-Line "LastWriteTime: $($iso.LastWriteTime.ToString('o'))"
    try {
        $hash = Get-FileHash -LiteralPath $IsoPath -Algorithm SHA256 -ErrorAction Stop
        Add-Line "SHA256: $($hash.Hash)"
    } catch {
        Add-Line "SHA256: BLOCKED — $($_.Exception.Message)"
    }

    $sevenZip = Get-Command 7z,7zz -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($sevenZip) {
        try {
            $listing = @(& $sevenZip.Source l -slt $IsoPath 2>&1)
            $listExit = $LASTEXITCODE
            if ($listExit -eq 0) {
                # 7-Zip's technical listing uses one backslash per path separator.
                # Parse Path fields rather than matching the entire diagnostic output.
                $paths = @(
                    $listing |
                    ForEach-Object {
                        if ("$_" -match '^Path = (.+)$') { $Matches[1].Replace('/', '\\') }
                    }
                )
                $hasBootEfi = [bool]($paths | Where-Object { $_ -match '(?i)(^|\\\\)EFI\\\\BOOT\\\\BOOTX64\\.EFI$' } | Select-Object -First 1)
                $hasBootWim = [bool]($paths | Where-Object { $_ -match '(?i)(^|\\\\)sources\\\\boot\\.wim$' } | Select-Object -First 1)
                $hasInstallImage = [bool]($paths | Where-Object { $_ -match '(?i)(^|\\\\)sources\\\\install\\.(wim|esd)$' } | Select-Object -First 1)
                Add-Line 'ISO_LISTING: PASS'
                Add-Line "UEFI_BOOTX64_EFI: $(if ($hasBootEfi) {'FOUND'} else {'NOT_FOUND'})"
                Add-Line "SOURCES_BOOT_WIM: $(if ($hasBootWim) {'FOUND'} else {'NOT_FOUND'})"
                Add-Line "SOURCES_INSTALL_WIM_OR_ESD: $(if ($hasInstallImage) {'FOUND'} else {'NOT_FOUND'})"
                if ($hasBootEfi -and $hasBootWim -and $hasInstallImage) {
                    Add-Line 'ISO_STRUCTURE: PASS (required entries present; this does not prove successful boot or accessibility)'
                } else {
                    Add-Line 'ISO_STRUCTURE: FAIL (one or more required entries are missing)'
                }
            } else {
                Add-Line "ISO_LISTING: FAIL (7-Zip exit code $listExit)"
                $listing | Select-Object -Last 12 | ForEach-Object { Add-Line "$_" }
            }
        } catch {
            Add-Line "ISO_LISTING: BLOCKED — $($_.Exception.Message)"
        }
    } else {
        Add-Line 'ISO_LISTING: NOT_VALIDATED (7z/7zz not found; no ISO mount attempted)'
    }
} else {
    Add-Line 'ISO: NOT_FOUND at configured path'
    Add-Line "Configured path: $IsoPath"
    Add-Line 'ISO_STRUCTURE: NOT_VALIDATED'
}
Add-Line ''

Add-Line '=== EXISTING UUP / INSTALL FILES ==='
$extensions = @('.iso','.wim','.esd','.cab','.msu','.aria2')
$seen = @{}
foreach ($root in $SearchRoots) {
    if (-not (Test-Path -LiteralPath $root -PathType Container)) { continue }
    Add-Line "--- ROOT: $root ---"
    try {
        Get-ChildItem -LiteralPath $root -File -Recurse -ErrorAction SilentlyContinue |
            Where-Object {
                $_.Extension.ToLowerInvariant() -in $extensions -or
                $_.Name -match '(?i)uup|convertconfig|aria2_download_windows|uup_download_windows'
            } |
            ForEach-Object {
                if (-not $seen.ContainsKey($_.FullName)) {
                    $seen[$_.FullName] = $true
                    Add-Line ("FILE | {0} | {1} bytes | {2}" -f $_.FullName,$_.Length,$_.LastWriteTime.ToString('o'))
                }
            }
    } catch {
        Add-Line "SEARCH: PARTIAL — $($_.Exception.Message)"
    }
}
if ($seen.Count -eq 0) { Add-Line 'UUP_FILES: NOT_FOUND in configured roots' }
else { Add-Line "UUP_FILES: FOUND ($($seen.Count) matching files)" }
Add-Line ''

Add-Line '=== LOCAL ACCESSIBILITY COMPONENTS ==='
foreach ($path in @(
    "$env:WINDIR\System32\Narrator.exe",
    "$env:WINDIR\System32\utilman.exe"
)) {
    if (Test-Path -LiteralPath $path -PathType Leaf) { Add-Line "FOUND: $path" }
    else { Add-Line "NOT_FOUND: $path" }
}
Add-Line 'NOTE: local Narrator presence does not prove speech works in Windows PE/Setup or in the custom UEFI environment.'
Add-Line ''

Add-Line '=== FINAL STATUS ==='
Add-Line 'Inventory: COMPLETE if the report was written successfully.'
Add-Line 'Windows Setup screen-reader speech: NOT_VALIDATED until tested in the real setup environment.'
Add-Line 'Physical ASUS M1603QA proof: NOT_VALIDATED unless a human confirms audible speech on physical hardware.'
Add-Line 'QEMU/OVMF or VMware evidence is not physical-hardware proof.'
Add-Line ''

$parent = Split-Path -Parent $ReportPath
if ($parent -and -not (Test-Path -LiteralPath $parent -PathType Container)) {
    New-Item -ItemType Directory -Path $parent -Force | Out-Null
}
$lines | Set-Content -LiteralPath $ReportPath -Encoding utf8
Write-Output "Audit report: $ReportPath"
Write-Output "Matching UUP/media files: $($seen.Count)"
Write-Output 'No downloads, ISO mounts, media writes, firmware/NVRAM writes, USB formatting, or reboots were performed.'
