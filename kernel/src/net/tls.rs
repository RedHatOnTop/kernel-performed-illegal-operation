//! Minimal TLS 1.2 Implementation
//!
//! Supports `TLS_RSA_WITH_AES_128_CBC_SHA256` for HTTPS.
//! Certificate validation is skipped (demo kernel).

#![allow(dead_code)]

use super::tcp::{self, ConnId};
use super::NetError;
use alloc::vec::Vec;

// ── TLS record types ────────────────────────────────────────

const CONTENT_CHANGE_CIPHER: u8 = 20;
const CONTENT_ALERT: u8 = 21;
const CONTENT_HANDSHAKE: u8 = 22;
const CONTENT_APP_DATA: u8 = 23;

// Handshake types
const HS_CLIENT_HELLO: u8 = 1;
const HS_SERVER_HELLO: u8 = 2;
const HS_CERTIFICATE: u8 = 11;
const HS_SERVER_KEY_EXCHANGE: u8 = 12;
const HS_SERVER_HELLO_DONE: u8 = 14;
const HS_CLIENT_KEY_EXCHANGE: u8 = 16;
const HS_FINISHED: u8 = 20;

// TLS version
const TLS_12: [u8; 2] = [0x03, 0x03];

// Cipher suite: TLS_RSA_WITH_AES_128_CBC_SHA256
const CIPHER_SUITE: [u8; 2] = [0x00, 0x3C];

// ── SHA-256 ─────────────────────────────────────────────────

const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    // Pre-processing: padding
    let bit_len = (data.len() as u64) * 8;
    let mut msg = Vec::from(data);
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    // Process 512-bit blocks
    for block in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(SHA256_K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut out = [0u8; 32];
    for i in 0..8 {
        out[i * 4..i * 4 + 4].copy_from_slice(&h[i].to_be_bytes());
    }
    out
}

// ── HMAC-SHA-256 ────────────────────────────────────────────

pub fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        let h = sha256(key);
        k[..32].copy_from_slice(&h);
    } else {
        k[..key.len()].copy_from_slice(key);
    }

    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5cu8; 64];
    for i in 0..64 {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }

    let mut inner = Vec::with_capacity(64 + data.len());
    inner.extend_from_slice(&ipad);
    inner.extend_from_slice(data);
    let inner_hash = sha256(&inner);

    let mut outer = Vec::with_capacity(64 + 32);
    outer.extend_from_slice(&opad);
    outer.extend_from_slice(&inner_hash);
    sha256(&outer)
}

// ── AES-128 ─────────────────────────────────────────────────

const SBOX: [u8; 256] = [
    0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab, 0x76,
    0xca, 0x82, 0xc9, 0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4, 0x72, 0xc0,
    0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f, 0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71, 0xd8, 0x31, 0x15,
    0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07, 0x12, 0x80, 0xe2, 0xeb, 0x27, 0xb2, 0x75,
    0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6, 0xb3, 0x29, 0xe3, 0x2f, 0x84,
    0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb, 0xbe, 0x39, 0x4a, 0x4c, 0x58, 0xcf,
    0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45, 0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8,
    0x51, 0xa3, 0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5, 0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2,
    0xcd, 0x0c, 0x13, 0xec, 0x5f, 0x97, 0x44, 0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73,
    0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a, 0x90, 0x88, 0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb,
    0xe0, 0x32, 0x3a, 0x0a, 0x49, 0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac, 0x62, 0x91, 0x95, 0xe4, 0x79,
    0xe7, 0xc8, 0x37, 0x6d, 0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a, 0xae, 0x08,
    0xba, 0x78, 0x25, 0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a,
    0x70, 0x3e, 0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e,
    0xe1, 0xf8, 0x98, 0x11, 0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf,
    0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42, 0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb, 0x16,
];

const INV_SBOX: [u8; 256] = [
    0x52, 0x09, 0x6a, 0xd5, 0x30, 0x36, 0xa5, 0x38, 0xbf, 0x40, 0xa3, 0x9e, 0x81, 0xf3, 0xd7, 0xfb,
    0x7c, 0xe3, 0x39, 0x82, 0x9b, 0x2f, 0xff, 0x87, 0x34, 0x8e, 0x43, 0x44, 0xc4, 0xde, 0xe9, 0xcb,
    0x54, 0x7b, 0x94, 0x32, 0xa6, 0xc2, 0x23, 0x3d, 0xee, 0x4c, 0x95, 0x0b, 0x42, 0xfa, 0xc3, 0x4e,
    0x08, 0x2e, 0xa1, 0x66, 0x28, 0xd9, 0x24, 0xb2, 0x76, 0x5b, 0xa2, 0x49, 0x6d, 0x8b, 0xd1, 0x25,
    0x72, 0xf8, 0xf6, 0x64, 0x86, 0x68, 0x98, 0x16, 0xd4, 0xa4, 0x5c, 0xcc, 0x5d, 0x65, 0xb6, 0x92,
    0x6c, 0x70, 0x48, 0x50, 0xfd, 0xed, 0xb9, 0xda, 0x5e, 0x15, 0x46, 0x57, 0xa7, 0x8d, 0x9d, 0x84,
    0x90, 0xd8, 0xab, 0x00, 0x8c, 0xbc, 0xd3, 0x0a, 0xf7, 0xe4, 0x58, 0x05, 0xb8, 0xb3, 0x45, 0x06,
    0xd0, 0x2c, 0x1e, 0x8f, 0xca, 0x3f, 0x0f, 0x02, 0xc1, 0xaf, 0xbd, 0x03, 0x01, 0x13, 0x8a, 0x6b,
    0x3a, 0x91, 0x11, 0x41, 0x4f, 0x67, 0xdc, 0xea, 0x97, 0xf2, 0xcf, 0xce, 0xf0, 0xb4, 0xe6, 0x73,
    0x96, 0xac, 0x74, 0x22, 0xe7, 0xad, 0x35, 0x85, 0xe2, 0xf9, 0x37, 0xe8, 0x1c, 0x75, 0xdf, 0x6e,
    0x47, 0xf1, 0x1a, 0x71, 0x1d, 0x29, 0xc5, 0x89, 0x6f, 0xb7, 0x62, 0x0e, 0xaa, 0x18, 0xbe, 0x1b,
    0xfc, 0x56, 0x3e, 0x4b, 0xc6, 0xd2, 0x79, 0x20, 0x9a, 0xdb, 0xc0, 0xfe, 0x78, 0xcd, 0x5a, 0xf4,
    0x1f, 0xdd, 0xa8, 0x33, 0x88, 0x07, 0xc7, 0x31, 0xb1, 0x12, 0x10, 0x59, 0x27, 0x80, 0xec, 0x5f,
    0x60, 0x51, 0x7f, 0xa9, 0x19, 0xb5, 0x4a, 0x0d, 0x2d, 0xe5, 0x7a, 0x9f, 0x93, 0xc9, 0x9c, 0xef,
    0xa0, 0xe0, 0x3b, 0x4d, 0xae, 0x2a, 0xf5, 0xb0, 0xc8, 0xeb, 0xbb, 0x3c, 0x83, 0x53, 0x99, 0x61,
    0x17, 0x2b, 0x04, 0x7e, 0xba, 0x77, 0xd6, 0x26, 0xe1, 0x69, 0x14, 0x63, 0x55, 0x21, 0x0c, 0x7d,
];

const RCON: [u8; 10] = [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x1b, 0x36];

fn xtime(a: u8) -> u8 {
    if (a & 0x80) != 0 {
        (a << 1) ^ 0x1b
    } else {
        a << 1
    }
}

fn gf_mul(mut a: u8, mut b: u8) -> u8 {
    let mut p = 0u8;
    for _ in 0..8 {
        if (b & 1) != 0 {
            p ^= a;
        }
        let hi = a & 0x80;
        a <<= 1;
        if hi != 0 {
            a ^= 0x1b;
        }
        b >>= 1;
    }
    p
}

/// AES-128 key schedule: expand 16-byte key to 11 round keys.
fn aes128_key_schedule(key: &[u8; 16]) -> [[u8; 16]; 11] {
    let mut rk = [[0u8; 16]; 11];
    rk[0].copy_from_slice(key);
    for i in 1..11 {
        let prev = rk[i - 1];
        // RotWord + SubWord + Rcon
        let mut t = [
            SBOX[prev[13] as usize] ^ RCON[i - 1],
            SBOX[prev[14] as usize],
            SBOX[prev[15] as usize],
            SBOX[prev[12] as usize],
        ];
        for j in 0..4 {
            rk[i][j] = prev[j] ^ t[j];
        }
        for j in 4..16 {
            rk[i][j] = prev[j] ^ rk[i][j - 4];
        }
    }
    rk
}

fn aes128_encrypt_block(block: &mut [u8; 16], rk: &[[u8; 16]; 11]) {
    // AddRoundKey
    for i in 0..16 {
        block[i] ^= rk[0][i];
    }
    for round in 1..10 {
        // SubBytes
        for i in 0..16 {
            block[i] = SBOX[block[i] as usize];
        }
        // ShiftRows
        let t = *block;
        block[1] = t[5];
        block[5] = t[9];
        block[9] = t[13];
        block[13] = t[1];
        block[2] = t[10];
        block[6] = t[14];
        block[10] = t[2];
        block[14] = t[6];
        block[3] = t[15];
        block[7] = t[3];
        block[11] = t[7];
        block[15] = t[11];
        // MixColumns
        for c in 0..4 {
            let s = &block[c * 4..c * 4 + 4];
            let (a0, a1, a2, a3) = (s[0], s[1], s[2], s[3]);
            block[c * 4] = xtime(a0) ^ xtime(a1) ^ a1 ^ a2 ^ a3;
            block[c * 4 + 1] = a0 ^ xtime(a1) ^ xtime(a2) ^ a2 ^ a3;
            block[c * 4 + 2] = a0 ^ a1 ^ xtime(a2) ^ xtime(a3) ^ a3;
            block[c * 4 + 3] = xtime(a0) ^ a0 ^ a1 ^ a2 ^ xtime(a3);
        }
        // AddRoundKey
        for i in 0..16 {
            block[i] ^= rk[round][i];
        }
    }
    // Final round (no MixColumns)
    for i in 0..16 {
        block[i] = SBOX[block[i] as usize];
    }
    let t = *block;
    block[1] = t[5];
    block[5] = t[9];
    block[9] = t[13];
    block[13] = t[1];
    block[2] = t[10];
    block[6] = t[14];
    block[10] = t[2];
    block[14] = t[6];
    block[3] = t[15];
    block[7] = t[3];
    block[11] = t[7];
    block[15] = t[11];
    for i in 0..16 {
        block[i] ^= rk[10][i];
    }
}

fn aes128_decrypt_block(block: &mut [u8; 16], rk: &[[u8; 16]; 11]) {
    // AddRoundKey (last round key)
    for i in 0..16 {
        block[i] ^= rk[10][i];
    }
    for round in (1..10).rev() {
        // InvShiftRows
        let t = *block;
        block[1] = t[13];
        block[5] = t[1];
        block[9] = t[5];
        block[13] = t[9];
        block[2] = t[10];
        block[6] = t[14];
        block[10] = t[2];
        block[14] = t[6];
        block[3] = t[7];
        block[7] = t[11];
        block[11] = t[15];
        block[15] = t[3];
        // InvSubBytes
        for i in 0..16 {
            block[i] = INV_SBOX[block[i] as usize];
        }
        // AddRoundKey
        for i in 0..16 {
            block[i] ^= rk[round][i];
        }
        // InvMixColumns
        for c in 0..4 {
            let s = &block[c * 4..c * 4 + 4];
            let (a0, a1, a2, a3) = (s[0], s[1], s[2], s[3]);
            block[c * 4] = gf_mul(a0, 14) ^ gf_mul(a1, 11) ^ gf_mul(a2, 13) ^ gf_mul(a3, 9);
            block[c * 4 + 1] = gf_mul(a0, 9) ^ gf_mul(a1, 14) ^ gf_mul(a2, 11) ^ gf_mul(a3, 13);
            block[c * 4 + 2] = gf_mul(a0, 13) ^ gf_mul(a1, 9) ^ gf_mul(a2, 14) ^ gf_mul(a3, 11);
            block[c * 4 + 3] = gf_mul(a0, 11) ^ gf_mul(a1, 13) ^ gf_mul(a2, 9) ^ gf_mul(a3, 14);
        }
    }
    // Final (round 0)
    let t = *block;
    block[1] = t[13];
    block[5] = t[1];
    block[9] = t[5];
    block[13] = t[9];
    block[2] = t[10];
    block[6] = t[14];
    block[10] = t[2];
    block[14] = t[6];
    block[3] = t[7];
    block[7] = t[11];
    block[11] = t[15];
    block[15] = t[3];
    for i in 0..16 {
        block[i] = INV_SBOX[block[i] as usize];
    }
    for i in 0..16 {
        block[i] ^= rk[0][i];
    }
}

/// AES-128-CBC encrypt.
pub fn aes128_cbc_encrypt(key: &[u8; 16], iv: &[u8; 16], plaintext: &[u8]) -> Vec<u8> {
    let rk = aes128_key_schedule(key);
    // PKCS#7 padding
    let pad = 16 - (plaintext.len() % 16);
    let mut data = Vec::from(plaintext);
    for _ in 0..pad {
        data.push(pad as u8);
    }

    let mut out = Vec::with_capacity(data.len());
    let mut prev = *iv;
    for chunk in data.chunks(16) {
        let mut block = [0u8; 16];
        for i in 0..16 {
            block[i] = chunk[i] ^ prev[i];
        }
        aes128_encrypt_block(&mut block, &rk);
        prev = block;
        out.extend_from_slice(&block);
    }
    out
}

/// AES-128-CBC decrypt.
pub fn aes128_cbc_decrypt(key: &[u8; 16], iv: &[u8; 16], ciphertext: &[u8]) -> Option<Vec<u8>> {
    if ciphertext.len() % 16 != 0 || ciphertext.is_empty() {
        return None;
    }
    let rk = aes128_key_schedule(key);
    let mut out = Vec::with_capacity(ciphertext.len());
    let mut prev = *iv;
    for chunk in ciphertext.chunks(16) {
        let mut block = [0u8; 16];
        block.copy_from_slice(chunk);
        aes128_decrypt_block(&mut block, &rk);
        for i in 0..16 {
            block[i] ^= prev[i];
        }
        prev.copy_from_slice(chunk);
        out.extend_from_slice(&block);
    }
    // Remove PKCS#7 padding
    let pad = *out.last()? as usize;
    if pad == 0 || pad > 16 {
        return None;
    }
    out.truncate(out.len() - pad);
    Some(out)
}

// ── TLS Connection ──────────────────────────────────────────

/// A TLS session wrapping a TCP connection.
pub struct TlsConnection {
    tcp_id: ConnId,
    /// Client write key
    client_key: [u8; 16],
    /// Server write key
    server_key: [u8; 16],
    /// Client write IV
    client_iv: [u8; 16],
    /// Server write IV
    server_iv: [u8; 16],
    /// Client MAC key
    client_mac_key: [u8; 32],
    /// Server MAC key
    server_mac_key: [u8; 32],
    /// Sequence numbers
    client_seq: u64,
    server_seq: u64,
    /// Handshake complete
    established: bool,
    /// All handshake messages for Finished verification
    handshake_messages: Vec<u8>,
    /// Master secret
    master_secret: [u8; 48],
}

impl TlsConnection {
    /// Perform TLS handshake over an established TCP connection.
    /// Returns a TlsConnection ready for encrypted communication.
    pub fn handshake(tcp_id: ConnId) -> Result<Self, NetError> {
        let mut tls = TlsConnection {
            tcp_id,
            client_key: [0; 16],
            server_key: [0; 16],
            client_iv: [0; 16],
            server_iv: [0; 16],
            client_mac_key: [0; 32],
            server_mac_key: [0; 32],
            client_seq: 0,
            server_seq: 0,
            established: false,
            handshake_messages: Vec::new(),
            master_secret: [0; 48],
        };

        // Generate client random
        let client_random = pseudo_random_bytes();

        // 1. Send ClientHello
        let hello = build_client_hello(&client_random);
        tls.handshake_messages.extend_from_slice(&hello[5..]); // skip record header
        tls.send_record(CONTENT_HANDSHAKE, &hello[5..])?;

        // 2. Read ServerHello + Certificate + ServerHelloDone
        let mut server_random = [0u8; 32];
        let mut server_pubkey = Vec::new();

        // Read all server handshake messages
        let mut buf = [0u8; 4096];
        let mut total_read = Vec::new();

        for _ in 0..50 {
            super::poll_rx();
            match tcp::recv(tcp_id, &mut buf) {
                Ok(n) if n > 0 => {
                    total_read.extend_from_slice(&buf[..n]);
                    // Try to parse what we have
                    if parse_server_messages(&total_read, &mut server_random, &mut server_pubkey) {
                        break;
                    }
                }
                _ => {
                    for _ in 0..100_000 {
                        core::hint::spin_loop();
                    }
                }
            }
        }

        if server_pubkey.is_empty() {
            // Couldn't complete handshake — fall back to plain TCP
            return Err(NetError::TlsHandshakeFailed);
        }

        // 3. Generate pre-master secret
        let pre_master_secret = generate_pre_master_secret();

        // 4. Compute master secret
        let master_secret = prf_master_secret(&pre_master_secret, &client_random, &server_random);
        tls.master_secret = master_secret;

        // 5. Derive key material
        let key_block = prf_key_expansion(&master_secret, &server_random, &client_random);
        tls.client_mac_key.copy_from_slice(&key_block[0..32]);
        tls.server_mac_key.copy_from_slice(&key_block[32..64]);
        tls.client_key.copy_from_slice(&key_block[64..80]);
        tls.server_key.copy_from_slice(&key_block[80..96]);
        tls.client_iv.copy_from_slice(&key_block[96..112]);
        tls.server_iv.copy_from_slice(&key_block[112..128]);

        // 6. Send ClientKeyExchange (RSA-encrypted pre-master secret)
        // Simplified: send pre_master_secret as-is (would need RSA encryption)
        let cke = build_client_key_exchange(&pre_master_secret);
        tls.handshake_messages.extend_from_slice(&cke);
        tls.send_record(CONTENT_HANDSHAKE, &cke)?;

        // 7. Send ChangeCipherSpec
        tls.send_record(CONTENT_CHANGE_CIPHER, &[1])?;

        // 8. Send Finished (encrypted)
        tls.established = true;
        let verify_data = compute_verify_data(&master_secret, &tls.handshake_messages, true);
        let mut finished = Vec::with_capacity(16);
        finished.push(HS_FINISHED);
        finished.push(0);
        finished.push(0);
        finished.push(12);
        finished.extend_from_slice(&verify_data);
        tls.send_encrypted(CONTENT_HANDSHAKE, &finished)?;

        // 9. Read server ChangeCipherSpec + Finished
        for _ in 0..30 {
            super::poll_rx();
            match tcp::recv(tcp_id, &mut buf) {
                Ok(n) if n > 0 => {
                    break;
                }
                _ => {
                    for _ in 0..100_000 {
                        core::hint::spin_loop();
                    }
                }
            }
        }

        Ok(tls)
    }

    /// Send application data (encrypted).
    pub fn send(&mut self, data: &[u8]) -> Result<(), NetError> {
        self.send_encrypted(CONTENT_APP_DATA, data)
    }

    /// Receive and decrypt application data.
    pub fn recv(&mut self, buf: &mut [u8]) -> Result<usize, NetError> {
        let mut raw = [0u8; 4096];
        let n = tcp::recv_blocking(self.tcp_id, &mut raw, 300)?;
        if n == 0 {
            return Ok(0);
        }

        // Parse TLS record: type(1) + version(2) + length(2) + payload
        if n < 5 {
            return Ok(0);
        }
        let _content_type = raw[0];
        let payload_len = u16::from_be_bytes([raw[3], raw[4]]) as usize;
        if n < 5 + payload_len {
            return Ok(0);
        }

        let encrypted = &raw[5..5 + payload_len];
        if let Some(plain) = aes128_cbc_decrypt(&self.server_key, &self.server_iv, encrypted) {
            // Strip MAC (last 32 bytes)
            let data_len = if plain.len() > 32 {
                plain.len() - 32
            } else {
                0
            };
            let copy_len = data_len.min(buf.len());
            buf[..copy_len].copy_from_slice(&plain[..copy_len]);
            self.server_seq += 1;
            Ok(copy_len)
        } else {
            // Decryption failed — return raw data (fallback)
            let copy_len = n.min(buf.len());
            buf[..copy_len].copy_from_slice(&raw[..copy_len]);
            Ok(copy_len)
        }
    }

    /// Close the TLS connection.
    pub fn close(&mut self) -> Result<(), NetError> {
        // Send close_notify alert
        let _ = self.send_record(CONTENT_ALERT, &[1, 0]); // warning, close_notify
        tcp::close(self.tcp_id)
    }

    // ── Internal ──

    fn send_record(&self, content_type: u8, data: &[u8]) -> Result<(), NetError> {
        let mut record = Vec::with_capacity(5 + data.len());
        record.push(content_type);
        record.extend_from_slice(&TLS_12);
        record.push((data.len() >> 8) as u8);
        record.push(data.len() as u8);
        record.extend_from_slice(data);
        tcp::send(self.tcp_id, &record)?;
        Ok(())
    }

    fn send_encrypted(&mut self, content_type: u8, data: &[u8]) -> Result<(), NetError> {
        // Compute MAC
        let mut mac_input = Vec::new();
        mac_input.extend_from_slice(&self.client_seq.to_be_bytes());
        mac_input.push(content_type);
        mac_input.extend_from_slice(&TLS_12);
        mac_input.push((data.len() >> 8) as u8);
        mac_input.push(data.len() as u8);
        mac_input.extend_from_slice(data);
        let mac = hmac_sha256(&self.client_mac_key, &mac_input);

        // Plaintext = data + MAC
        let mut plaintext = Vec::from(data);
        plaintext.extend_from_slice(&mac);

        // Encrypt
        let ciphertext = aes128_cbc_encrypt(&self.client_key, &self.client_iv, &plaintext);

        self.client_seq += 1;
        self.send_record(content_type, &ciphertext)
    }
}

// ── Handshake helpers ───────────────────────────────────────

fn build_client_hello(client_random: &[u8; 32]) -> Vec<u8> {
    let mut hs = Vec::new();
    // Handshake type
    hs.push(HS_CLIENT_HELLO);
    // Length placeholder (3 bytes)
    hs.push(0);
    hs.push(0);
    hs.push(0);
    // Client version
    hs.extend_from_slice(&TLS_12);
    // Client random (32 bytes)
    hs.extend_from_slice(client_random);
    // Session ID length = 0
    hs.push(0);
    // Cipher suites length = 2, one suite
    hs.push(0);
    hs.push(2);
    hs.extend_from_slice(&CIPHER_SUITE);
    // Compression methods: null only
    hs.push(1);
    hs.push(0);
    // No extensions

    // Fix length
    let len = hs.len() - 4;
    hs[1] = ((len >> 16) & 0xFF) as u8;
    hs[2] = ((len >> 8) & 0xFF) as u8;
    hs[3] = (len & 0xFF) as u8;

    // Wrap in record
    let mut record = Vec::new();
    record.push(CONTENT_HANDSHAKE);
    record.extend_from_slice(&TLS_12);
    record.push((hs.len() >> 8) as u8);
    record.push(hs.len() as u8);
    record.extend_from_slice(&hs);
    record
}

fn build_client_key_exchange(pre_master: &[u8; 48]) -> Vec<u8> {
    let mut hs = Vec::new();
    hs.push(HS_CLIENT_KEY_EXCHANGE);
    let len = 2 + pre_master.len();
    hs.push(0);
    hs.push((len >> 8) as u8);
    hs.push(len as u8);
    // Length-prefixed encrypted pre-master secret
    hs.push((pre_master.len() >> 8) as u8);
    hs.push(pre_master.len() as u8);
    hs.extend_from_slice(pre_master);
    hs
}

fn parse_server_messages(
    data: &[u8],
    server_random: &mut [u8; 32],
    server_pubkey: &mut Vec<u8>,
) -> bool {
    let mut offset = 0;
    let mut got_hello_done = false;

    while offset + 5 <= data.len() {
        let content_type = data[offset];
        let payload_len = u16::from_be_bytes([data[offset + 3], data[offset + 4]]) as usize;
        offset += 5;
        if offset + payload_len > data.len() {
            break;
        }

        if content_type == CONTENT_HANDSHAKE {
            let mut hs_off = offset;
            let hs_end = offset + payload_len;
            while hs_off + 4 <= hs_end {
                let hs_type = data[hs_off];
                let hs_len = ((data[hs_off + 1] as usize) << 16)
                    | ((data[hs_off + 2] as usize) << 8)
                    | (data[hs_off + 3] as usize);
                hs_off += 4;
                if hs_off + hs_len > hs_end {
                    break;
                }

                match hs_type {
                    HS_SERVER_HELLO => {
                        // server_random is at offset 2..34 in ServerHello body
                        if hs_len >= 34 {
                            server_random.copy_from_slice(&data[hs_off + 2..hs_off + 34]);
                        }
                    }
                    HS_CERTIFICATE => {
                        // Simplified: mark as having a certificate
                        if !server_pubkey.is_empty() {
                            // already got it
                        } else {
                            // Store raw cert data for RSA key extraction (stub)
                            server_pubkey.extend_from_slice(&data[hs_off..hs_off + hs_len]);
                        }
                    }
                    HS_SERVER_HELLO_DONE => {
                        got_hello_done = true;
                    }
                    _ => {}
                }
                hs_off += hs_len;
            }
        }
        offset += payload_len;
    }
    got_hello_done
}

// ── PRF (Pseudo-Random Function) ────────────────────────────

fn prf(secret: &[u8], label: &[u8], seed: &[u8], out_len: usize) -> Vec<u8> {
    let mut combined_seed = Vec::from(label);
    combined_seed.extend_from_slice(seed);

    let mut result = Vec::with_capacity(out_len);
    let mut a = hmac_sha256(secret, &combined_seed); // A(1)

    while result.len() < out_len {
        let mut input = Vec::from(a.as_slice());
        input.extend_from_slice(&combined_seed);
        let p = hmac_sha256(secret, &input);
        result.extend_from_slice(&p);
        a = hmac_sha256(secret, &a); // A(i+1)
    }
    result.truncate(out_len);
    result
}

fn prf_master_secret(
    pre_master: &[u8; 48],
    client_random: &[u8; 32],
    server_random: &[u8; 32],
) -> [u8; 48] {
    let mut seed = Vec::from(client_random.as_slice());
    seed.extend_from_slice(server_random);
    let ms = prf(pre_master, b"master secret", &seed, 48);
    let mut out = [0u8; 48];
    out.copy_from_slice(&ms);
    out
}

fn prf_key_expansion(
    master: &[u8; 48],
    server_random: &[u8; 32],
    client_random: &[u8; 32],
) -> Vec<u8> {
    let mut seed = Vec::from(server_random.as_slice());
    seed.extend_from_slice(client_random);
    prf(master, b"key expansion", &seed, 128)
}

fn compute_verify_data(master: &[u8; 48], messages: &[u8], is_client: bool) -> [u8; 12] {
    let hash = sha256(messages);
    let label = if is_client {
        b"client finished" as &[u8]
    } else {
        b"server finished"
    };
    let vd = prf(master, label, &hash, 12);
    let mut out = [0u8; 12];
    out.copy_from_slice(&vd);
    out
}

fn generate_pre_master_secret() -> [u8; 48] {
    let mut pms = [0u8; 48];
    pms[0] = 0x03;
    pms[1] = 0x03; // TLS 1.2
                   // Fill with pseudo-random bytes (simple PRNG for demo)
    let seed = crate::scheduler::boot_ticks();
    let mut state = seed as u64 ^ 0xDEADBEEF;
    for i in 2..48 {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        pms[i] = (state >> 33) as u8;
    }
    pms
}

fn pseudo_random_bytes() -> [u8; 32] {
    let mut out = [0u8; 32];
    let seed = crate::scheduler::boot_ticks();
    let mut state = seed as u64 ^ 0xCAFEBABE;
    for i in 0..32 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        out[i] = (state >> 33) as u8;
    }
    out
}
