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
$text=$raw -replace "`r",""

$required=@(
  'QEVARYNOX-UEFI-PHYSICAL-BOOT-PROOF-V1',
  'STATUS=PASS',
  'HII_PROMPT_SOURCE=PASS',
  'HDA_CONTROLLER_SELECTION=PREFERRED_AMD_1022_15E3',
  'HDA_CODEC_VENDOR_DEVICE=0x10EC0256',
  'HDA_CODEC_SELECTION=REALTEK_10EC_0256',
  'HDA_GRAPH_SEARCH_LIVE=PASS',
  'HDA_SELECTOR_APPLY_LIVE=PASS',
  'HDA_OUTPUT_PATH_CONFIGURATION=PASS',
  'HII_GRAPH_SPEECH_DMA=PASS',
  'LPIB_PROGRESS=PASS',
  'HII_GRAPH_NAV_UP=PASS',
  'HII_GRAPH_NAV_DOWN=PASS',
  'HII_GRAPH_NAV_HOME=PASS',
  'HII_GRAPH_NAV_END=PASS',
  'HII_GRAPH_NAV_PAGE_UP=PASS',
  'HII_GRAPH_NAV_PAGE_DOWN=PASS',
  'HII_GRAPH_NAV_REPEAT=PASS',
  'HII_GRAPH_NAV_EXIT=PASS',
  'HII_GRAPH_SPEECH_DMA_REUSE=PASS',
  'AUDIBLE_PHYSICAL_SPEAKER=REQUIRES_HUMAN_CONFIRMATION'
)
foreach($token in $required){
  if($text -notmatch [regex]::Escape($token)){
    throw "Physical proof missing token: $token"
  }
}

if($text -match 'HDA_CONTROLLER_SELECTION=GENERIC_CLASS_0403'){
  throw 'Physical ASUS proof used generic/possibly HDMI controller instead of AMD 1022:15E3'
}
if($text -match 'HDA_CODEC_SELECTION=GENERIC_RUNTIME'){
  throw 'Physical ASUS proof did not bind the native codec to Realtek 10EC:0256'
}

function Get-ProofField([string]$Name,[string]$Pattern){
  $m=[regex]::Match($text,"(?m)^$([regex]::Escape($Name))=($Pattern)$")
  if(-not $m.Success){ throw "Physical proof field missing or invalid: $Name" }
  return $m.Groups[1].Value
}

$pin=Get-ProofField 'HDA_PIN_NID' '0x[0-9A-F]{2}'
$dac=Get-ProofField 'HDA_DAC_NID' '0x[0-9A-F]{2}'
$depth=Get-ProofField 'HDA_ROUTE_DEPTH' '0x[0-9A-F]{2}'
$navEvents=Get-ProofField 'HII_GRAPH_NAV_SPEECH_EVENTS' '0x[0-9A-F]{2}'
$navEventCount=[Convert]::ToInt32($navEvents.Substring(2),16)
if($navEventCount -lt 7){
  throw "Physical HII navigation produced only $navEventCount speech events; expected at least 7"
}

[pscustomobject]@{
  Result='PASS'
  ProofPath=$ProofPath
  Controller='PCI 1022:15E3'
  Codec='Realtek 10EC:0256 verified by native HDA verb'
  PinNid=$pin
  DacNid=$dac
  RouteDepth=$depth
  NativeUefiHdaExecution='PASS'
  HiiNavigation='UP_DOWN_HOME_END_PAGEUP_PAGEDOWN_R_ESC_PASS'
  NavigationSpeechEvents=$navEventCount
  DmaReuse='PASS'
  AudiblePhysicalSpeaker='REQUIRES_HUMAN_CONFIRMATION'
} | ConvertTo-Json -Depth 4

'PHYSICAL_UEFI_HDA_EXECUTION=PASS'
'PHYSICAL_UEFI_HII_NAVIGATION=PASS'
'PHYSICAL_UEFI_DMA_REUSE=PASS'
'PHYSICAL_UEFI_SPEAKER_AUDIBLE=REQUIRES_HUMAN_CONFIRMATION'
