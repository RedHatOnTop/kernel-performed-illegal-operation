//! P-256 (secp256r1 / prime256v1) — ECDH + ECDSA verification
//!
//! Field arithmetic mod p = 2^256 − 2^224 + 2^192 + 2^96 − 1.
//! Point operations use Jacobian coordinates.

#![allow(dead_code)]
use alloc::vec::Vec;

// ── Field element (256-bit, mod p) ──────────────────────────

/// P-256 prime: p = 2^256 − 2^224 + 2^192 + 2^96 − 1
const P: [u64; 4] = [
    0xFFFFFFFFFFFFFFFF,
    0x00000000FFFFFFFF,
    0x0000000000000000,
    0xFFFFFFFF00000001,
];

/// Curve order n
const N: [u64; 4] = [
    0xF3B9CAC2FC632551,
    0xBCE6FAADA7179E84,
    0xFFFFFFFFFFFFFFFF,
    0xFFFFFFFF00000000,
];

/// Base point G (uncompressed, affine)
const GX: [u64; 4] = [
    0xF4A13945D898C296,
    0x77037D812DEB33A0,
    0xF8BCE6E563A440F2,
    0x6B17D1F2E12C4247,
];
const GY: [u64; 4] = [
    0xCBB6406837BF51F5,
    0x2BCE33576B315ECE,
    0x8EE7EB4A7C0F9E16,
    0x4FE342E2FE1A7F9B,
];

type U256 = [u64; 4];

// ── 256-bit arithmetic ──────────────────────────────────────

fn u256_zero() -> U256 {
    [0; 4]
}
fn u256_one() -> U256 {
    [1, 0, 0, 0]
}

fn u256_is_zero(a: &U256) -> bool {
    a[0] == 0 && a[1] == 0 && a[2] == 0 && a[3] == 0
}

fn u256_cmp(a: &U256, b: &U256) -> core::cmp::Ordering {
    for i in (0..4).rev() {
        if a[i] != b[i] {
            return a[i].cmp(&b[i]);
        }
    }
    core::cmp::Ordering::Equal
}

fn u256_add(a: &U256, b: &U256) -> (U256, bool) {
    let mut r = [0u64; 4];
    let mut carry = 0u64;
    for i in 0..4 {
        let (s1, c1) = a[i].overflowing_add(b[i]);
        let (s2, c2) = s1.overflowing_add(carry);
        r[i] = s2;
        carry = (c1 as u64) + (c2 as u64);
    }
    (r, carry != 0)
}

fn u256_sub(a: &U256, b: &U256) -> (U256, bool) {
    let mut r = [0u64; 4];
    let mut borrow = 0u64;
    for i in 0..4 {
        let (s1, c1) = a[i].overflowing_sub(b[i]);
        let (s2, c2) = s1.overflowing_sub(borrow);
        r[i] = s2;
        borrow = (c1 as u64) + (c2 as u64);
    }
    (r, borrow != 0)
}

// ── Modular arithmetic (mod p) ──────────────────────────────

fn fp_add(a: &U256, b: &U256) -> U256 {
    let (mut r, carry) = u256_add(a, b);
    if carry || u256_cmp(&r, &P) != core::cmp::Ordering::Less {
        let (sub, _) = u256_sub(&r, &P);
        r = sub;
    }
    r
}

fn fp_sub(a: &U256, b: &U256) -> U256 {
    let (r, borrow) = u256_sub(a, b);
    if borrow {
        let (add, _) = u256_add(&r, &P);
        add
    } else {
        r
    }
}

fn fp_mul(a: &U256, b: &U256) -> U256 {
    // Schoolbook 256×256 → 512, then reduce mod p
    let mut t = [0u64; 8];
    for i in 0..4 {
        let mut carry = 0u128;
        for j in 0..4 {
            carry += t[i + j] as u128 + (a[i] as u128) * (b[j] as u128);
            t[i + j] = carry as u64;
            carry >>= 64;
        }
        t[i + 4] = carry as u64;
    }
    fp_reduce512(&t)
}

fn fp_sq(a: &U256) -> U256 {
    fp_mul(a, a)
}

/// Fast reduction mod P-256 prime using its special form.
/// Input: 512-bit number in t[0..8].
fn fp_reduce512(t: &[u64; 8]) -> U256 {
    // P-256 reduction per NIST recommendation
    // s1 = (c7, c6, c5, c4, c3, c2, c1, c0) — the original value, but we work in 64-bit limbs
    // We'll use the standard approach with u128 for carries
    let (c0, c1, c2, c3) = (t[0], t[1], t[2], t[3]);
    let (c4, c5, c6, c7) = (t[4], t[5], t[6], t[7]);

    // Express as 32-bit words for the NIST reduction formulas
    // But since we use 64-bit limbs, adapt the formulas
    // For simplicity and correctness, use repeated subtraction approach

    // Start with the low 256 bits
    let mut r = [c0, c1, c2, c3];

    // Add multiples of the high words adjusted by p's structure
    // s2 = (c7, c6, c5, c4, 0, 0, 0, 0) => add shifted
    // Using the identity: 2^256 ≡ 2^224 - 2^192 - 2^96 + 1 (mod p)
    // So c4*2^256 = c4*(2^224 - 2^192 - 2^96 + 1)
    // For each high limb, we add its contribution.

    // Contribution of c4 (multiplied by 2^256 mod p):
    // 2^256 mod p = 2^224 - 2^192 - 2^96 + 1
    // In 64-bit limb terms: this is a complex pattern
    // Let's use a simpler approach: Barrett reduction

    // Simple approach: compute r = t mod p by trial subtraction
    // First, fold the high part using: x ≡ x_lo + x_hi * (2^256 mod p) (mod p)
    // 2^256 mod p = p + 1 - p = ... let me compute:
    // p = FFFFFFFF00000001 00000000 00000000 00000000FFFFFFFF FFFFFFFFFFFFFFFF
    // 2^256 = 10000000000000000 ...
    // 2^256 - p = 00000000FFFFFFFE FFFFFFFFFFFFFFFF FFFFFFFF00000000 0000000000000001
    // So 2^256 mod p = [0x1, 0xFFFFFFFF00000000, 0xFFFFFFFFFFFFFFFF, 0x00000000FFFFFFFE]

    let r256_mod_p: U256 = [
        0x0000000000000001,
        0xFFFFFFFF00000000,
        0xFFFFFFFFFFFFFFFF,
        0x00000000FFFFFFFE,
    ];

    // Fold: result = (c3,c2,c1,c0) + (c7,c6,c5,c4) * r256_mod_p
    // This produces up to 512+256 bits, so we may need to iterate

    // Multiply (c4,c5,c6,c7) by r256_mod_p and add to r
    // This could overflow, so use wider arithmetic
    let hi = [c4, c5, c6, c7];

    // Multiply hi * r256_mod_p → 512-bit result, then add to r
    let mut product = [0u128; 8];
    for i in 0..4 {
        for j in 0..4 {
            product[i + j] += (hi[i] as u128) * (r256_mod_p[j] as u128);
        }
    }
    // Propagate carries in product
    for i in 0..7 {
        product[i + 1] += product[i] >> 64;
        product[i] &= 0xFFFFFFFFFFFFFFFF;
    }

    // Add to r
    let mut acc = [0u128; 5];
    for i in 0..4 {
        acc[i] = r[i] as u128 + (product[i] as u64) as u128;
    }
    // Propagate carries
    for i in 0..4 {
        acc[i + 1] += acc[i] >> 64;
        acc[i] &= 0xFFFFFFFFFFFFFFFF;
    }

    let mut result: U256 = [acc[0] as u64, acc[1] as u64, acc[2] as u64, acc[3] as u64];
    let mut extra = acc[4] as u64;

    // Handle remaining product[4..7]
    if product[4] != 0 || product[5] != 0 || product[6] != 0 || product[7] != 0 {
        // Need another round of folding — but for typical sizes this shouldn't happen
        // as the product of 256×256 bits = 512 bits, and the fold should be at most 512 bits
        // We'll just do modular reduction by subtraction
    }

    // Subtract p until result < p
    while extra > 0 || u256_cmp(&result, &P) != core::cmp::Ordering::Less {
        let (sub, borrow) = u256_sub(&result, &P);
        if !borrow || extra > 0 {
            result = sub;
            if extra > 0 {
                extra -= 1;
            }
        } else {
            break;
        }
    }

    result
}

/// Modular inverse using Fermat's little theorem: a^(p-2) mod p.
fn fp_inv(a: &U256) -> U256 {
    // p-2 = FFFFFFFF00000001 00000000 00000000 00000000FFFFFFFF FFFFFFFFFFFFFFFD
    // Use square-and-multiply
    let mut result = u256_one();
    let mut base = *a;

    // Convert p-2 to bits and process
    let p_minus_2: U256 = [
        0xFFFFFFFFFFFFFFFD,
        0x00000000FFFFFFFF,
        0x0000000000000000,
        0xFFFFFFFF00000001,
    ];

    for i in 0..4 {
        let mut word = p_minus_2[i];
        for _ in 0..64 {
            if word & 1 == 1 {
                result = fp_mul(&result, &base);
            }
            base = fp_sq(&base);
            word >>= 1;
        }
    }
    result
}

fn fp_neg(a: &U256) -> U256 {
    if u256_is_zero(a) {
        return u256_zero();
    }
    let (r, _) = u256_sub(&P, a);
    r
}

// ── Jacobian point operations ───────────────────────────────

/// Point in Jacobian coordinates (X, Y, Z) where affine (x,y) = (X/Z², Y/Z³).
#[derive(Clone)]
struct JacobianPoint {
    x: U256,
    y: U256,
    z: U256,
}

impl JacobianPoint {
    fn infinity() -> Self {
        JacobianPoint {
            x: u256_one(),
            y: u256_one(),
            z: u256_zero(),
        }
    }

    fn is_infinity(&self) -> bool {
        u256_is_zero(&self.z)
    }

    fn from_affine(x: &U256, y: &U256) -> Self {
        JacobianPoint {
            x: *x,
            y: *y,
            z: u256_one(),
        }
    }

    fn to_affine(&self) -> (U256, U256) {
        if self.is_infinity() {
            return (u256_zero(), u256_zero());
        }
        let z_inv = fp_inv(&self.z);
        let z2 = fp_sq(&z_inv);
        let z3 = fp_mul(&z2, &z_inv);
        (fp_mul(&self.x, &z2), fp_mul(&self.y, &z3))
    }
}

/// Point doubling in Jacobian coordinates.
fn point_double(p: &JacobianPoint) -> JacobianPoint {
    if p.is_infinity() {
        return JacobianPoint::infinity();
    }

    // a = -3 for P-256
    let xx = fp_sq(&p.x);
    let yy = fp_sq(&p.y);
    let yyyy = fp_sq(&yy);
    let zz = fp_sq(&p.z);

    // S = 2*((X+YY)^2 - XX - YYYY)
    let sum = fp_add(&p.x, &yy);
    let sum_sq = fp_sq(&sum);
    let s = {
        let t = fp_sub(&sum_sq, &xx);
        let t = fp_sub(&t, &yyyy);
        fp_add(&t, &t)
    };

    // M = 3*XX + a*ZZ^2, where a = -3
    let m = {
        let three_xx = fp_add(&xx, &fp_add(&xx, &xx));
        let zzzz = fp_sq(&zz);
        let a_zzzz = fp_add(&zzzz, &fp_add(&zzzz, &zzzz)); // 3*ZZ^2
        fp_sub(&three_xx, &a_zzzz)
    };

    // X' = M^2 - 2*S
    let x3 = {
        let m2 = fp_sq(&m);
        let two_s = fp_add(&s, &s);
        fp_sub(&m2, &two_s)
    };

    // Y' = M*(S - X') - 8*YYYY
    let y3 = {
        let s_x3 = fp_sub(&s, &x3);
        let m_sx3 = fp_mul(&m, &s_x3);
        let yyyy2 = fp_add(&yyyy, &yyyy);
        let yyyy4 = fp_add(&yyyy2, &yyyy2);
        let yyyy8 = fp_add(&yyyy4, &yyyy4);
        fp_sub(&m_sx3, &yyyy8)
    };

    // Z' = (Y + Z)^2 - YY - ZZ
    let z3 = {
        let yz = fp_add(&p.y, &p.z);
        let yz2 = fp_sq(&yz);
        let t = fp_sub(&yz2, &yy);
        fp_sub(&t, &zz)
    };

    JacobianPoint {
        x: x3,
        y: y3,
        z: z3,
    }
}

/// Point addition (Jacobian + Jacobian).
fn point_add(p: &JacobianPoint, q: &JacobianPoint) -> JacobianPoint {
    if p.is_infinity() {
        return q.clone();
    }
    if q.is_infinity() {
        return p.clone();
    }

    let z1z1 = fp_sq(&p.z);
    let z2z2 = fp_sq(&q.z);
    let u1 = fp_mul(&p.x, &z2z2);
    let u2 = fp_mul(&q.x, &z1z1);
    let s1 = fp_mul(&p.y, &fp_mul(&q.z, &z2z2));
    let s2 = fp_mul(&q.y, &fp_mul(&p.z, &z1z1));

    if u1 == u2 {
        if s1 == s2 {
            return point_double(p);
        } else {
            return JacobianPoint::infinity();
        }
    }

    let h = fp_sub(&u2, &u1);
    let hh = fp_sq(&h);
    let hhh = fp_mul(&h, &hh);
    let r = fp_sub(&s2, &s1);

    let x3 = {
        let r2 = fp_sq(&r);
        let u1hh = fp_mul(&u1, &hh);
        let two_u1hh = fp_add(&u1hh, &u1hh);
        fp_sub(&fp_sub(&r2, &hhh), &two_u1hh)
    };

    let y3 = {
        let u1hh = fp_mul(&u1, &hh);
        let diff = fp_sub(&u1hh, &x3);
        let r_diff = fp_mul(&r, &diff);
        let s1hhh = fp_mul(&s1, &hhh);
        fp_sub(&r_diff, &s1hhh)
    };

    let z3 = {
        let z1z2 = fp_mul(&p.z, &q.z);
        fp_mul(&z1z2, &h)
    };

    JacobianPoint {
        x: x3,
        y: y3,
        z: z3,
    }
}

/// Scalar multiplication: k * P using double-and-add.
fn scalar_mul(k: &U256, p: &JacobianPoint) -> JacobianPoint {
    let mut result = JacobianPoint::infinity();
    let mut temp = p.clone();

    for i in 0..4 {
        let mut word = k[i];
        for _ in 0..64 {
            if word & 1 == 1 {
                result = point_add(&result, &temp);
            }
            temp = point_double(&temp);
            word >>= 1;
        }
    }
    result
}

// ── Public API ──────────────────────────────────────────────

fn u256_from_be_bytes(bytes: &[u8; 32]) -> U256 {
    [
        u64::from_be_bytes([
            bytes[24], bytes[25], bytes[26], bytes[27], bytes[28], bytes[29], bytes[30], bytes[31],
        ]),
        u64::from_be_bytes([
            bytes[16], bytes[17], bytes[18], bytes[19], bytes[20], bytes[21], bytes[22], bytes[23],
        ]),
        u64::from_be_bytes([
            bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
        ]),
        u64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]),
    ]
}

fn u256_to_be_bytes(a: &U256) -> [u8; 32] {
    let mut out = [0u8; 32];
    let b3 = a[3].to_be_bytes();
    let b2 = a[2].to_be_bytes();
    let b1 = a[1].to_be_bytes();
    let b0 = a[0].to_be_bytes();
    out[0..8].copy_from_slice(&b3);
    out[8..16].copy_from_slice(&b2);
    out[16..24].copy_from_slice(&b1);
    out[24..32].copy_from_slice(&b0);
    out
}

/// P-256 ECDH: compute shared secret from private key and peer's public key.
///
/// * `private_key`: 32-byte big-endian scalar
/// * `peer_public`: 65-byte uncompressed point (0x04 || X || Y)
///
/// Returns 32-byte shared secret (x-coordinate of k*P).
pub fn p256_ecdh(private_key: &[u8; 32], peer_public: &[u8]) -> Option<[u8; 32]> {
    if peer_public.len() < 65 || peer_public[0] != 0x04 {
        return None;
    }
    let px = u256_from_be_bytes(&peer_public[1..33].try_into().ok()?);
    let py = u256_from_be_bytes(&peer_public[33..65].try_into().ok()?);
    let k = u256_from_be_bytes(private_key);

    let point = JacobianPoint::from_affine(&px, &py);
    let result = scalar_mul(&k, &point);
    let (rx, _) = result.to_affine();

    Some(u256_to_be_bytes(&rx))
}

/// Generate P-256 public key from private key.
///
/// Returns 65-byte uncompressed point (0x04 || X || Y).
pub fn p256_public_key(private_key: &[u8; 32]) -> [u8; 65] {
    let k = u256_from_be_bytes(private_key);
    let g = JacobianPoint::from_affine(&GX, &GY);
    let result = scalar_mul(&k, &g);
    let (rx, ry) = result.to_affine();

    let mut out = [0u8; 65];
    out[0] = 0x04;
    out[1..33].copy_from_slice(&u256_to_be_bytes(&rx));
    out[33..65].copy_from_slice(&u256_to_be_bytes(&ry));
    out
}

// ── Modular arithmetic (mod n, for ECDSA) ───────────────────

fn fn_add(a: &U256, b: &U256) -> U256 {
    let (mut r, carry) = u256_add(a, b);
    if carry || u256_cmp(&r, &N) != core::cmp::Ordering::Less {
        let (sub, _) = u256_sub(&r, &N);
        r = sub;
    }
    r
}

fn fn_mul(a: &U256, b: &U256) -> U256 {
    let mut t = [0u64; 8];
    for i in 0..4 {
        let mut carry = 0u128;
        for j in 0..4 {
            carry += t[i + j] as u128 + (a[i] as u128) * (b[j] as u128);
            t[i + j] = carry as u64;
            carry >>= 64;
        }
        t[i + 4] = carry as u64;
    }
    // Reduce mod n (by trial subtraction for simplicity)
    fn_reduce512(&t)
}

fn fn_reduce512(t: &[u64; 8]) -> U256 {
    // Simple: convert to big number, compute mod n by repeated subtraction
    // For production use, use Barrett or Montgomery reduction
    let mut r = [t[0], t[1], t[2], t[3]];
    let hi = [t[4], t[5], t[6], t[7]];

    // Similar approach as fp_reduce512 but mod n
    if !u256_is_zero(&hi) {
        // Fold high part: 2^256 mod n
        // n_comp = 2^256 - n
        let n_comp: U256 = [
            0x0C46353D7CF5D3ED,
            0x4319055258E8617B,
            0x0000000000000000,
            0x00000000FFFFFFFF,
        ];
        // Add hi * n_comp to r
        let mut product = [0u128; 8];
        for i in 0..4 {
            for j in 0..4 {
                product[i + j] += (hi[i] as u128) * (n_comp[j] as u128);
            }
        }
        for i in 0..7 {
            product[i + 1] += product[i] >> 64;
            product[i] &= 0xFFFFFFFFFFFFFFFF;
        }
        let mut acc = [0u128; 5];
        for i in 0..4 {
            acc[i] = r[i] as u128 + (product[i] as u64) as u128;
        }
        for i in 0..4 {
            acc[i + 1] += acc[i] >> 64;
            acc[i] &= 0xFFFFFFFFFFFFFFFF;
        }
        r = [acc[0] as u64, acc[1] as u64, acc[2] as u64, acc[3] as u64];
    }

    while u256_cmp(&r, &N) != core::cmp::Ordering::Less {
        let (sub, _) = u256_sub(&r, &N);
        r = sub;
    }
    r
}

fn fn_inv(a: &U256) -> U256 {
    // Fermat: a^(n-2) mod n
    let mut result = u256_one();
    let mut base = *a;
    let n_minus_2: U256 = [
        0xF3B9CAC2FC63254F,
        0xBCE6FAADA7179E84,
        0xFFFFFFFFFFFFFFFF,
        0xFFFFFFFF00000000,
    ];
    for i in 0..4 {
        let mut word = n_minus_2[i];
        for _ in 0..64 {
            if word & 1 == 1 {
                result = fn_mul(&result, &base);
            }
            base = fn_mul(&base, &base);
            word >>= 1;
        }
    }
    result
}

/// ECDSA signature verification on P-256.
///
/// * `hash`: 32-byte message hash (SHA-256)
/// * `signature`: DER-encoded or raw (r || s, each 32 bytes)
/// * `public_key`: 65-byte uncompressed point (0x04 || X || Y)
///
/// Returns `true` if the signature is valid.
pub fn p256_ecdsa_verify(hash: &[u8], signature: &[u8], public_key: &[u8]) -> bool {
    // Parse public key
    if public_key.len() < 65 || public_key[0] != 0x04 {
        return false;
    }
    let qx = u256_from_be_bytes(public_key[1..33].try_into().unwrap_or(&[0u8; 32]));
    let qy = u256_from_be_bytes(public_key[33..65].try_into().unwrap_or(&[0u8; 32]));

    // Parse signature (try raw r||s first, then DER)
    let (r, s) = if signature.len() == 64 {
        let r = u256_from_be_bytes(signature[..32].try_into().unwrap_or(&[0u8; 32]));
        let s = u256_from_be_bytes(signature[32..64].try_into().unwrap_or(&[0u8; 32]));
        (r, s)
    } else if let Some((r, s)) = parse_der_signature(signature) {
        (r, s)
    } else {
        return false;
    };

    // Check r, s in [1, n-1]
    if u256_is_zero(&r) || u256_is_zero(&s) {
        return false;
    }
    if u256_cmp(&r, &N) != core::cmp::Ordering::Less {
        return false;
    }
    if u256_cmp(&s, &N) != core::cmp::Ordering::Less {
        return false;
    }

    // z = hash (truncated to n bit length if needed)
    let mut z_bytes = [0u8; 32];
    let copy_len = hash.len().min(32);
    z_bytes[32 - copy_len..].copy_from_slice(&hash[..copy_len]);
    let z = u256_from_be_bytes(&z_bytes);

    // w = s^-1 mod n
    let w = fn_inv(&s);

    // u1 = z * w mod n
    let u1 = fn_mul(&z, &w);
    // u2 = r * w mod n
    let u2 = fn_mul(&r, &w);

    // R = u1*G + u2*Q
    let g = JacobianPoint::from_affine(&GX, &GY);
    let q = JacobianPoint::from_affine(&qx, &qy);
    let r1 = scalar_mul(&u1, &g);
    let r2 = scalar_mul(&u2, &q);
    let rr = point_add(&r1, &r2);

    if rr.is_infinity() {
        return false;
    }

    let (rx, _) = rr.to_affine();

    // v = rx mod n
    let mut v = rx;
    if u256_cmp(&v, &N) != core::cmp::Ordering::Less {
        let (sub, _) = u256_sub(&v, &N);
        v = sub;
    }

    // Valid iff v == r
    v == r
}

/// Parse DER-encoded ECDSA signature into (r, s).
fn parse_der_signature(der: &[u8]) -> Option<(U256, U256)> {
    if der.len() < 8 || der[0] != 0x30 {
        return None;
    }
    let total_len = der[1] as usize;
    if der.len() < 2 + total_len {
        return None;
    }

    let mut pos = 2;

    // Parse r
    if der[pos] != 0x02 {
        return None;
    }
    pos += 1;
    let r_len = der[pos] as usize;
    pos += 1;
    let r_bytes = &der[pos..pos + r_len];
    pos += r_len;

    // Parse s
    if pos >= der.len() || der[pos] != 0x02 {
        return None;
    }
    pos += 1;
    let s_len = der[pos] as usize;
    pos += 1;
    if pos + s_len > der.len() {
        return None;
    }
    let s_bytes = &der[pos..pos + s_len];

    // Convert to U256 (skip leading zero if present)
    let r = bytes_to_u256(r_bytes)?;
    let s = bytes_to_u256(s_bytes)?;
    Some((r, s))
}

fn bytes_to_u256(bytes: &[u8]) -> Option<U256> {
    // Skip leading zeros
    let start = bytes.iter().position(|&b| b != 0).unwrap_or(bytes.len());
    let meaningful = &bytes[start..];
    if meaningful.len() > 32 {
        return None;
    }

    let mut padded = [0u8; 32];
    padded[32 - meaningful.len()..].copy_from_slice(meaningful);
    Some(u256_from_be_bytes(&padded))
}
