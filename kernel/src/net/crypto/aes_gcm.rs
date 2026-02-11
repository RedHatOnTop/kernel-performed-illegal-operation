//! AES-GCM Authenticated Encryption — NIST SP 800-38D
//!
//! Provides AES-128-GCM and AES-256-GCM with 12-byte nonce, 16-byte tag.

use alloc::vec::Vec;
use super::aes::{aes128_key_schedule, aes256_key_schedule, aes128_encrypt, aes256_encrypt};

// ── GF(2^128) multiplication for GHASH ──────────────────────

/// Multiply two 128-bit GF(2^128) elements.
/// Uses the GCM polynomial R = x^128 + x^7 + x^2 + x + 1.
/// Both a and b are big-endian 16-byte blocks.
fn gf128_mul(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut z = [0u8; 16];
    let mut v = *a;

    for i in 0..128 {
        let byte_idx = i / 8;
        let bit_idx = 7 - (i % 8);
        if (b[byte_idx] >> bit_idx) & 1 == 1 {
            for j in 0..16 { z[j] ^= v[j]; }
        }
        // Shift V right by 1, reduce if needed
        let lsb = v[15] & 1;
        for j in (1..16).rev() {
            v[j] = (v[j] >> 1) | (v[j - 1] << 7);
        }
        v[0] >>= 1;
        if lsb == 1 {
            v[0] ^= 0xe1; // R = 0xe1 || 0^120
        }
    }
    z
}

// ── GHASH ───────────────────────────────────────────────────

/// GHASH(H, A, C) — compute authentication tag from H, AAD, and ciphertext.
fn ghash(h: &[u8; 16], aad: &[u8], ciphertext: &[u8]) -> [u8; 16] {
    let mut y = [0u8; 16];

    // Process AAD blocks
    ghash_update(&mut y, h, aad);
    // Process ciphertext blocks
    ghash_update(&mut y, h, ciphertext);

    // Final block: len(A) || len(C) in bits, both as 64-bit big-endian
    let mut len_block = [0u8; 16];
    let aad_bits = (aad.len() as u64) * 8;
    let ct_bits = (ciphertext.len() as u64) * 8;
    len_block[..8].copy_from_slice(&aad_bits.to_be_bytes());
    len_block[8..].copy_from_slice(&ct_bits.to_be_bytes());
    for j in 0..16 { y[j] ^= len_block[j]; }
    y = gf128_mul(&y, h);

    y
}

fn ghash_update(y: &mut [u8; 16], h: &[u8; 16], data: &[u8]) {
    let mut off = 0;
    while off < data.len() {
        let take = (data.len() - off).min(16);
        for j in 0..take { y[j] ^= data[off + j]; }
        *y = gf128_mul(y, h);
        off += 16;
    }
}

// ── AES-CTR ─────────────────────────────────────────────────

/// Increment the last 4 bytes of `ctr` as a big-endian 32-bit counter.
fn inc32(ctr: &mut [u8; 16]) {
    let c = u32::from_be_bytes([ctr[12], ctr[13], ctr[14], ctr[15]]);
    let nc = c.wrapping_add(1);
    ctr[12..16].copy_from_slice(&nc.to_be_bytes());
}

// ── AES-128-GCM ─────────────────────────────────────────────

/// AES-128-GCM seal (encrypt + authenticate).
///
/// * `key`: 16 bytes
/// * `nonce`: 12 bytes
/// * `aad`: additional authenticated data
/// * `plaintext`: data to encrypt
///
/// Returns `(ciphertext, tag)` where tag is 16 bytes.
pub fn aes128_gcm_seal(
    key: &[u8; 16],
    nonce: &[u8; 12],
    aad: &[u8],
    plaintext: &[u8],
) -> (Vec<u8>, [u8; 16]) {
    let rk = aes128_key_schedule(key);

    // H = AES_K(0^128)
    let mut h_block = [0u8; 16];
    aes128_encrypt(&mut h_block, &rk);

    // J0 = nonce || 0x00000001
    let mut j0 = [0u8; 16];
    j0[..12].copy_from_slice(nonce);
    j0[15] = 1;

    // Encrypt plaintext with AES-CTR starting at J0 + 1
    let mut ctr = j0;
    inc32(&mut ctr);
    let mut ciphertext = Vec::with_capacity(plaintext.len());

    let mut off = 0;
    while off < plaintext.len() {
        let mut block = ctr;
        aes128_encrypt(&mut block, &rk);
        let take = (plaintext.len() - off).min(16);
        for j in 0..take {
            ciphertext.push(plaintext[off + j] ^ block[j]);
        }
        off += take;
        inc32(&mut ctr);
    }

    // GHASH
    let s = ghash(&h_block, aad, &ciphertext);

    // Tag = GHASH XOR AES_K(J0)
    let mut tag_mask = j0;
    aes128_encrypt(&mut tag_mask, &rk);
    let mut tag = [0u8; 16];
    for j in 0..16 { tag[j] = s[j] ^ tag_mask[j]; }

    (ciphertext, tag)
}

/// AES-128-GCM open (decrypt + verify).
///
/// Returns `None` if authentication fails.
pub fn aes128_gcm_open(
    key: &[u8; 16],
    nonce: &[u8; 12],
    aad: &[u8],
    ciphertext: &[u8],
    tag: &[u8; 16],
) -> Option<Vec<u8>> {
    let rk = aes128_key_schedule(key);

    let mut h_block = [0u8; 16];
    aes128_encrypt(&mut h_block, &rk);

    let mut j0 = [0u8; 16];
    j0[..12].copy_from_slice(nonce);
    j0[15] = 1;

    // Verify tag first
    let s = ghash(&h_block, aad, ciphertext);
    let mut tag_mask = j0;
    aes128_encrypt(&mut tag_mask, &rk);
    let mut computed_tag = [0u8; 16];
    for j in 0..16 { computed_tag[j] = s[j] ^ tag_mask[j]; }

    // Constant-time compare
    let mut diff = 0u8;
    for j in 0..16 { diff |= computed_tag[j] ^ tag[j]; }
    if diff != 0 { return None; }

    // Decrypt
    let mut ctr = j0;
    inc32(&mut ctr);
    let mut plaintext = Vec::with_capacity(ciphertext.len());
    let mut off = 0;
    while off < ciphertext.len() {
        let mut block = ctr;
        aes128_encrypt(&mut block, &rk);
        let take = (ciphertext.len() - off).min(16);
        for j in 0..take {
            plaintext.push(ciphertext[off + j] ^ block[j]);
        }
        off += take;
        inc32(&mut ctr);
    }

    Some(plaintext)
}

// ── AES-256-GCM ─────────────────────────────────────────────

/// AES-256-GCM seal.
pub fn aes256_gcm_seal(
    key: &[u8; 32],
    nonce: &[u8; 12],
    aad: &[u8],
    plaintext: &[u8],
) -> (Vec<u8>, [u8; 16]) {
    let rk = aes256_key_schedule(key);

    let mut h_block = [0u8; 16];
    aes256_encrypt(&mut h_block, &rk);

    let mut j0 = [0u8; 16];
    j0[..12].copy_from_slice(nonce);
    j0[15] = 1;

    let mut ctr = j0;
    inc32(&mut ctr);
    let mut ciphertext = Vec::with_capacity(plaintext.len());

    let mut off = 0;
    while off < plaintext.len() {
        let mut block = ctr;
        aes256_encrypt(&mut block, &rk);
        let take = (plaintext.len() - off).min(16);
        for j in 0..take {
            ciphertext.push(plaintext[off + j] ^ block[j]);
        }
        off += take;
        inc32(&mut ctr);
    }

    let s = ghash(&h_block, aad, &ciphertext);
    let mut tag_mask = j0;
    aes256_encrypt(&mut tag_mask, &rk);
    let mut tag = [0u8; 16];
    for j in 0..16 { tag[j] = s[j] ^ tag_mask[j]; }

    (ciphertext, tag)
}

/// AES-256-GCM open.
pub fn aes256_gcm_open(
    key: &[u8; 32],
    nonce: &[u8; 12],
    aad: &[u8],
    ciphertext: &[u8],
    tag: &[u8; 16],
) -> Option<Vec<u8>> {
    let rk = aes256_key_schedule(key);

    let mut h_block = [0u8; 16];
    aes256_encrypt(&mut h_block, &rk);

    let mut j0 = [0u8; 16];
    j0[..12].copy_from_slice(nonce);
    j0[15] = 1;

    let s = ghash(&h_block, aad, ciphertext);
    let mut tag_mask = j0;
    aes256_encrypt(&mut tag_mask, &rk);
    let mut computed_tag = [0u8; 16];
    for j in 0..16 { computed_tag[j] = s[j] ^ tag_mask[j]; }

    let mut diff = 0u8;
    for j in 0..16 { diff |= computed_tag[j] ^ tag[j]; }
    if diff != 0 { return None; }

    let mut ctr = j0;
    inc32(&mut ctr);
    let mut plaintext = Vec::with_capacity(ciphertext.len());
    let mut off = 0;
    while off < ciphertext.len() {
        let mut block = ctr;
        aes256_encrypt(&mut block, &rk);
        let take = (ciphertext.len() - off).min(16);
        for j in 0..take {
            plaintext.push(ciphertext[off + j] ^ block[j]);
        }
        off += take;
        inc32(&mut ctr);
    }

    Some(plaintext)
}
