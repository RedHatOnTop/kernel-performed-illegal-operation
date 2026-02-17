// WASI Preview 2 — Random (wasi:random/random + insecure + insecure-seed)
//
// This module wraps the existing xorshift64 RNG into the WASI P2
// random interfaces.

extern crate alloc;

use alloc::vec::Vec;

// ---------------------------------------------------------------------------
// Random Generator
// ---------------------------------------------------------------------------

/// WASI P2 random number generator.
///
/// Uses xorshift64* for deterministic PRNG. Not cryptographically secure
/// but suitable for the kernel environment.
pub struct RandomGenerator {
    state: u64,
}

impl RandomGenerator {
    /// Create a new RNG with a default seed.
    pub fn new() -> Self {
        Self {
            state: 0x1234_5678_9ABC_DEF0,
        }
    }

    /// Create a new RNG with a specific seed.
    pub fn with_seed(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    /// xorshift64* step.
    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    // --- wasi:random/random ---

    /// Get `len` random bytes (cryptographic quality — best effort).
    pub fn get_random_bytes(&mut self, len: usize) -> Vec<u8> {
        let mut result = Vec::with_capacity(len);
        let mut remaining = len;
        while remaining > 0 {
            let val = self.next_u64();
            let bytes = val.to_le_bytes();
            let take = remaining.min(8);
            result.extend_from_slice(&bytes[..take]);
            remaining -= take;
        }
        result
    }

    /// Get a random u64.
    pub fn get_random_u64(&mut self) -> u64 {
        self.next_u64()
    }

    // --- wasi:random/insecure ---

    /// Get `len` insecure random bytes (faster, not for crypto).
    pub fn get_insecure_random_bytes(&mut self, len: usize) -> Vec<u8> {
        // Same implementation — in a real system, this would be faster
        // but less secure.
        self.get_random_bytes(len)
    }

    /// Get an insecure random u64.
    pub fn get_insecure_random_u64(&mut self) -> u64 {
        self.next_u64()
    }

    // --- wasi:random/insecure-seed ---

    /// Get a pair of u64 values that can be used to seed other PRNGs.
    pub fn insecure_seed(&mut self) -> (u64, u64) {
        (self.next_u64(), self.next_u64())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_bytes_correct_length() {
        let mut rng = RandomGenerator::new();
        let bytes = rng.get_random_bytes(32);
        assert_eq!(bytes.len(), 32);
    }

    #[test]
    fn random_bytes_not_all_zeros() {
        let mut rng = RandomGenerator::new();
        let bytes = rng.get_random_bytes(16);
        assert!(bytes.iter().any(|&b| b != 0));
    }

    #[test]
    fn random_u64_different_values() {
        let mut rng = RandomGenerator::new();
        let a = rng.get_random_u64();
        let b = rng.get_random_u64();
        assert_ne!(a, b);
    }

    #[test]
    fn insecure_random_bytes() {
        let mut rng = RandomGenerator::new();
        let bytes = rng.get_insecure_random_bytes(8);
        assert_eq!(bytes.len(), 8);
        assert!(bytes.iter().any(|&b| b != 0));
    }

    #[test]
    fn insecure_seed_returns_pair() {
        let mut rng = RandomGenerator::new();
        let (a, b) = rng.insecure_seed();
        assert_ne!(a, 0);
        assert_ne!(b, 0);
        assert_ne!(a, b);
    }

    #[test]
    fn deterministic_with_same_seed() {
        let mut rng1 = RandomGenerator::with_seed(42);
        let mut rng2 = RandomGenerator::with_seed(42);
        assert_eq!(rng1.get_random_u64(), rng2.get_random_u64());
        assert_eq!(rng1.get_random_bytes(16), rng2.get_random_bytes(16));
    }

    #[test]
    fn different_seeds_different_output() {
        let mut rng1 = RandomGenerator::with_seed(1);
        let mut rng2 = RandomGenerator::with_seed(2);
        assert_ne!(rng1.get_random_u64(), rng2.get_random_u64());
    }

    #[test]
    fn random_bytes_edge_cases() {
        let mut rng = RandomGenerator::new();
        // Zero length
        let empty = rng.get_random_bytes(0);
        assert!(empty.is_empty());
        // One byte
        let one = rng.get_random_bytes(1);
        assert_eq!(one.len(), 1);
        // Non-aligned (not multiple of 8)
        let seven = rng.get_random_bytes(7);
        assert_eq!(seven.len(), 7);
    }
}
