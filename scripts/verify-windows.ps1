#Requires -Version 7.0
[CmdletBinding()]
param(
    [string]$Qemu = 'C:\Program Files\qemu\qemu-system-x86_64.exe',
    [ValidateRange(5, 120)][int]$BootTimeoutSeconds = 30
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
function Invoke-Checked {
    param([string]$Command, [string[]]$Arguments)
    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Command failed with exit code $LASTEXITCODE" }
}
$repo = Split-Path $PSScriptRoot -Parent
Push-Location $repo
try {
    Invoke-Checked cargo @('fmt', '--all', '--', '--check')
    Invoke-Checked cargo @('check', '--locked', '--workspace', '--all-targets')
    Invoke-Checked cargo @('test', '--locked', '--workspace')
    Invoke-Checked cargo @('clippy', '--locked', '--workspace', '--all-targets', '--', '-D', 'warnings')
    Invoke-Checked cargo @('build', '--locked', '--manifest-path', 'boot/uefi/Cargo.toml', '--target', 'x86_64-unknown-uefi', '--release')
    Invoke-Checked cargo @('build', '--locked', '--manifest-path', 'kernel/x86_64/Cargo.toml', '--target', 'x86_64-unknown-none', '--release')
    $sysroot = & rustc --print sysroot
    if ($LASTEXITCODE -ne 0) { throw 'Cannot resolve Rust sysroot' }
    $objcopy = @(Get-ChildItem -LiteralPath (Join-Path $sysroot 'lib/rustlib') -Recurse -Filter llvm-objcopy.exe)
    if ($objcopy.Count -ne 1) { throw 'Install llvm-tools-preview with rustup component add llvm-tools-preview' }
    $run = Join-Path $repo ('target/windows-verify/' + [guid]::NewGuid().ToString('N'))
    $esp = Join-Path $run 'esp'
    New-Item -ItemType Directory -Path (Join-Path $esp 'EFI/BOOT') -Force | Out-Null
    Copy-Item -LiteralPath 'boot/uefi/target/x86_64-unknown-uefi/release/aw-uefi-boot.efi' -Destination (Join-Path $esp 'EFI/BOOT/BOOTX64.EFI')
    Invoke-Checked $objcopy[0].FullName @('-O', 'binary', 'kernel/x86_64/target/x86_64-unknown-none/release/aw-kernel-x86_64', (Join-Path $esp 'KERNEL.BIN'))
    $share = Join-Path (Split-Path $Qemu -Parent) 'share'
    $code = Join-Path $share 'edk2-x86_64-code.fd'
    $vars = Join-Path $run 'vars.fd'
    foreach ($required in @($Qemu, $code, (Join-Path $share 'edk2-i386-vars.fd'))) {
        if (-not (Test-Path -LiteralPath $required -PathType Leaf)) { throw "Missing dependency: $required" }
    }
    Copy-Item -LiteralPath (Join-Path $share 'edk2-i386-vars.fd') -Destination $vars
    $log = Join-Path $run 'debug.log'
    # A fresh virtual FAT drive with a disposable snapshot exercises the boot files.
    $start = [System.Diagnostics.ProcessStartInfo]::new($Qemu)
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardError = $true
    foreach ($argument in @('-machine','q35','-accel','tcg','-cpu','max','-m','256M','-display','none','-serial','none','-monitor','none','-no-reboot','-net','none','-debugcon',"file:$log",'-drive',"if=pflash,format=raw,readonly=on,file=$code",'-drive',"if=pflash,format=raw,file=$vars",'-drive',"format=raw,snapshot=on,file=fat:ro:$esp")) {
        $start.ArgumentList.Add($argument)
    }
    $process = [System.Diagnostics.Process]::Start($start)
    $stderr = $process.StandardError.ReadToEndAsync()
    try {
        if (-not $process.WaitForExit($BootTimeoutSeconds * 1000)) {
            $process.Kill()
            $process.WaitForExit()
        } elseif ($process.ExitCode -ne 0) { throw "QEMU failed: $($process.ExitCode): $($stderr.GetAwaiter().GetResult())" }
    } finally {
        if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
        $process.Dispose()
    }
    $evidence = Get-Content -LiteralPath $log -Raw
    $markers = @('AW_BOOT_OK stage=uefi_init arch=x86_64','AW_KERNEL_FILE_READ_OK','AW_NATIVE_KERNEL_LOAD_OK','mode=dynamic_pic','AW_MEMORY_MAP_OK','AW_ACPI_OK','AW_ACPI_VALIDATE_OK','AW_GOP_OK','AW_FRAMEBUFFER_OK','AW_EXIT_BOOT_SERVICES_OK','AW_MEMORY_MAP_HANDOFF_OK','AW_KERNEL_HANDOFF_OK','AW_NATIVE_KERNEL_TRANSFER','AW_NATIVE_KERNEL_ENTRY_OK','AW_MEMORY_MAP_VALIDATE_OK','AW_MEMORY_MAP_CONVENTIONAL_OK','AW_BOOTSTRAP_PAGE_ALLOC_OK','AW_CPU_NX_OK','AW_CPU_ADDRESS_WIDTH_OK physical=','AW_PAGING_BASELINE_OK','AW_NATIVE_FRAMEBUFFER_WRITE_OK','AW_NATIVE_KERNEL_IDLE')
    $missing = @($markers | Where-Object { -not $evidence.Contains($_) })
    if ($missing.Count -gt 0) { throw "Missing boot evidence: $($missing -join ', '). Log: $log" }
    Write-Host "PASS: workspace checks, UEFI/kernel build and QEMU virtual-FAT boot. Log: $log"
    Write-Host 'GPT disk-image boot and physical hardware are separate validations.'
} finally { Pop-Location }
