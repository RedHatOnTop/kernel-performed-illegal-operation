//! RSA PKCS#1 v1.5 signature verification
//!
//! Supports common public exponents (65537) and key sizes (2048, 4096).
//! Only verification is implemented (no signing — kernel only verifies certs).

#![allow(dead_code)]
use super::sha::{sha256, sha384, sha512};
use alloc::vec::Vec;

// ── Big integer (variable-length, little-endian u64 limbs) ──

#[derive(Clone)]
struct BigUint {
    limbs: Vec<u64>,
}

impl BigUint {
    fn zero() -> Self {
        BigUint { limbs: Vec::new() }
    }
    fn one() -> Self {
        BigUint {
            limbs: alloc::vec![1],
        }
    }

    fn from_be_bytes(bytes: &[u8]) -> Self {
        // Skip leading zeros
        let start = bytes.iter().position(|&b| b != 0).unwrap_or(bytes.len());
        let bytes = &bytes[start..];
        if bytes.is_empty() {
            return Self::zero();
        }

        let n_limbs = (bytes.len() + 7) / 8;
        let mut limbs = alloc::vec![0u64; n_limbs];

        for (i, &byte) in bytes.iter().rev().enumerate() {
            limbs[i / 8] |= (byte as u64) << ((i % 8) * 8);
        }
        let mut r = BigUint { limbs };
        r.trim();
        r
    }

    fn to_be_bytes(&self, min_len: usize) -> Vec<u8> {
        let bit_len = self.bit_len();
        let byte_len = (bit_len + 7) / 8;
        let out_len = byte_len.max(min_len);
        let mut out = alloc::vec![0u8; out_len];

        for i in 0..byte_len {
            let limb_idx = i / 8;
            let byte_idx = i % 8;
            if limb_idx < self.limbs.len() {
                out[out_len - 1 - i] = (self.limbs[limb_idx] >> (byte_idx * 8)) as u8;
            }
        }
        out
    }

    fn bit_len(&self) -> usize {
        if self.limbs.is_empty() {
            return 0;
        }
        let top = self.limbs.len() - 1;
        (top * 64) + (64 - self.limbs[top].leading_zeros() as usize)
    }

    fn is_zero(&self) -> bool {
        self.limbs.is_empty() || self.limbs.iter().all(|&l| l == 0)
    }

    fn trim(&mut self) {
        while self.limbs.last() == Some(&0) {
            self.limbs.pop();
        }
    }

    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        let al = self.limbs.len();
        let bl = other.limbs.len();
        if al != bl {
            return al.cmp(&bl);
        }
        for i in (0..al).rev() {
            if self.limbs[i] != other.limbs[i] {
                return self.limbs[i].cmp(&other.limbs[i]);
            }
        }
        core::cmp::Ordering::Equal
    }

    fn mul(&self, other: &Self) -> Self {
        if self.is_zero() || other.is_zero() {
            return Self::zero();
        }
        let n = self.limbs.len() + other.limbs.len();
        let mut limbs = alloc::vec![0u64; n];

        for i in 0..self.limbs.len() {
            let mut carry = 0u128;
            for j in 0..other.limbs.len() {
                carry += limbs[i + j] as u128 + (self.limbs[i] as u128) * (other.limbs[j] as u128);
                limbs[i + j] = carry as u64;
                carry >>= 64;
            }
            limbs[i + other.limbs.len()] = carry as u64;
        }

        let mut r = BigUint { limbs };
        r.trim();
        r
    }

    /// Modular exponentiation: self^exp mod modulus.
    /// Uses square-and-multiply (left-to-right binary method).
    fn mod_pow(&self, exp: &Self, modulus: &Self) -> Self {
        if modulus.is_zero() {
            return Self::zero();
        }
        let mut result = Self::one();
        let mut base = self.mod_reduce(modulus);

        let exp_bits = exp.bit_len();
        for i in 0..exp_bits {
            let limb_idx = i / 64;
            let bit_idx = i % 64;
            if limb_idx < exp.limbs.len() && (exp.limbs[limb_idx] >> bit_idx) & 1 == 1 {
                result = result.mul(&base).mod_reduce(modulus);
            }
            base = base.mul(&base).mod_reduce(modulus);
        }
        result
    }

    /// Reduce self mod modulus (simple division-based).
    fn mod_reduce(&self, modulus: &Self) -> Self {
        if self.cmp(modulus) == core::cmp::Ordering::Less {
            return self.clone();
        }
        // Use long division
        let (_, rem) = self.div_rem(modulus);
        rem
    }

    fn div_rem(&self, divisor: &Self) -> (Self, Self) {
        if divisor.is_zero() {
            return (Self::zero(), Self::zero());
        }
        if self.cmp(divisor) == core::cmp::Ordering::Less {
            return (Self::zero(), self.clone());
        }

        let mut remainder = self.clone();
        let mut quotient_limbs = alloc::vec![0u64; self.limbs.len()];

        let d_bits = divisor.bit_len();
        let r_bits = self.bit_len();

        if d_bits == 0 {
            return (Self::zero(), self.clone());
        }

        for i in (0..=(r_bits - d_bits)).rev() {
            let shifted = divisor.shl(i);
            if remainder.cmp(&shifted) != core::cmp::Ordering::Less {
                remainder = remainder.sub(&shifted);
                quotient_limbs[i / 64] |= 1u64 << (i % 64);
            }
        }

        let mut q = BigUint {
            limbs: quotient_limbs,
        };
        q.trim();
        remainder.trim();
        (q, remainder)
    }

    fn sub(&self, other: &Self) -> Self {
        let n = self.limbs.len().max(other.limbs.len());
        let mut limbs = alloc::vec![0u64; n];
        let mut borrow = 0i128;

        for i in 0..n {
            let a = if i < self.limbs.len() {
                self.limbs[i] as i128
            } else {
                0
            };
            let b = if i < other.limbs.len() {
                other.limbs[i] as i128
            } else {
                0
            };
            let diff = a - b - borrow;
            if diff < 0 {
                limbs[i] = (diff + (1i128 << 64)) as u64;
                borrow = 1;
            } else {
                limbs[i] = diff as u64;
                borrow = 0;
            }
        }

        let mut r = BigUint { limbs };
        r.trim();
        r
    }

    fn shl(&self, bits: usize) -> Self {
        if self.is_zero() {
            return Self::zero();
        }
        let word_shift = bits / 64;
        let bit_shift = bits % 64;

        let new_len = self.limbs.len() + word_shift + 1;
        let mut limbs = alloc::vec![0u64; new_len];

        for i in 0..self.limbs.len() {
            limbs[i + word_shift] |= self.limbs[i] << bit_shift;
            if bit_shift > 0 && i + word_shift + 1 < new_len {
                limbs[i + word_shift + 1] |= self.limbs[i] >> (64 - bit_shift);
            }
        }

        let mut r = BigUint { limbs };
        r.trim();
        r
    }
}

// ── RSA PKCS#1 v1.5 verification ────────────────────────────

/// Verify an RSA PKCS#1 v1.5 signature.
///
/// * `modulus`: RSA modulus n (big-endian bytes)
/// * `exponent`: RSA public exponent e (big-endian bytes, typically 65537)
/// * `signature`: signature bytes (big-endian, same length as modulus)
/// * `hash`: message hash (SHA-256 or SHA-384)
/// * `hash_algo`: hash algorithm identifier ("sha256", "sha384", "sha512")
///
/// Returns `true` if the signature is valid.
pub fn rsa_pkcs1_verify(
    modulus: &[u8],
    exponent: &[u8],
    signature: &[u8],
    hash: &[u8],
    hash_algo: &str,
) -> bool {
    let n = BigUint::from_be_bytes(modulus);
    let e = BigUint::from_be_bytes(exponent);
    let s = BigUint::from_be_bytes(signature);

    // Compute m = s^e mod n
    let m = s.mod_pow(&e, &n);
    let em = m.to_be_bytes(modulus.len());

    // Verify EMSA-PKCS1-v1_5 encoding
    verify_pkcs1_encoding(&em, hash, hash_algo)
}

/// Verify EMSA-PKCS1-v1_5 encoding.
///
/// Format: 0x00 || 0x01 || PS || 0x00 || DigestInfo
/// where PS = 0xFF bytes, len(PS) >= 8
fn verify_pkcs1_encoding(em: &[u8], hash: &[u8], hash_algo: &str) -> bool {
    if em.len() < 11 {
        return false;
    }
    if em[0] != 0x00 || em[1] != 0x01 {
        return false;
    }

    // Find 0x00 separator after PS
    let mut sep_pos = None;
    for i in 2..em.len() {
        if em[i] == 0x00 {
            sep_pos = Some(i);
            break;
        }
        if em[i] != 0xff {
            return false;
        }
    }

    let sep = match sep_pos {
        Some(p) if p >= 10 => p, // PS must be at least 8 bytes
        _ => return false,
    };

    let digest_info = &em[sep + 1..];

    // Build expected DigestInfo
    let expected_di = match hash_algo {
        "sha256" => build_digest_info_sha256(hash),
        "sha384" => build_digest_info_sha384(hash),
        "sha512" => build_digest_info_sha512(hash),
        _ => return false,
    };

    // Constant-time compare
    if digest_info.len() != expected_di.len() {
        return false;
    }
    let mut diff = 0u8;
    for i in 0..digest_info.len() {
        diff |= digest_info[i] ^ expected_di[i];
    }
    diff == 0
}

/// Build DigestInfo for SHA-256: SEQUENCE { AlgorithmIdentifier, OCTET STRING hash }
fn build_digest_info_sha256(hash: &[u8]) -> Vec<u8> {
    // DER encoding of DigestInfo for SHA-256:
    // 30 31 30 0d 06 09 60 86 48 01 65 03 04 02 01 05 00 04 20 <hash>
    let prefix: &[u8] = &[
        0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01,
        0x05, 0x00, 0x04, 0x20,
    ];
    let mut di = Vec::from(prefix);
    di.extend_from_slice(hash);
    di
}

fn build_digest_info_sha384(hash: &[u8]) -> Vec<u8> {
    let prefix: &[u8] = &[
        0x30, 0x41, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x02,
        0x05, 0x00, 0x04, 0x30,
    ];
    let mut di = Vec::from(prefix);
    di.extend_from_slice(hash);
    di
}

fn build_digest_info_sha512(hash: &[u8]) -> Vec<u8> {
    let prefix: &[u8] = &[
        0x30, 0x51, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x03,
        0x05, 0x00, 0x04, 0x40,
    ];
    let mut di = Vec::from(prefix);
    di.extend_from_slice(hash);
    di
}
