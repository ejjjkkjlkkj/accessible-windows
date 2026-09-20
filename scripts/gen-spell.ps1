#Requires -Version 7.0
<#
.SYNOPSIS
    Regenerate the spelling alphabet the accessible firmware setup uses to read
    dynamic text (boot-device names, machine-state values) character by character.

.DESCRIPTION
    The setup's fixed lines carry pre-recorded clips, but dynamic lines - the
    enumerated Boot#### device names, and the Main/Advanced/Security values - are
    composed at runtime and cannot be pre-recorded whole. A screen reader's answer is
    "read by character": this synthesizes one clip per letter (a-z), digit (0-9) and
    space, so boot/uefi/src/setup.rs can spell any dynamic line aloud on demand (the
    "S" key) through the same HDA codec and the same voice as every other clip. The
    .pcm files are committed, so CI and the build use them as-is.
#>
[CmdletBinding()]
param(
    [string]$Voice = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName System.Speech

$outDir = Join-Path (Split-Path $PSScriptRoot -Parent) 'boot/uefi/src/speech'
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

# Clip name -> spoken text. The setup maps a character to the clip: letters to the
# letter's name, digits to the digit's name, and a space to the word "space".
$phrases = [ordered]@{}
foreach ($letter in [char[]]([int][char]'a'..[int][char]'z')) {
    $phrases["spell_$letter"] = [string]$letter
}
foreach ($digit in 0..9) {
    $phrases["spell_$digit"] = [string]$digit
}
$phrases['spell_space'] = 'space'

$fmt = New-Object System.Speech.AudioFormat.SpeechAudioFormatInfo(
    24000,
    [System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen,
    [System.Speech.AudioFormat.AudioChannel]::Mono)

$synth = New-Object System.Speech.Synthesis.SpeechSynthesizer
if ($Voice) { $synth.SelectVoice($Voice) }
Write-Host "Using voice: $($synth.Voice.Name)`n"

foreach ($name in $phrases.Keys) {
    $wav = Join-Path $env:TEMP "aw_$name.wav"
    $synth.SetOutputToWaveFile($wav, $fmt)
    $synth.Speak($phrases[$name])
    $synth.SetOutputToNull()

    # Extract the raw PCM 'data' chunk from the WAV container.
    $bytes = [System.IO.File]::ReadAllBytes($wav)
    $pos = 12  # skip 'RIFF' <size> 'WAVE'
    $pcm = $null
    while ($pos -lt $bytes.Length - 8) {
        $id = [System.Text.Encoding]::ASCII.GetString($bytes, $pos, 4)
        $size = [BitConverter]::ToInt32($bytes, $pos + 4)
        if ($id -eq 'data') {
            $pcm = New-Object byte[] $size
            [Array]::Copy($bytes, $pos + 8, $pcm, 0, $size)
            break
        }
        $pos += 8 + $size + ($size % 2)
    }
    if (-not $pcm) { throw "no data chunk in $wav" }

    $pcmPath = Join-Path $outDir "$name.pcm"
    [System.IO.File]::WriteAllBytes($pcmPath, $pcm)
    Remove-Item -LiteralPath $wav -Force
    Write-Host ("  {0}.pcm: {1} bytes" -f $name, $pcm.Length)
}

$synth.Dispose()
Write-Host "`nWrote spelling clips to $outDir"
