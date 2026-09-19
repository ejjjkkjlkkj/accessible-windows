#requires -Version 7.0
#requires -RunAsAdministrator

[CmdletBinding()]
param(
    [string]$RunnerDir = 'C:\actions-runner-accessible-windows',
    [string]$ExpectedRunnerName = 'AMD-5800H-REAL'
)

$ErrorActionPreference = 'Stop'

function Write-Gate {
    param([string]$Name, [bool]$Pass, [string]$Detail = '')
    $state = if ($Pass) { 'PASS' } else { 'FAIL' }
    if ($Detail) {
        Write-Host ("{0} = {1} :: {2}" -f $Name, $state, $Detail)
    } else {
        Write-Host ("{0} = {1}" -f $Name, $state)
    }
}

function Get-RunnerService {
    param([string]$Root)
    Get-CimInstance Win32_Service |
        Where-Object {
            $_.PathName -match 'RunnerService\.exe' -and
            $_.PathName -like "*$Root*"
        } |
        Select-Object -First 1
}

function Get-RunnerListener {
    param([string]$Root)
    Get-CimInstance Win32_Process |
        Where-Object {
            $_.Name -eq 'Runner.Listener.exe' -and
            $_.ExecutablePath -like "$Root*"
        }
}

function Show-Diagnostics {
    param([string]$Root, [string]$ServiceName)

    Write-Host ''
    Write-Host '=== RUNNER LOG ==='
    $diag = Join-Path $Root '_diag'
    if (Test-Path $diag) {
        $latest = Get-ChildItem $diag -File -Filter 'Runner_*.log' -ErrorAction SilentlyContinue |
            Sort-Object LastWriteTime -Descending |
            Select-Object -First 1
        if ($latest) {
            Write-Host ("LOG = {0}" -f $latest.FullName)
            Get-Content $latest.FullName -Tail 160
        } else {
            Write-Host 'Runner_*.log absent.'
        }
    } else {
        Write-Host '_diag absent.'
    }

    Write-Host ''
    Write-Host '=== SERVICE CONTROL MANAGER ==='
    Get-WinEvent -FilterHashtable @{
        LogName = 'System'
        StartTime = (Get-Date).AddMinutes(-30)
    } -ErrorAction SilentlyContinue |
        Where-Object {
            $_.ProviderName -eq 'Service Control Manager' -and
            ($_.Message -like "*$ServiceName*" -or $_.Id -in 7000,7001,7009,7011,7023,7024,7031,7034)
        } |
        Select-Object -First 20 TimeCreated, Id, LevelDisplayName, Message |
        Format-List
}

Write-Host '=== AMD-5800H-REAL RUNNER RECOVERY ==='

if (-not (Test-Path $RunnerDir)) {
    $candidate = Get-ChildItem 'C:\' -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -like 'actions-runner-*accessible*windows*' } |
        Select-Object -First 1
    if (-not $candidate) {
        throw "Runner directory not found: $RunnerDir"
    }
    $RunnerDir = $candidate.FullName
}

$RunnerDir = (Resolve-Path $RunnerDir).Path
Set-Location $RunnerDir
Write-Host ("RUNNER_DIR = {0}" -f $RunnerDir)

$required = @(
    '.runner',
    'bin\RunnerService.exe',
    'bin\Runner.Listener.exe'
)

foreach ($item in $required) {
    $path = Join-Path $RunnerDir $item
    if (-not (Test-Path $path)) {
        throw "Required runner file missing: $path"
    }
}
Write-Gate 'RUNNER_FILES' $true

try {
    $cfg = Get-Content (Join-Path $RunnerDir '.runner') -Raw | ConvertFrom-Json
    $configuredName = [string]$cfg.agentName
    if ($configuredName) {
        Write-Host ("CONFIGURED_RUNNER_NAME = {0}" -f $configuredName)
        if ($configuredName -ne $ExpectedRunnerName) {
            throw "Runner identity mismatch: expected '$ExpectedRunnerName', got '$configuredName'"
        }
        Write-Gate 'RUNNER_IDENTITY' $true $configuredName
    }
} catch {
    if ($_.Exception.Message -like 'Runner identity mismatch:*') { throw }
    Write-Host ("RUNNER_CONFIG_PARSE = WARNING :: {0}" -f $_.Exception.Message)
}

$service = Get-RunnerService $RunnerDir

if (-not $service) {
    Write-Host 'SERVICE = NOT_INSTALLED; rebuilding native Windows service'

    $manual = @(Get-RunnerListener $RunnerDir)
    foreach ($proc in $manual) {
        Write-Host ("STOP_MANUAL_LISTENER_PID = {0}" -f $proc.ProcessId)
        Stop-Process -Id $proc.ProcessId -Force -ErrorAction SilentlyContinue
    }
    if ($manual.Count -gt 0) {
        Start-Sleep -Seconds 2
    }

    $runnerServiceExe = Join-Path $RunnerDir 'bin\RunnerService.exe'

    $repoOrOrg = $null
    if ($cfg.GitHubUrl) {
        try {
            $repoOrOrg = ([Uri]([string]$cfg.GitHubUrl)).AbsolutePath.Trim('/')
        } catch {
            $repoOrOrg = $null
        }
    }
    if (-not $repoOrOrg -and $cfg.ServerUrl) {
        try {
            $repoOrOrg = (([Uri]([string]$cfg.ServerUrl)).AbsolutePath.Trim('/') -split '/')[0]
        } catch {
            $repoOrOrg = $null
        }
    }
    if (-not $repoOrOrg) {
        throw 'Unable to derive repository/organization name from .runner.'
    }

    $repoOrOrg = $repoOrOrg -replace '[^0-9A-Za-z._-]', '-'
    $serviceName = "actions.runner.$repoOrOrg.$configuredName"
    $serviceDisplayName = "GitHub Actions Runner ($repoOrOrg.$configuredName)"

    if ($serviceName.Length -gt 80) {
        throw "Calculated Windows service name is too long: $serviceName"
    }

    Write-Host ("SERVICE_INSTALL_NAME = {0}" -f $serviceName)
    Write-Host ("SERVICE_INSTALL_DISPLAY = {0}" -f $serviceDisplayName)

    & $runnerServiceExe init
    if ($LASTEXITCODE -ne 0) {
        throw "RunnerService.exe init failed with exit code $LASTEXITCODE"
    }

    & icacls.exe $RunnerDir /grant '*S-1-5-20:(OI)(CI)F' /T /C | Out-Host
    if ($LASTEXITCODE -ne 0) {
        throw "icacls failed with exit code $LASTEXITCODE"
    }

    $binPathArg = 'binPath= "' + $runnerServiceExe + '"'
    $displayArg = 'DisplayName= ' + $serviceDisplayName
    & sc.exe create $serviceName $binPathArg 'start= auto' 'obj= NT AUTHORITY\NetworkService' $displayArg | Out-Host
    if ($LASTEXITCODE -ne 0) {
        throw "sc.exe create failed with exit code $LASTEXITCODE"
    }

    $serviceFile = Join-Path $RunnerDir '.service'
    [IO.File]::WriteAllText($serviceFile, $serviceName, [Text.UTF8Encoding]::new($false))
    (Get-Item $serviceFile).Attributes = (Get-Item $serviceFile).Attributes -bor [IO.FileAttributes]::Hidden

    Start-Sleep -Seconds 2
    $service = Get-RunnerService $RunnerDir
}

if (-not $service) {
    throw 'GitHub Actions Runner service still not found after install.'
}

Write-Host ("SERVICE = {0}" -f $service.Name)
Write-Host ("SERVICE_ACCOUNT = {0}" -f $service.StartName)
Write-Host ("SERVICE_STATE_BEFORE = {0}" -f $service.State)
Write-Host ("SERVICE_STARTMODE_BEFORE = {0}" -f $service.StartMode)

# A manually launched listener can prevent the service listener from owning the runner session.
if ($service.State -ne 'Running') {
    $manual = @(Get-RunnerListener $RunnerDir)
    foreach ($proc in $manual) {
        Write-Host ("STOP_MANUAL_LISTENER_PID = {0}" -f $proc.ProcessId)
        Stop-Process -Id $proc.ProcessId -Force -ErrorAction SilentlyContinue
    }
    if ($manual.Count -gt 0) {
        Start-Sleep -Seconds 2
    }
}

Set-Service -Name $service.Name -StartupType Automatic

# Restart the service automatically if the listener/service process fails.
& sc.exe failure $service.Name reset= 0 actions= restart/5000/restart/5000/restart/5000 | Out-Host
& sc.exe failureflag $service.Name 1 | Out-Host

try {
    Start-Service -Name $service.Name -ErrorAction Stop
} catch {
    Write-Host ("SERVICE_START_EXCEPTION = {0}" -f $_.Exception.Message)
}

$deadline = (Get-Date).AddSeconds(20)
do {
    Start-Sleep -Seconds 2
    $service = Get-CimInstance Win32_Service -Filter "Name='$($service.Name)'"
    $listener = @(Get-RunnerListener $RunnerDir)
    if ($service.State -eq 'Running' -and $listener.Count -gt 0) {
        break
    }
} while ((Get-Date) -lt $deadline)

$service = Get-CimInstance Win32_Service -Filter "Name='$($service.Name)'"
$listener = @(Get-RunnerListener $RunnerDir)

Write-Host ''
Write-Host '=== FINAL STATE ==='
Write-Host ("STATE = {0}" -f $service.State)
Write-Host ("START = {0}" -f $service.StartMode)
Write-Host ("EXITCODE = {0}" -f $service.ExitCode)
Write-Host ("SERVICE_EXITCODE = {0}" -f $service.ServiceSpecificExitCode)
Write-Host ("LISTENER_COUNT = {0}" -f $listener.Count)

if ($listener.Count -gt 0) {
    $listener |
        Select-Object ProcessId, Name, ExecutablePath, CommandLine |
        Format-List
}

$permanent = (
    $service.State -eq 'Running' -and
    $service.StartMode -eq 'Auto' -and
    $listener.Count -gt 0
)

Write-Host ''
if ($permanent) {
    Write-Gate 'RUNNER_AMD_5800H_REAL_PERMANENT' $true
    Write-Gate 'WINDOWS_AUTOSTART' $true
    Write-Gate 'LISTENER_SERVICE_OWNED_RUNTIME' $true
    exit 0
}

Write-Gate 'RUNNER_AMD_5800H_REAL_PERMANENT' $false ("State={0}; Start={1}; ListenerCount={2}" -f $service.State, $service.StartMode, $listener.Count)
Show-Diagnostics -Root $RunnerDir -ServiceName $service.Name
exit 1
