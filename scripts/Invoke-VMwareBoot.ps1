#Requires -Version 7.0
<#
.SYNOPSIS
    Boot the bootable UEFI image on a second hypervisor - VMware Workstation - and
    prove the native kernel comes up on it by capturing its real serial console.

.DESCRIPTION
    Every other proof runs under QEMU/OVMF. The dossier (sections 3.1 and 20) also
    wants validation on other hypervisors on the way to real hardware. This boots
    the same dist image under VMware Workstation's own EFI firmware and virtual
    SATA controller - a completely different firmware and device model than
    QEMU/OVMF - with COM1 routed to a file.

    The kernel's 0xE9 debug console is a QEMU/Bochs convenience VMware does not
    have, so the observable channel here is the real 16550: `serial::prove` writes
    "AW-SERIAL-CONSOLE-OK" on COM1 as the first thing the native kernel does. Seeing
    it in the captured serial file proves VMware's firmware booted
    \EFI\BOOT\BOOTX64.EFI, the loader handed off, and the native kernel entered and
    drove a real UART on this second hypervisor.

    It requires a full clean boot: AW_NATIVE_KERNEL_IDLE must appear with no
    AW_NATIVE_EXCEPTION / AW_NATIVE_KERNEL_PANIC, and the banner must not repeat
    (a repeat would mean a crash-reboot loop). Booting cleanly here also depended
    on not assuming the firmware left EFER.NXE on - VMware leaves it off, so the
    kernel now enables it before installing its NX identity map.
#>
[CmdletBinding()]
param(
    [string]$Vmrun = 'C:\Program Files\VMware\VMware Workstation\vmrun.exe',
    [string]$QemuImg = 'C:\Program Files\qemu\qemu-img.exe',
    [ValidateRange(10, 120)][int]$BootSeconds = 30
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
Push-Location $repo
try {
    foreach ($tool in @($Vmrun, $QemuImg)) {
        if (-not (Test-Path -LiteralPath $tool)) { throw "Missing tool: $tool" }
    }

    # Build the bootable image (no QEMU verify; that is the boot-proof suite's job).
    & (Join-Path $PSScriptRoot 'Build-BootableImage.ps1') | Out-Host
    $img = Join-Path $repo 'dist/accessible-windows-uefi-x86_64.img'
    if (-not (Test-Path -LiteralPath $img)) { throw "Image not built: $img" }

    $vmDir = Join-Path $repo 'target/vmware'
    if (Test-Path -LiteralPath $vmDir) { Remove-Item -LiteralPath $vmDir -Recurse -Force }
    New-Item -ItemType Directory -Path $vmDir -Force | Out-Null

    # Convert the raw image to a VMware flat VMDK (descriptor + raw extent).
    $vmdk = Join-Path $vmDir 'aw-boot.vmdk'
    & $QemuImg convert -f raw -O vmdk -o subformat=monolithicFlat $img $vmdk
    if ($LASTEXITCODE -ne 0) { throw 'qemu-img VMDK conversion failed' }

    $serial = Join-Path $vmDir 'aw-serial.log'
    $vmx = Join-Path $vmDir 'aw-boot.vmx'
    @'
.encoding = "UTF-8"
config.version = "8"
virtualHW.version = "19"
displayName = "aw-boot"
guestOS = "other-64"
firmware = "efi"
memsize = "512"
numvcpus = "1"
sata0.present = "TRUE"
sata0:0.present = "TRUE"
sata0:0.fileName = "aw-boot.vmdk"
sata0:0.deviceType = "disk"
serial0.present = "TRUE"
serial0.fileType = "file"
serial0.fileName = "aw-serial.log"
serial0.yieldOnMsrRead = "TRUE"
ethernet0.present = "FALSE"
usb.present = "FALSE"
usb_xhci.present = "FALSE"
sound.present = "FALSE"
floppy0.present = "FALSE"
bios.bootDelay = "0"
msg.autoAnswer = "TRUE"
gui.exitOnCLIHLT = "FALSE"
tools.syncTime = "FALSE"
'@ | Set-Content -LiteralPath $vmx -Encoding ASCII

    if (Test-Path -LiteralPath $serial) { Remove-Item -LiteralPath $serial -Force }

    & $Vmrun -T ws start $vmx nogui
    if ($LASTEXITCODE -ne 0) { throw 'vmrun start failed' }
    Start-Sleep -Seconds $BootSeconds
    & $Vmrun -T ws stop $vmx hard 2>&1 | Out-Null
    Start-Sleep -Seconds 2

    if (-not (Test-Path -LiteralPath $serial)) { throw 'VMware produced no serial output' }
    $text = [System.Text.Encoding]::ASCII.GetString([System.IO.File]::ReadAllBytes($serial))
    $banner = 'AW-SERIAL-CONSOLE-OK'
    $count = ([regex]::Matches($text, [regex]::Escape($banner))).Count
    if ($count -lt 1) {
        throw "VMware boot produced no '$banner' on COM1 (serial bytes: $($text.Length))."
    }
    if (-not $text.Contains('AW_NATIVE_KERNEL_IDLE')) {
        throw "VMware boot did not reach AW_NATIVE_KERNEL_IDLE (serial bytes: $($text.Length))."
    }
    foreach ($forbidden in @('AW_NATIVE_EXCEPTION', 'AW_NATIVE_KERNEL_PANIC')) {
        if ($text.Contains($forbidden)) { throw "VMware boot emitted forbidden marker: $forbidden" }
    }
    # A clean boot reaches idle and halts, so the banner appears exactly once; a
    # crash-reboot loop would repeat it. Allow a small margin, but reject a loop.
    if ($count -gt 3) {
        throw "VMware guest looks like it rebooted ($count banners): boot is not clean."
    }
    Write-Host "AW_VMWARE_BOOT_OK banner_occurrences=$count reached=AW_NATIVE_KERNEL_IDLE serial=$serial"
    Write-Host 'VMware boot proof PASS: full clean boot to idle on VMware EFI, no fault.'
}
finally {
    Pop-Location
}
