//! HKDF (HMAC-based Key Derivation Function) — RFC 5869
//!
//! Also provides TLS 1.3–specific `HKDF-Expand-Label` and `Derive-Secret`
//! as defined in RFC 8446 §7.1.

use alloc::vec::Vec;
use super::hmac::{hmac_sha256, hmac_sha384};

// ── Generic HKDF (SHA-256) ──────────────────────────────────

/// HKDF-Extract(salt, IKM) → 32-byte PRK.
pub fn hkdf_extract(salt: &[u8], ikm: &[u8]) -> [u8; 32] {
    let s: &[u8] = if salt.is_empty() { &[0u8; 32] } else { salt };
    hmac_sha256(s, ikm)
}

/// HKDF-Expand(PRK, info, L) → L bytes of OKM.
pub fn hkdf_expand(prk: &[u8], info: &[u8], length: usize) -> Vec<u8> {
    let hash_len = 32;
    let n = (length + hash_len - 1) / hash_len;
    let mut okm = Vec::with_capacity(length);
    let mut t: Vec<u8> = Vec::new();

    for i in 1..=n {
        let mut input = Vec::with_capacity(t.len() + info.len() + 1);
        input.extend_from_slice(&t);
        input.extend_from_slice(info);
        input.push(i as u8);
        let h = hmac_sha256(prk, &input);
        t = h.to_vec();
        okm.extend_from_slice(&t);
    }
    okm.truncate(length);
    okm
}

// ── Generic HKDF (SHA-384) ─────────────────────────────────

/// HKDF-Extract-SHA-384.
pub fn hkdf_extract_384(salt: &[u8], ikm: &[u8]) -> [u8; 48] {
    let s: &[u8] = if salt.is_empty() { &[0u8; 48] } else { salt };
    hmac_sha384(s, ikm)
}

/// HKDF-Expand-SHA-384.
pub fn hkdf_expand_384(prk: &[u8], info: &[u8], length: usize) -> Vec<u8> {
    let hash_len = 48;
    let n = (length + hash_len - 1) / hash_len;
    let mut okm = Vec::with_capacity(length);
    let mut t: Vec<u8> = Vec::new();

    for i in 1..=n {
        let mut input = Vec::with_capacity(t.len() + info.len() + 1);
        input.extend_from_slice(&t);
        input.extend_from_slice(info);
        input.push(i as u8);
        let h = hmac_sha384(prk, &input);
        t = h.to_vec();
        okm.extend_from_slice(&t);
    }
    okm.truncate(length);
    okm
}

// ── TLS 1.3 helpers (RFC 8446 §7.1) ────────────────────────

/// HKDF-Expand-Label(Secret, Label, Context, Length)
///
/// ```text
/// struct {
///     uint16 length;
///     opaque label<7..255> = "tls13 " + Label;
///     opaque context<0..255>;
/// } HkdfLabel;
/// ```
pub fn hkdf_expand_label(
    secret: &[u8],
    label: &[u8],
    context: &[u8],
    length: usize,
) -> Vec<u8> {
    let mut hkdf_label = Vec::new();
    // uint16 length
    hkdf_label.push((length >> 8) as u8);
    hkdf_label.push(length as u8);
    // opaque label<7..255> = "tls13 " + Label
    let full_len = 6 + label.len();
    hkdf_label.push(full_len as u8);
    hkdf_label.extend_from_slice(b"tls13 ");
    hkdf_label.extend_from_slice(label);
    // opaque context<0..255>
    hkdf_label.push(context.len() as u8);
    hkdf_label.extend_from_slice(context);

    hkdf_expand(secret, &hkdf_label, length)
}

/// Derive-Secret(Secret, Label, Messages) =
///   HKDF-Expand-Label(Secret, Label, Hash(Messages), Hash.length)
///
/// `transcript_hash` is the already-computed SHA-256 hash of the transcript.
pub fn derive_secret(
    secret: &[u8],
    label: &[u8],
    transcript_hash: &[u8],
) -> Vec<u8> {
    hkdf_expand_label(secret, label, transcript_hash, 32)
}
