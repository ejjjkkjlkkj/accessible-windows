#Requires -Version 7.0
<#
.SYNOPSIS
    Regenerate the boot screen's speech clips for the firmware-stage screen reader.

.DESCRIPTION
    The UEFI screen reader speaks its fixed lines through the HDA codec by playing
    pre-recorded PCM clips (boot/uefi/src/speech/*.pcm), embedded in the .efi with
    include_bytes!. This script synthesizes them with the Windows Speech API to
    24 kHz 16-bit mono raw PCM (the format boot/uefi/src/hda.rs plays).

    Run it to change the wording or the voice. Pick a voice whose language matches
    the phrases with -Voice (see the installed voices it prints); the default is
    the system voice, which may not be English. The .pcm files are committed, so
    CI and the build use them as-is and never re-synthesize.
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

# Filename -> spoken text. Must match the utterances in boot/uefi/src/screen_reader.rs
# and the clip wiring in run().
$phrases = [ordered]@{
    'welcome'  = 'Accessible Windows, window'
    'active'   = 'Screen reader active at firmware stage'
    'starting' = 'Starting Accessible Windows'
    'loading'  = 'Loading the operating system'
}

$fmt = New-Object System.Speech.AudioFormat.SpeechAudioFormatInfo(
    24000,
    [System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen,
    [System.Speech.AudioFormat.AudioChannel]::Mono)

$synth = New-Object System.Speech.Synthesis.SpeechSynthesizer
Write-Host 'Installed voices:'
$synth.GetInstalledVoices() | ForEach-Object { '  ' + $_.VoiceInfo.Name } | Write-Host
if ($Voice) { $synth.SelectVoice($Voice) }
Write-Host "Using voice: $($synth.Voice.Name)`n"

foreach ($name in $phrases.Keys) {
    $wav = Join-Path $env:TEMP "aw_speech_$name.wav"
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
    Write-Host ("  {0}.pcm: {1} bytes ({2:N2}s)" -f $name, $pcm.Length, ($pcm.Length / 48000.0))
}

$synth.Dispose()
Write-Host "`nWrote clips to $outDir"
