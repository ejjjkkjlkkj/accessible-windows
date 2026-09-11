#![no_std]
#![forbid(unsafe_code)]

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompatibilityTarget {
    AndroidDex,
    AndroidNativeX86_64,
    AndroidNativeArm64,
    LinuxElfX86_64,
    LinuxElfArm64,
    DarwinMachOX86_64,
    DarwinMachOArm64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeBoundary {
    /// Hardware-assisted VM boundary used when the guest expects a Linux kernel/runtime contract.
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
    WindowIntegration,
    LauncherRegistration,
    ClipboardBridge,
    NotificationBridge,
    FileOpenPortal,
    UrlIntentBridge,
}

impl RuntimeCapability {
    const fn bit(self) -> u32 {
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
            Self::WindowIntegration => 1 << 10,
            Self::LauncherRegistration => 1 << 11,
            Self::ClipboardBridge => 1 << 12,
            Self::NotificationBridge => 1 << 13,
            Self::FileOpenPortal => 1 << 14,
            Self::UrlIntentBridge => 1 << 15,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeProfile {
    target: CompatibilityTarget,
    boundary: RuntimeBoundary,
    surface: AppSurface,
    capabilities: u32,
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
    LinuxRequiresHardwareVm,
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

const REQUIRED_SYSTEM_INTEGRATION: [RuntimeCapability; 7] = [
    RuntimeCapability::LauncherRegistration,
    RuntimeCapability::ClipboardBridge,
    RuntimeCapability::NotificationBridge,
    RuntimeCapability::FileOpenPortal,
    RuntimeCapability::UrlIntentBridge,
    RuntimeCapability::AudioBridge,
    RuntimeCapability::NetworkBroker,
];

/// Validates the execution boundary of a local compatibility runtime before applications may run.
///
/// Android and Linux use hardware-isolated local VMs rather than sharing the host kernel. Darwin
/// compatibility is a clean-room userspace personality, not a bundled or virtualized copy of
/// macOS. Foreign-ISA native binaries require an explicit translation capability.
pub fn admit_runtime(profile: RuntimeProfile) -> Result<RuntimeReady, RuntimeAdmissionError> {
    match profile.target {
        CompatibilityTarget::AndroidDex
        | CompatibilityTarget::AndroidNativeX86_64
        | CompatibilityTarget::AndroidNativeArm64 => {
            if profile.boundary != RuntimeBoundary::HardwareIsolatedVm {
                return Err(RuntimeAdmissionError::AndroidRequiresHardwareVm);
            }
        }
        CompatibilityTarget::LinuxElfX86_64 | CompatibilityTarget::LinuxElfArm64 => {
            if profile.boundary != RuntimeBoundary::HardwareIsolatedVm {
                return Err(RuntimeAdmissionError::LinuxRequiresHardwareVm);
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
        CompatibilityTarget::AndroidNativeArm64
            | CompatibilityTarget::LinuxElfArm64
            | CompatibilityTarget::DarwinMachOArm64
    ) && !profile.has(RuntimeCapability::ForeignIsaTranslation)
    {
        return Err(RuntimeAdmissionError::ForeignIsaTranslationUnavailable);
    }

    Ok(RuntimeReady {
        target: profile.target,
        surface: profile.surface,
    })
}

/// Requires a foreign application to participate in the host desktop as a first-class app.
///
/// This is deliberately stricter than merely being executable. An installed Android, Linux or
/// Darwin app is system-integrated only when it can be discovered by the launcher, exchange
/// clipboard data, publish notifications, open user-approved files and URLs, use host audio and
/// networking brokers, and expose accessibility semantics. Graphical apps additionally require a
/// host-managed window bridge. Runtimes may provide more capabilities, but cannot claim integrated
/// status with fewer.
pub fn admit_system_integrated_app(
    profile: RuntimeProfile,
) -> Result<RuntimeReady, RuntimeAdmissionError> {
    let ready = admit_runtime(profile)?;

    for capability in REQUIRED_SYSTEM_INTEGRATION {
        if !profile.has(capability) {
            return Err(RuntimeAdmissionError::MissingCapability(capability));
        }
    }

    if profile.surface == AppSurface::Graphical && !profile.has(RuntimeCapability::WindowIntegration)
    {
        return Err(RuntimeAdmissionError::MissingCapability(
            RuntimeCapability::WindowIntegration,
        ));
    }

    Ok(ready)
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

    fn fully_integrated_profile(
        target: CompatibilityTarget,
        boundary: RuntimeBoundary,
        surface: AppSurface,
    ) -> RuntimeProfile {
        let mut profile = base_profile(target, boundary, surface);
        for capability in REQUIRED_SYSTEM_INTEGRATION {
            profile.mark_capability(capability);
        }
        if surface == AppSurface::Graphical {
            profile.mark_capability(RuntimeCapability::GraphicsBridge);
            profile.mark_capability(RuntimeCapability::WindowIntegration);
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
    fn linux_is_not_admitted_as_shared_host_userspace() {
        let profile = base_profile(
            CompatibilityTarget::LinuxElfX86_64,
            RuntimeBoundary::UserModeCompatibility,
            AppSurface::CommandLine,
        );
        assert_eq!(
            admit_runtime(profile),
            Err(RuntimeAdmissionError::LinuxRequiresHardwareVm)
        );
    }

    #[test]
    fn android_and_linux_x86_64_can_run_locally_in_isolated_vms() {
        for target in [
            CompatibilityTarget::AndroidNativeX86_64,
            CompatibilityTarget::LinuxElfX86_64,
        ] {
            let profile = base_profile(
                target,
                RuntimeBoundary::HardwareIsolatedVm,
                AppSurface::CommandLine,
            );
            assert!(admit_runtime(profile).is_ok());
        }
    }

    #[test]
    fn foreign_arm_binaries_require_explicit_translation() {
        for target in [
            CompatibilityTarget::AndroidNativeArm64,
            CompatibilityTarget::LinuxElfArm64,
            CompatibilityTarget::DarwinMachOArm64,
        ] {
            let boundary = if target == CompatibilityTarget::DarwinMachOArm64 {
                RuntimeBoundary::UserModeCompatibility
            } else {
                RuntimeBoundary::HardwareIsolatedVm
            };
            let profile = base_profile(target, boundary, AppSurface::CommandLine);
            assert_eq!(
                admit_runtime(profile),
                Err(RuntimeAdmissionError::ForeignIsaTranslationUnavailable)
            );
        }
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
    fn graphical_apps_require_graphics_bridge() {
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
    fn executable_app_is_not_automatically_system_integrated() {
        let profile = base_profile(
            CompatibilityTarget::AndroidDex,
            RuntimeBoundary::HardwareIsolatedVm,
            AppSurface::CommandLine,
        );
        assert_eq!(
            admit_system_integrated_app(profile),
            Err(RuntimeAdmissionError::MissingCapability(
                RuntimeCapability::LauncherRegistration
            ))
        );
    }

    #[test]
    fn graphical_integrated_app_requires_host_window_integration() {
        let mut profile = fully_integrated_profile(
            CompatibilityTarget::LinuxElfX86_64,
            RuntimeBoundary::HardwareIsolatedVm,
            AppSurface::Graphical,
        );
        profile.capabilities &= !RuntimeCapability::WindowIntegration.bit();
        assert_eq!(
            admit_system_integrated_app(profile),
            Err(RuntimeAdmissionError::MissingCapability(
                RuntimeCapability::WindowIntegration
            ))
        );
    }

    #[test]
    fn android_linux_and_darwin_can_share_one_host_integration_contract() {
        for (target, boundary) in [
            (
                CompatibilityTarget::AndroidDex,
                RuntimeBoundary::HardwareIsolatedVm,
            ),
            (
                CompatibilityTarget::LinuxElfX86_64,
                RuntimeBoundary::HardwareIsolatedVm,
            ),
            (
                CompatibilityTarget::DarwinMachOX86_64,
                RuntimeBoundary::UserModeCompatibility,
            ),
        ] {
            let profile = fully_integrated_profile(target, boundary, AppSurface::Graphical);
            let ready = admit_system_integrated_app(profile).unwrap();
            assert_eq!(ready.target(), target);
            assert_eq!(ready.surface(), AppSurface::Graphical);
        }
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
