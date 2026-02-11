//! CSPRNG — RDRAND-seeded, ChaCha20-based PRNG
//!
//! Uses x86 RDRAND for hardware entropy.  Falls back to a timestamp-seeded
//! LCG when RDRAND is unavailable (e.g. old CPUs, some VMs).

use spin::Mutex;

/// Global PRNG state (ChaCha20-based) seeded once at first use.
static RNG: Mutex<ChaChaRng> = Mutex::new(ChaChaRng::new_unseeded());

/// Fill `dest` with cryptographically strong random bytes.
pub fn csprng_fill(dest: &mut [u8]) {
    let mut rng = RNG.lock();
    if !rng.seeded {
        rng.seed();
    }
    rng.fill(dest);
}

/// Generate `n` random bytes.
pub fn csprng_bytes(n: usize) -> alloc::vec::Vec<u8> {
    let mut buf = alloc::vec![0u8; n];
    csprng_fill(&mut buf);
    buf
}

/// Return a single random u64.
pub fn csprng_u64() -> u64 {
    let mut buf = [0u8; 8];
    csprng_fill(&mut buf);
    u64::from_le_bytes(buf)
}

// ─── ChaCha20 core (used internally for the PRNG stream) ───

struct ChaChaRng {
    state: [u32; 16],
    buffer: [u8; 64],
    buf_pos: usize,
    seeded: bool,
}

impl ChaChaRng {
    const fn new_unseeded() -> Self {
        Self {
            state: [0; 16],
            buffer: [0; 64],
            buf_pos: 64,
            seeded: false,
        }
    }

    /// Seed from RDRAND (or fallback).
    fn seed(&mut self) {
        // "expand 32-byte k"
        self.state[0] = 0x61707865;
        self.state[1] = 0x3320646e;
        self.state[2] = 0x79622d32;
        self.state[3] = 0x6b206574;

        // 256-bit key from hardware RNG
        for i in 4..12 {
            self.state[i] = rdrand_u32();
        }
        // Counter = 0
        self.state[12] = 0;
        self.state[13] = 0;
        // 64-bit nonce from hardware RNG
        self.state[14] = rdrand_u32();
        self.state[15] = rdrand_u32();

        self.buf_pos = 64; // force generation on first use
        self.seeded = true;
    }

    fn fill(&mut self, dest: &mut [u8]) {
        let mut off = 0;
        while off < dest.len() {
            if self.buf_pos >= 64 {
                self.generate_block();
            }
            let avail = 64 - self.buf_pos;
            let take = avail.min(dest.len() - off);
            dest[off..off + take].copy_from_slice(&self.buffer[self.buf_pos..self.buf_pos + take]);
            self.buf_pos += take;
            off += take;
        }
    }

    fn generate_block(&mut self) {
        let mut x = self.state;
        for _ in 0..10 {
            // column rounds
            quarter_round(&mut x, 0, 4,  8, 12);
            quarter_round(&mut x, 1, 5,  9, 13);
            quarter_round(&mut x, 2, 6, 10, 14);
            quarter_round(&mut x, 3, 7, 11, 15);
            // diagonal rounds
            quarter_round(&mut x, 0, 5, 10, 15);
            quarter_round(&mut x, 1, 6, 11, 12);
            quarter_round(&mut x, 2, 7,  8, 13);
            quarter_round(&mut x, 3, 4,  9, 14);
        }
        for i in 0..16 {
            x[i] = x[i].wrapping_add(self.state[i]);
        }
        for i in 0..16 {
            let bytes = x[i].to_le_bytes();
            self.buffer[i * 4..i * 4 + 4].copy_from_slice(&bytes);
        }
        // Increment 64-bit counter
        self.state[12] = self.state[12].wrapping_add(1);
        if self.state[12] == 0 {
            self.state[13] = self.state[13].wrapping_add(1);
        }
        self.buf_pos = 0;
    }
}

#[inline]
fn quarter_round(x: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    x[a] = x[a].wrapping_add(x[b]); x[d] ^= x[a]; x[d] = x[d].rotate_left(16);
    x[c] = x[c].wrapping_add(x[d]); x[b] ^= x[c]; x[b] = x[b].rotate_left(12);
    x[a] = x[a].wrapping_add(x[b]); x[d] ^= x[a]; x[d] = x[d].rotate_left(8);
    x[c] = x[c].wrapping_add(x[d]); x[b] ^= x[c]; x[b] = x[b].rotate_left(7);
}

// ── RDRAND wrapper ──────────────────────────────────────────

/// Try to read a hardware random u32 via RDRAND.
/// Falls back to a timestamp-based LCG if RDRAND is not available.
fn rdrand_u32() -> u32 {
    #[cfg(target_arch = "x86_64")]
    {
        let mut val: u32;
        let ok: u8;
        unsafe {
            core::arch::asm!(
                "rdrand {val:e}",
                "setc {ok}",
                val = out(reg) val,
                ok = out(reg_byte) ok,
                options(nomem, nostack),
            );
        }
        if ok != 0 {
            return val;
        }
    }
    // Fallback: timestamp-seeded LCG
    let tsc = read_tsc();
    let mut s = tsc ^ 0xDEAD_BEEF_CAFE_BABE;
    s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (s >> 33) as u32
}

#[inline]
fn read_tsc() -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        let lo: u32;
        let hi: u32;
        unsafe {
            core::arch::asm!("rdtsc", out("eax") lo, out("edx") hi, options(nomem, nostack));
        }
        ((hi as u64) << 32) | (lo as u64)
    }
    #[cfg(not(target_arch = "x86_64"))]
    { 0 }
}
