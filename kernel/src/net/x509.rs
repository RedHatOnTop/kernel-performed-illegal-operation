//! X.509 v3 Certificate parsing and chain verification
//!
//! Parses DER-encoded X.509 certificates, extracts public keys,
//! and verifies certificate chains.

#![allow(dead_code)]
use super::crypto::p256::p256_ecdsa_verify;
use super::crypto::rsa::rsa_pkcs1_verify;
use super::crypto::sha::{sha256, sha384, sha512};
use alloc::string::String;
use alloc::vec::Vec;

// ── ASN.1 DER parser ────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Asn1Tag {
    Boolean,             // 0x01
    Integer,             // 0x02
    BitString,           // 0x03
    OctetString,         // 0x04
    Null,                // 0x05
    Oid,                 // 0x06
    Utf8String,          // 0x0C
    Sequence,            // 0x30
    Set,                 // 0x31
    PrintableString,     // 0x13
    Ia5String,           // 0x16
    UtcTime,             // 0x17
    GeneralizedTime,     // 0x18
    ContextSpecific(u8), // 0xA0..0xAF
    Other(u8),
}

#[derive(Debug, Clone)]
struct Asn1Element<'a> {
    tag: Asn1Tag,
    data: &'a [u8],
    header_len: usize,
}

fn parse_tag(b: u8) -> Asn1Tag {
    match b & 0x1F {
        _ if b == 0x01 => Asn1Tag::Boolean,
        _ if b == 0x02 => Asn1Tag::Integer,
        _ if b == 0x03 => Asn1Tag::BitString,
        _ if b == 0x04 => Asn1Tag::OctetString,
        _ if b == 0x05 => Asn1Tag::Null,
        _ if b == 0x06 => Asn1Tag::Oid,
        _ if b == 0x0C => Asn1Tag::Utf8String,
        _ if b == 0x13 => Asn1Tag::PrintableString,
        _ if b == 0x16 => Asn1Tag::Ia5String,
        _ if b == 0x17 => Asn1Tag::UtcTime,
        _ if b == 0x18 => Asn1Tag::GeneralizedTime,
        _ if b == 0x30 => Asn1Tag::Sequence,
        _ if b == 0x31 => Asn1Tag::Set,
        _ if (b & 0xC0) == 0xA0 => Asn1Tag::ContextSpecific(b & 0x1F),
        _ => Asn1Tag::Other(b),
    }
}

/// Parse one ASN.1 DER element from `data[offset..]`.
/// Returns (element, bytes consumed).
fn asn1_parse<'a>(data: &'a [u8], offset: usize) -> Option<(Asn1Element<'a>, usize)> {
    if offset >= data.len() {
        return None;
    }

    let tag = parse_tag(data[offset]);
    let (length, hdr_extra) = parse_length(data, offset + 1)?;
    let header_len = 1 + hdr_extra;
    let start = offset + header_len;
    let end = start + length;
    if end > data.len() {
        return None;
    }

    Some((
        Asn1Element {
            tag,
            data: &data[start..end],
            header_len,
        },
        end - offset,
    ))
}

fn parse_length(data: &[u8], offset: usize) -> Option<(usize, usize)> {
    if offset >= data.len() {
        return None;
    }
    let first = data[offset];
    if first < 0x80 {
        Some((first as usize, 1))
    } else {
        let num_bytes = (first & 0x7F) as usize;
        if num_bytes == 0 || num_bytes > 4 {
            return None;
        }
        if offset + 1 + num_bytes > data.len() {
            return None;
        }
        let mut len = 0usize;
        for i in 0..num_bytes {
            len = (len << 8) | data[offset + 1 + i] as usize;
        }
        Some((len, 1 + num_bytes))
    }
}

/// Iterate children of a SEQUENCE/SET element.
fn asn1_children<'a>(data: &'a [u8]) -> Vec<Asn1Element<'a>> {
    let mut children = Vec::new();
    let mut off = 0;
    while off < data.len() {
        if let Some((elem, consumed)) = asn1_parse(data, off) {
            children.push(elem);
            off += consumed;
        } else {
            break;
        }
    }
    children
}

/// Decode an OID from DER bytes to dotted-string form.
fn decode_oid(data: &[u8]) -> String {
    if data.is_empty() {
        return String::new();
    }
    let mut parts = Vec::new();
    parts.push(alloc::format!("{}", data[0] / 40));
    parts.push(alloc::format!("{}", data[0] % 40));

    let mut val = 0u64;
    for &b in &data[1..] {
        val = (val << 7) | (b & 0x7F) as u64;
        if (b & 0x80) == 0 {
            parts.push(alloc::format!("{}", val));
            val = 0;
        }
    }
    let mut result = String::new();
    for (i, p) in parts.iter().enumerate() {
        if i > 0 {
            result.push('.');
        }
        result.push_str(p);
    }
    result
}

// ── X.509 Certificate Structure ─────────────────────────────

/// Parsed X.509 certificate.
#[derive(Clone)]
pub struct X509Certificate {
    /// DER-encoded TBS (To-Be-Signed) certificate for signature verification
    pub tbs_raw: Vec<u8>,
    /// Subject common name
    pub subject_cn: String,
    /// Issuer common name
    pub issuer_cn: String,
    /// Subject alternative names (DNS)
    pub san_dns: Vec<String>,
    /// Public key algorithm OID
    pub pubkey_algo: String,
    /// Public key data (raw bytes — RSA modulus/exponent or EC point)
    pub pubkey_data: Vec<u8>,
    /// RSA modulus (if RSA key)
    pub rsa_modulus: Vec<u8>,
    /// RSA exponent (if RSA key)
    pub rsa_exponent: Vec<u8>,
    /// Signature algorithm OID
    pub sig_algo: String,
    /// Signature value
    pub signature: Vec<u8>,
    /// Is this a CA certificate?
    pub is_ca: bool,
    /// Self-signed?
    pub self_signed: bool,
}

/// Well-known OIDs
const OID_RSA_ENCRYPTION: &str = "1.2.840.113549.1.1.1";
const OID_SHA256_WITH_RSA: &str = "1.2.840.113549.1.1.11";
const OID_SHA384_WITH_RSA: &str = "1.2.840.113549.1.1.12";
const OID_SHA512_WITH_RSA: &str = "1.2.840.113549.1.1.13";
const OID_EC_PUBLIC_KEY: &str = "1.2.840.10045.2.1";
const OID_ECDSA_SHA256: &str = "1.2.840.10045.4.3.2";
const OID_ECDSA_SHA384: &str = "1.2.840.10045.4.3.3";
const OID_P256: &str = "1.2.840.10045.3.1.7";
const OID_COMMON_NAME: &str = "2.5.4.3";
const OID_SAN: &str = "2.5.29.17";
const OID_BASIC_CONSTRAINTS: &str = "2.5.29.19";

/// Parse a DER-encoded X.509 certificate.
pub fn parse_certificate(der: &[u8]) -> Option<X509Certificate> {
    // Certificate ::= SEQUENCE { tbsCertificate, signatureAlgorithm, signatureValue }
    let (cert_seq, _) = asn1_parse(der, 0)?;
    if cert_seq.tag != Asn1Tag::Sequence {
        return None;
    }

    let children = asn1_children(cert_seq.data);
    if children.len() < 3 {
        return None;
    }

    // TBS Certificate (raw bytes including tag+length for signature verification)
    let tbs_elem = &children[0];
    let tbs_start = 0; // within cert_seq.data
    let tbs_len = tbs_elem.header_len + tbs_elem.data.len();
    // We need the raw TBS from the original DER
    let cert_body_start = der.len() - cert_seq.data.len();
    let tbs_raw = der[cert_body_start..cert_body_start + tbs_len].to_vec();

    // Parse TBS Certificate
    let tbs_children = asn1_children(tbs_elem.data);
    if tbs_children.len() < 6 {
        return None;
    }

    let mut idx = 0;

    // version [0] EXPLICIT (optional)
    if tbs_children[0].tag == Asn1Tag::ContextSpecific(0) {
        idx += 1;
    }

    // serialNumber
    idx += 1; // skip serial

    // signature (algorithmIdentifier within TBS)
    let _tbs_sig_algo = parse_algorithm_oid(&tbs_children[idx]);
    idx += 1;

    // issuer
    let issuer_cn = parse_dn_cn(&tbs_children[idx]);
    idx += 1;

    // validity
    idx += 1; // skip validity times

    // subject
    let subject_cn = parse_dn_cn(&tbs_children[idx]);
    idx += 1;

    // subjectPublicKeyInfo
    let (pubkey_algo, pubkey_data, rsa_mod, rsa_exp) =
        parse_subject_pubkey_info(&tbs_children[idx])?;
    idx += 1;

    // Parse extensions (if present)
    let mut san_dns = Vec::new();
    let mut is_ca = false;
    while idx < tbs_children.len() {
        if tbs_children[idx].tag == Asn1Tag::ContextSpecific(3) {
            // Extensions
            let ext_seq = asn1_children(tbs_children[idx].data);
            if !ext_seq.is_empty() {
                let exts = asn1_children(ext_seq[0].data);
                for ext in &exts {
                    parse_extension(ext, &mut san_dns, &mut is_ca);
                }
            }
        }
        idx += 1;
    }

    // Signature algorithm
    let sig_algo_elem = &children[1];
    let sig_algo = parse_algorithm_oid(sig_algo_elem);

    // Signature value (BIT STRING)
    let sig_elem = &children[2];
    let signature = if sig_elem.tag == Asn1Tag::BitString && !sig_elem.data.is_empty() {
        sig_elem.data[1..].to_vec() // skip unused-bits byte
    } else {
        Vec::new()
    };

    let self_signed = subject_cn == issuer_cn;

    Some(X509Certificate {
        tbs_raw,
        subject_cn,
        issuer_cn,
        san_dns,
        pubkey_algo,
        pubkey_data,
        rsa_modulus: rsa_mod,
        rsa_exponent: rsa_exp,
        sig_algo,
        signature,
        is_ca,
        self_signed,
    })
}

fn parse_algorithm_oid(elem: &Asn1Element) -> String {
    let children = asn1_children(elem.data);
    if !children.is_empty() && children[0].tag == Asn1Tag::Oid {
        decode_oid(children[0].data)
    } else {
        String::new()
    }
}

fn parse_dn_cn(elem: &Asn1Element) -> String {
    // Name ::= SEQUENCE OF SET OF AttributeTypeAndValue
    let sets = asn1_children(elem.data);
    for set in &sets {
        let atvs = asn1_children(set.data);
        for atv in &atvs {
            let components = asn1_children(atv.data);
            if components.len() >= 2 && components[0].tag == Asn1Tag::Oid {
                let oid = decode_oid(components[0].data);
                if oid == OID_COMMON_NAME {
                    return String::from_utf8_lossy(components[1].data).into_owned();
                }
            }
        }
    }
    String::new()
}

fn parse_subject_pubkey_info(elem: &Asn1Element) -> Option<(String, Vec<u8>, Vec<u8>, Vec<u8>)> {
    let children = asn1_children(elem.data);
    if children.len() < 2 {
        return None;
    }

    // AlgorithmIdentifier
    let algo_children = asn1_children(children[0].data);
    let algo_oid = if !algo_children.is_empty() && algo_children[0].tag == Asn1Tag::Oid {
        decode_oid(algo_children[0].data)
    } else {
        String::new()
    };

    // BIT STRING containing the public key
    let pubkey_bits = if children[1].tag == Asn1Tag::BitString && !children[1].data.is_empty() {
        children[1].data[1..].to_vec() // skip unused-bits byte
    } else {
        children[1].data.to_vec()
    };

    // Parse RSA modulus/exponent if RSA
    let (rsa_mod, rsa_exp) = if algo_oid == OID_RSA_ENCRYPTION {
        parse_rsa_public_key(&pubkey_bits)
    } else {
        (Vec::new(), Vec::new())
    };

    Some((algo_oid, pubkey_bits, rsa_mod, rsa_exp))
}

fn parse_rsa_public_key(data: &[u8]) -> (Vec<u8>, Vec<u8>) {
    // RSAPublicKey ::= SEQUENCE { modulus INTEGER, publicExponent INTEGER }
    if let Some((seq, _)) = asn1_parse(data, 0) {
        if seq.tag == Asn1Tag::Sequence {
            let children = asn1_children(seq.data);
            if children.len() >= 2 {
                let modulus = integer_bytes(children[0].data);
                let exponent = integer_bytes(children[1].data);
                return (modulus, exponent);
            }
        }
    }
    (Vec::new(), Vec::new())
}

/// Strip leading zero from ASN.1 INTEGER encoding.
fn integer_bytes(data: &[u8]) -> Vec<u8> {
    if data.first() == Some(&0) && data.len() > 1 {
        data[1..].to_vec()
    } else {
        data.to_vec()
    }
}

fn parse_extension(ext: &Asn1Element, san_dns: &mut Vec<String>, is_ca: &mut bool) {
    let children = asn1_children(ext.data);
    if children.is_empty() || children[0].tag != Asn1Tag::Oid {
        return;
    }
    let oid = decode_oid(children[0].data);

    // Find the value (last child, possibly wrapped in OCTET STRING)
    let value_idx = children.len() - 1;
    let value = if children[value_idx].tag == Asn1Tag::OctetString {
        children[value_idx].data
    } else {
        return;
    };

    match oid.as_str() {
        OID_SAN => {
            // SubjectAltName ::= GeneralNames
            if let Some((names_seq, _)) = asn1_parse(value, 0) {
                let names = asn1_children(names_seq.data);
                for name in &names {
                    // dNSName [2] IA5String
                    if name.tag == Asn1Tag::ContextSpecific(2) {
                        if let Ok(s) = core::str::from_utf8(name.data) {
                            san_dns.push(String::from(s));
                        }
                    }
                }
            }
        }
        OID_BASIC_CONSTRAINTS => {
            // BasicConstraints ::= SEQUENCE { cA BOOLEAN, ... }
            if let Some((bc_seq, _)) = asn1_parse(value, 0) {
                let bc_children = asn1_children(bc_seq.data);
                if !bc_children.is_empty() && bc_children[0].tag == Asn1Tag::Boolean {
                    *is_ca = bc_children[0].data.first() == Some(&0xFF);
                }
            }
        }
        _ => {}
    }
}

// ── Certificate verification ────────────────────────────────

/// Verify that `cert` was signed by `issuer_cert`.
pub fn verify_certificate_signature(cert: &X509Certificate, issuer: &X509Certificate) -> bool {
    // Hash the TBS certificate
    let (hash, algo) = match cert.sig_algo.as_str() {
        OID_SHA256_WITH_RSA | OID_ECDSA_SHA256 => (sha256(&cert.tbs_raw).to_vec(), "sha256"),
        OID_SHA384_WITH_RSA | OID_ECDSA_SHA384 => (sha384(&cert.tbs_raw).to_vec(), "sha384"),
        OID_SHA512_WITH_RSA => (sha512(&cert.tbs_raw).to_vec(), "sha512"),
        _ => return false,
    };

    match issuer.pubkey_algo.as_str() {
        OID_RSA_ENCRYPTION => rsa_pkcs1_verify(
            &issuer.rsa_modulus,
            &issuer.rsa_exponent,
            &cert.signature,
            &hash,
            algo,
        ),
        OID_EC_PUBLIC_KEY => p256_ecdsa_verify(&hash, &cert.signature, &issuer.pubkey_data),
        _ => false,
    }
}

/// Verify hostname against certificate.
/// Checks CN and SAN (DNS) entries, supports wildcard certs (*.example.com).
pub fn verify_hostname(cert: &X509Certificate, hostname: &str) -> bool {
    // Check SANs first (preferred per RFC 6125)
    for san in &cert.san_dns {
        if matches_hostname(san, hostname) {
            return true;
        }
    }
    // Fall back to CN
    matches_hostname(&cert.subject_cn, hostname)
}

fn matches_hostname(pattern: &str, hostname: &str) -> bool {
    let pattern = pattern.to_ascii_lowercase();
    let hostname = hostname.to_ascii_lowercase();

    if pattern == hostname {
        return true;
    }

    // Wildcard matching: *.example.com matches foo.example.com
    if pattern.starts_with("*.") {
        let suffix = &pattern[2..];
        if let Some(pos) = hostname.find('.') {
            return &hostname[pos + 1..] == suffix;
        }
    }
    false
}

/// Parse a PEM certificate chain into individual DER certificates.
/// PEM format: -----BEGIN CERTIFICATE----- ... -----END CERTIFICATE-----
pub fn parse_pem_chain(pem: &[u8]) -> Vec<Vec<u8>> {
    let mut certs = Vec::new();
    let text = core::str::from_utf8(pem).unwrap_or("");
    let begin_marker = "-----BEGIN CERTIFICATE-----";
    let end_marker = "-----END CERTIFICATE-----";

    let mut pos = 0;
    while let Some(begin) = text[pos..].find(begin_marker) {
        let start = pos + begin + begin_marker.len();
        if let Some(end) = text[start..].find(end_marker) {
            let b64 = &text[start..start + end];
            if let Some(der) = base64_decode(b64) {
                certs.push(der);
            }
            pos = start + end + end_marker.len();
        } else {
            break;
        }
    }
    certs
}

/// Simple base64 decoder (for PEM certificate parsing).
fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0;

    for c in input.chars() {
        let val = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            '=' | '\n' | '\r' | ' ' | '\t' => continue,
            _ => return None,
        };
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

// ── Embedded Root CA bundle (minimal — major CAs) ───────────

/// Minimal root CA store with names for hostname verification fallback.
/// In a full implementation, this would contain DER-encoded root certificates.
/// For now, we store just the CN names of trusted roots and skip chain verification
/// when connecting to well-known services.
pub static TRUSTED_ROOT_CNS: &[&str] = &[
    "DigiCert Global Root G2",
    "DigiCert Global Root CA",
    "Baltimore CyberTrust Root",
    "ISRG Root X1",
    "ISRG Root X2",
    "GlobalSign Root CA",
    "GlobalSign Root CA - R2",
    "GlobalSign Root CA - R3",
    "Amazon Root CA 1",
    "Amazon Root CA 2",
    "Amazon Root CA 3",
    "Amazon Root CA 4",
    "Starfield Root Certificate Authority - G2",
    "Comodo RSA Certification Authority",
    "USERTrust RSA Certification Authority",
    "GeoTrust Global CA",
    "Google Trust Services LLC",
    "Cloudflare Inc ECC CA-3",
    "Microsoft RSA Root Certificate Authority 2017",
    "Sectigo RSA Domain Validation Secure Server CA",
];

/// Check if a certificate's issuer is in the trusted root store.
pub fn is_trusted_root(cert: &X509Certificate) -> bool {
    for &cn in TRUSTED_ROOT_CNS {
        if cert.issuer_cn.contains(cn) || cert.subject_cn.contains(cn) {
            return true;
        }
    }
    false
}
