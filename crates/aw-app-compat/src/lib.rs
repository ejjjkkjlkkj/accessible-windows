#![no_std]
#![forbid(unsafe_code)]

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompatibilityTarget {
    AndroidDex,
    AndroidNativeX86_64,
    AndroidNativeArm64,
    DarwinMachOX86_64,
    DarwinMachOArm64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeBoundary {
    /// Hardware-assisted VM boundary used for Android framework/runtime execution.
    HardwareIsolatedVm,
    /// User-mode personality translating a foreign userspace ABI into host services.
    UserModeCompatibility,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppSurface {
    CommandLine,
    Graphical,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeCapability {
    PackageIdentity,
    SyscallIsolation,
    IpcBroker,
    FilesystemBroker,
    InputBridge,
    AccessibilityBridge,
    GraphicsBridge,
    AudioBridge,
    NetworkBroker,
    ForeignIsaTranslation,
}

impl RuntimeCapability {
    const fn bit(self) -> u16 {
        match self {
            Self::PackageIdentity => 1 << 0,
            Self::SyscallIsolation => 1 << 1,
            Self::IpcBroker => 1 << 2,
            Self::FilesystemBroker => 1 << 3,
            Self::InputBridge => 1 << 4,
            Self::AccessibilityBridge => 1 << 5,
            Self::GraphicsBridge => 1 << 6,
            Self::AudioBridge => 1 << 7,
            Self::NetworkBroker => 1 << 8,
            Self::ForeignIsaTranslation => 1 << 9,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeProfile {
    target: CompatibilityTarget,
    boundary: RuntimeBoundary,
    surface: AppSurface,
    capabilities: u16,
}

impl RuntimeProfile {
    #[must_use]
    pub const fn new(
        target: CompatibilityTarget,
        boundary: RuntimeBoundary,
        surface: AppSurface,
    ) -> Self {
        Self {
            target,
            boundary,
            surface,
            capabilities: 0,
        }
    }

    pub fn mark_capability(&mut self, capability: RuntimeCapability) {
        self.capabilities |= capability.bit();
    }

    #[must_use]
    pub const fn has(self, capability: RuntimeCapability) -> bool {
        self.capabilities & capability.bit() != 0
    }

    #[must_use]
    pub const fn target(self) -> CompatibilityTarget {
        self.target
    }

    #[must_use]
    pub const fn boundary(self) -> RuntimeBoundary {
        self.boundary
    }

    #[must_use]
    pub const fn surface(self) -> AppSurface {
        self.surface
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAdmissionError {
    AndroidRequiresHardwareVm,
    DarwinRequiresUserModeCompatibility,
    MissingCapability(RuntimeCapability),
    ForeignIsaTranslationUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeReady {
    target: CompatibilityTarget,
    surface: AppSurface,
}

impl RuntimeReady {
    #[must_use]
    pub const fn target(self) -> CompatibilityTarget {
        self.target
    }

    #[must_use]
    pub const fn surface(self) -> AppSurface {
        self.surface
    }
}

const REQUIRED_BASE: [RuntimeCapability; 6] = [
    RuntimeCapability::PackageIdentity,
    RuntimeCapability::SyscallIsolation,
    RuntimeCapability::IpcBroker,
    RuntimeCapability::FilesystemBroker,
    RuntimeCapability::InputBridge,
    RuntimeCapability::AccessibilityBridge,
];

/// Validates a local compatibility runtime before applications may be launched.
///
/// Android is deliberately required to run behind a hardware VM boundary rather than a shared
/// host-kernel container. Darwin/macOS compatibility is a clean-room userspace personality, not a
/// bundled or virtualized copy of macOS. Graphical applications additionally require a graphics
/// bridge. Foreign-ISA native binaries require an explicit translation capability.
pub fn admit_runtime(profile: RuntimeProfile) -> Result<RuntimeReady, RuntimeAdmissionError> {
    match profile.target {
        CompatibilityTarget::AndroidDex
        | CompatibilityTarget::AndroidNativeX86_64
        | CompatibilityTarget::AndroidNativeArm64 => {
            if profile.boundary != RuntimeBoundary::HardwareIsolatedVm {
                return Err(RuntimeAdmissionError::AndroidRequiresHardwareVm);
            }
        }
        CompatibilityTarget::DarwinMachOX86_64 | CompatibilityTarget::DarwinMachOArm64 => {
            if profile.boundary != RuntimeBoundary::UserModeCompatibility {
                return Err(RuntimeAdmissionError::DarwinRequiresUserModeCompatibility);
            }
        }
    }

    for capability in REQUIRED_BASE {
        if !profile.has(capability) {
            return Err(RuntimeAdmissionError::MissingCapability(capability));
        }
    }

    if profile.surface == AppSurface::Graphical && !profile.has(RuntimeCapability::GraphicsBridge) {
        return Err(RuntimeAdmissionError::MissingCapability(
            RuntimeCapability::GraphicsBridge,
        ));
    }

    if matches!(
        profile.target,
        CompatibilityTarget::AndroidNativeArm64 | CompatibilityTarget::DarwinMachOArm64
    ) && !profile.has(RuntimeCapability::ForeignIsaTranslation)
    {
        return Err(RuntimeAdmissionError::ForeignIsaTranslationUnavailable);
    }

    Ok(RuntimeReady {
        target: profile.target,
        surface: profile.surface,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_profile(
        target: CompatibilityTarget,
        boundary: RuntimeBoundary,
        surface: AppSurface,
    ) -> RuntimeProfile {
        let mut profile = RuntimeProfile::new(target, boundary, surface);
        for capability in REQUIRED_BASE {
            profile.mark_capability(capability);
        }
        profile
    }

    #[test]
    fn android_is_not_admitted_as_shared_host_userspace() {
        let profile = base_profile(
            CompatibilityTarget::AndroidDex,
            RuntimeBoundary::UserModeCompatibility,
            AppSurface::CommandLine,
        );
        assert_eq!(
            admit_runtime(profile),
            Err(RuntimeAdmissionError::AndroidRequiresHardwareVm)
        );
    }

    #[test]
    fn android_x86_64_can_run_locally_in_isolated_vm() {
        let profile = base_profile(
            CompatibilityTarget::AndroidNativeX86_64,
            RuntimeBoundary::HardwareIsolatedVm,
            AppSurface::CommandLine,
        );
        assert!(admit_runtime(profile).is_ok());
    }

    #[test]
    fn arm_android_requires_explicit_translation() {
        let profile = base_profile(
            CompatibilityTarget::AndroidNativeArm64,
            RuntimeBoundary::HardwareIsolatedVm,
            AppSurface::CommandLine,
        );
        assert_eq!(
            admit_runtime(profile),
            Err(RuntimeAdmissionError::ForeignIsaTranslationUnavailable)
        );
    }

    #[test]
    fn darwin_is_userspace_compatibility_not_macos_vm() {
        let profile = base_profile(
            CompatibilityTarget::DarwinMachOX86_64,
            RuntimeBoundary::HardwareIsolatedVm,
            AppSurface::CommandLine,
        );
        assert_eq!(
            admit_runtime(profile),
            Err(RuntimeAdmissionError::DarwinRequiresUserModeCompatibility)
        );
    }

    #[test]
    fn graphical_apps_require_graphics_and_accessibility_bridges() {
        let mut profile = base_profile(
            CompatibilityTarget::DarwinMachOX86_64,
            RuntimeBoundary::UserModeCompatibility,
            AppSurface::Graphical,
        );
        assert_eq!(
            admit_runtime(profile),
            Err(RuntimeAdmissionError::MissingCapability(
                RuntimeCapability::GraphicsBridge
            ))
        );
        profile.mark_capability(RuntimeCapability::GraphicsBridge);
        assert!(admit_runtime(profile).is_ok());
    }

    #[test]
    fn accessibility_bridge_is_mandatory_for_every_runtime() {
        let mut profile = RuntimeProfile::new(
            CompatibilityTarget::AndroidDex,
            RuntimeBoundary::HardwareIsolatedVm,
            AppSurface::CommandLine,
        );
        for capability in [
            RuntimeCapability::PackageIdentity,
            RuntimeCapability::SyscallIsolation,
            RuntimeCapability::IpcBroker,
            RuntimeCapability::FilesystemBroker,
            RuntimeCapability::InputBridge,
        ] {
            profile.mark_capability(capability);
        }
        assert_eq!(
            admit_runtime(profile),
            Err(RuntimeAdmissionError::MissingCapability(
                RuntimeCapability::AccessibilityBridge
            ))
        );
    }
}
