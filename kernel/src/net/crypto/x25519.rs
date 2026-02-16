//! X25519 Diffie-Hellman key exchange — RFC 7748
//!
//! Uses Curve25519 (Montgomery form) with radix-2^51 limb representation.

#![allow(dead_code)]

/// X25519 scalar multiplication: compute `scalar * point`.
///
/// Both `scalar` and `point` are 32-byte little-endian values.
/// Returns the 32-byte result (shared secret).
pub fn x25519(scalar: &[u8; 32], point: &[u8; 32]) -> [u8; 32] {
    let mut k = *scalar;
    // Clamp scalar per RFC 7748 §5
    k[0] &= 248;
    k[31] &= 127;
    k[31] |= 64;

    let u = fe_decode(point);
    let result = montgomery_ladder(&k, &u);
    fe_encode(&result)
}

/// X25519 with the standard base point (u=9).
pub fn x25519_basepoint(scalar: &[u8; 32]) -> [u8; 32] {
    let basepoint: [u8; 32] = {
        let mut bp = [0u8; 32];
        bp[0] = 9;
        bp
    };
    x25519(scalar, &basepoint)
}

// ── Field element: GF(2^255-19), radix 2^51 ────────────────

/// Field element: 5 limbs, each < 2^52 (allows lazy reduction).
type Fe = [u64; 5];

const FE_ZERO: Fe = [0; 5];
const FE_ONE: Fe = [1, 0, 0, 0, 0];

/// Reduction constant: 2^255 ≡ 19 (mod p).
const REDUCE_MASK: u64 = (1u64 << 51) - 1;

fn fe_decode(bytes: &[u8; 32]) -> Fe {
    let mut f = FE_ZERO;
    // Read 256 bits little-endian, split into 5 × 51-bit limbs
    let load8 = |b: &[u8]| -> u64 {
        let mut v = 0u64;
        for i in 0..b.len().min(8) {
            v |= (b[i] as u64) << (8 * i);
        }
        v
    };
    f[0] = load8(&bytes[0..]) & REDUCE_MASK;
    f[1] = (load8(&bytes[6..]) >> 3) & REDUCE_MASK;
    f[2] = (load8(&bytes[12..]) >> 6) & REDUCE_MASK;
    f[3] = (load8(&bytes[19..]) >> 1) & REDUCE_MASK;
    f[4] = (load8(&bytes[24..]) >> 12) & REDUCE_MASK;
    f
}

fn fe_encode(f: &Fe) -> [u8; 32] {
    let mut h = *f;
    fe_reduce(&mut h);

    let mut out = [0u8; 32];
    let mut q: u64;
    // Full reduction to [0, p)
    q = (h[0] + 19) >> 51;
    q = (h[1] + q) >> 51;
    q = (h[2] + q) >> 51;
    q = (h[3] + q) >> 51;
    q = (h[4] + q) >> 51;

    h[0] += 19 * q;
    let carry = h[0] >> 51;
    h[0] &= REDUCE_MASK;
    h[1] += carry;
    let carry = h[1] >> 51;
    h[1] &= REDUCE_MASK;
    h[2] += carry;
    let carry = h[2] >> 51;
    h[2] &= REDUCE_MASK;
    h[3] += carry;
    let carry = h[3] >> 51;
    h[3] &= REDUCE_MASK;
    h[4] += carry;
    h[4] &= REDUCE_MASK;

    let v: u128 = (h[0] as u128) | ((h[1] as u128) << 51) | ((h[2] as u128) << 102);
    let w: u128 = ((h[2] as u128) >> 26) | ((h[3] as u128) << 25) | ((h[4] as u128) << 76);

    for i in 0..16 {
        out[i] = (v >> (8 * i)) as u8;
    }
    for i in 0..16 {
        out[16 + i] = (w >> (8 * i)) as u8;
    }
    // Mask the top bit (255-bit field)
    out[31] &= 0x7f;
    out
}

fn fe_reduce(f: &mut Fe) {
    let carry = f[0] >> 51;
    f[0] &= REDUCE_MASK;
    f[1] += carry;
    let carry = f[1] >> 51;
    f[1] &= REDUCE_MASK;
    f[2] += carry;
    let carry = f[2] >> 51;
    f[2] &= REDUCE_MASK;
    f[3] += carry;
    let carry = f[3] >> 51;
    f[3] &= REDUCE_MASK;
    f[4] += carry;
    let carry = f[4] >> 51;
    f[4] &= REDUCE_MASK;
    f[0] += carry * 19; // 2^255 ≡ 19
}

fn fe_add(a: &Fe, b: &Fe) -> Fe {
    [
        a[0] + b[0],
        a[1] + b[1],
        a[2] + b[2],
        a[3] + b[3],
        a[4] + b[4],
    ]
}

fn fe_sub(a: &Fe, b: &Fe) -> Fe {
    // Add 2*p to avoid underflow
    const TWO_P: [u64; 5] = [
        0xFFFFFFFFFFFDA,
        0x7FFFFFFFFFFFF,
        0x7FFFFFFFFFFFF,
        0x7FFFFFFFFFFFF,
        0x7FFFFFFFFFFFF,
    ];
    [
        a[0] + TWO_P[0] - b[0],
        a[1] + TWO_P[1] - b[1],
        a[2] + TWO_P[2] - b[2],
        a[3] + TWO_P[3] - b[3],
        a[4] + TWO_P[4] - b[4],
    ]
}

fn fe_mul(a: &Fe, b: &Fe) -> Fe {
    let (a0, a1, a2, a3, a4) = (
        a[0] as u128,
        a[1] as u128,
        a[2] as u128,
        a[3] as u128,
        a[4] as u128,
    );
    let (b0, b1, b2, b3, b4) = (
        b[0] as u128,
        b[1] as u128,
        b[2] as u128,
        b[3] as u128,
        b[4] as u128,
    );

    let b1_19 = b1 * 19;
    let b2_19 = b2 * 19;
    let b3_19 = b3 * 19;
    let b4_19 = b4 * 19;

    let mut r0 = a0 * b0 + a1 * b4_19 + a2 * b3_19 + a3 * b2_19 + a4 * b1_19;
    let mut r1 = a0 * b1 + a1 * b0 + a2 * b4_19 + a3 * b3_19 + a4 * b2_19;
    let mut r2 = a0 * b2 + a1 * b1 + a2 * b0 + a3 * b4_19 + a4 * b3_19;
    let mut r3 = a0 * b3 + a1 * b2 + a2 * b1 + a3 * b0 + a4 * b4_19;
    let mut r4 = a0 * b4 + a1 * b3 + a2 * b2 + a3 * b1 + a4 * b0;

    let mask = REDUCE_MASK as u128;
    let c = r0 >> 51;
    r0 &= mask;
    r1 += c;
    let c = r1 >> 51;
    r1 &= mask;
    r2 += c;
    let c = r2 >> 51;
    r2 &= mask;
    r3 += c;
    let c = r3 >> 51;
    r3 &= mask;
    r4 += c;
    let c = r4 >> 51;
    r4 &= mask;
    r0 += c * 19;
    let c = r0 >> 51;
    r0 &= mask;
    r1 += c;

    [r0 as u64, r1 as u64, r2 as u64, r3 as u64, r4 as u64]
}

fn fe_sq(a: &Fe) -> Fe {
    fe_mul(a, a)
}

fn fe_mul_scalar(a: &Fe, s: u64) -> Fe {
    let s = s as u128;
    let mask = REDUCE_MASK as u128;
    let mut r0 = (a[0] as u128) * s;
    let mut r1 = (a[1] as u128) * s;
    let mut r2 = (a[2] as u128) * s;
    let mut r3 = (a[3] as u128) * s;
    let mut r4 = (a[4] as u128) * s;

    let c = r0 >> 51;
    r0 &= mask;
    r1 += c;
    let c = r1 >> 51;
    r1 &= mask;
    r2 += c;
    let c = r2 >> 51;
    r2 &= mask;
    r3 += c;
    let c = r3 >> 51;
    r3 &= mask;
    r4 += c;
    let c = r4 >> 51;
    r4 &= mask;
    r0 += c * 19;

    [r0 as u64, r1 as u64, r2 as u64, r3 as u64, r4 as u64]
}

/// Compute a^(p-2) mod p using Fermat's little theorem for inverse.
/// p = 2^255 - 19, so p-2 = 2^255 - 21.
fn fe_invert(a: &Fe) -> Fe {
    // Use an addition chain for 2^255-21
    let z2 = fe_sq(a); // a^2
    let z8 = {
        let t = fe_sq(&z2);
        fe_sq(&t)
    }; // a^8
    let z9 = fe_mul(a, &z8); // a^9
    let z11 = fe_mul(&z2, &z9); // a^11
    let z22 = fe_sq(&z11); // a^22
    let z_5_0 = fe_mul(&z9, &z22); // a^(2^5-1)

    let mut t = fe_sq(&z_5_0);
    for _ in 1..5 {
        t = fe_sq(&t);
    }
    let z_10_0 = fe_mul(&z_5_0, &t); // a^(2^10-1)

    t = fe_sq(&z_10_0);
    for _ in 1..10 {
        t = fe_sq(&t);
    }
    let z_20_0 = fe_mul(&z_10_0, &t); // a^(2^20-1)

    t = fe_sq(&z_20_0);
    for _ in 1..20 {
        t = fe_sq(&t);
    }
    t = fe_mul(&z_20_0, &t); // a^(2^40-1)

    t = fe_sq(&t);
    for _ in 1..10 {
        t = fe_sq(&t);
    }
    let z_50_0 = fe_mul(&z_10_0, &t); // a^(2^50-1)

    t = fe_sq(&z_50_0);
    for _ in 1..50 {
        t = fe_sq(&t);
    }
    let z_100_0 = fe_mul(&z_50_0, &t); // a^(2^100-1)

    t = fe_sq(&z_100_0);
    for _ in 1..100 {
        t = fe_sq(&t);
    }
    t = fe_mul(&z_100_0, &t); // a^(2^200-1)

    t = fe_sq(&t);
    for _ in 1..50 {
        t = fe_sq(&t);
    }
    t = fe_mul(&z_50_0, &t); // a^(2^250-1)

    t = fe_sq(&t);
    t = fe_sq(&t);
    t = fe_mul(a, &t); // a^(2^252-3)

    t = fe_sq(&t);
    t = fe_sq(&t);
    t = fe_sq(&t);
    fe_mul(&z11, &t) // a^(2^255-21) = a^(p-2)
}

/// Constant-time conditional swap.
fn fe_cswap(a: &mut Fe, b: &mut Fe, swap: u64) {
    let mask = 0u64.wrapping_sub(swap); // 0 or all 1s
    for i in 0..5 {
        let t = mask & (a[i] ^ b[i]);
        a[i] ^= t;
        b[i] ^= t;
    }
}

// ── Montgomery ladder ───────────────────────────────────────

/// a24 = (A-2)/4 = 121665 for Curve25519 (A = 486662).
const A24: u64 = 121666;

fn montgomery_ladder(k: &[u8; 32], u: &Fe) -> Fe {
    let mut x_2 = FE_ONE;
    let mut z_2 = FE_ZERO;
    let mut x_3 = *u;
    let mut z_3 = FE_ONE;
    let mut swap: u64 = 0;

    for t in (0..255).rev() {
        let byte = t / 8;
        let bit = t % 8;
        let k_t = ((k[byte] >> bit) & 1) as u64;

        swap ^= k_t;
        fe_cswap(&mut x_2, &mut x_3, swap);
        fe_cswap(&mut z_2, &mut z_3, swap);
        swap = k_t;

        let a = fe_add(&x_2, &z_2);
        let mut aa = fe_sq(&a);
        let b = fe_sub(&x_2, &z_2);
        let mut bb = fe_sq(&b);
        let e = fe_sub(&aa, &bb);
        let cc = fe_add(&x_3, &z_3);
        let dd = fe_sub(&x_3, &z_3);
        let da = fe_mul(&dd, &a);
        let cb = fe_mul(&cc, &b);

        let sum = fe_add(&da, &cb);
        x_3 = fe_sq(&sum);
        let diff = fe_sub(&da, &cb);
        let diff_sq = fe_sq(&diff);
        z_3 = fe_mul(u, &diff_sq);

        x_2 = fe_mul(&aa, &bb);
        let a24_e = fe_mul_scalar(&e, A24);
        aa = fe_add(&aa, &a24_e);
        fe_reduce(&mut aa);
        fe_reduce(&mut bb);
        z_2 = fe_mul(&e, &aa);
    }

    fe_cswap(&mut x_2, &mut x_3, swap);
    fe_cswap(&mut z_2, &mut z_3, swap);

    // Return x_2 / z_2  (= x_2 * z_2^(p-2))
    let z_inv = fe_invert(&z_2);
    fe_mul(&x_2, &z_inv)
}
