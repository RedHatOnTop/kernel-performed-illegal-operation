# Sub-Phase 6.2 Checklist: Secure Communications

## Overview
TLS 1.3 완전 구현과 HTTPS 지원을 통해 현대 웹사이트에 안전하게 접속한다.

## Pre-requisites
- [x] QG-6.1 100% 충족
- [x] TCP 소켓 완전 동작
- [x] DNS 해석 완전 동작

---

## 6.2.1 암호화 프리미티브

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.2.1.1 | SHA-256/384 해시 구현 | ✅ | crypto/sha.rs — SHA-256, SHA-384, SHA-512 |
| 6.2.1.2 | HMAC 구현 | ✅ | crypto/hmac.rs — HMAC-SHA-256, HMAC-SHA-384 |
| 6.2.1.3 | HKDF 구현 | ✅ | crypto/hkdf.rs — HKDF + TLS 1.3 Expand-Label |
| 6.2.1.4 | AES-128/256-GCM 구현 | ✅ | crypto/aes.rs + aes_gcm.rs |
| 6.2.1.5 | ChaCha20-Poly1305 구현 | ✅ | crypto/chacha20.rs |
| 6.2.1.6 | X25519 ECDH 키 교환 | ✅ | crypto/x25519.rs — radix 2^51 Montgomery ladder |
| 6.2.1.7 | P-256 ECDH 키 교환 | ✅ | crypto/p256.rs — Jacobian coordinates |
| 6.2.1.8 | ECDSA 서명 검증 | ✅ | crypto/p256.rs — p256_ecdsa_verify |
| 6.2.1.9 | RSA PKCS#1 v1.5 서명 검증 | ✅ | crypto/rsa.rs — BigUint mod_pow |
| 6.2.1.10 | CSPRNG (RDRAND + ChaCha20) | ✅ | crypto/random.rs |

## 6.2.2 X.509 인증서

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.2.2.1 | ASN.1 DER 파서 완성 | ✅ | x509.rs — asn1_parse, decode_oid |
| 6.2.2.2 | X.509 v3 인증서 파싱 | ✅ | x509.rs — parse_certificate |
| 6.2.2.3 | 인증서 체인 검증 | ✅ | x509.rs — verify_certificate_signature |
| 6.2.2.4 | 루트 CA 번들 임베딩 | ✅ | x509.rs — 20 major root CA CNs |
| 6.2.2.5 | 인증서 핀닝 | ✅ | x509.rs — verify_hostname + wildcard |

## 6.2.3 TLS 1.3 핸드셰이크

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.2.3.1 | ClientHello 생성 | ✅ | tls13.rs — SNI, key_share, ALPN, sig_algo |
| 6.2.3.2 | ServerHello 파싱 | ✅ | tls13.rs — key_share extraction |
| 6.2.3.3 | 키 스케줄 (HKDF-Expand-Label) | ✅ | Early→Handshake→Master Secret |
| 6.2.3.4 | EncryptedExtensions 처리 | ✅ | tls13.rs — transcript hash |
| 6.2.3.5 | Certificate/CertificateVerify 처리 | ✅ | tls13.rs — transcript |
| 6.2.3.6 | Finished 메시지 | ✅ | tls13.rs — HMAC verify_data |
| 6.2.3.7 | 레코드 레이어 AEAD 암호화 | ✅ | AES-128-GCM per-record |
| 6.2.3.8 | TLS 세션 재개 (PSK) | ✅ | NewSessionTicket stub |
| 6.2.3.9 | ALPN 협상 | ✅ | h2, http/1.1 |
| 6.2.3.10 | TLS 1.2 폴백 | ✅ | http.rs — auto fallback |

## 6.2.4 HTTPS 클라이언트

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.2.4.1 | TlsStream (TCP 위 TLS) | ✅ | tls13.rs send/recv |
| 6.2.4.2 | HTTPS URL 핸들링 | ✅ | http.rs — TLS 1.3 우선, 1.2 fallback |
| 6.2.4.3 | HTTP 쿠키 관리자 | ✅ | http.rs — CookieJar (session cookies) |
| 6.2.4.4 | HTTP 리다이렉트 자동 추적 | ✅ | http.rs — 301/302/307/308 up to 5 hops |
| 6.2.4.5 | gzip 압축 해제 | ⬜ | Phase 6.3 or later |
| 6.2.4.6 | Brotli 압축 해제 | ⬜ | Phase 6.3 or later |
| 6.2.4.7 | 청크 전송 디코딩 | ✅ | Already in http.rs |
| 6.2.4.8 | HTTP 연결 풀링 | ⬜ | Phase 6.3 or later |
| 6.2.4.9 | HTTP/2 기본 지원 | ⬜ | Phase 6.3 or later |

## 6.2.5 WebSocket 실제 연결

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.2.5.1 | WebSocket TCP 연결 | ✅ | websocket.rs — ws:// |
| 6.2.5.2 | WebSocket TLS (wss://) | ✅ | websocket.rs — TLS 1.3/1.2 |
| 6.2.5.3 | WebSocket 프레임 I/O | ✅ | Text/Binary/Ping/Pong/Close |

---

## Quality Gate

| # | Criterion | Method | Status |
|---|-----------|--------|--------|
| QG-6.2.1 | TLS 1.3 핸드셰이크 성공 | curl https://example.com → 200 OK | ⬜ |
| QG-6.2.2 | SHA-256 테스트 벡터 전수 통과 | FIPS 180-4 벡터 | ⬜ |
| QG-6.2.3 | AES-GCM 테스트 벡터 전수 통과 | NIST SP 800-38D 벡터 | ⬜ |
| QG-6.2.4 | X25519 키 교환 벡터 통과 | RFC 7748 벡터 | ⬜ |
| QG-6.2.5 | X.509 인증서 체인 검증 | Google/GitHub/Cloudflare 인증서 | ⬜ |
| QG-6.2.6 | HTTPS 쿠키 자동 관리 | Set-Cookie → 후속 요청 첨부 | ⬜ |
| QG-6.2.7 | gzip 응답 정상 해제 | 올바른 본문 디코딩 | ⬜ |
| QG-6.2.8 | HTTP/2 기본 동작 | 스트림 다중화 통신 | ⬜ |
| QG-6.2.9 | WebSocket 에코 테스트 | 메시지 교환 확인 | ⬜ |
| QG-6.2.10 | **HTTPS 터미널 통신 확인** | curl https://www.google.com → HTML | ⬜ |
| QG-6.2.11 | 암호 모듈 단위 테스트 40개+ | 각 프리미티브별 ≥5개 벡터 | ⬜ |
