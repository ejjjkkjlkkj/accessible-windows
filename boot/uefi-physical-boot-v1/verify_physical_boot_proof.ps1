param(
  [string]$ProofPath
)
$ErrorActionPreference='Stop'

if(-not $ProofPath){
  $candidates=@()
  foreach($drive in [IO.DriveInfo]::GetDrives()){
    if(-not $drive.IsReady){ continue }
    $p=Join-Path $drive.RootDirectory.FullName 'QEVARYNOX-PHYSICAL-PROOF.TXT'
    if(Test-Path $p){ $candidates += $p }
  }
  if($candidates.Count -ne 1){
    throw "Expected exactly one QEVARYNOX-PHYSICAL-PROOF.TXT on mounted media; found $($candidates.Count)"
  }
  $ProofPath=$candidates[0]
}

$ProofPath=(Resolve-Path $ProofPath).Path
$raw=Get-Content $ProofPath -Raw
$required=@(
  'QEVARYNOX-UEFI-PHYSICAL-BOOT-PROOF-V1',
  'STATUS=PASS',
  'HII_PROMPT_SOURCE=PASS',
  'HDA_CONTROLLER_SELECTION=PREFERRED_AMD_1022_15E3',
  'HDA_GRAPH_SEARCH_LIVE=PASS',
  'HDA_SELECTOR_APPLY_LIVE=PASS',
  'HDA_OUTPUT_PATH_CONFIGURATION=PASS',
  'HII_GRAPH_SPEECH_DMA=PASS',
  'LPIB_PROGRESS=PASS',
  'AUDIBLE_PHYSICAL_SPEAKER=REQUIRES_HUMAN_CONFIRMATION'
)
foreach($token in $required){
  if($raw -notmatch [regex]::Escape($token)){
    throw "Physical proof missing token: $token"
  }
}
if($raw -match 'HDA_CONTROLLER_SELECTION=GENERIC_CLASS_0403'){
  throw 'Physical ASUS proof used generic/possibly HDMI controller instead of AMD 1022:15E3'
}
$pin=[regex]::Match($raw,'(?m)^HDA_PIN_NID=(0x[0-9A-F]{2})$').Groups[1].Value
$dac=[regex]::Match($raw,'(?m)^HDA_DAC_NID=(0x[0-9A-F]{2})$').Groups[1].Value
$depth=[regex]::Match($raw,'(?m)^HDA_ROUTE_DEPTH=(0x[0-9A-F]{2})$').Groups[1].Value
if(-not $pin -or -not $dac -or -not $depth){ throw 'Physical HDA route fields missing' }

[pscustomobject]@{
  Result='PASS'
  ProofPath=$ProofPath
  Controller='PCI 1022:15E3'
  Codec='Realtek 10EC:0256 target'
  PinNid=$pin
  DacNid=$dac
  RouteDepth=$depth
  NativeUefiHdaExecution='PASS'
  AudiblePhysicalSpeaker='REQUIRES_HUMAN_CONFIRMATION'
} | ConvertTo-Json -Depth 4

'PHYSICAL_UEFI_HDA_EXECUTION=PASS'
'PHYSICAL_UEFI_SPEAKER_AUDIBLE=REQUIRES_HUMAN_CONFIRMATION'
