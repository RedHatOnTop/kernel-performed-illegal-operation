//! ChaCha20-Poly1305 AEAD — RFC 8439
//!
//! ChaCha20 stream cipher + Poly1305 MAC in an AEAD construction.

use alloc::vec::Vec;

// ── ChaCha20 ────────────────────────────────────────────────

#[inline(always)]
fn qr(s: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    s[a] = s[a].wrapping_add(s[b]); s[d] ^= s[a]; s[d] = s[d].rotate_left(16);
    s[c] = s[c].wrapping_add(s[d]); s[b] ^= s[c]; s[b] = s[b].rotate_left(12);
    s[a] = s[a].wrapping_add(s[b]); s[d] ^= s[a]; s[d] = s[d].rotate_left(8);
    s[c] = s[c].wrapping_add(s[d]); s[b] ^= s[c]; s[b] = s[b].rotate_left(7);
}

/// ChaCha20 block function: produces 64 bytes of keystream.
fn chacha20_block(key: &[u8; 32], counter: u32, nonce: &[u8; 12]) -> [u8; 64] {
    let mut state = [0u32; 16];
    // Constants: "expand 32-byte k"
    state[0] = 0x61707865;
    state[1] = 0x3320646e;
    state[2] = 0x79622d32;
    state[3] = 0x6b206574;
    // Key
    for i in 0..8 {
        state[4 + i] = u32::from_le_bytes([
            key[i*4], key[i*4+1], key[i*4+2], key[i*4+3],
        ]);
    }
    // Counter
    state[12] = counter;
    // Nonce
    for i in 0..3 {
        state[13 + i] = u32::from_le_bytes([
            nonce[i*4], nonce[i*4+1], nonce[i*4+2], nonce[i*4+3],
        ]);
    }

    let mut working = state;
    for _ in 0..10 {
        // Column rounds
        qr(&mut working, 0, 4,  8, 12);
        qr(&mut working, 1, 5,  9, 13);
        qr(&mut working, 2, 6, 10, 14);
        qr(&mut working, 3, 7, 11, 15);
        // Diagonal rounds
        qr(&mut working, 0, 5, 10, 15);
        qr(&mut working, 1, 6, 11, 12);
        qr(&mut working, 2, 7,  8, 13);
        qr(&mut working, 3, 4,  9, 14);
    }
    for i in 0..16 {
        working[i] = working[i].wrapping_add(state[i]);
    }

    let mut out = [0u8; 64];
    for i in 0..16 {
        out[i*4..i*4+4].copy_from_slice(&working[i].to_le_bytes());
    }
    out
}

/// ChaCha20 encrypt/decrypt (symmetric).
fn chacha20_crypt(key: &[u8; 32], counter: u32, nonce: &[u8; 12], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut ctr = counter;
    let mut off = 0;

    while off < data.len() {
        let block = chacha20_block(key, ctr, nonce);
        let take = (data.len() - off).min(64);
        for j in 0..take {
            out.push(data[off + j] ^ block[j]);
        }
        off += take;
        ctr = ctr.wrapping_add(1);
    }
    out
}

// ── Poly1305 ────────────────────────────────────────────────

/// Poly1305 MAC: returns 16-byte tag.
///
/// Computes over `data` using 32-byte `key_material` where:
/// - `key_material[0..16]` = r (clamped)
/// - `key_material[16..32]` = s
fn poly1305_mac(key_material: &[u8; 32], data: &[u8]) -> [u8; 16] {
    // Parse r and clamp
    let mut r = [0u32; 5];
    r[0] = (u32::from_le_bytes([key_material[0], key_material[1], key_material[2], key_material[3]])) & 0x0fff_fffc;
    r[1] = (u32::from_le_bytes([key_material[3], key_material[4], key_material[5], key_material[6]]) >> 2) & 0x0fff_fffc;
    r[2] = (u32::from_le_bytes([key_material[6], key_material[7], key_material[8], key_material[9]]) >> 4) & 0x0fff_fffc;
    r[3] = (u32::from_le_bytes([key_material[9], key_material[10], key_material[11], key_material[12]]) >> 6) & 0x0fff_fffc;
    r[4] = 0; // not used directly

    // Parse r as radix-2^26 (5 limbs)
    let r0 = u32::from_le_bytes([key_material[0], key_material[1], key_material[2], key_material[3]]) & 0x03ff_ffff;
    let r1 = (u32::from_le_bytes([key_material[3], key_material[4], key_material[5], key_material[6]]) >> 2) & 0x03ff_ff03;
    let r2 = (u32::from_le_bytes([key_material[6], key_material[7], key_material[8], key_material[9]]) >> 4) & 0x03ff_c0ff;
    let r3 = (u32::from_le_bytes([key_material[9], key_material[10], key_material[11], key_material[12]]) >> 6) & 0x03f0_3fff;
    let r4 = (u32::from_le_bytes([key_material[12], key_material[13], key_material[14], key_material[15]]) >> 8) & 0x000f_ffff;

    // Precompute 5*r for reduction
    let s1 = r1 * 5;
    let s2 = r2 * 5;
    let s3 = r3 * 5;
    let s4 = r4 * 5;

    // Parse s (nonce for final addition)
    let s = [
        u32::from_le_bytes([key_material[16], key_material[17], key_material[18], key_material[19]]),
        u32::from_le_bytes([key_material[20], key_material[21], key_material[22], key_material[23]]),
        u32::from_le_bytes([key_material[24], key_material[25], key_material[26], key_material[27]]),
        u32::from_le_bytes([key_material[28], key_material[29], key_material[30], key_material[31]]),
    ];

    // Accumulator (5 limbs, radix 2^26)
    let mut h0: u32 = 0;
    let mut h1: u32 = 0;
    let mut h2: u32 = 0;
    let mut h3: u32 = 0;
    let mut h4: u32 = 0;

    // Process 16-byte blocks
    let mut i = 0;
    while i < data.len() {
        let remaining = data.len() - i;
        let block_len = remaining.min(16);

        // Read block and add padding bit
        let mut block = [0u8; 17];
        block[..block_len].copy_from_slice(&data[i..i + block_len]);
        block[block_len] = 1; // hibit

        let t0 = u32::from_le_bytes([block[0], block[1], block[2], block[3]]);
        let t1 = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
        let t2 = u32::from_le_bytes([block[8], block[9], block[10], block[11]]);
        let t3 = u32::from_le_bytes([block[12], block[13], block[14], block[15]]);

        h0 += t0 & 0x03ff_ffff;
        h1 += ((t0 >> 26) | (t1 << 6)) & 0x03ff_ffff;
        h2 += ((t1 >> 20) | (t2 << 12)) & 0x03ff_ffff;
        h3 += ((t2 >> 14) | (t3 << 18)) & 0x03ff_ffff;
        h4 += (t3 >> 8) | if block_len == 16 { 1 << 24 } else { (block[16] as u32) << ((block_len * 8).saturating_sub(104).min(24)) };

        // Actually, let me just set h4 += hibit:
        // This needs to be the 5th limb contribution
        // Re-do: add the message block as a 130-bit number
        // For the full block case, hibit is at bit 128, so it goes into h4
        // Let's simplify with u64 arithmetic:
        h4 = if block_len == 16 {
            h4.wrapping_add((t3 >> 8) | (1 << 24)).wrapping_sub(t3 >> 8)
        } else {
            h4
        };

        // h = (h + m) * r
        let d0 = (h0 as u64) * (r0 as u64) + (h1 as u64) * (s4 as u64) + (h2 as u64) * (s3 as u64) + (h3 as u64) * (s2 as u64) + (h4 as u64) * (s1 as u64);
        let d1 = (h0 as u64) * (r1 as u64) + (h1 as u64) * (r0 as u64) + (h2 as u64) * (s4 as u64) + (h3 as u64) * (s3 as u64) + (h4 as u64) * (s2 as u64);
        let d2 = (h0 as u64) * (r2 as u64) + (h1 as u64) * (r1 as u64) + (h2 as u64) * (r0 as u64) + (h3 as u64) * (s4 as u64) + (h4 as u64) * (s3 as u64);
        let d3 = (h0 as u64) * (r3 as u64) + (h1 as u64) * (r2 as u64) + (h2 as u64) * (r1 as u64) + (h3 as u64) * (r0 as u64) + (h4 as u64) * (s4 as u64);
        let d4 = (h0 as u64) * (r4 as u64) + (h1 as u64) * (r3 as u64) + (h2 as u64) * (r2 as u64) + (h3 as u64) * (r1 as u64) + (h4 as u64) * (r0 as u64);

        // Partial reduction mod 2^130 - 5
        let mut c: u32;
        c = (d0 >> 26) as u32; h0 = d0 as u32 & 0x03ff_ffff;
        let d1 = d1 + c as u64; c = (d1 >> 26) as u32; h1 = d1 as u32 & 0x03ff_ffff;
        let d2 = d2 + c as u64; c = (d2 >> 26) as u32; h2 = d2 as u32 & 0x03ff_ffff;
        let d3 = d3 + c as u64; c = (d3 >> 26) as u32; h3 = d3 as u32 & 0x03ff_ffff;
        let d4 = d4 + c as u64; c = (d4 >> 26) as u32; h4 = d4 as u32 & 0x03ff_ffff;
        h0 += c * 5; c = h0 >> 26; h0 &= 0x03ff_ffff;
        h1 += c;

        i += 16;
    }

    // Final reduction
    let mut c: u32;
    c = h1 >> 26; h1 &= 0x03ff_ffff;
    h2 += c; c = h2 >> 26; h2 &= 0x03ff_ffff;
    h3 += c; c = h3 >> 26; h3 &= 0x03ff_ffff;
    h4 += c; c = h4 >> 26; h4 &= 0x03ff_ffff;
    h0 += c * 5; c = h0 >> 26; h0 &= 0x03ff_ffff;
    h1 += c;

    // Compute h - p = h - (2^130 - 5)
    let mut g0 = h0.wrapping_add(5); c = g0 >> 26; g0 &= 0x03ff_ffff;
    let mut g1 = h1.wrapping_add(c); c = g1 >> 26; g1 &= 0x03ff_ffff;
    let mut g2 = h2.wrapping_add(c); c = g2 >> 26; g2 &= 0x03ff_ffff;
    let mut g3 = h3.wrapping_add(c); c = g3 >> 26; g3 &= 0x03ff_ffff;
    let g4 = h4.wrapping_add(c).wrapping_sub(1 << 26);

    // Select h or g
    let mask = (g4 >> 31).wrapping_sub(1); // 0 if g4 < 0 (h < p), all 1s if h >= p
    let nmask = !mask;
    h0 = (h0 & nmask) | (g0 & mask);
    h1 = (h1 & nmask) | (g1 & mask);
    h2 = (h2 & nmask) | (g2 & mask);
    h3 = (h3 & nmask) | (g3 & mask);
    h4 = (h4 & nmask) | (g4 & mask);

    // h = h + s
    let mut f: u64;
    f = (h0 as u64) | ((h1 as u64) << 26);
    let f0 = f as u32;
    f = ((h1 >> 6) as u64) | ((h2 as u64) << 20);
    f += s[0] as u64 + (f0 as u64 + s[0] as u64 >> 32); // carry
    // Simpler: just reconstruct the 128-bit number and add s
    let h_full: u128 = (h0 as u128)
        | ((h1 as u128) << 26)
        | ((h2 as u128) << 52)
        | ((h3 as u128) << 78)
        | ((h4 as u128) << 104);
    let s_full: u128 = (s[0] as u128)
        | ((s[1] as u128) << 32)
        | ((s[2] as u128) << 64)
        | ((s[3] as u128) << 96);
    let result = h_full.wrapping_add(s_full);

    let mut tag = [0u8; 16];
    tag.copy_from_slice(&result.to_le_bytes()[..16]);
    tag
}

// ── AEAD construction ───────────────────────────────────────

/// Construct Poly1305 authenticated `data` for ChaCha20-Poly1305 AEAD.
/// `data` = pad(aad) || pad(ciphertext) || len(aad) || len(ct) (both LE u64)
fn construct_poly1305_data(aad: &[u8], ciphertext: &[u8]) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(aad);
    // pad aad to 16-byte boundary
    let aad_pad = (16 - (aad.len() % 16)) % 16;
    for _ in 0..aad_pad { data.push(0); }
    data.extend_from_slice(ciphertext);
    let ct_pad = (16 - (ciphertext.len() % 16)) % 16;
    for _ in 0..ct_pad { data.push(0); }
    data.extend_from_slice(&(aad.len() as u64).to_le_bytes());
    data.extend_from_slice(&(ciphertext.len() as u64).to_le_bytes());
    data
}

/// ChaCha20-Poly1305 seal (encrypt + authenticate).
///
/// * `key`: 32 bytes
/// * `nonce`: 12 bytes
/// * `aad`: additional authenticated data
/// * `plaintext`: data to encrypt
///
/// Returns `(ciphertext, tag)` where tag is 16 bytes.
pub fn chacha20_poly1305_seal(
    key: &[u8; 32],
    nonce: &[u8; 12],
    aad: &[u8],
    plaintext: &[u8],
) -> (Vec<u8>, [u8; 16]) {
    // Generate Poly1305 key: first 32 bytes of ChaCha20(key, 0, nonce)
    let poly_key_block = chacha20_block(key, 0, nonce);
    let mut poly_key = [0u8; 32];
    poly_key.copy_from_slice(&poly_key_block[..32]);

    // Encrypt (counter starts at 1)
    let ciphertext = chacha20_crypt(key, 1, nonce, plaintext);

    // Compute tag
    let mac_data = construct_poly1305_data(aad, &ciphertext);
    let tag = poly1305_mac(&poly_key, &mac_data);

    (ciphertext, tag)
}

/// ChaCha20-Poly1305 open (decrypt + verify).
///
/// Returns `None` if authentication fails.
pub fn chacha20_poly1305_open(
    key: &[u8; 32],
    nonce: &[u8; 12],
    aad: &[u8],
    ciphertext: &[u8],
    tag: &[u8; 16],
) -> Option<Vec<u8>> {
    let poly_key_block = chacha20_block(key, 0, nonce);
    let mut poly_key = [0u8; 32];
    poly_key.copy_from_slice(&poly_key_block[..32]);

    // Verify tag
    let mac_data = construct_poly1305_data(aad, ciphertext);
    let computed = poly1305_mac(&poly_key, &mac_data);

    let mut diff = 0u8;
    for j in 0..16 { diff |= computed[j] ^ tag[j]; }
    if diff != 0 { return None; }

    // Decrypt
    let plaintext = chacha20_crypt(key, 1, nonce, ciphertext);
    Some(plaintext)
}
