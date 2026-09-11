#![no_std]
#![forbid(unsafe_code)]

pub const RSDP_V1_LEN: usize = 20;
pub const RSDP_V2_MIN_LEN: usize = 36;
pub const RSDP_MAX_LEN: usize = 4096;
const RSDP_SIGNATURE: &[u8; 8] = b"RSD PTR ";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RsdpError {
    TooShort,
    InvalidSignature,
    InvalidChecksum,
    InvalidLength,
    InvalidExtendedChecksum,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RsdpInfo {
    pub revision: u8,
    pub length: usize,
    pub rsdt_address: u32,
    pub xsdt_address: Option<u64>,
}

#[must_use]
fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().copied().fold(0_u8, u8::wrapping_add)
}

/// Returns the total RSDP length after validating the signature and the
/// ACPI 1.0 checksum. A caller can use this to determine how many bytes must
/// be made available for full ACPI 2.0+ validation.
pub fn declared_length(prefix: &[u8]) -> Result<usize, RsdpError> {
    if prefix.len() < RSDP_V1_LEN {
        return Err(RsdpError::TooShort);
    }

    if &prefix[..RSDP_SIGNATURE.len()] != RSDP_SIGNATURE {
        return Err(RsdpError::InvalidSignature);
    }

    if checksum(&prefix[..RSDP_V1_LEN]) != 0 {
        return Err(RsdpError::InvalidChecksum);
    }

    if prefix[15] < 2 {
        return Ok(RSDP_V1_LEN);
    }

    if prefix.len() < 24 {
        return Err(RsdpError::TooShort);
    }

    let length = u32::from_le_bytes([prefix[20], prefix[21], prefix[22], prefix[23]]) as usize;
    if !(RSDP_V2_MIN_LEN..=RSDP_MAX_LEN).contains(&length) {
        return Err(RsdpError::InvalidLength);
    }

    Ok(length)
}

pub fn validate_rsdp(bytes: &[u8]) -> Result<RsdpInfo, RsdpError> {
    let length = declared_length(bytes)?;
    if bytes.len() < length {
        return Err(RsdpError::TooShort);
    }

    let revision = bytes[15];
    if revision >= 2 && checksum(&bytes[..length]) != 0 {
        return Err(RsdpError::InvalidExtendedChecksum);
    }

    let rsdt_address = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let xsdt_address = if revision >= 2 {
        Some(u64::from_le_bytes([
            bytes[24], bytes[25], bytes[26], bytes[27], bytes[28], bytes[29], bytes[30], bytes[31],
        ]))
    } else {
        None
    };

    Ok(RsdpInfo {
        revision,
        length,
        rsdt_address,
        xsdt_address,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set_checksum(bytes: &mut [u8], checksum_offset: usize, length: usize) {
        bytes[checksum_offset] = 0;
        let sum = checksum(&bytes[..length]);
        bytes[checksum_offset] = 0_u8.wrapping_sub(sum);
    }

    fn valid_v2_rsdp() -> [u8; RSDP_V2_MIN_LEN] {
        let mut rsdp = [0_u8; RSDP_V2_MIN_LEN];
        rsdp[..8].copy_from_slice(RSDP_SIGNATURE);
        rsdp[9..15].copy_from_slice(b"AWOS  ");
        rsdp[15] = 2;
        rsdp[16..20].copy_from_slice(&0x1234_5000_u32.to_le_bytes());
        rsdp[20..24].copy_from_slice(&(RSDP_V2_MIN_LEN as u32).to_le_bytes());
        rsdp[24..32].copy_from_slice(&0x1234_5678_9abc_d000_u64.to_le_bytes());
        set_checksum(&mut rsdp, 8, RSDP_V1_LEN);
        set_checksum(&mut rsdp, 32, RSDP_V2_MIN_LEN);
        rsdp
    }

    #[test]
    fn accepts_valid_v2_rsdp() {
        let rsdp = valid_v2_rsdp();
        let info = validate_rsdp(&rsdp).expect("valid RSDP must parse");

        assert_eq!(info.revision, 2);
        assert_eq!(info.length, RSDP_V2_MIN_LEN);
        assert_eq!(info.rsdt_address, 0x1234_5000);
        assert_eq!(info.xsdt_address, Some(0x1234_5678_9abc_d000));
    }

    #[test]
    fn rejects_bad_signature() {
        let mut rsdp = valid_v2_rsdp();
        rsdp[0] = b'X';
        assert_eq!(validate_rsdp(&rsdp), Err(RsdpError::InvalidSignature));
    }

    #[test]
    fn rejects_bad_v1_checksum() {
        let mut rsdp = valid_v2_rsdp();
        rsdp[10] ^= 1;
        assert_eq!(validate_rsdp(&rsdp), Err(RsdpError::InvalidChecksum));
    }

    #[test]
    fn rejects_bad_extended_checksum() {
        let mut rsdp = valid_v2_rsdp();
        rsdp[35] ^= 1;
        assert_eq!(
            validate_rsdp(&rsdp),
            Err(RsdpError::InvalidExtendedChecksum)
        );
    }

    #[test]
    fn rejects_unreasonable_declared_length() {
        let mut rsdp = valid_v2_rsdp();
        rsdp[20..24].copy_from_slice(&8_u32.to_le_bytes());
        set_checksum(&mut rsdp, 8, RSDP_V1_LEN);
        assert_eq!(validate_rsdp(&rsdp), Err(RsdpError::InvalidLength));
    }
}
