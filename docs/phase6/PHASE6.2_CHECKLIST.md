# Sub-Phase 6.2 Checklist: Secure Communications

## Overview
TLS 1.3 완전 구현과 HTTPS 지원을 통해 현대 웹사이트에 안전하게 접속한다.

## Pre-requisites
- [ ] QG-6.1 100% 충족
- [ ] TCP 소켓 완전 동작
- [ ] DNS 해석 완전 동작

---

## 6.2.1 암호화 프리미티브

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.2.1.1 | SHA-256/384 해시 구현 | ⬜ | FIPS 180-4 |
| 6.2.1.2 | HMAC 구현 | ⬜ | |
| 6.2.1.3 | HKDF 구현 | ⬜ | |
| 6.2.1.4 | AES-128/256-GCM 구현 | ⬜ | NIST SP 800-38D |
| 6.2.1.5 | ChaCha20-Poly1305 구현 | ⬜ | |
| 6.2.1.6 | X25519 ECDH 키 교환 | ⬜ | RFC 7748 |
| 6.2.1.7 | P-256 ECDH 키 교환 | ⬜ | |
| 6.2.1.8 | ECDSA 서명 검증 | ⬜ | |
| 6.2.1.9 | RSA PKCS#1 v1.5 서명 검증 | ⬜ | |
| 6.2.1.10 | CSPRNG (RDRAND + ChaCha20) | ⬜ | |

## 6.2.2 X.509 인증서

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.2.2.1 | ASN.1 DER 파서 완성 | ⬜ | |
| 6.2.2.2 | X.509 v3 인증서 파싱 | ⬜ | |
| 6.2.2.3 | 인증서 체인 검증 | ⬜ | |
| 6.2.2.4 | 루트 CA 번들 임베딩 | ⬜ | Mozilla ~130개 |
| 6.2.2.5 | 인증서 핀닝 | ⬜ | |

## 6.2.3 TLS 1.3 핸드셰이크

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.2.3.1 | ClientHello 생성 | ⬜ | |
| 6.2.3.2 | ServerHello 파싱 | ⬜ | |
| 6.2.3.3 | 키 스케줄 (HKDF-Expand-Label) | ⬜ | |
| 6.2.3.4 | EncryptedExtensions 처리 | ⬜ | |
| 6.2.3.5 | Certificate/CertificateVerify 처리 | ⬜ | |
| 6.2.3.6 | Finished 메시지 | ⬜ | |
| 6.2.3.7 | 레코드 레이어 AEAD 암호화 | ⬜ | |
| 6.2.3.8 | TLS 세션 재개 (PSK) | ⬜ | |
| 6.2.3.9 | ALPN 협상 | ⬜ | |
| 6.2.3.10 | TLS 1.2 폴백 | ⬜ | |

## 6.2.4 HTTPS 클라이언트

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.2.4.1 | TlsStream (TCP 위 TLS) | ⬜ | |
| 6.2.4.2 | HTTPS URL 핸들링 | ⬜ | |
| 6.2.4.3 | HTTP 쿠키 관리자 | ⬜ | |
| 6.2.4.4 | HTTP 리다이렉트 자동 추적 | ⬜ | |
| 6.2.4.5 | gzip 압축 해제 | ⬜ | |
| 6.2.4.6 | Brotli 압축 해제 | ⬜ | |
| 6.2.4.7 | 청크 전송 디코딩 | ⬜ | |
| 6.2.4.8 | HTTP 연결 풀링 | ⬜ | |
| 6.2.4.9 | HTTP/2 기본 지원 | ⬜ | |

## 6.2.5 WebSocket 실제 연결

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.2.5.1 | WebSocket TCP 연결 | ⬜ | |
| 6.2.5.2 | WebSocket TLS (wss://) | ⬜ | |
| 6.2.5.3 | WebSocket 프레임 I/O | ⬜ | |

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
