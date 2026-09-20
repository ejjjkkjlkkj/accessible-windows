#Requires -Version 7.0
<#
.SYNOPSIS
    Generate all firmware speech assets used before the operating system starts.

.DESCRIPTION
    Produces raw 24 kHz, 16-bit, mono PCM for the boot screen and the complete
    accessible UEFI Setup. Fixed controls are recorded as whole phrases. A compact
    spelling bank (a-z, 0-9 and common symbols) makes runtime values such as
    firmware strings and boot identifiers audible without a firmware TTS engine.

    The text follows the selected Windows voice culture. When the only installed
    voice is French, French phrases are synthesized instead of forcing a French
    voice to pronounce English UI text.
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

$fmt = New-Object System.Speech.AudioFormat.SpeechAudioFormatInfo(
    24000,
    [System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen,
    [System.Speech.AudioFormat.AudioChannel]::Mono)

$synth = New-Object System.Speech.Synthesis.SpeechSynthesizer
Write-Host 'Installed voices:'
$synth.GetInstalledVoices() | ForEach-Object {
    "  $($_.VoiceInfo.Name) [$($_.VoiceInfo.Culture.Name)]"
} | Write-Host
if ($Voice) { $synth.SelectVoice($Voice) }

$culture = $synth.Voice.Culture.TwoLetterISOLanguageName
Write-Host "Using voice: $($synth.Voice.Name) [$($synth.Voice.Culture.Name)]"

$en = [ordered]@{
    'welcome'                           = 'Accessible Windows, window'
    'active'                            = 'Screen reader active at firmware stage'
    'starting'                          = 'Starting Accessible Windows'
    'loading'                           = 'Loading the operating system'
    'setup_setup'                       = 'Accessible Windows firmware setup'
    'setup_main'                        = 'Main'
    'setup_advanced'                    = 'Advanced'
    'setup_boot'                        = 'Boot'
    'setup_security'                    = 'Security'
    'setup_save_exit'                   = 'Save and Exit'
    'setup_system_information'          = 'System information'
    'setup_cpu_model'                   = 'CPU model'
    'setup_firmware_time'               = 'Firmware time'
    'setup_firmware_vendor'             = 'Firmware vendor'
    'setup_firmware_revision'           = 'Firmware revision'
    'setup_uefi_revision'               = 'UEFI specification revision'
    'setup_display_information'         = 'Display information'
    'setup_display_resolution'          = 'Display resolution'
    'setup_boot_current'                = 'Current boot option'
    'setup_cpu_configuration'           = 'CPU configuration'
    'setup_architecture'                = 'Architecture x86 64'
    'setup_virtualization_capability'   = 'Virtualization capability'
    'setup_boot_option_priorities'      = 'Boot option priorities'
    'setup_boot_now'                    = 'Boot now'
    'setup_set_as_default'              = 'Set as default'
    'setup_secure_boot'                 = 'Secure Boot'
    'setup_secure_boot_status'          = 'Secure Boot status'
    'setup_setup_mode'                  = 'Setup mode'
    'setup_boot_normally'               = 'Boot normally'
    'setup_reset_system'                = 'Reset system'
    'setup_shut_down'                   = 'Shut down'
    'setup_back'                        = 'Back'
    'setup_enabled'                     = 'Enabled'
    'setup_disabled'                    = 'Disabled'
    'setup_unavailable'                 = 'Unavailable'
    'setup_supported'                   = 'Supported'
    'setup_not_supported'               = 'Not supported'
    'setup_selected'                    = 'Selected'
    'setup_boot_option'                 = 'Boot option'
    'setup_action_succeeded'            = 'Action succeeded'
    'setup_action_failed'               = 'Action failed'
    'setup_value'                       = 'Value'
    'setup_instructions_navigation'     = 'Use up and down arrows or Tab to move'
    'setup_instructions_select'         = 'Press Enter to open or activate, Escape to go back'
    'setup_instructions_timeout'        = 'Automatic boot stops as soon as you press a key'
    'setup_confirm_restart'             = 'Confirm restart'
    'setup_confirm_shutdown'            = 'Confirm shut down'
    'setup_cancel'                      = 'Cancel'
}

$fr = [ordered]@{
    'welcome'                           = 'Accessible Windows, fenêtre'
    'active'                            = "Lecteur d'écran actif au démarrage UEFI"
    'starting'                          = "Démarrage d'Accessible Windows"
    'loading'                           = "Chargement du système d'exploitation"
    'setup_setup'                       = 'Configuration UEFI Accessible Windows'
    'setup_main'                        = 'Principal'
    'setup_advanced'                    = 'Avancé'
    'setup_boot'                        = 'Démarrage'
    'setup_security'                    = 'Sécurité'
    'setup_save_exit'                   = 'Enregistrer et quitter'
    'setup_system_information'          = 'Informations système'
    'setup_cpu_model'                   = 'Modèle du processeur'
    'setup_firmware_time'               = 'Heure du micrologiciel'
    'setup_firmware_vendor'             = 'Fabricant du micrologiciel'
    'setup_firmware_revision'           = 'Révision du micrologiciel'
    'setup_uefi_revision'               = 'Révision de la spécification UEFI'
    'setup_display_information'         = "Informations d'affichage"
    'setup_display_resolution'          = "Résolution d'affichage"
    'setup_boot_current'                = 'Option de démarrage actuelle'
    'setup_cpu_configuration'           = 'Configuration du processeur'
    'setup_architecture'                = 'Architecture x86 64'
    'setup_virtualization_capability'   = 'Capacité de virtualisation'
    'setup_boot_option_priorities'      = 'Priorité des options de démarrage'
    'setup_boot_now'                    = 'Démarrer maintenant'
    'setup_set_as_default'              = 'Définir par défaut'
    'setup_secure_boot'                 = 'Démarrage sécurisé'
    'setup_secure_boot_status'          = 'État du démarrage sécurisé'
    'setup_setup_mode'                  = 'Mode de configuration'
    'setup_boot_normally'               = 'Démarrer normalement'
    'setup_reset_system'                = 'Redémarrer le système'
    'setup_shut_down'                   = 'Éteindre'
    'setup_back'                        = 'Retour'
    'setup_enabled'                     = 'Activé'
    'setup_disabled'                    = 'Désactivé'
    'setup_unavailable'                 = 'Indisponible'
    'setup_supported'                   = 'Pris en charge'
    'setup_not_supported'               = 'Non pris en charge'
    'setup_selected'                    = 'Sélectionné'
    'setup_boot_option'                 = 'Option de démarrage'
    'setup_action_succeeded'            = 'Action réussie'
    'setup_action_failed'               = "Échec de l'action"
    'setup_value'                       = 'Valeur'
    'setup_instructions_navigation'     = 'Utilisez les flèches haut et bas ou Tabulation pour vous déplacer'
    'setup_instructions_select'         = 'Appuyez sur Entrée pour ouvrir ou activer, Échap pour revenir'
    'setup_instructions_timeout'        = 'Le démarrage automatique est annulé dès que vous appuyez sur une touche'
    'setup_confirm_restart'             = 'Confirmer le redémarrage'
    'setup_confirm_shutdown'            = "Confirmer l'arrêt"
    'setup_cancel'                      = 'Annuler'
}

$phrases = if ($culture -eq 'fr') { $fr } else { $en }

foreach ($character in 'abcdefghijklmnopqrstuvwxyz'.ToCharArray()) {
    $phrases["spell_$character"] = [string]$character
}
foreach ($digit in '0123456789'.ToCharArray()) {
    $phrases["spell_$digit"] = [string]$digit
}

if ($culture -eq 'fr') {
    $phrases['spell_dash']       = 'tiret'
    $phrases['spell_dot']        = 'point'
    $phrases['spell_colon']      = 'deux points'
    $phrases['spell_slash']      = 'barre oblique'
    $phrases['spell_underscore'] = 'souligné'
} else {
    $phrases['spell_dash']       = 'dash'
    $phrases['spell_dot']        = 'dot'
    $phrases['spell_colon']      = 'colon'
    $phrases['spell_slash']      = 'slash'
    $phrases['spell_underscore'] = 'underscore'
}

Write-Host "Speech language: $culture"
Write-Host "Asset count: $($phrases.Count)"

foreach ($name in $phrases.Keys) {
    $wav = Join-Path $env:TEMP "aw_speech_$name.wav"
    $synth.SetOutputToWaveFile($wav, $fmt)
    $synth.Speak($phrases[$name])
    $synth.SetOutputToNull()

    $bytes = [System.IO.File]::ReadAllBytes($wav)
    $pos = 12
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
    if (-not $pcm -or $pcm.Length -eq 0) {
        throw "No PCM data generated for $name"
    }

    $pcmPath = Join-Path $outDir "$name.pcm"
    [System.IO.File]::WriteAllBytes($pcmPath, $pcm)
    Remove-Item -LiteralPath $wav -Force
    Write-Host ("  {0}.pcm: {1} bytes ({2:N2}s)" -f $name, $pcm.Length, ($pcm.Length / 48000.0))
}

$synth.Dispose()
Write-Host ""
Write-Host "UEFI_SETUP_SPEECH_GENERATED=PASS"
Write-Host "Wrote $($phrases.Count) clips to $outDir"
