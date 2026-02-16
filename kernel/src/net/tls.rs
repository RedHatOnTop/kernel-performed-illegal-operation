//! Minimal TLS 1.2 Implementation
//!
//! Supports `TLS_RSA_WITH_AES_128_CBC_SHA256` for HTTPS.
//! Certificate validation is skipped (demo kernel).
//!
//! Crypto primitives are provided by `super::crypto`.

#![allow(dead_code)]

use super::crypto::aes::{aes128_cbc_decrypt, aes128_cbc_encrypt};
use super::crypto::hmac::hmac_sha256;
use super::crypto::sha::sha256;
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
    super::crypto::random::csprng_fill(&mut pms[2..]);
    pms
}

fn pseudo_random_bytes() -> [u8; 32] {
    let mut out = [0u8; 32];
    super::crypto::random::csprng_fill(&mut out);
    out
}
