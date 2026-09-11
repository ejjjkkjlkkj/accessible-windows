#![no_std]
#![forbid(unsafe_code)]

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryCapability {
    KeyboardInput,
    StructuredDiagnostics,
    SpeechOutput,
    BrailleOutput,
    RollbackSelection,
    SignedReinstall,
    DiagnosticExport,
}

impl RecoveryCapability {
    const fn bit(self) -> u16 {
        match self {
            Self::KeyboardInput => 1 << 0,
            Self::StructuredDiagnostics => 1 << 1,
            Self::SpeechOutput => 1 << 2,
            Self::BrailleOutput => 1 << 3,
            Self::RollbackSelection => 1 << 4,
            Self::SignedReinstall => 1 << 5,
            Self::DiagnosticExport => 1 << 6,
        }
    }
}

const REQUIRED_RECOVERY_CAPABILITIES: [RecoveryCapability; 5] = [
    RecoveryCapability::KeyboardInput,
    RecoveryCapability::StructuredDiagnostics,
    RecoveryCapability::RollbackSelection,
    RecoveryCapability::SignedReinstall,
    RecoveryCapability::DiagnosticExport,
];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RecoveryProbeReport {
    passed: u16,
}

impl RecoveryProbeReport {
    #[must_use]
    pub const fn new() -> Self {
        Self { passed: 0 }
    }

    pub fn mark_passed(&mut self, capability: RecoveryCapability) {
        self.passed |= capability.bit();
    }

    #[must_use]
    pub const fn passed(self, capability: RecoveryCapability) -> bool {
        self.passed & capability.bit() != 0
    }

    /// Produces a private readiness token only when recovery is independently usable without
    /// sight. A framebuffer or visual console never counts as a nonvisual output channel.
    pub fn accessible_ready(self) -> Result<AccessibleRecoveryReady, RecoveryReadinessError> {
        for capability in REQUIRED_RECOVERY_CAPABILITIES {
            if !self.passed(capability) {
                return Err(RecoveryReadinessError::MissingCapability(capability));
            }
        }

        let speech = self.passed(RecoveryCapability::SpeechOutput);
        let braille = self.passed(RecoveryCapability::BrailleOutput);
        if !speech && !braille {
            return Err(RecoveryReadinessError::NoDirectNonVisualOutput);
        }

        Ok(AccessibleRecoveryReady { speech, braille })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryReadinessError {
    MissingCapability(RecoveryCapability),
    NoDirectNonVisualOutput,
}

/// Proof that recovery has deterministic keyboard control, structured diagnostics, rollback and
/// signed-reinstall actions, diagnostic export, plus at least one direct nonvisual output channel.
///
/// The fields are private so visual-only recovery code cannot fabricate this token directly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccessibleRecoveryReady {
    speech: bool,
    braille: bool,
}

impl AccessibleRecoveryReady {
    #[must_use]
    pub const fn speech_available(self) -> bool {
        self.speech
    }

    #[must_use]
    pub const fn braille_available(self) -> bool {
        self.braille
    }
}

#[repr(u16)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryDiagnosticCode {
    BootStateCorrupt = 0x1001,
    NoBootableGeneration = 0x1002,
    ManifestRejected = 0x1101,
    ObjectVerificationFailed = 0x1102,
    StorageReadFailed = 0x1201,
    RollbackActivated = 0x1301,
    InputUnavailable = 0x1401,
    AudioUnavailable = 0x1402,
    SpeechUnavailable = 0x1403,
    AccessibilityBrokerUnavailable = 0x1404,
    RecoveryIntegrityFailed = 0x1501,
}

impl RecoveryDiagnosticCode {
    #[must_use]
    pub const fn code(self) -> u16 {
        self as u16
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoverySeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryAction {
    RetryCurrentGeneration,
    BootPreviousGeneration,
    EnterRecovery,
    ReinstallSignedImage,
    ExportDiagnostics,
    PowerOffSafely,
}

/// Machine-readable recovery event. UI, speech, braille and serial frontends all consume the same
/// event instead of inventing separate visual-only error paths.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryEvent {
    code: RecoveryDiagnosticCode,
    severity: RecoverySeverity,
    action: RecoveryAction,
    generation: Option<u64>,
}

impl RecoveryEvent {
    #[must_use]
    pub const fn new(
        code: RecoveryDiagnosticCode,
        severity: RecoverySeverity,
        action: RecoveryAction,
        generation: Option<u64>,
    ) -> Self {
        Self {
            code,
            severity,
            action,
            generation,
        }
    }

    #[must_use]
    pub const fn code(self) -> RecoveryDiagnosticCode {
        self.code
    }

    #[must_use]
    pub const fn severity(self) -> RecoverySeverity {
        self.severity
    }

    #[must_use]
    pub const fn action(self) -> RecoveryAction {
        self.action
    }

    #[must_use]
    pub const fn generation(self) -> Option<u64> {
        self.generation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_report() -> RecoveryProbeReport {
        let mut report = RecoveryProbeReport::new();
        for capability in REQUIRED_RECOVERY_CAPABILITIES {
            report.mark_passed(capability);
        }
        report
    }

    #[test]
    fn visual_only_recovery_is_rejected() {
        assert_eq!(
            base_report().accessible_ready(),
            Err(RecoveryReadinessError::NoDirectNonVisualOutput)
        );
    }

    #[test]
    fn speech_recovery_is_accepted() {
        let mut report = base_report();
        report.mark_passed(RecoveryCapability::SpeechOutput);
        let ready = report.accessible_ready().unwrap();
        assert!(ready.speech_available());
        assert!(!ready.braille_available());
    }

    #[test]
    fn braille_recovery_is_accepted_without_speech() {
        let mut report = base_report();
        report.mark_passed(RecoveryCapability::BrailleOutput);
        let ready = report.accessible_ready().unwrap();
        assert!(!ready.speech_available());
        assert!(ready.braille_available());
    }

    #[test]
    fn keyboard_is_mandatory_even_with_speech() {
        let mut report = RecoveryProbeReport::new();
        report.mark_passed(RecoveryCapability::StructuredDiagnostics);
        report.mark_passed(RecoveryCapability::SpeechOutput);
        report.mark_passed(RecoveryCapability::RollbackSelection);
        report.mark_passed(RecoveryCapability::SignedReinstall);
        report.mark_passed(RecoveryCapability::DiagnosticExport);
        assert_eq!(
            report.accessible_ready(),
            Err(RecoveryReadinessError::MissingCapability(
                RecoveryCapability::KeyboardInput
            ))
        );
    }

    #[test]
    fn diagnostic_codes_are_stable_machine_readable_values() {
        assert_eq!(RecoveryDiagnosticCode::BootStateCorrupt.code(), 0x1001);
        assert_eq!(RecoveryDiagnosticCode::StorageReadFailed.code(), 0x1201);
        assert_eq!(RecoveryDiagnosticCode::SpeechUnavailable.code(), 0x1403);
    }

    #[test]
    fn one_event_can_feed_visual_speech_braille_and_serial_frontends() {
        let event = RecoveryEvent::new(
            RecoveryDiagnosticCode::RollbackActivated,
            RecoverySeverity::Warning,
            RecoveryAction::BootPreviousGeneration,
            Some(41),
        );
        assert_eq!(event.code().code(), 0x1301);
        assert_eq!(event.action(), RecoveryAction::BootPreviousGeneration);
        assert_eq!(event.generation(), Some(41));
    }
}
