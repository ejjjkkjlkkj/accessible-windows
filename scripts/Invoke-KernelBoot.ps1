#Requires -Version 7.0
<#
.SYNOPSIS
    Builds the UEFI loader + native kernel and boots them under QEMU/OVMF,
    returning the captured debug-console log.

.DESCRIPTION
    Single reusable harness for every bare-metal proof in this repository.
    One invocation = one kernel feature configuration = one QEMU run.

    Evidence rule (dossier section 1.1): a build success is never a PASS.
    The caller must assert on markers found in the returned log.
#>
[CmdletBinding()]
param(
    # Kernel cargo feature set for this run. Empty = normal boot path.
    [string[]]$Features = @(),

    # Label used for the per-run evidence directory.
    [string]$Name = 'normal',

    [string]$Qemu = 'C:\Program Files\qemu\qemu-system-x86_64.exe',

    [ValidateRange(5, 300)][int]$TimeoutSeconds = 40,

    # Skip cargo build and reuse the artifacts already staged for this run name.
    [switch]$NoBuild,

    # Extra QEMU arguments (e.g. -smp 2).
    [string[]]$QemuArgs = @()
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Invoke-Checked {
    param([string]$Command, [string[]]$Arguments)
    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Command $($Arguments -join ' ') failed with exit code $LASTEXITCODE" }
}

$repo = Split-Path $PSScriptRoot -Parent
$run = Join-Path $repo "target/boot-evidence/$Name"
$esp = Join-Path $run 'esp'

Push-Location $repo
try {
    if (-not $NoBuild) {
        if (Test-Path -LiteralPath $run) { Remove-Item -LiteralPath $run -Recurse -Force }
        New-Item -ItemType Directory -Path (Join-Path $esp 'EFI/BOOT') -Force | Out-Null

        Invoke-Checked cargo @('build', '--locked', '--manifest-path', 'boot/uefi/Cargo.toml',
            '--target', 'x86_64-unknown-uefi', '--release')

        $kernelArgs = @('build', '--locked', '--manifest-path', 'kernel/x86_64/Cargo.toml',
            '--target', 'x86_64-unknown-none', '--release')
        if ($Features.Count -gt 0) {
            $kernelArgs += @('--features', ($Features -join ','))
        }
        Invoke-Checked cargo $kernelArgs

        $sysroot = & rustc --print sysroot
        if ($LASTEXITCODE -ne 0) { throw 'Cannot resolve Rust sysroot' }
        $objcopy = @(Get-ChildItem -LiteralPath (Join-Path $sysroot 'lib/rustlib') -Recurse -Filter llvm-objcopy.exe)
        if ($objcopy.Count -lt 1) { throw 'Install llvm-tools-preview: rustup component add llvm-tools-preview' }

        Copy-Item -LiteralPath 'boot/uefi/target/x86_64-unknown-uefi/release/aw-uefi-boot.efi' `
            -Destination (Join-Path $esp 'EFI/BOOT/BOOTX64.EFI') -Force
        Invoke-Checked $objcopy[0].FullName @('-O', 'binary',
            'kernel/x86_64/target/x86_64-unknown-none/release/aw-kernel-x86_64',
            (Join-Path $esp 'KERNEL.BIN'))

        # Keep the unstripped ELF next to the flat image for symbol-level triage.
        Copy-Item -LiteralPath 'kernel/x86_64/target/x86_64-unknown-none/release/aw-kernel-x86_64' `
            -Destination (Join-Path $run 'kernel.elf') -Force
    }

    if (-not (Test-Path -LiteralPath (Join-Path $esp 'KERNEL.BIN'))) {
        throw "No staged boot files for run '$Name'. Run without -NoBuild first."
    }

    $share = Join-Path (Split-Path $Qemu -Parent) 'share'
    $code = Join-Path $share 'edk2-x86_64-code.fd'
    $varsSrc = Join-Path $share 'edk2-i386-vars.fd'
    foreach ($required in @($Qemu, $code, $varsSrc)) {
        if (-not (Test-Path -LiteralPath $required -PathType Leaf)) { throw "Missing dependency: $required" }
    }
    $vars = Join-Path $run 'vars.fd'
    Copy-Item -LiteralPath $varsSrc -Destination $vars -Force

    $log = Join-Path $run 'debug.log'
    if (Test-Path -LiteralPath $log) { Remove-Item -LiteralPath $log -Force }

    $start = [System.Diagnostics.ProcessStartInfo]::new($Qemu)
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardError = $true
    $baseArgs = @(
        '-machine', 'q35', '-accel', 'tcg', '-cpu', 'max', '-m', '256M',
        '-display', 'none', '-serial', 'none', '-monitor', 'none',
        '-no-reboot', '-net', 'none',
        '-debugcon', "file:$log",
        '-drive', "if=pflash,format=raw,readonly=on,file=$code",
        '-drive', "if=pflash,format=raw,file=$vars",
        '-drive', "format=raw,snapshot=on,file=fat:ro:$esp"
    )
    foreach ($argument in ($baseArgs + $QemuArgs)) { $start.ArgumentList.Add($argument) }

    $process = [System.Diagnostics.Process]::Start($start)
    $stderrTask = $process.StandardError.ReadToEndAsync()
    try {
        if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
            $process.Kill()
            $process.WaitForExit()
        }
    } finally {
        if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
        $process.Dispose()
    }
    $stderr = $stderrTask.GetAwaiter().GetResult()
    if ($stderr) { Write-Verbose "QEMU stderr: $stderr" }

    if (-not (Test-Path -LiteralPath $log)) { throw "QEMU produced no debug log for run '$Name'." }
    $text = ((Get-Content -LiteralPath $log -Raw) -replace "`0", '')

    [pscustomobject]@{
        Name    = $Name
        Log     = $log
        Text    = $text
        Markers = @($text -split "`r?`n" | Where-Object { $_ -match '^AW_' })
    }
} finally {
    Pop-Location
}
