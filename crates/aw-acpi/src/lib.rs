#![no_std]
#![forbid(unsafe_code)]

pub const RSDP_V1_LEN: usize = 20;
pub const RSDP_V2_MIN_LEN: usize = 36;
pub const RSDP_MAX_LEN: usize = 4096;
pub const SDT_HEADER_LEN: usize = 36;
pub const MCFG_HEADER_LEN: usize = 44;
pub const MCFG_ALLOCATION_LEN: usize = 16;
const RSDP_SIGNATURE: &[u8; 8] = b"RSD PTR ";
const MCFG_SIGNATURE: [u8; 4] = *b"MCFG";

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SdtError {
    TooShort,
    InvalidLength,
    InvalidChecksum,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SdtHeaderInfo {
    pub signature: [u8; 4],
    pub length: usize,
    pub revision: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum McfgError {
    InvalidSdt(SdtError),
    InvalidSignature,
    InvalidAllocationLength,
    InvalidBusRange,
}

impl From<SdtError> for McfgError {
    fn from(value: SdtError) -> Self {
        Self::InvalidSdt(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct McfgAllocation {
    pub base_address: u64,
    pub segment_group: u16,
    pub start_bus: u8,
    pub end_bus: u8,
}

impl McfgAllocation {
    pub const fn contains_bus(self, bus: u8) -> bool {
        bus >= self.start_bus && bus <= self.end_bus
    }

    pub const fn ecam_address(
        self,
        bus: u8,
        device: u8,
        function: u8,
        register_offset: u16,
    ) -> Option<u64> {
        if !self.contains_bus(bus) || device > 31 || function > 7 || register_offset > 0x0fff {
            return None;
        }

        let relative_bus = (bus - self.start_bus) as u64;
        Some(
            self.base_address
                + (relative_bus << 20)
                + ((device as u64) << 15)
                + ((function as u64) << 12)
                + register_offset as u64,
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Mcfg<'a> {
    table: &'a [u8],
}

impl<'a> Mcfg<'a> {
    pub fn allocations(self) -> McfgAllocations<'a> {
        McfgAllocations {
            chunks: self.table[MCFG_HEADER_LEN..].chunks_exact(MCFG_ALLOCATION_LEN),
        }
    }
}

pub struct McfgAllocations<'a> {
    chunks: core::slice::ChunksExact<'a, u8>,
}

impl Iterator for McfgAllocations<'_> {
    type Item = McfgAllocation;

    fn next(&mut self) -> Option<Self::Item> {
        self.chunks.next().map(parse_mcfg_allocation)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.chunks.size_hint()
    }
}

impl ExactSizeIterator for McfgAllocations<'_> {}

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

pub fn validate_sdt(bytes: &[u8]) -> Result<SdtHeaderInfo, SdtError> {
    if bytes.len() < SDT_HEADER_LEN {
        return Err(SdtError::TooShort);
    }

    let length = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    if length < SDT_HEADER_LEN || length > bytes.len() {
        return Err(SdtError::InvalidLength);
    }
    if checksum(&bytes[..length]) != 0 {
        return Err(SdtError::InvalidChecksum);
    }

    Ok(SdtHeaderInfo {
        signature: [bytes[0], bytes[1], bytes[2], bytes[3]],
        length,
        revision: bytes[8],
    })
}

pub fn validate_mcfg(bytes: &[u8]) -> Result<Mcfg<'_>, McfgError> {
    let header = validate_sdt(bytes)?;
    if header.signature != MCFG_SIGNATURE {
        return Err(McfgError::InvalidSignature);
    }
    if header.length < MCFG_HEADER_LEN
        || (header.length - MCFG_HEADER_LEN) % MCFG_ALLOCATION_LEN != 0
    {
        return Err(McfgError::InvalidAllocationLength);
    }

    let table = &bytes[..header.length];
    for chunk in table[MCFG_HEADER_LEN..].chunks_exact(MCFG_ALLOCATION_LEN) {
        let allocation = parse_mcfg_allocation(chunk);
        if allocation.start_bus > allocation.end_bus {
            return Err(McfgError::InvalidBusRange);
        }
    }

    Ok(Mcfg { table })
}

fn parse_mcfg_allocation(bytes: &[u8]) -> McfgAllocation {
    McfgAllocation {
        base_address: u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]),
        segment_group: u16::from_le_bytes([bytes[8], bytes[9]]),
        start_bus: bytes[10],
        end_bus: bytes[11],
    }
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

    fn valid_mcfg() -> [u8; MCFG_HEADER_LEN + MCFG_ALLOCATION_LEN] {
        let mut table = [0_u8; MCFG_HEADER_LEN + MCFG_ALLOCATION_LEN];
        table[..4].copy_from_slice(b"MCFG");
        let table_len = table.len();
        table[4..8].copy_from_slice(&(table_len as u32).to_le_bytes());
        table[8] = 1;
        table[10..16].copy_from_slice(b"AWOS  ");
        table[16..24].copy_from_slice(b"GENERIC ");

        let entry = &mut table[MCFG_HEADER_LEN..];
        entry[0..8].copy_from_slice(&0xe000_0000_u64.to_le_bytes());
        entry[8..10].copy_from_slice(&0_u16.to_le_bytes());
        entry[10] = 0;
        entry[11] = 0xff;

        set_checksum(&mut table, 9, table_len);
        table
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

    #[test]
    fn validates_sdt_checksum_and_length() {
        let table = valid_mcfg();
        let header = validate_sdt(&table).expect("valid SDT must parse");
        assert_eq!(header.signature, *b"MCFG");
        assert_eq!(header.length, table.len());
    }

    #[test]
    fn parses_mcfg_ecam_allocation() {
        let table = valid_mcfg();
        let mcfg = validate_mcfg(&table).expect("valid MCFG must parse");
        let allocation = mcfg.allocations().next().expect("one allocation");
        assert_eq!(allocation.base_address, 0xe000_0000);
        assert_eq!(allocation.segment_group, 0);
        assert_eq!(allocation.start_bus, 0);
        assert_eq!(allocation.end_bus, 0xff);
        assert_eq!(
            allocation.ecam_address(2, 5, 3, 0x120),
            Some(0xe000_0000 + (2_u64 << 20) + (5_u64 << 15) + (3_u64 << 12) + 0x120)
        );
    }

    #[test]
    fn rejects_mcfg_with_invalid_bus_range() {
        let mut table = valid_mcfg();
        table[MCFG_HEADER_LEN + 10] = 10;
        table[MCFG_HEADER_LEN + 11] = 2;
        let table_len = table.len();
        set_checksum(&mut table, 9, table_len);
        assert!(matches!(
            validate_mcfg(&table),
            Err(McfgError::InvalidBusRange)
        ));
    }
}
