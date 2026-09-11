#![no_std]
#![forbid(unsafe_code)]

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpuidRegisters {
    pub eax: u32,
    pub ebx: u32,
    pub ecx: u32,
    pub edx: u32,
}

impl CpuidRegisters {
    pub const ZERO: Self = Self {
        eax: 0,
        ebx: 0,
        ecx: 0,
        edx: 0,
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CpuVendor {
    Amd,
    Intel,
    Other([u8; 12]),
}

impl CpuVendor {
    pub fn from_leaf0(leaf0: CpuidRegisters) -> Self {
        let mut bytes = [0_u8; 12];
        bytes[0..4].copy_from_slice(&leaf0.ebx.to_le_bytes());
        bytes[4..8].copy_from_slice(&leaf0.edx.to_le_bytes());
        bytes[8..12].copy_from_slice(&leaf0.ecx.to_le_bytes());

        match &bytes {
            b"AuthenticAMD" => Self::Amd,
            b"GenuineIntel" => Self::Intel,
            _ => Self::Other(bytes),
        }
    }

    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::Amd => "amd",
            Self::Intel => "intel",
            Self::Other(_) => "other",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpuSignature {
    pub family: u16,
    pub model: u16,
    pub stepping: u8,
}

impl CpuSignature {
    pub const fn from_leaf1_eax(eax: u32) -> Self {
        let stepping = (eax & 0x0f) as u8;
        let base_model = ((eax >> 4) & 0x0f) as u16;
        let base_family = ((eax >> 8) & 0x0f) as u16;
        let extended_model = ((eax >> 16) & 0x0f) as u16;
        let extended_family = ((eax >> 20) & 0xff) as u16;

        let family = if base_family == 0x0f {
            base_family + extended_family
        } else {
            base_family
        };

        let model = if base_family == 0x06 || base_family == 0x0f {
            base_model | (extended_model << 4)
        } else {
            base_model
        };

        Self {
            family,
            model,
            stepping,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CpuFeatures {
    pub tsc: bool,
    pub apic: bool,
    pub sse2: bool,
    pub xsave: bool,
    pub avx: bool,
    pub x2apic: bool,
    pub hypervisor_present: bool,
    pub long_mode: bool,
    pub rdtscp: bool,
    pub invariant_tsc: bool,
    pub smep: bool,
    pub smap: bool,
}

impl CpuFeatures {
    pub const fn from_leaves(
        leaf1: CpuidRegisters,
        leaf7_sub0: Option<CpuidRegisters>,
        extended_leaf1: Option<CpuidRegisters>,
        extended_leaf7: Option<CpuidRegisters>,
    ) -> Self {
        let leaf7 = match leaf7_sub0 {
            Some(value) => value,
            None => CpuidRegisters::ZERO,
        };
        let ext1 = match extended_leaf1 {
            Some(value) => value,
            None => CpuidRegisters::ZERO,
        };
        let ext7 = match extended_leaf7 {
            Some(value) => value,
            None => CpuidRegisters::ZERO,
        };

        Self {
            tsc: leaf1.edx & (1 << 4) != 0,
            apic: leaf1.edx & (1 << 9) != 0,
            sse2: leaf1.edx & (1 << 26) != 0,
            xsave: leaf1.ecx & (1 << 26) != 0,
            avx: leaf1.ecx & (1 << 28) != 0,
            x2apic: leaf1.ecx & (1 << 21) != 0,
            hypervisor_present: leaf1.ecx & (1 << 31) != 0,
            long_mode: ext1.edx & (1 << 29) != 0,
            rdtscp: ext1.edx & (1 << 27) != 0,
            invariant_tsc: ext7.edx & (1 << 8) != 0,
            smep: leaf7.ebx & (1 << 7) != 0,
            smap: leaf7.ebx & (1 << 20) != 0,
        }
    }

    pub const fn meets_boot_baseline(self) -> bool {
        self.apic && self.sse2 && self.long_mode
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpuIdentity {
    pub vendor: CpuVendor,
    pub signature: CpuSignature,
    pub max_basic_leaf: u32,
    pub max_extended_leaf: u32,
    pub features: CpuFeatures,
}

impl CpuIdentity {
    pub const fn is_supported_vendor(self) -> bool {
        matches!(self.vendor, CpuVendor::Amd | CpuVendor::Intel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vendor_leaf(vendor: &[u8; 12], max_leaf: u32) -> CpuidRegisters {
        CpuidRegisters {
            eax: max_leaf,
            ebx: u32::from_le_bytes(vendor[0..4].try_into().unwrap()),
            edx: u32::from_le_bytes(vendor[4..8].try_into().unwrap()),
            ecx: u32::from_le_bytes(vendor[8..12].try_into().unwrap()),
        }
    }

    #[test]
    fn detects_amd_and_intel_vendor_strings() {
        assert_eq!(
            CpuVendor::from_leaf0(vendor_leaf(b"AuthenticAMD", 7)),
            CpuVendor::Amd
        );
        assert_eq!(
            CpuVendor::from_leaf0(vendor_leaf(b"GenuineIntel", 7)),
            CpuVendor::Intel
        );
    }

    #[test]
    fn preserves_unknown_vendor_string() {
        let vendor = *b"ExampleCPU12";
        assert_eq!(
            CpuVendor::from_leaf0(vendor_leaf(&vendor, 1)),
            CpuVendor::Other(vendor)
        );
    }

    #[test]
    fn decodes_family_model_and_stepping() {
        // Family 0x19, model 0x50, stepping 0 from a Zen-family signature.
        let signature = CpuSignature::from_leaf1_eax(0x00a5_0f00);
        assert_eq!(signature.family, 0x19);
        assert_eq!(signature.model, 0x50);
        assert_eq!(signature.stepping, 0);
    }

    #[test]
    fn detects_generic_boot_features() {
        let features = CpuFeatures::from_leaves(
            CpuidRegisters {
                eax: 0,
                ebx: 0,
                ecx: (1 << 21) | (1 << 26) | (1 << 28),
                edx: (1 << 4) | (1 << 9) | (1 << 26),
            },
            Some(CpuidRegisters {
                eax: 0,
                ebx: (1 << 7) | (1 << 20),
                ecx: 0,
                edx: 0,
            }),
            Some(CpuidRegisters {
                eax: 0,
                ebx: 0,
                ecx: 0,
                edx: (1 << 27) | (1 << 29),
            }),
            Some(CpuidRegisters {
                eax: 0,
                ebx: 0,
                ecx: 0,
                edx: 1 << 8,
            }),
        );

        assert!(features.meets_boot_baseline());
        assert!(features.x2apic);
        assert!(features.xsave);
        assert!(features.avx);
        assert!(features.smep);
        assert!(features.smap);
        assert!(features.rdtscp);
        assert!(features.invariant_tsc);
    }
}
