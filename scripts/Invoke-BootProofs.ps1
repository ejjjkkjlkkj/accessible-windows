#Requires -Version 7.0
<#
.SYNOPSIS
    Bare-metal anti-regression suite: boots every kernel configuration under
    QEMU/OVMF and asserts the markers each one must produce.

.DESCRIPTION
    Section 1.1 of the project dossier: a component is never PASS because it
    compiled. Every claim below is tied to a marker the kernel only emits after
    the CPU actually did the thing - delivered an interrupt, refused a write,
    refused an instruction fetch, entered #DF on its IST stack.

    Forbidden markers matter as much as required ones: a boot that reaches
    AW_NATIVE_KERNEL_IDLE while also printing AW_MEMORY_PROTECTION_FAIL is a
    failure, not a pass.
#>
[CmdletBinding()]
param(
    [string]$Qemu = 'C:\Program Files\qemu\qemu-system-x86_64.exe',
    [ValidateRange(5, 300)][int]$TimeoutSeconds = 90
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$boot = Join-Path $PSScriptRoot 'Invoke-KernelBoot.ps1'

$configurations = @(
    @{
        Name     = 'normal'
        Features = @()
        Required = @(
            # UEFI loader stage.
            'AW_BOOT_OK stage=uefi_init arch=x86_64'
            'AW_KERNEL_FILE_READ_OK'
            'AW_KERNEL_IMAGE_HEADER_OK base=0x200000'
            'AW_NATIVE_KERNEL_LOAD_OK address=0x200000'
            'mode=fixed_base'
            'AW_MEMORY_MAP_OK'
            'AW_ACPI_OK'
            'AW_ACPI_VALIDATE_OK'
            'AW_GOP_OK'
            'AW_FRAMEBUFFER_OK'
            'AW_EXIT_BOOT_SERVICES_OK'
            'AW_MEMORY_MAP_HANDOFF_OK'
            'AW_KERNEL_HANDOFF_OK'
            'AW_NATIVE_KERNEL_TRANSFER address=0x200000 entry=0x201000'
            # Fixed-base image load and segment reload.
            'AW_GDT_IDT_BEGIN'
            'AW_GDT_LOADED'
            'AW_GDT_SEGMENTS_RELOADED cs=0x0000000000000008 ss=0x0000000000000010'
            'AW_IDT_LOADED'
            'AW_TSS_IST_READY vector=8 ist=1'
            'AW_IDT_VECTOR_COUNT 32/256'
            'AW_NATIVE_KERNEL_ENTRY_OK'
            # Kernel-owned W^X page tables.
            'AW_VMM_IDENTITY_MAP_OK'
            'AW_VMM_ACTIVE'
            # CPU protection bits actually latched.
            'AW_SECURITY_ENFORCED wp=1 nx=1'
            'AW_SECURITY_BASELINE_OK'
            # Protections proved by real faults, with the right error codes.
            'AW_MEMORY_PROTECTION_OK name=nx-execute-rodata error_code=0x0000000000000011'
            'AW_MEMORY_PROTECTION_OK name=nx-execute-data error_code=0x0000000000000011'
            'AW_MEMORY_PROTECTION_OK name=wx-write-text error_code=0x0000000000000003'
            'AW_MEMORY_PROTECTION_OK name=guard-page error_code=0x0000000000000000'
            'AW_MEMORY_PROTECTION_PROOF_OK'
            # Real APIC timer delivery, monotonic, and stopped by masking.
            'AW_APIC_TIMER_ARMED mode=periodic'
            'AW_APIC_TIMER_FIRED'
            'AW_APIC_TIMER_MONOTONIC_OK'
            'AW_APIC_TIMER_MASKED_STOPPED'
            'AW_APIC_TIMER_UNMASKED_RESUMED'
            'AW_APIC_TIMER_DELIVERY_PROOF_OK'
            # Rest of bring-up still clean.
            'AW_MEMORY_MAP_VALIDATE_OK'
            'AW_BOOTSTRAP_PAGE_ALLOC_OK'
            'AW_KERNEL_RANGE_PROTECTED_OK'
            'AW_CPU_NX_OK'
            'AW_PAGING_BASELINE_OK'
            'AW_NATIVE_FRAMEBUFFER_WRITE_OK'
            'AW_NATIVE_KERNEL_IDLE'
        )
        Forbidden = @(
            'AW_NATIVE_EXCEPTION'
            'AW_NATIVE_KERNEL_PANIC'
            'AW_MEMORY_PROTECTION_FAIL'
            'AW_MEMORY_PROTECTION_SKIPPED'
            'AW_VMM_FAIL'
            'AW_GDT_SEGMENTS_FAIL'
            'AW_APIC_TIMER_NOT_FIRED'
            'AW_APIC_TIMER_MASK_INEFFECTIVE'
            'AW_APIC_TIMER_DID_NOT_RESUME'
            'AW_SECURITY_BASELINE_GAP'
        )
    }
    @{
        Name     = 'exception-smoke'
        Features = @('exception-smoke-test')
        Required = @(
            'AW_EXCEPTION_SMOKE_TRIGGER vector=6'
            'AW_NATIVE_EXCEPTION vector=6 name=invalid-opcode'
            'AW_INVALID_OPCODE_HANDLER_OK'
        )
        Forbidden = @('AW_NATIVE_KERNEL_PANIC')
    }
    @{
        Name     = 'double-fault-smoke'
        Features = @('exception-smoke-test', 'double-fault-smoke-test')
        Required = @(
            'AW_DOUBLE_FAULT_SMOKE_ARMED'
            'AW_NATIVE_EXCEPTION vector=8 name=double-fault'
            'AW_DOUBLE_FAULT_IST_OK'
        )
        Forbidden = @('AW_DOUBLE_FAULT_IST_FAIL', 'AW_NATIVE_KERNEL_PANIC')
    }
)

$failures = @()

foreach ($configuration in $configurations) {
    Write-Host "== $($configuration.Name) =="
    $result = & $boot -Name $configuration.Name -Features $configuration.Features `
        -Qemu $Qemu -TimeoutSeconds $TimeoutSeconds

    $missing = @($configuration.Required | Where-Object { -not $result.Text.Contains($_) })
    $present = @($configuration.Forbidden | Where-Object { $result.Text.Contains($_) })

    foreach ($marker in $missing) {
        $failures += "$($configuration.Name): missing '$marker'"
    }
    foreach ($marker in $present) {
        $failures += "$($configuration.Name): forbidden '$marker' present"
    }

    if ($missing.Count -eq 0 -and $present.Count -eq 0) {
        Write-Host "   PASS  ($($configuration.Required.Count) markers)  log: $($result.Log)"
    } else {
        Write-Host "   FAIL  log: $($result.Log)"
    }
}

if ($failures.Count -gt 0) {
    throw "Boot proof failures:`n  " + ($failures -join "`n  ")
}

Write-Host ''
Write-Host 'ALL BOOT PROOFS PASS (QEMU/OVMF q35, tcg, -cpu max).'
Write-Host 'Physical hardware and GPT disk-image boot remain separate validations.'
