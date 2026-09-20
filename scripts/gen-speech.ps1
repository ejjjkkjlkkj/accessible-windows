#Requires -Version 7.0
<#
.SYNOPSIS
    Generate all firmware-stage speech clips used by Accessible Windows.

.DESCRIPTION
    The UEFI reader plays pre-recorded 24 kHz, 16-bit, mono PCM through the HDA
    driver. Fixed boot/setup labels are synthesized as full French phrases so the
    target ASUS host's French Windows voice remains intelligible. Dynamic values
    are made nonvisual with a spelling bank (letters, digits and punctuation).

    The four boot clips are regenerated together with the Setup clips so a
    physical image uses one voice from the first spoken line to the last menu.
    CI that cannot run Windows Speech uses an explicit non-speech build fallback;
    the AMD5800H physical workflow requires this script to succeed before build.
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

# Filename -> spoken text. Debug/semantic markers remain stable English strings;
# these clips are their French spoken equivalents for the physical nonvisual UI.
$phrases = [ordered]@{
    'welcome'                         = 'Accessible Windows'
    'active'                          = "Lecteur d'écran actif au démarrage"
    'starting'                        = "Démarrage d'Accessible Windows"
    'loading'                         = "Chargement du système d'exploitation"
    'setup_setup'                     = 'Configuration U E F I accessible'
    'setup_main'                      = 'Principal'
    'setup_advanced'                  = 'Avancé'
    'setup_boot'                      = 'Démarrage'
    'setup_security'                  = 'Sécurité'
    'setup_save_exit'                 = 'Enregistrer et quitter'
    'setup_system_information'        = 'Informations système'
    'setup_cpu_model'                 = 'Modèle du processeur'
    'setup_firmware_time'             = 'Heure du micrologiciel'
    'setup_firmware_vendor'           = 'Fabricant du micrologiciel'
    'setup_firmware_revision'         = 'Révision du micrologiciel'
    'setup_uefi_revision'             = 'Révision U E F I'
    'setup_display_information'       = "Informations d'affichage"
    'setup_display_resolution'        = "Résolution d'affichage"
    'setup_boot_current'              = 'Option de démarrage actuelle'
    'setup_cpu_configuration'         = 'Configuration du processeur'
    'setup_architecture'              = 'Architecture'
    'setup_virtualization_capability' = 'Capacité de virtualisation'
    'setup_boot_option_priorities'    = 'Priorités de démarrage'
    'setup_boot_now'                  = 'Démarrer maintenant'
    'setup_set_as_default'            = 'Définir par défaut'
    'setup_secure_boot'               = 'Démarrage sécurisé'
    'setup_secure_boot_status'        = 'État du démarrage sécurisé'
    'setup_setup_mode'                = 'Mode de configuration'
    'setup_boot_normally'             = 'Démarrer normalement'
    'setup_reset_system'              = 'Redémarrer le système'
    'setup_shut_down'                 = 'Éteindre le système'
    'setup_back'                      = 'Retour'
    'setup_enabled'                   = 'Activé'
    'setup_disabled'                  = 'Désactivé'
    'setup_unavailable'               = 'Indisponible'
    'setup_supported'                 = 'Pris en charge'
    'setup_not_supported'             = 'Non pris en charge'
    'setup_selected'                  = 'Sélectionné'
    'setup_boot_option'               = 'Option de démarrage'
    'setup_action_succeeded'          = 'Action réussie'
    'setup_action_failed'             = "Échec de l'action"
    'setup_value'                     = 'Valeur'
    'setup_instructions_navigation'   = 'Flèches haut et bas ou Tabulation pour naviguer'
    'setup_instructions_select'       = 'Entrée pour sélectionner. Échap pour revenir ou démarrer'
    'setup_instructions_timeout'      = 'Sans action, démarrage normal dans trois secondes'
    'setup_confirm_restart'           = 'Confirmer le redémarrage'
    'setup_confirm_shutdown'          = "Confirmer l'arrêt"
    'setup_cancel'                    = 'Annuler'
}

foreach ($character in [char[]]'ABCDEFGHIJKLMNOPQRSTUVWXYZ') {
    $phrases["spell_$($character.ToString().ToLowerInvariant())"] = $character.ToString()
}
foreach ($digit in [char[]]'0123456789') {
    $phrases["spell_$digit"] = $digit.ToString()
}
$phrases['spell_dash'] = 'tiret'
$phrases['spell_dot'] = 'point'
$phrases['spell_colon'] = 'deux points'
$phrases['spell_slash'] = 'barre oblique'
$phrases['spell_underscore'] = 'tiret bas'

$fmt = New-Object System.Speech.AudioFormat.SpeechAudioFormatInfo(
    24000,
    [System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen,
    [System.Speech.AudioFormat.AudioChannel]::Mono)

$synth = New-Object System.Speech.Synthesis.SpeechSynthesizer
Write-Host 'Installed voices:'
$synth.GetInstalledVoices() | ForEach-Object { '  ' + $_.VoiceInfo.Name } | Write-Host
if ($Voice) { $synth.SelectVoice($Voice) }
$synth.Rate = -1
$synth.Volume = 100
$voiceName = $synth.Voice.Name
Write-Host "Using voice: $voiceName"
Write-Host "Rate: $($synth.Rate)  Volume: $($synth.Volume)`n"

$maxMonoBytes = 196608 # boot/uefi/src/hda.rs: AUDIO_BYTES / 2
$manifest = [System.Collections.Generic.List[string]]::new()

foreach ($name in $phrases.Keys) {
    $wav = Join-Path $env:TEMP "aw_speech_$name.wav"
    $synth.SetOutputToWaveFile($wav, $fmt)
    $synth.Speak([string]$phrases[$name])
    $synth.SetOutputToNull()

    # Extract the raw PCM 'data' chunk from the WAV container.
    $bytes = [System.IO.File]::ReadAllBytes($wav)
    $pos = 12 # skip 'RIFF' <size> 'WAVE'
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
    Remove-Item -LiteralPath $wav -Force

    if (-not $pcm) { throw "No PCM data chunk in $name" }
    if ($pcm.Length -eq 0) { throw "Empty PCM clip: $name" }
    if ($pcm.Length -gt $maxMonoBytes) {
        throw "PCM clip $name is too long: $($pcm.Length) > $maxMonoBytes bytes"
    }

    $pcmPath = Join-Path $outDir "$name.pcm"
    [System.IO.File]::WriteAllBytes($pcmPath, $pcm)
    $sha = (Get-FileHash -Algorithm SHA256 -LiteralPath $pcmPath).Hash.ToLowerInvariant()
    $seconds = $pcm.Length / 48000.0
    $manifest.Add("$name|$($pcm.Length)|$('{0:N2}' -f $seconds)|$sha")
    Write-Host ("  {0}.pcm: {1} bytes ({2:N2}s)" -f $name, $pcm.Length, $seconds)
}

$synth.Dispose()
$manifestPath = Join-Path $outDir 'speech-manifest.txt'
@(
    "voice=$voiceName"
    'format=pcm_s16le_24000_mono'
    "clip-count=$($phrases.Count)"
    $manifest
) | Set-Content -LiteralPath $manifestPath -Encoding utf8

Write-Host "`nWrote $($phrases.Count) clips to $outDir"
Write-Host "Manifest: $manifestPath"
