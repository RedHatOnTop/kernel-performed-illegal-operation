//! Cryptographic primitives for TLS, HTTPS, and secure communications.
//!
//! All implementations are pure Rust, `no_std` compatible, designed for
//! the KPIO kernel environment.
//!
//! Primitives provided:
//!   - **Hash**:   SHA-256, SHA-384, SHA-512
//!   - **MAC**:    HMAC-SHA-256, HMAC-SHA-384
//!   - **KDF**:    HKDF-Extract, HKDF-Expand, HKDF-Expand-Label (TLS 1.3)
//!   - **AEAD**:   AES-128-GCM, AES-256-GCM, ChaCha20-Poly1305
//!   - **KE**:     X25519 ECDH, P-256 ECDH
//!   - **Sig**:    ECDSA (P-256) verification, RSA PKCS#1 v1.5 verification
//!   - **PRNG**:   CSPRNG (RDRAND + ChaCha20)

pub mod aes;
pub mod aes_gcm;
pub mod chacha20;
pub mod hkdf;
pub mod hmac;
pub mod p256;
pub mod random;
pub mod rsa;
pub mod sha;
pub mod x25519;

// Convenience re-exports
pub use aes_gcm::{aes128_gcm_open, aes128_gcm_seal, aes256_gcm_open, aes256_gcm_seal};
pub use chacha20::{chacha20_poly1305_open, chacha20_poly1305_seal};
pub use hkdf::{derive_secret, hkdf_expand, hkdf_expand_label, hkdf_extract};
pub use hmac::{hmac_sha256, hmac_sha384};
pub use random::csprng_fill;
pub use sha::{sha256, sha384, sha512};
pub use x25519::{x25519, x25519_basepoint};
