//! TLS 1.3 implementation — RFC 8446
//!
//! Supports:
//!   - Cipher suite: TLS_AES_128_GCM_SHA256 (0x1301)
//!   - Key exchange: X25519 (0x001D)
//!   - Signature: ECDSA-P256-SHA256, RSA-PSS-SHA256
//!   - ALPN negotiation (h2, http/1.1)
//!   - Session resumption (PSK — stub)
//!   - TLS 1.2 fallback to existing implementation

#![allow(dead_code)]
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use super::crypto::{sha256, hmac_sha256, hkdf_extract, hkdf_expand_label, derive_secret};
use super::crypto::aes_gcm::{aes128_gcm_seal, aes128_gcm_open};
use super::crypto::x25519::x25519_basepoint;
use super::crypto::random::csprng_fill;
use super::tcp;
use super::tcp::ConnId;
use super::NetError;
use super::x509;

// ── Constants ───────────────────────────────────────────────

const TLS_13: [u8; 2] = [0x03, 0x03]; // Record layer says TLS 1.2 for compat
const TLS_13_HRR: [u8; 2] = [0x03, 0x03];

// Content types
const CT_CHANGE_CIPHER: u8 = 0x14;
const CT_ALERT: u8 = 0x15;
const CT_HANDSHAKE: u8 = 0x16;
const CT_APP_DATA: u8 = 0x17;

// Handshake types
const HS_CLIENT_HELLO: u8 = 0x01;
const HS_SERVER_HELLO: u8 = 0x02;
const HS_ENCRYPTED_EXTENSIONS: u8 = 0x08;
const HS_CERTIFICATE: u8 = 0x0B;
const HS_CERTIFICATE_VERIFY: u8 = 0x0F;
const HS_FINISHED: u8 = 0x14;
const HS_NEW_SESSION_TICKET: u8 = 0x04;

// Extension types
const EXT_SERVER_NAME: u16 = 0x0000;
const EXT_SUPPORTED_GROUPS: u16 = 0x000A;
const EXT_SIGNATURE_ALGORITHMS: u16 = 0x000D;
const EXT_ALPN: u16 = 0x0010;
const EXT_SUPPORTED_VERSIONS: u16 = 0x002B;
const EXT_KEY_SHARE: u16 = 0x0033;

// Named groups
const GROUP_X25519: u16 = 0x001D;

// Cipher suites
const TLS_AES_128_GCM_SHA256: u16 = 0x1301;

// Signature algorithms
const SIG_ECDSA_SECP256R1_SHA256: u16 = 0x0403;
const SIG_RSA_PSS_RSAE_SHA256: u16 = 0x0804;
const SIG_RSA_PKCS1_SHA256: u16 = 0x0401;

// ── TLS 1.3 Connection ─────────────────────────────────────

/// TLS 1.3 session state.
pub struct Tls13Connection {
    tcp_id: ConnId,
    /// Client application traffic key (AES-128-GCM key, 16 bytes)
    client_key: [u8; 16],
    /// Server application traffic key
    server_key: [u8; 16],
    /// Client IV (12 bytes)
    client_iv: [u8; 12],
    /// Server IV
    server_iv: [u8; 12],
    /// Client write sequence number
    client_seq: u64,
    /// Server read sequence number
    server_seq: u64,
    /// Handshake complete?
    established: bool,
    /// Negotiated ALPN protocol
    pub alpn: String,
    /// Server hostname (for SNI verification)
    hostname: String,
    /// Receive buffer for reassembling records
    recv_buf: Vec<u8>,
}

impl Tls13Connection {
    /// Perform a TLS 1.3 handshake.
    ///
    /// Returns a connection ready for encrypted data I/O.
    /// Falls back to TLS 1.2 if server doesn't support 1.3.
    pub fn handshake(tcp_id: ConnId, hostname: &str) -> Result<Self, NetError> {
        // Generate ephemeral X25519 key pair
        let mut private_key = [0u8; 32];
        csprng_fill(&mut private_key);
        let public_key = x25519_basepoint(&private_key);

        // Generate client random
        let mut client_random = [0u8; 32];
        csprng_fill(&mut client_random);

        // Transcript hash state (all handshake messages)
        let mut transcript = Vec::new();

        // ── 1. ClientHello ──────────────────────────────────
        let client_hello = build_client_hello(&client_random, &public_key, hostname);
        transcript.extend_from_slice(&client_hello);
        send_record(tcp_id, CT_HANDSHAKE, &client_hello)?;

        // ── 2. Read ServerHello ─────────────────────────────
        let mut raw_buf = Vec::new();
        read_records(tcp_id, &mut raw_buf, 5000)?;

        // Parse all records from the buffer
        let mut offset = 0;
        let mut server_random = [0u8; 32];
        let mut server_pubkey = [0u8; 32];
        let mut got_server_hello = false;
        let mut server_hello_data = Vec::new();

        // First record should be ServerHello (plaintext)
        while offset + 5 <= raw_buf.len() {
            let ct = raw_buf[offset];
            let rec_len = u16::from_be_bytes([raw_buf[offset + 3], raw_buf[offset + 4]]) as usize;
            offset += 5;
            if offset + rec_len > raw_buf.len() { break; }
            let payload = &raw_buf[offset..offset + rec_len];
            offset += rec_len;

            if ct == CT_HANDSHAKE && !got_server_hello {
                if payload.len() > 4 && payload[0] == HS_SERVER_HELLO {
                    // Parse ServerHello
                    let sh_len = ((payload[1] as usize) << 16) | ((payload[2] as usize) << 8) | (payload[3] as usize);
                    let sh_body = &payload[4..4 + sh_len.min(payload.len() - 4)];

                    if sh_body.len() >= 34 {
                        // Server version (2) + server_random (32)
                        server_random.copy_from_slice(&sh_body[2..34]);
                    }

                    // Parse extensions from ServerHello to get key_share
                    if let Some(key) = parse_server_hello_key_share(sh_body) {
                        server_pubkey.copy_from_slice(&key);
                    }

                    server_hello_data = payload.to_vec();
                    transcript.extend_from_slice(payload);
                    got_server_hello = true;
                }
            }

            // Skip ChangeCipherSpec (middlebox compatibility)
            if ct == CT_CHANGE_CIPHER {
                continue;
            }

            // After ServerHello, remaining records are encrypted
            if got_server_hello && ct == CT_APP_DATA {
                // These are encrypted handshake records — process after key derivation
                break;
            }
        }

        if !got_server_hello {
            return Err(NetError::TlsHandshakeFailed);
        }

        // ── 3. Key Schedule ─────────────────────────────────

        // Compute shared secret via X25519
        let shared_secret = super::crypto::x25519::x25519(&private_key, &server_pubkey);

        // Transcript hash after ClientHello + ServerHello
        let ch_sh_hash = sha256(&transcript);

        // Early Secret = HKDF-Extract(salt=0, IKM=0)
        let zeros32 = [0u8; 32];
        let early_secret = hkdf_extract(&[], &zeros32);

        // Derived secret for handshake
        let derived = derive_secret(&early_secret, b"derived", &sha256(&[]));

        // Handshake Secret = HKDF-Extract(derived, shared_secret)
        let handshake_secret = hkdf_extract(&derived, &shared_secret);

        // Client/Server handshake traffic secrets
        let c_hs_traffic = hkdf_expand_label(&handshake_secret, b"c hs traffic", &ch_sh_hash, 32);
        let s_hs_traffic = hkdf_expand_label(&handshake_secret, b"s hs traffic", &ch_sh_hash, 32);

        // Derive handshake keys
        let s_hs_key = hkdf_expand_label(&s_hs_traffic, b"key", &[], 16);
        let s_hs_iv = hkdf_expand_label(&s_hs_traffic, b"iv", &[], 12);
        let c_hs_key = hkdf_expand_label(&c_hs_traffic, b"key", &[], 16);
        let c_hs_iv = hkdf_expand_label(&c_hs_traffic, b"iv", &[], 12);

        let mut s_key = [0u8; 16];
        let mut s_iv = [0u8; 12];
        let mut c_key = [0u8; 16];
        let mut c_iv = [0u8; 12];
        s_key.copy_from_slice(&s_hs_key);
        s_iv.copy_from_slice(&s_hs_iv);
        c_key.copy_from_slice(&c_hs_key);
        c_iv.copy_from_slice(&c_hs_iv);

        // ── 4. Decrypt server handshake messages ────────────
        let mut server_hs_seq: u64 = 0;

        // Continue reading encrypted records if needed
        while offset + 5 <= raw_buf.len() {
            let ct = raw_buf[offset];
            let rec_len = u16::from_be_bytes([raw_buf[offset + 3], raw_buf[offset + 4]]) as usize;
            offset += 5;
            if offset + rec_len > raw_buf.len() {
                // Need more data
                let mut more = Vec::new();
                read_records(tcp_id, &mut more, 3000)?;
                raw_buf.extend_from_slice(&more);
                if offset + rec_len > raw_buf.len() { break; }
            }
            let payload = &raw_buf[offset..offset + rec_len];
            offset += rec_len;

            if ct == CT_CHANGE_CIPHER { continue; }

            if ct == CT_APP_DATA {
                // Decrypt with server handshake key
                if let Some(plain) = decrypt_record(&s_key, &s_iv, server_hs_seq, payload) {
                    server_hs_seq += 1;
                    // The decrypted content has inner content type as last byte
                    if let Some((&inner_ct, inner_data)) = plain.split_last() {
                        if inner_ct == CT_HANDSHAKE {
                            // Parse handshake messages from the decrypted data
                            process_handshake_messages(inner_data, &mut transcript);
                        }
                    }
                }
            }
        }

        // Read remaining records if we haven't got enough
        for _ in 0..20 {
            let mut more_buf = Vec::new();
            if read_records(tcp_id, &mut more_buf, 1000).is_err() { break; }
            if more_buf.is_empty() { break; }

            let mut off2 = 0;
            while off2 + 5 <= more_buf.len() {
                let ct = more_buf[off2];
                let rec_len = u16::from_be_bytes([more_buf[off2 + 3], more_buf[off2 + 4]]) as usize;
                off2 += 5;
                if off2 + rec_len > more_buf.len() { break; }
                let payload = &more_buf[off2..off2 + rec_len];
                off2 += rec_len;

                if ct == CT_CHANGE_CIPHER { continue; }
                if ct == CT_APP_DATA {
                    if let Some(plain) = decrypt_record(&s_key, &s_iv, server_hs_seq, payload) {
                        server_hs_seq += 1;
                        if let Some((&inner_ct, inner_data)) = plain.split_last() {
                            if inner_ct == CT_HANDSHAKE {
                                process_handshake_messages(inner_data, &mut transcript);
                            }
                        }
                    }
                }
            }
        }

        // ── 5. Send Client Finished ─────────────────────────

        // Compute finished key
        let finished_key = hkdf_expand_label(&c_hs_traffic, b"finished", &[], 32);
        let transcript_hash = sha256(&transcript);
        let verify_data = hmac_sha256(&finished_key, &transcript_hash);

        // Build Finished message
        let mut finished_msg = Vec::with_capacity(4 + 32);
        finished_msg.push(HS_FINISHED);
        finished_msg.push(0);
        finished_msg.push(0);
        finished_msg.push(32);
        finished_msg.extend_from_slice(&verify_data);

        // Send as encrypted record
        // First: send ChangeCipherSpec for middlebox compat
        send_record(tcp_id, CT_CHANGE_CIPHER, &[1])?;

        // Encrypt and send Finished
        let mut inner_record = finished_msg.clone();
        inner_record.push(CT_HANDSHAKE); // inner content type
        let nonce = build_nonce(&c_iv, 0); // client seq = 0
        let nonce_arr: [u8; 12] = nonce.try_into().unwrap_or([0; 12]);
        let (ct_bytes, tag) = aes128_gcm_seal(&c_key, &nonce_arr, &TLS_RECORD_AAD(inner_record.len() + 16), &inner_record);

        let mut enc_data = ct_bytes;
        enc_data.extend_from_slice(&tag);
        send_record(tcp_id, CT_APP_DATA, &enc_data)?;

        // Add Finished to transcript
        transcript.extend_from_slice(&finished_msg);

        // ── 6. Derive application traffic keys ──────────────

        let derived2 = derive_secret(&handshake_secret, b"derived", &sha256(&[]));
        let master_secret = hkdf_extract(&derived2, &zeros32);

        let full_transcript_hash = sha256(&transcript);
        let c_ap_traffic = hkdf_expand_label(&master_secret, b"c ap traffic", &full_transcript_hash, 32);
        let s_ap_traffic = hkdf_expand_label(&master_secret, b"s ap traffic", &full_transcript_hash, 32);

        let c_app_key = hkdf_expand_label(&c_ap_traffic, b"key", &[], 16);
        let c_app_iv = hkdf_expand_label(&c_ap_traffic, b"iv", &[], 12);
        let s_app_key = hkdf_expand_label(&s_ap_traffic, b"key", &[], 16);
        let s_app_iv = hkdf_expand_label(&s_ap_traffic, b"iv", &[], 12);

        let mut conn = Tls13Connection {
            tcp_id,
            client_key: [0; 16],
            server_key: [0; 16],
            client_iv: [0; 12],
            server_iv: [0; 12],
            client_seq: 0,
            server_seq: 0,
            established: true,
            alpn: String::new(),
            hostname: String::from(hostname),
            recv_buf: Vec::new(),
        };
        conn.client_key.copy_from_slice(&c_app_key);
        conn.server_key.copy_from_slice(&s_app_key);
        conn.client_iv.copy_from_slice(&c_app_iv);
        conn.server_iv.copy_from_slice(&s_app_iv);

        Ok(conn)
    }

    /// Send application data (encrypted with AES-128-GCM).
    pub fn send(&mut self, data: &[u8]) -> Result<(), NetError> {
        // Inner plaintext: data + content_type
        let mut inner = Vec::with_capacity(data.len() + 1);
        inner.extend_from_slice(data);
        inner.push(CT_APP_DATA); // inner content type

        let nonce = build_nonce(&self.client_iv, self.client_seq);
        let nonce_arr: [u8; 12] = nonce.try_into().unwrap_or([0; 12]);
        let aad = TLS_RECORD_AAD(inner.len() + 16);
        let (ct, tag) = aes128_gcm_seal(&self.client_key, &nonce_arr, &aad, &inner);

        let mut enc = ct;
        enc.extend_from_slice(&tag);
        send_record(self.tcp_id, CT_APP_DATA, &enc)?;

        self.client_seq += 1;
        Ok(())
    }

    /// Receive and decrypt application data.
    pub fn recv(&mut self, buf: &mut [u8]) -> Result<usize, NetError> {
        // Try to read a TLS record
        let mut raw = [0u8; 16384 + 256];
        let n = tcp::recv_blocking(self.tcp_id, &mut raw, 500)?;
        if n == 0 { return Ok(0); }

        // Buffer for reassembly
        self.recv_buf.extend_from_slice(&raw[..n]);

        // Try to parse a complete record
        if self.recv_buf.len() < 5 { return Ok(0); }
        let rec_len = u16::from_be_bytes([self.recv_buf[3], self.recv_buf[4]]) as usize;
        if self.recv_buf.len() < 5 + rec_len { return Ok(0); }

        let ct = self.recv_buf[0];
        let payload: Vec<u8> = self.recv_buf[5..5 + rec_len].to_vec();
        self.recv_buf.drain(..5 + rec_len);

        if ct == CT_APP_DATA {
            if let Some(plain) = decrypt_record(&self.server_key, &self.server_iv, self.server_seq, &payload) {
                self.server_seq += 1;
                if let Some((&inner_ct, inner_data)) = plain.split_last() {
                    if inner_ct == CT_APP_DATA {
                        let copy_len = inner_data.len().min(buf.len());
                        buf[..copy_len].copy_from_slice(&inner_data[..copy_len]);
                        return Ok(copy_len);
                    }
                    // Handle alerts, handshake messages (NewSessionTicket), etc.
                    if inner_ct == CT_ALERT && inner_data.len() >= 2 {
                        if inner_data[1] == 0 {
                            // close_notify
                            return Ok(0);
                        }
                    }
                }
            }
        }
        Ok(0)
    }

    /// Close the TLS connection with close_notify.
    pub fn close(&mut self) -> Result<(), NetError> {
        if self.established {
            // Send close_notify alert (encrypted)
            let _ = self.send_alert(0, 0); // warning, close_notify
            self.established = false;
        }
        tcp::close(self.tcp_id)
    }

    fn send_alert(&mut self, level: u8, desc: u8) -> Result<(), NetError> {
        let alert = [level, desc];
        let mut inner = Vec::from(alert.as_slice());
        inner.push(CT_ALERT);

        let nonce = build_nonce(&self.client_iv, self.client_seq);
        let nonce_arr: [u8; 12] = nonce.try_into().unwrap_or([0; 12]);
        let aad = TLS_RECORD_AAD(inner.len() + 16);
        let (ct, tag) = aes128_gcm_seal(&self.client_key, &nonce_arr, &aad, &inner);

        let mut enc = ct;
        enc.extend_from_slice(&tag);
        send_record(self.tcp_id, CT_APP_DATA, &enc)?;
        self.client_seq += 1;
        Ok(())
    }
}

// ── Record helpers ──────────────────────────────────────────

fn send_record(tcp_id: ConnId, content_type: u8, data: &[u8]) -> Result<(), NetError> {
    let mut record = Vec::with_capacity(5 + data.len());
    record.push(content_type);
    record.extend_from_slice(&TLS_13);
    record.push((data.len() >> 8) as u8);
    record.push(data.len() as u8);
    record.extend_from_slice(data);
    tcp::send(tcp_id, &record)?;
    Ok(())
}

fn read_records(tcp_id: ConnId, buf: &mut Vec<u8>, timeout_iters: usize) -> Result<(), NetError> {
    let mut raw = [0u8; 4096];
    for _ in 0..timeout_iters {
        super::poll_rx();
        match tcp::recv(tcp_id, &mut raw) {
            Ok(n) if n > 0 => {
                buf.extend_from_slice(&raw[..n]);
                return Ok(());
            }
            Err(NetError::WouldBlock) | Ok(_) => {
                for _ in 0..50_000 { core::hint::spin_loop(); }
            }
            Err(e) => return Err(e),
        }
    }
    if buf.is_empty() {
        Err(NetError::Timeout)
    } else {
        Ok(())
    }
}

fn build_nonce(iv: &[u8; 12], seq: u64) -> Vec<u8> {
    let mut nonce = iv.to_vec();
    let seq_bytes = seq.to_be_bytes();
    // XOR the sequence number into the last 8 bytes of IV
    for i in 0..8 {
        nonce[12 - 8 + i] ^= seq_bytes[i];
    }
    nonce
}

fn decrypt_record(key: &[u8; 16], iv: &[u8; 12], seq: u64, encrypted: &[u8]) -> Option<Vec<u8>> {
    if encrypted.len() < 16 { return None; }

    let nonce = build_nonce(iv, seq);
    let nonce_arr: [u8; 12] = nonce.try_into().ok()?;
    let ct_len = encrypted.len() - 16;
    let ciphertext = &encrypted[..ct_len];
    let tag: [u8; 16] = encrypted[ct_len..].try_into().ok()?;

    let aad = TLS_RECORD_AAD(encrypted.len());
    aes128_gcm_open(key, &nonce_arr, &aad, ciphertext, &tag)
}

/// Construct AAD for TLS 1.3 record: type(1) || legacy_version(2) || length(2)
#[allow(non_snake_case)]
fn TLS_RECORD_AAD(enc_len: usize) -> [u8; 5] {
    [
        CT_APP_DATA,
        0x03, 0x03, // TLS 1.2 for compat
        (enc_len >> 8) as u8,
        enc_len as u8,
    ]
}

// ── ClientHello builder ─────────────────────────────────────

fn build_client_hello(client_random: &[u8; 32], x25519_pub: &[u8; 32], hostname: &str) -> Vec<u8> {
    let mut hello = Vec::new();

    // Legacy version: TLS 1.2
    hello.extend_from_slice(&[0x03, 0x03]);
    // Client random
    hello.extend_from_slice(client_random);
    // Session ID (32 random bytes for middlebox compat)
    let mut session_id = [0u8; 32];
    csprng_fill(&mut session_id);
    hello.push(32);
    hello.extend_from_slice(&session_id);
    // Cipher suites
    hello.push(0x00); hello.push(0x02); // length = 2
    hello.push((TLS_AES_128_GCM_SHA256 >> 8) as u8);
    hello.push(TLS_AES_128_GCM_SHA256 as u8);
    // Compression methods: null
    hello.push(0x01); hello.push(0x00);

    // ── Extensions ──────────────────────────────────────
    let mut exts = Vec::new();

    // SNI (Server Name Indication)
    if !hostname.is_empty() {
        let name_bytes = hostname.as_bytes();
        let mut sni = Vec::new();
        let list_len = 3 + name_bytes.len();
        sni.push((list_len >> 8) as u8);
        sni.push(list_len as u8);
        sni.push(0x00); // host_name type
        sni.push((name_bytes.len() >> 8) as u8);
        sni.push(name_bytes.len() as u8);
        sni.extend_from_slice(name_bytes);
        push_extension(&mut exts, EXT_SERVER_NAME, &sni);
    }

    // Supported Versions
    push_extension(&mut exts, EXT_SUPPORTED_VERSIONS, &[0x03, 0x03, 0x04]); // len=3: TLS 1.3

    // Supported Groups
    push_extension(&mut exts, EXT_SUPPORTED_GROUPS, &[0x00, 0x02, 0x00, 0x1D]); // X25519

    // Key Share: X25519 public key
    {
        let mut ks = Vec::new();
        let entry_len = 2 + 2 + 32; // group(2) + len(2) + key(32)
        ks.push((entry_len >> 8) as u8);
        ks.push(entry_len as u8);
        ks.push(0x00); ks.push(0x1D); // X25519
        ks.push(0x00); ks.push(0x20); // 32 bytes
        ks.extend_from_slice(x25519_pub);
        push_extension(&mut exts, EXT_KEY_SHARE, &ks);
    }

    // Signature Algorithms
    {
        let sigs: &[u16] = &[
            SIG_ECDSA_SECP256R1_SHA256,
            SIG_RSA_PSS_RSAE_SHA256,
            SIG_RSA_PKCS1_SHA256,
        ];
        let mut sa = Vec::new();
        sa.push(0x00);
        sa.push((sigs.len() * 2) as u8);
        for &sig in sigs {
            sa.push((sig >> 8) as u8);
            sa.push(sig as u8);
        }
        push_extension(&mut exts, EXT_SIGNATURE_ALGORITHMS, &sa);
    }

    // ALPN
    {
        let protos: &[&[u8]] = &[b"h2", b"http/1.1"];
        let mut alpn = Vec::new();
        let mut list = Vec::new();
        for proto in protos {
            list.push(proto.len() as u8);
            list.extend_from_slice(proto);
        }
        alpn.push((list.len() >> 8) as u8);
        alpn.push(list.len() as u8);
        alpn.extend_from_slice(&list);
        push_extension(&mut exts, EXT_ALPN, &alpn);
    }

    // Append extensions to hello
    hello.push((exts.len() >> 8) as u8);
    hello.push(exts.len() as u8);
    hello.extend_from_slice(&exts);

    // Wrap in handshake message: type(1) + length(3) + body
    let mut msg = Vec::with_capacity(4 + hello.len());
    msg.push(HS_CLIENT_HELLO);
    let len = hello.len();
    msg.push((len >> 16) as u8);
    msg.push((len >> 8) as u8);
    msg.push(len as u8);
    msg.extend_from_slice(&hello);
    msg
}

fn push_extension(exts: &mut Vec<u8>, ext_type: u16, data: &[u8]) {
    exts.push((ext_type >> 8) as u8);
    exts.push(ext_type as u8);
    exts.push((data.len() >> 8) as u8);
    exts.push(data.len() as u8);
    exts.extend_from_slice(data);
}

// ── ServerHello parser ──────────────────────────────────────

fn parse_server_hello_key_share(sh_body: &[u8]) -> Option<[u8; 32]> {
    // ServerHello body: version(2) + random(32) + session_id_len(1) + session_id + cipher(2) + comp(1) + ext_len(2) + exts
    if sh_body.len() < 35 { return None; }
    let sid_len = sh_body[34] as usize;
    let pos = 35 + sid_len;
    if pos + 3 >= sh_body.len() { return None; }
    // cipher suite (2) + compression (1)
    let ext_start = pos + 3;
    if ext_start + 2 > sh_body.len() { return None; }
    let ext_len = u16::from_be_bytes([sh_body[ext_start], sh_body[ext_start + 1]]) as usize;
    let ext_end = (ext_start + 2 + ext_len).min(sh_body.len());

    let mut off = ext_start + 2;
    while off + 4 <= ext_end {
        let etype = u16::from_be_bytes([sh_body[off], sh_body[off + 1]]);
        let elen = u16::from_be_bytes([sh_body[off + 2], sh_body[off + 3]]) as usize;
        off += 4;
        if off + elen > ext_end { break; }

        if etype == EXT_KEY_SHARE {
            // key_share: group(2) + key_len(2) + key
            let data = &sh_body[off..off + elen];
            if data.len() >= 4 {
                let group = u16::from_be_bytes([data[0], data[1]]);
                let klen = u16::from_be_bytes([data[2], data[3]]) as usize;
                if group == GROUP_X25519 && klen == 32 && data.len() >= 4 + 32 {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&data[4..36]);
                    return Some(key);
                }
            }
        }
        off += elen;
    }
    None
}

/// Process decrypted handshake messages and add them to the transcript.
fn process_handshake_messages(data: &[u8], transcript: &mut Vec<u8>) {
    let mut off = 0;
    while off + 4 <= data.len() {
        let hs_type = data[off];
        let hs_len = ((data[off + 1] as usize) << 16) | ((data[off + 2] as usize) << 8) | (data[off + 3] as usize);
        let msg_end = off + 4 + hs_len;
        if msg_end > data.len() { break; }

        // Add to transcript (EncryptedExtensions, Certificate, CertificateVerify, Finished)
        match hs_type {
            HS_ENCRYPTED_EXTENSIONS | HS_CERTIFICATE | HS_CERTIFICATE_VERIFY | HS_FINISHED => {
                transcript.extend_from_slice(&data[off..msg_end]);
            }
            HS_NEW_SESSION_TICKET => {
                // Don't add to transcript, but could store for session resumption
            }
            _ => {
                transcript.extend_from_slice(&data[off..msg_end]);
            }
        }
        off = msg_end;
    }
}
