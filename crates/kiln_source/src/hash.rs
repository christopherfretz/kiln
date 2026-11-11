//! The content hash shared by the module file checksum, the PCH file
//! table and the declaration image's file table (spec/source.md
//! "Content hash"): FNV-1a 64.

/// The FNV-1a 64 offset basis: the state before any byte.
pub const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// Continues an FNV-1a 64 hash in state `h` over `bytes`. Each step is a
/// bijection of the state for a fixed input byte, so changing any one
/// byte changes the result.
#[must_use]
pub fn fnv1a64_extend(mut h: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// The FNV-1a 64 hash of `bytes`.
#[must_use]
pub fn content_hash(bytes: &[u8]) -> u64 {
    fnv1a64_extend(FNV_OFFSET, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_values() {
        assert_eq!(content_hash(b""), FNV_OFFSET);
        assert_eq!(content_hash(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64_extend(content_hash(b"ab"), b"c"), content_hash(b"abc"));
    }
}
