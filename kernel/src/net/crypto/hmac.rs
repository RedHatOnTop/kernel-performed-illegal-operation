//! HMAC (Hash-based Message Authentication Code) — RFC 2104
//!
//! Provides HMAC-SHA-256 (32-byte tag) and HMAC-SHA-384 (48-byte tag).

use alloc::vec::Vec;
use super::sha::{sha256, sha384};

/// HMAC-SHA-256 → 32-byte MAC.
pub fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut k = [0u8; BLOCK];
    if key.len() > BLOCK {
        let h = sha256(key);
        k[..32].copy_from_slice(&h);
    } else {
        k[..key.len()].copy_from_slice(key);
    }

    let mut ipad = [0x36u8; BLOCK];
    let mut opad = [0x5cu8; BLOCK];
    for i in 0..BLOCK {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }

    let mut inner = Vec::with_capacity(BLOCK + data.len());
    inner.extend_from_slice(&ipad);
    inner.extend_from_slice(data);
    let inner_hash = sha256(&inner);

    let mut outer = Vec::with_capacity(BLOCK + 32);
    outer.extend_from_slice(&opad);
    outer.extend_from_slice(&inner_hash);
    sha256(&outer)
}

/// HMAC-SHA-384 → 48-byte MAC.
pub fn hmac_sha384(key: &[u8], data: &[u8]) -> [u8; 48] {
    const BLOCK: usize = 128;
    let mut k = [0u8; BLOCK];
    if key.len() > BLOCK {
        let h = sha384(key);
        k[..48].copy_from_slice(&h);
    } else {
        k[..key.len()].copy_from_slice(key);
    }

    let mut ipad = [0x36u8; BLOCK];
    let mut opad = [0x5cu8; BLOCK];
    for i in 0..BLOCK {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }

    let mut inner = Vec::with_capacity(BLOCK + data.len());
    inner.extend_from_slice(&ipad);
    inner.extend_from_slice(data);
    let inner_hash = sha384(&inner);

    let mut outer = Vec::with_capacity(BLOCK + 48);
    outer.extend_from_slice(&opad);
    outer.extend_from_slice(&inner_hash);
    sha384(&outer)
}
