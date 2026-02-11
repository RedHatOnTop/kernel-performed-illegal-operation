# Phase 6: Complete Web Stack — Full Web Application Support

> **목표**: KPIO OS에서 실제 웹 어플리케이션(Gmail, GitHub, Twitter, YouTube 등)을 구동할 수 있는 완전한 웹 스택을 구현한다. 네트워크 계층부터 JavaScript 엔진, CSS/레이아웃 엔진, Web API, 브라우저 핵심 기능까지 모든 계층을 상용 OS 수준으로 구현하며, 기존 코드베이스에 자연스럽고 긴밀하게 통합한다.

---

## 현재 상태 요약

| 계층 | 현재 수준 | 실제 동작 비율 |
|------|----------|--------------|
| 네트워크 (TCP/IP/DNS/TLS) | 구조체·상태 머신 정의 | ~10% |
| HTTP 클라이언트 | 요청·응답 직렬화/파싱 | ~25% |
| JavaScript 엔진 | 기본 문법 실행 | ~15% |
| CSS 엔진 | 파싱·값 타입 | ~30% |
| 레이아웃 엔진 | Block/Inline/Flex 기초 | ~25% |
| DOM/Web API | DOM 트리·이벤트 기초 | ~10% |
| 브라우저 쉘 | 렌더 파이프라인 골격 | ~20% |
| 그래픽 렌더링 | 프레임버퍼·배치 렌더러 | ~40% |

**추정 총 갭: ~80-85%의 실제 동작 로직이 누락**

---

## 아키텍처 원칙

1. **계층화 (Layering)**: NIC Driver → IP → TCP/UDP → TLS → HTTP → Browser → Web APIs
2. **모듈 격리**: 각 크레이트의 책임 경계를 명확히 유지 (순환 의존 금지)
3. **no_std 우선**: 커널 컨텍스트에서 동작해야 하므로 모든 핵심 로직은 `no_std` + `alloc`
4. **테스트 주도**: 각 서브페이즈의 퀄리티 게이트에 단위 테스트·통합 테스트 포함
5. **점진적 통합**: 하위 계층을 완성한 후에만 상위 계층 구현 진행
6. **성능 의식**: O(n²) 이상의 알고리즘 사용 시 명시적 정당화 필요

---

## 서브페이즈 구성 총괄

```
Phase 6.1 — 네트워크 기반 (Network Foundation)
Phase 6.2 — 보안 통신 (Secure Communications)
Phase 6.3 — JavaScript 엔진 완성 (JS Engine Completion)
Phase 6.4 — CSS & 레이아웃 완성 (CSS & Layout Completion)
Phase 6.5 — Web API 계층 (Web Platform APIs)
Phase 6.6 — 브라우저 핵심 기능 (Browser Core Features)
Phase 6.7 — 미디어 & 그래픽 (Media & Graphics Pipeline)
Phase 6.8 — 고급 웹 플랫폼 (Advanced Web Platform)
Phase 6.9 — 프레임워크 호환성 (Framework Compatibility)
Phase 6.10 — 통합 검증 & 폴리시 (Integration Verification & Polish)
```

---

# Sub-Phase 6.1: Network Foundation

## 목적
NIC 드라이버에서 TCP/UDP 소켓까지의 완전한 데이터 경로를 구현하여 KPIO OS에서 실제 인터넷 패킷을 송수신할 수 있게 한다.

## 선행 조건
- [x] VirtIO-Net 드라이버 구조체 존재
- [x] smoltcp 0.11 의존성 설정
- [x] TCP 상태 머신 enum 정의
- [x] IPv4 주소 타입 구현

## 작업 목록

### 6.1.1 NIC 드라이버 ↔ smoltcp 통합

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.1.1.1 | `smoltcp::phy::Device` 트레이트 구현 | `network/src/driver/virtio_net.rs` | VirtIO-Net 큐에서 패킷 수신/송신을 smoltcp 인터페이스로 연결 |
| 6.1.1.2 | E1000 드라이버 `Device` 구현 | `network/src/driver/e1000.rs` | E1000 MMIO 레지스터를 통한 패킷 I/O |
| 6.1.1.3 | NIC 인터럽트 핸들러 등록 | `kernel/src/interrupts.rs` | IRQ 핸들러에서 `network::poll()` 호출 |
| 6.1.1.4 | 네트워크 폴링 루프 | `network/src/stack.rs` (신규) | smoltcp `Interface::poll()` 주기적 호출, 타이머 기반 재전송 |

### 6.1.2 IP 계층

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.1.2.1 | ARP 테이블 관리 | `network/src/arp.rs` (신규) | MAC 주소 해석, 캐시, 타임아웃, gratuitous ARP |
| 6.1.2.2 | IP 라우팅 테이블 | `network/src/routing.rs` (신규) | 기본 게이트웨이, 서브넷 매칭, 라우팅 결정 |
| 6.1.2.3 | ICMP 처리 | `network/src/icmp.rs` (신규) | ping 응답, 에러 메시지 (destination unreachable 등) |
| 6.1.2.4 | IPv6 기본 지원 | `network/src/ipv6.rs` (신규) | 주소 파싱, NDP, smoltcp IPv6 소켓 |

### 6.1.3 TCP 구현 완성

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.1.3.1 | smoltcp TCP 소켓 래퍼 | `network/src/tcp.rs` | `TcpSocket` — connect/bind/listen/accept/send/recv/close |
| 6.1.3.2 | TCP 소켓 풀 관리자 | `network/src/socket_pool.rs` (신규) | 소켓 할당, FD 매핑, 수명 주기 관리 |
| 6.1.3.3 | 비동기 I/O 인터페이스 | `network/src/async_io.rs` (신규) | 논블로킹 소켓, 준비 상태 알림, epoll 호환 인터페이스 |
| 6.1.3.4 | TCP 혼잡 제어 확인 | `network/src/tcp.rs` | smoltcp의 NewReno 혼잡 제어 파라미터 튜닝 |

### 6.1.4 UDP 구현

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.1.4.1 | smoltcp UDP 소켓 래퍼 | `network/src/udp.rs` | `UdpSocket` — bind/send_to/recv_from |
| 6.1.4.2 | 멀티캐스트 기본 지원 | `network/src/udp.rs` | IGMP 가입/이탈 |

### 6.1.5 DNS 클라이언트 완성

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.1.5.1 | UDP 기반 DNS 질의 전송 | `network/src/dns.rs` | DNS 질의 직렬화 → UDP 53번 포트 송신 → 응답 수신·파싱 |
| 6.1.5.2 | DNS 캐시 (TTL 존중) | `network/src/dns.rs` | BTreeMap 기반 캐시, TTL 만료, 네거티브 캐싱 |
| 6.1.5.3 | /etc/hosts 통합 | `network/src/dns.rs` | VFS의 hosts 파일 조회 후 DNS 질의 폴백 |
| 6.1.5.4 | DNS-over-TCP 폴백 | `network/src/dns.rs` | 응답이 512바이트 초과 시 TCP 폴백 |
| 6.1.5.5 | AAAA 레코드 (IPv6) | `network/src/dns.rs` | IPv6 주소 해석 |

### 6.1.6 DHCP 클라이언트 완성

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.1.6.1 | DHCP Discover/Offer/Request/Ack | `network/src/dhcp.rs` | 완전한 DHCP 4-단계 흐름 |
| 6.1.6.2 | IP/게이트웨이/DNS 자동 설정 | `network/src/dhcp.rs` | DHCP 응답에서 얻은 정보로 인터페이스·라우팅·DNS 설정 |
| 6.1.6.3 | 리스 갱신 | `network/src/dhcp.rs` | T1/T2 타이머 기반 리스 갱신 |

### 6.1.7 소켓 API ↔ 시스콜 연결

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.1.7.1 | socket/connect/send/recv 시스콜 구현 | `kernel/src/syscall/mod.rs` | 기존 스텁을 실제 네트워크 스택 호출로 교체 |
| 6.1.7.2 | 네트워크 FD 통합 | `kernel/src/vfs/mod.rs` | 소켓을 FD 테이블에 통합 (통합 I/O 모델) |
| 6.1.7.3 | epoll 이벤트 소켓 지원 | `kernel/src/syscall/mod.rs` | epoll에 소켓 FD 등록·알림 |

### 6.1.8 셸 네트워크 명령어

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.1.8.1 | `ping` 명령어 구현 | `kernel/src/terminal/commands.rs` | ICMP echo 실제 전송·수신, RTT 측정 |
| 6.1.8.2 | `nslookup`/`dig` 구현 | `kernel/src/terminal/commands.rs` | DNS 질의 → 결과 출력 |
| 6.1.8.3 | `ifconfig`/`ip` 구현 | `kernel/src/terminal/commands.rs` | NIC 상태, IP 주소, 라우팅 테이블 표시 |
| 6.1.8.4 | `curl` 기본 구현 | `kernel/src/terminal/commands.rs` | HTTP GET → 응답 본문 출력 (6.2 완료 후 HTTPS) |

## 퀄리티 게이트 (6.1)

> **아래 항목이 100% 충족되기 전까지 6.2로 진행 금지**

| # | 검증 항목 | 검증 방법 |
|---|----------|----------|
| QG-6.1.1 | QEMU VirtIO-Net을 통해 외부 네트워크와 패킷 송수신 가능 | QEMU `-netdev user` 환경에서 ARP 해석 → ICMP 핑 성공 |
| QG-6.1.2 | DNS 이름 해석이 실제 동작 | `nslookup google.com` → 실제 IP 주소 반환 (10.0.2.3 DNS 포워더 경유) |
| QG-6.1.3 | TCP 3-웨이 핸드셰이크 성공 | TCP SYN → SYN-ACK → ACK 완료, 데이터 송수신 확인 |
| QG-6.1.4 | HTTP GET 요청 성공 | `curl http://example.com` → HTML 응답 본문 수신 |
| QG-6.1.5 | DHCP 자동 IP 획득 | 부팅 시 DHCP로 IP/게이트웨이/DNS 자동 설정 |
| QG-6.1.6 | 소켓 시스콜 동작 | 유저스페이스에서 socket→connect→write→read→close 정상 동작 |
| QG-6.1.7 | 단위 테스트 100% 통과 | TCP/UDP/DNS/DHCP/ICMP 모듈별 최소 10개 테스트 |
| QG-6.1.8 | **터미널에서 인터넷 통신 확인** | 실제 QEMU 실행 후 `ping 8.8.8.8` 및 `curl http://example.com` 성공 로그 캡처 |

---

# Sub-Phase 6.2: Secure Communications

## 목적
TLS 1.3 완전 구현과 HTTPS 지원을 통해 현대 웹사이트에 안전하게 접속할 수 있게 한다.

## 선행 조건
- [x] QG-6.1 100% 충족
- [x] TCP 소켓 완전 동작
- [x] DNS 해석 완전 동작

## 작업 목록

### 6.2.1 암호화 프리미티브

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.2.1.1 | SHA-256/384 해시 구현 | `network/src/crypto/sha2.rs` (신규) | RFC 6234 준수, no_std 순수 Rust 구현 |
| 6.2.1.2 | HMAC 구현 | `network/src/crypto/hmac.rs` (신규) | HMAC-SHA256/384 |
| 6.2.1.3 | HKDF 구현 | `network/src/crypto/hkdf.rs` (신규) | TLS 1.3 키 파생에 필수 |
| 6.2.1.4 | AES-128/256-GCM 구현 | `network/src/crypto/aes_gcm.rs` (신규) | AEAD 암호화/복호화, GF(2^128) 곱셈 |
| 6.2.1.5 | ChaCha20-Poly1305 구현 | `network/src/crypto/chacha20.rs` (신규) | 대체 AEAD 스위트 |
| 6.2.1.6 | X25519 ECDH 키 교환 | `network/src/crypto/x25519.rs` (신규) | Curve25519 스칼라 곱, 키 합의 |
| 6.2.1.7 | P-256 ECDH 키 교환 | `network/src/crypto/p256.rs` (신규) | secp256r1 곡선, 폴백 옵션 |
| 6.2.1.8 | ECDSA 서명 검증 | `network/src/crypto/ecdsa.rs` (신규) | P-256/P-384 서명 검증 (인증서 체인용) |
| 6.2.1.9 | RSA PKCS#1 v1.5 서명 검증 | `network/src/crypto/rsa.rs` (신규) | 2048/4096-bit RSA, 레거시 서버 호환 |
| 6.2.1.10 | CSPRNG | `network/src/crypto/rng.rs` (신규) | RDRAND 명령어 + ChaCha20 기반 CSPRNG |

### 6.2.2 X.509 인증서

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.2.2.1 | ASN.1 DER 파서 완성 | `network/src/tls/asn1.rs` (신규) | Tag-Length-Value 완전 파싱, SEQUENCE/SET/OID/INTEGER/BIT STRING 등 |
| 6.2.2.2 | X.509 v3 인증서 파싱 | `network/src/tls/certificate.rs` | subject/issuer/유효기간/공개키/확장(SAN, BasicConstraints, KeyUsage) |
| 6.2.2.3 | 인증서 체인 검증 | `network/src/tls/verify.rs` (신규) | 경로 구축, 서명 검증, 유효기간 확인, 이름 매칭(와일드카드) |
| 6.2.2.4 | 루트 CA 번들 임베딩 | `network/src/tls/root_ca.rs` (신규) | Mozilla 루트 CA 목록 임베디드 (DER 인코딩), ~130개 |
| 6.2.2.5 | 인증서 핀닝 | `network/src/tls/pinning.rs` (신규) | HPKP 호환, 주요 사이트 핀 |

### 6.2.3 TLS 1.3 핸드셰이크

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.2.3.1 | ClientHello 생성 | `network/src/tls/handshake.rs` | supported_versions, key_share, signature_algorithms, SNI |
| 6.2.3.2 | ServerHello 파싱 | `network/src/tls/handshake.rs` | 선택된 cipher suite, key_share, 0-RTT 결정 |
| 6.2.3.3 | 키 스케줄 | `network/src/tls/key_schedule.rs` (신규) | Early/Handshake/Application 시크릿 파생 (HKDF-Expand-Label) |
| 6.2.3.4 | EncryptedExtensions 처리 | `network/src/tls/handshake.rs` | ALPN, max_fragment_length 등 |
| 6.2.3.5 | Certificate/CertificateVerify 처리 | `network/src/tls/handshake.rs` | 서버 인증서 수신, 서명 검증 |
| 6.2.3.6 | Finished 메시지 | `network/src/tls/handshake.rs` | transcript_hash 기반 verify_data 교환 |
| 6.2.3.7 | 레코드 레이어 암호화 | `network/src/tls/record.rs` | 실제 AEAD encrypt/decrypt, 시퀀스 번호, 패딩 |
| 6.2.3.8 | TLS 세션 재개 (PSK) | `network/src/tls/session.rs` (신규) | NewSessionTicket, PSK 기반 0-RTT |
| 6.2.3.9 | ALPN 협상 | `network/src/tls/handshake.rs` | HTTP/1.1, h2 프로토콜 상위 계층 협상 |
| 6.2.3.10 | TLS 1.2 폴백 | `network/src/tls/tls12.rs` (신규) | 레거시 서버 호환, RSA/ECDHE 키 교환 |

### 6.2.4 HTTPS 클라이언트

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.2.4.1 | TLS over TCP 래퍼 | `network/src/tls/stream.rs` (신규) | `TlsStream` — TCP 소켓 위 TLS 레코드 레이어 |
| 6.2.4.2 | HTTPS URL 핸들링 | `network/src/http.rs` | `https://` 스킴 → TLS 연결 → HTTP 요청 |
| 6.2.4.3 | HTTP 쿠키 관리자 | `network/src/http/cookie.rs` (신규) | Cookie 파싱 (Set-Cookie), 도메인/경로 매칭, Secure/HttpOnly, SameSite, 만료 |
| 6.2.4.4 | HTTP 리다이렉트 자동 추적 | `network/src/http.rs` | 301/302/307/308 → 위치 헤더 팔로우, 최대 20회 |
| 6.2.4.5 | HTTP 압축 해제 (gzip) | `network/src/http/compression.rs` (신규) | DEFLATE 디코딩 (no_std), Content-Encoding 처리 |
| 6.2.4.6 | HTTP 압축 해제 (Brotli) | `network/src/http/compression.rs` | Brotli 디코딩 |
| 6.2.4.7 | HTTP 청크 전송 디코딩 | `network/src/http.rs` | Transfer-Encoding: chunked 파싱 |
| 6.2.4.8 | HTTP 연결 풀링 (Keep-Alive) | `network/src/http/pool.rs` (신규) | 동일 호스트 연결 재사용, 타임아웃 |
| 6.2.4.9 | HTTP/2 기본 지원 | `network/src/http2/mod.rs` (신규) | HPACK 헤더 압축, 스트림 다중화, 프레임 처리 |

### 6.2.5 WebSocket 실제 연결

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.2.5.1 | WebSocket TCP 연결 | `network/src/websocket.rs` | HTTP Upgrade → WebSocket 핸드셰이크 (실제 TCP) |
| 6.2.5.2 | WebSocket TLS 지원 | `network/src/websocket.rs` | `wss://` → TLS 위 WebSocket |
| 6.2.5.3 | WebSocket 프레임 I/O | `network/src/websocket.rs` | 실제 send/recv 루프, ping/pong, close |

## 퀄리티 게이트 (6.2)

> **아래 항목이 100% 충족되기 전까지 6.3으로 진행 금지**

| # | 검증 항목 | 검증 방법 |
|---|----------|----------|
| QG-6.2.1 | TLS 1.3 핸드셰이크 성공 | `curl https://example.com` → 200 OK, 인증서 체인 검증 통과 |
| QG-6.2.2 | SHA-256 테스트 벡터 전수 통과 | NIST 표준 테스트 벡터 (FIPS 180-4) |
| QG-6.2.3 | AES-GCM 테스트 벡터 전수 통과 | NIST SP 800-38D 테스트 벡터 |
| QG-6.2.4 | X25519 키 교환 벡터 통과 | RFC 7748 테스트 벡터 |
| QG-6.2.5 | X.509 인증서 체인 검증 | Google/GitHub/Cloudflare 인증서 정상 검증 |
| QG-6.2.6 | HTTPS 쿠키 자동 관리 | Set-Cookie → 후속 요청에 자동 첨부 확인 |
| QG-6.2.7 | gzip 응답 정상 해제 | gzip 인코딩 응답 → 올바른 본문 디코딩 |
| QG-6.2.8 | HTTP/2 기본 동작 | h2 지원 서버와 스트림 다중화 통신 확인 |
| QG-6.2.9 | WebSocket 에코 테스트 | `wss://echo.websocket.org` 또는 동등 서버와 메시지 교환 |
| QG-6.2.10 | **터미널에서 HTTPS 통신 확인** | `curl https://www.google.com` → HTML 응답 수신 로그 캡처 |
| QG-6.2.11 | 암호 모듈 단위 테스트 40개+ | 각 암호 프리미티브별 최소 5개 테스트 벡터 |

---

# Sub-Phase 6.3: JavaScript Engine Completion

## 목적
현대 웹 어플리케이션이 요구하는 ES2020+ JavaScript 기능을 완전히 구현하여, React/Vue/Angular 등의 프레임워크 코드를 해석·실행할 수 있게 한다.

## 선행 조건
- [x] QG-6.2 100% 충족
- [x] 기본 JS 인터프리터 동작 (변수, 함수, 조건문, 루프)
- [x] GC 기본 프레임워크 존재

## 작업 목록

### 6.3.1 핵심 런타임 기반

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.3.1.1 | 이벤트 루프 구현 | `kpio-js/src/event_loop.rs` (신규) | 매크로태스크 큐 + 마이크로태스크 큐, `process_pending()` |
| 6.3.1.2 | 타이머 API | `kpio-js/src/timers.rs` (신규) | setTimeout, setInterval, clearTimeout, clearInterval |
| 6.3.1.3 | requestAnimationFrame | `kpio-js/src/timers.rs` | 16.67ms 프레임 콜백, cancelAnimationFrame |
| 6.3.1.4 | Promise 완전 구현 | `kpio-js/src/promise.rs` (신규) | new Promise, then, catch, finally, resolve, reject, all, allSettled, race, any |
| 6.3.1.5 | async/await 실행 | `kpio-js/src/interpreter.rs` | async 함수 → Promise 래핑, await → .then 변환, 실행 재개 |
| 6.3.1.6 | Generator/Iterator | `kpio-js/src/generator.rs` (신규) | function*, yield, next(), return(), throw(), for...of, Symbol.iterator |
| 6.3.1.7 | async Generator | `kpio-js/src/generator.rs` | async function*, for await...of |
| 6.3.1.8 | 모듈 시스템 (import/export) | `kpio-js/src/modules.rs` (신규) | ESModule 해석, 의존성 그래프, 순환 참조 처리, default/named export |

### 6.3.2 내장 객체 완성

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.3.2.1 | String.prototype 전체 | `kpio-js/src/builtin/string.rs` (신규) | slice, substring, indexOf, includes, startsWith, endsWith, replace, replaceAll, match, matchAll, split, trim, trimStart, trimEnd, padStart, padEnd, repeat, charAt, charCodeAt, normalize, toUpperCase, toLowerCase, localeCompare, at |
| 6.3.2.2 | Array.prototype 전체 | `kpio-js/src/builtin/array.rs` (신규) | map, filter, reduce, reduceRight, forEach, find, findIndex, findLast, findLastIndex, some, every, flat, flatMap, sort, reverse, splice, slice, concat, includes, indexOf, lastIndexOf, join, fill, copyWithin, entries, keys, values, from, isArray, of, at |
| 6.3.2.3 | Object 정적 메서드 | `kpio-js/src/builtin/object.rs` (신규) | keys, values, entries, assign, freeze, seal, defineProperty, defineProperties, getOwnPropertyDescriptors, getPrototypeOf, setPrototypeOf, create, fromEntries, hasOwn |
| 6.3.2.4 | JSON 완전 구현 | `kpio-js/src/builtin/json.rs` (신규) | parse (재귀 하강 JSON 파서), stringify (순환 참조 감지, replacer, space) |
| 6.3.2.5 | Math 객체 전체 | `kpio-js/src/builtin/math.rs` (신규) | abs, ceil, floor, round, trunc, max, min, random, sqrt, pow, log, log2, log10, sin, cos, tan, atan, atan2, PI, E, sign, cbrt, hypot, clz32, imul, fround |
| 6.3.2.6 | Date 객체 | `kpio-js/src/builtin/date.rs` (신규) | now(), getTime, getFullYear, getMonth, ..., toISOString, toLocaleDateString, parse |
| 6.3.2.7 | RegExp 엔진 | `kpio-js/src/builtin/regexp.rs` (신규) | NFA 기반 정규표현식 엔진, test, exec, 캡처 그룹, 플래그 (g, i, m, s, u, y), 유니코드 |
| 6.3.2.8 | Map / Set | `kpio-js/src/builtin/collections.rs` (신규) | Map (get/set/has/delete/forEach/entries/keys/values/size), Set (add/has/delete/forEach/entries/values/size) |
| 6.3.2.9 | WeakMap / WeakSet | `kpio-js/src/builtin/collections.rs` | 약한 참조 기반, GC 통합 |
| 6.3.2.10 | Symbol | `kpio-js/src/builtin/symbol.rs` (신규) | Symbol(), Symbol.for, well-known symbols (iterator, toPrimitive, toStringTag, hasInstance, species) |
| 6.3.2.11 | ArrayBuffer / TypedArray | `kpio-js/src/builtin/typed_array.rs` (신규) | ArrayBuffer, Int8/16/32Array, Uint8/16/32Array, Float32/64Array, DataView |
| 6.3.2.12 | Error 계층 | `kpio-js/src/builtin/error.rs` (신규) | Error, TypeError, RangeError, ReferenceError, SyntaxError — 스택 트레이스 |
| 6.3.2.13 | Proxy / Reflect | `kpio-js/src/builtin/proxy.rs` (신규) | Proxy handler 트랩 (get/set/has/deleteProperty/apply/construct 등), Revocable |
| 6.3.2.14 | Number/Boolean 메서드 | `kpio-js/src/builtin/number.rs` (신규) | toFixed, toPrecision, isNaN, isFinite, isInteger, parseInt, parseFloat |

### 6.3.3 언어 기능 보완

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.3.3.1 | 프로토타입 체인 완성 | `kpio-js/src/interpreter.rs` | __proto__ 조회, Object.getPrototypeOf, instanceof, new 연산자 정확한 동작 |
| 6.3.3.2 | this 바인딩 (call/apply/bind) | `kpio-js/src/interpreter.rs` | Function.prototype.call/apply/bind, 화살표 함수 this 렉시컬 스코프 |
| 6.3.3.3 | Getter/Setter 동작 | `kpio-js/src/interpreter.rs` | get/set 액세서 프로퍼티, Object.defineProperty 연동 |
| 6.3.3.4 | 구조 분해 기본값 | `kpio-js/src/interpreter.rs` | `const {a = 1} = obj`, 배열·중첩 구조 분해 |
| 6.3.3.5 | Optional chaining (?.) | `kpio-js/src/interpreter.rs` | `obj?.prop`, `obj?.[expr]`, `func?.()` |
| 6.3.3.6 | Nullish coalescing (??) | `kpio-js/src/interpreter.rs` | `null ?? 'default'` |
| 6.3.3.7 | computed property names | `kpio-js/src/interpreter.rs` | `{ [expr]: value }` |
| 6.3.3.8 | for...in / for...of | `kpio-js/src/interpreter.rs` | 열거 가능 프로퍼티, 이터러블 프로토콜 |
| 6.3.3.9 | 태그드 템플릿 리터럴 | `kpio-js/src/interpreter.rs` | `` tag`str ${expr}` `` |
| 6.3.3.10 | 클래스 상속 완성 | `kpio-js/src/interpreter.rs` | extends, super(), super.method(), 정적 메서드, 계산된 프로퍼티 이름, 프라이빗 필드 (#) |
| 6.3.3.11 | Logical assignment | `kpio-js/src/interpreter.rs` | `??=`, `&&=`, `\|\|=` |
| 6.3.3.12 | 동적 import() | `kpio-js/src/modules.rs` | `import('module')` → Promise |

### 6.3.4 GC 강화

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.3.4.1 | 세대별 GC | `kpio-js/src/gc.rs` | Young/Old 세대, 마이너 GC (영 세대 수거), 메이저 GC (전체 마킹) |
| 6.3.4.2 | 증분 마킹 | `kpio-js/src/gc.rs` | 긴 정지 방지, 쓰기 배리어 |
| 6.3.4.3 | WeakRef/FinalizationRegistry | `kpio-js/src/gc.rs` | WeakRef, FinalizationRegistry(weak ref 정리 콜백) |

## 퀄리티 게이트 (6.3)

> **아래 항목이 100% 충족되기 전까지 6.4로 진행 금지**

| # | 검증 항목 | 검증 방법 |
|---|----------|----------|
| QG-6.3.1 | Promise 체이닝 동작 | `Promise.resolve(1).then(x => x+1).then(x => console.log(x))` → `2` |
| QG-6.3.2 | async/await 동작 | `async function f() { return await Promise.resolve(42); }` → `42` |
| QG-6.3.3 | 이벤트 루프 순서 보장 | 마이크로태스크가 매크로태스크보다 먼저 실행됨을 검증 |
| QG-6.3.4 | Array.prototype.map/filter/reduce | `[1,2,3].map(x=>x*2).filter(x=>x>2).reduce((a,b)=>a+b, 0)` → `10` |
| QG-6.3.5 | JSON.parse 복잡 객체 | `JSON.parse('{"a":[1,{"b":true}]}')` → 올바른 객체 |
| QG-6.3.6 | JSON.stringify 객체/배열 | `JSON.stringify({a:1, b:[2,3]})` → `'{"a":1,"b":[2,3]}'` |
| QG-6.3.7 | RegExp 기본 동작 | `/(\d+)-(\d+)/g.exec('2024-01-15')` → 캡처 그룹 정상 |
| QG-6.3.8 | 프로토타입 상속 | `class A { f() { return 1; } } class B extends A {} new B().f()` → `1` |
| QG-6.3.9 | Map/Set 동작 | `new Map([['a',1]]).get('a')` → `1` |
| QG-6.3.10 | Generator 동작 | `function* g() { yield 1; yield 2; } [...g()]` → `[1, 2]` |
| QG-6.3.11 | 모듈 import/export | ESModule 정적 import, 순환 참조 해결 확인 |
| QG-6.3.12 | GC 메모리 누수 없음 | 10000회 객체 생성/파기 후 힙 크기 안정화 |
| QG-6.3.13 | Date.now() 동작 | 커널 타이머 연동하여 밀리초 타임스탬프 반환 |
| QG-6.3.14 | 내장 객체 테스트 100개+ | String/Array/Object/Math/Date/RegExp/Map/Set 각 최소 10개 |

---

# Sub-Phase 6.4: CSS & Layout Engine Completion

## 목적
현대 웹사이트의 레이아웃을 정확하게 계산하고 렌더링할 수 있도록 CSS 파싱, 캐스케이딩, 레이아웃 알고리즘을 완성한다.

## 선행 조건
- [x] QG-6.3 100% 충족 (JS 엔진이 CSSOM 조작에 필요)
- [x] CSS 기본 파싱 동작
- [x] Block/Inline/Flex 레이아웃 기초 존재

## 작업 목록

### 6.4.1 CSS 파서 확장

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.4.1.1 | @media 쿼리 파싱·평가 | `kpio-css/src/media.rs` (신규) | 미디어 타입 (screen/print), 조건 (width, height, orientation, prefers-color-scheme), AND/OR/NOT 결합 |
| 6.4.1.2 | @keyframes 파싱 | `kpio-css/src/animation.rs` (신규) | 키프레임 선언 (from/to/%), 프로퍼티 보간 |
| 6.4.1.3 | @font-face 파싱 | `kpio-css/src/font_face.rs` (신규) | font-family, src (url/local), font-weight, font-style, unicode-range |
| 6.4.1.4 | @import 처리 | `kpio-css/src/parser.rs` | URL 기반 외부 스타일시트 로드 (HTTP 연동) |
| 6.4.1.5 | @supports 평가 | `kpio-css/src/parser.rs` | 피처 검사 (`(display: grid)`, `not`, `and`, `or`) |
| 6.4.1.6 | calc() / min() / max() / clamp() | `kpio-css/src/values.rs` | 수학 표현식 파싱 및 실행 시점 평가, 단위 혼합 (`calc(100% - 20px)`) |
| 6.4.1.7 | var() / CSS Custom Properties | `kpio-css/src/custom_properties.rs` (신규) | `--var-name: value` 선언, `var(--name, fallback)` 해석, 상속 |
| 6.4.1.8 | Shorthand 속성 확장 | `kpio-css/src/shorthand.rs` (신규) | margin, padding, border, background, font, flex, grid, animation, transition → 개별 속성으로 분해 |
| 6.4.1.9 | !important 우선순위 | `kpio-css/src/cascade.rs` | important 플래그 기반 캐스케이드 순서 조정 |
| 6.4.1.10 | transform 함수 파싱 | `kpio-css/src/values.rs` | translate/rotate/scale/skew/matrix, 3D 변환 (translate3d, perspective) |
| 6.4.1.11 | gradient 파싱 | `kpio-css/src/values.rs` | linear-gradient, radial-gradient, conic-gradient, 색상 스톱 |
| 6.4.1.12 | box-shadow / text-shadow | `kpio-css/src/values.rs` | 오프셋/블러/스프레드/색상 파싱 |
| 6.4.1.13 | filter / backdrop-filter | `kpio-css/src/values.rs` | blur(), brightness(), contrast(), grayscale(), hue-rotate(), invert(), saturate(), sepia(), drop-shadow() |

### 6.4.2 CSS 캐스케이드·상속 완성

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.4.2.1 | 완전한 상속 모델 | `kpio-css/src/cascade.rs` | 상속 프로퍼티 목록 (color, font-*, line-height, text-align 등), inherit/initial/unset/revert 키워드 |
| 6.4.2.2 | 사용자 에이전트 스타일시트 | `kpio-css/src/ua_styles.rs` (신규) | HTML5 기본 스타일시트 (display, margin, font 등) |
| 6.4.2.3 | 셀렉터 매칭 최적화 | `kpio-css/src/selector.rs` | 우에서 좌로 매칭, 블룸 필터, 해시 맵 인덱스 |
| 6.4.2.4 | 고급 셀렉터 | `kpio-css/src/selector.rs` | :not(), :is(), :where(), :has(), :nth-child(An+B), :nth-of-type, :first-child, :last-child, :empty, :focus, :hover, :active, :visited, :checked, :disabled, ::before, ::after, ::placeholder, ::selection |
| 6.4.2.5 | 스타일 무효화 (Dirty 체크) | `kpio-css/src/style_invalidation.rs` (신규) | DOM 변경 시 영향받는 스타일만 재계산 (셀렉터 기반 dirty 비트) |

### 6.4.3 레이아웃 엔진 확장

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.4.3.1 | position: absolute/relative | `kpio-layout/src/positioned.rs` (신규) | Containing block 결정, offset (top/right/bottom/left), 절대 위치 계산 |
| 6.4.3.2 | position: fixed | `kpio-layout/src/positioned.rs` | 뷰포트 기준 고정, 스크롤 불변 |
| 6.4.3.3 | position: sticky | `kpio-layout/src/positioned.rs` | 스크롤 임계값 기반 고정/해제 |
| 6.4.3.4 | z-index / stacking context | `kpio-layout/src/stacking.rs` (신규) | 스태킹 컨텍스트 생성 규칙, 페인트 순서 결정 |
| 6.4.3.5 | float / clear | `kpio-layout/src/float.rs` (신규) | 좌/우 플로트, 문서 흐름 제외, clear 처리 |
| 6.4.3.6 | Grid 레이아웃 | `kpio-layout/src/grid.rs` (신규) | grid-template-rows/columns, fr 단위, repeat(), minmax(), auto-fill/auto-fit, gap, grid-area, alignment |
| 6.4.3.7 | Table 레이아웃 | `kpio-layout/src/table.rs` (신규) | table/tr/td 레이아웃, colspan/rowspan, border-collapse, 자동 너비 분배 |
| 6.4.3.8 | overflow: scroll/hidden/auto | `kpio-layout/src/overflow.rs` (신규) | 스크롤 컨테이너, 콘텐츠 클리핑, 스크롤바 |
| 6.4.3.9 | min/max-width/height | `kpio-layout/src/block.rs` | 너비/높이 제약 적용, 인트린직 사이즈 |
| 6.4.3.10 | 텍스트 줄바꿈 (Unicode) | `kpio-layout/src/text.rs` (신규) | Unicode Line Break Algorithm (UAX #14), word-break, overflow-wrap, hyphens |
| 6.4.3.11 | 실제 폰트 메트릭 | `kpio-layout/src/text.rs` | 글리프 advance width, ascent, descent, line height 계산 |
| 6.4.3.12 | Replaced elements (img) | `kpio-layout/src/replaced.rs` (신규) | 이미지 인트린직 사이즈, object-fit, aspect-ratio |
| 6.4.3.13 | Flexbox 완성 | `kpio-layout/src/flex.rs` | flex-grow/shrink/basis 분배, align-self, order, flex-wrap 여러 줄, gap |
| 6.4.3.14 | 마진 상쇄 완성 | `kpio-layout/src/block.rs` | 인접 형제, 부모-자식, 빈 블록 마진 상쇄, 마진 상쇄 차단 규칙 |

### 6.4.4 CSS 애니메이션 & 트랜지션

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.4.4.1 | CSS Transitions | `kpio-css/src/transition.rs` (신규) | transition-property, transition-duration, transition-timing-function, transition-delay, 프로퍼티 보간 |
| 6.4.4.2 | CSS Animations | `kpio-css/src/animation.rs` | animation-name, animation-duration, animation-iteration-count, animation-direction, animation-fill-mode, animation-play-state |
| 6.4.4.3 | 타이밍 함수 | `kpio-css/src/timing.rs` (신규) | linear, ease, ease-in, ease-out, ease-in-out, cubic-bezier(), steps() |
| 6.4.4.4 | 프로퍼티 보간 | `kpio-css/src/interpolation.rs` (신규) | 숫자, 길이, 색상, transform 보간 |
| 6.4.4.5 | 애니메이션 ↔ JS 이벤트 | `kpio-css/src/animation.rs` | animationstart, animationend, animationiteration, transitionend 이벤트 발화 |

## 퀄리티 게이트 (6.4)

> **아래 항목이 100% 충족되기 전까지 6.5로 진행 금지**

| # | 검증 항목 | 검증 방법 |
|---|----------|----------|
| QG-6.4.1 | position: absolute 동작 | 절대 위치 요소가 containing block 기준 정확한 좌표에 배치 |
| QG-6.4.2 | Flexbox 레이아웃 정확도 | CSS Flexbox 사양 테스트 10개 이상 통과 |
| QG-6.4.3 | Grid 레이아웃 기본 동작 | `grid-template-columns: 1fr 2fr 1fr` → 정확한 열 분배 |
| QG-6.4.4 | float 레이아웃 | 텍스트가 플로트된 이미지를 감싸는 레이아웃 정확 |
| QG-6.4.5 | @media 쿼리 반응 | 뷰포트 크기 변경 시 미디어 쿼리 재평가, 스타일 전환 |
| QG-6.4.6 | calc() 혼합 단위 | `calc(100% - 20px)` → 정확한 픽셀 값 |
| QG-6.4.7 | CSS Custom Properties | `var(--color)` → 상속된 값 정상 해석 |
| QG-6.4.8 | CSS Transitions | `transition: opacity 0.3s` → 부드러운 불투명도 전환 |
| QG-6.4.9 | 사용자 에이전트 스타일시트 | 스타일 없는 HTML → 합리적인 기본 렌더링 |
| QG-6.4.10 | 레이아웃 테스트 80개+ | Block/Inline/Flex/Grid/Float/Position 각 최소 10개 |
| QG-6.4.11 | 마진 상쇄 정확도 | CSS2.1 마진 상쇄 명세 테스트 통과 |

---

# Sub-Phase 6.5: Web Platform APIs

## 목적
브라우저 환경에서 웹 어플리케이션이 사용하는 핵심 Web API를 구현하여, JavaScript에서 네트워크 통신, 스토리지, DOM 조작, 타이머 등을 수행할 수 있게 한다.

## 선행 조건
- [x] QG-6.4 100% 충족
- [x] JS 엔진 Promise/async/await 동작
- [x] HTTPS 통신 동작
- [x] DOM 트리 기본 동작

## 작업 목록

### 6.5.1 네트워크 API

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.5.1.1 | fetch() API | `kpio-js/src/web_api/fetch.rs` (신규) | `fetch(url, options)` → Promise<Response>, Request/Response/Headers 객체, Body mixin (text/json/arrayBuffer/blob) |
| 6.5.1.2 | XMLHttpRequest | `kpio-js/src/web_api/xhr.rs` (신규) | open/send/onreadystatechange/responseText/responseXML, 동기/비동기 |
| 6.5.1.3 | WebSocket JS 바인딩 | `kpio-js/src/web_api/websocket.rs` (신규) | new WebSocket(url), onopen/onmessage/onerror/onclose, send, close |
| 6.5.1.4 | AbortController/AbortSignal | `kpio-js/src/web_api/abort.rs` (신규) | fetch 취소, 시그널 전파 |
| 6.5.1.5 | EventSource (SSE) | `kpio-js/src/web_api/sse.rs` (신규) | Server-Sent Events, text/event-stream 파싱 |
| 6.5.1.6 | navigator.onLine | `kpio-js/src/web_api/navigator.rs` (신규) | 네트워크 상태 감지, online/offline 이벤트 |

### 6.5.2 스토리지 API

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.5.2.1 | localStorage | `kpio-js/src/web_api/storage.rs` (신규) | getItem/setItem/removeItem/clear/key/length, 도메인별 격리, 영구 저장 (VFS 연동) |
| 6.5.2.2 | sessionStorage | `kpio-js/src/web_api/storage.rs` | 세션 스코프, 탭별 격리 |
| 6.5.2.3 | IndexedDB | `kpio-js/src/web_api/indexeddb.rs` (신규) | open/createObjectStore/transaction/objectStore, put/get/delete, 인덱스, 커서, 버전 관리 |
| 6.5.2.4 | Cache API | `kpio-js/src/web_api/cache.rs` (신규) | caches.open, match, put, delete — Service Worker 캐시 |
| 6.5.2.5 | Cookie API (document.cookie) | `kpio-js/src/web_api/cookie.rs` (신규) | 쿠키 읽기/쓰기, 도메인/경로 스코핑, Secure/HttpOnly 존중 |

### 6.5.3 DOM 확장 API

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.5.3.1 | MutationObserver | `kpio-dom/src/mutation_observer.rs` (신규) | DOM 변경 감시 (childList, attributes, characterData, subtree) |
| 6.5.3.2 | IntersectionObserver | `kpio-dom/src/intersection_observer.rs` (신규) | 요소 가시성 감시, 임계값 콜백 |
| 6.5.3.3 | ResizeObserver | `kpio-dom/src/resize_observer.rs` (신규) | 요소 크기 변경 감시 |
| 6.5.3.4 | DOMParser / innerHTML | `kpio-dom/src/dom_parser.rs` (신규) | HTML/XML 문자열 → DOM 트리 파싱 |
| 6.5.3.5 | Range / Selection API | `kpio-dom/src/range.rs` (신규) | 텍스트 범위, 선택 영역, createRange, getSelection |
| 6.5.3.6 | Element.classList | `kpio-dom/src/element.rs` | add/remove/toggle/contains/replace |
| 6.5.3.7 | Element.dataset | `kpio-dom/src/element.rs` | data-* 속성 접근 |
| 6.5.3.8 | Element.style (CSSOM) | `kpio-dom/src/cssom.rs` (신규) | style 프로퍼티 읽기/쓰기, getComputedStyle() |
| 6.5.3.9 | Element.getBoundingClientRect | `kpio-dom/src/element.rs` | 레이아웃 엔진 연동 → DOMRect 반환 |
| 6.5.3.10 | Element.scrollIntoView | `kpio-dom/src/element.rs` | 스크롤 위치 조정 |

### 6.5.4 Window/Navigator API

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.5.4.1 | window.location | `kpio-js/src/web_api/location.rs` (신규) | href, protocol, host, pathname, search, hash, assign, replace, reload |
| 6.5.4.2 | History API | `kpio-js/src/web_api/history.rs` (신규) | pushState, replaceState, back, forward, go, popstate 이벤트 |
| 6.5.4.3 | navigator.userAgent | `kpio-js/src/web_api/navigator.rs` | KPIO 고유 UA 문자열 |
| 6.5.4.4 | navigator.clipboard | `kpio-js/src/web_api/clipboard.rs` (신규) | readText, writeText (비동기 Promise 기반) |
| 6.5.4.5 | window.matchMedia | `kpio-js/src/web_api/media_query.rs` (신규) | MediaQueryList, matches, addEventListener('change') |
| 6.5.4.6 | Performance API | `kpio-js/src/web_api/performance.rs` (신규) | performance.now(), performance.mark(), performance.measure(), PerformanceObserver |
| 6.5.4.7 | console 확장 | `kpio-js/src/web_api/console.rs` (신규) | console.log/warn/error/info/debug/table/time/timeEnd/group/groupEnd/assert/count/trace |

### 6.5.5 기타 웹 API

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.5.5.1 | URL / URLSearchParams | `kpio-js/src/web_api/url.rs` (신규) | URL 생성·파싱, searchParams 조작 |
| 6.5.5.2 | FormData | `kpio-js/src/web_api/form_data.rs` (신규) | append, get, set, entries, 멀티파트 인코딩 |
| 6.5.5.3 | Blob / File / FileReader | `kpio-js/src/web_api/blob.rs` (신규) | Blob 생성, slice, FileReader (readAsText/readAsArrayBuffer/readAsDataURL) |
| 6.5.5.4 | TextEncoder / TextDecoder | `kpio-js/src/web_api/encoding.rs` (신규) | UTF-8/UTF-16 인코딩/디코딩 |
| 6.5.5.5 | Crypto (Web Crypto API) | `kpio-js/src/web_api/crypto.rs` (신규) | crypto.getRandomValues, crypto.subtle (digest, encrypt, decrypt, sign, verify, generateKey, deriveBits) |
| 6.5.5.6 | structuredClone | `kpio-js/src/web_api/structured_clone.rs` (신규) | 깊은 복제 (순환 참조, TypedArray, Map/Set, Date, RegExp) |
| 6.5.5.7 | queueMicrotask | `kpio-js/src/event_loop.rs` | 마이크로태스크 큐에 직접 추가 |

## 퀄리티 게이트 (6.5)

> **아래 항목이 100% 충족되기 전까지 6.6으로 진행 금지**

| # | 검증 항목 | 검증 방법 |
|---|----------|----------|
| QG-6.5.1 | fetch JSON | `fetch('https://httpbin.org/json').then(r=>r.json()).then(data=>console.log(data))` 동작 |
| QG-6.5.2 | localStorage 영속성 | 페이지 새로고침 후 저장된 값 유지 |
| QG-6.5.3 | MutationObserver 동작 | DOM 변경 시 콜백 정상 호출 |
| QG-6.5.4 | History pushState | 뒤로/앞으로 버튼 → popstate 이벤트 정상 발화 |
| QG-6.5.5 | FormData + fetch POST | 폼 데이터 직렬화 → POST 전송 → 서버 응답 수신 |
| QG-6.5.6 | URL 파싱 | `new URL('https://a:b@c.com:8080/d?e=f#g')` → 모든 컴포넌트 정상 |
| QG-6.5.7 | TextEncoder/Decoder | UTF-8 왕복 (encode → decode) 정확도 100% |
| QG-6.5.8 | Performance.now() 정밀도 | 마이크로초 정밀도, 단조 증가 |
| QG-6.5.9 | WebSocket 메시지 교환 | JS에서 `new WebSocket()` → 양방향 메시지 확인 |
| QG-6.5.10 | Web API 테스트 60개+ | 각 API 카테고리별 최소 5개 |

---

# Sub-Phase 6.6: Browser Core Features

## 목적
HTTP 리소스 로딩, 서브리소스 파이프라인, 이미지 디코딩, 폼 제출, 입력 요소, 스크롤, 리페인트 등 브라우저의 핵심 렌더링·인터랙션 기능을 완성한다.

## 선행 조건
- [x] QG-6.5 100% 충족
- [x] HTTPS + 쿠키 동작
- [x] JS 엔진 + Web API 동작
- [x] CSS 레이아웃 엔진 동작

## 작업 목록

### 6.6.1 리소스 로딩 파이프라인

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.6.1.1 | 리소스 로더 | `kpio-browser/src/loader/resource_loader.rs` (신규) | URL → fetch → 응답 → 파싱 (HTML/CSS/JS/이미지/폰트 분류), 우선순위, 동시 요청 제한 (6개/호스트) |
| 6.6.1.2 | CSS 서브리소스 로딩 | `kpio-browser/src/loader/css_loader.rs` (신규) | `<link rel="stylesheet">`, `@import` → CSS 파싱 → 스타일 병합 |
| 6.6.1.3 | JS 스크립트 로딩 | `kpio-browser/src/loader/script_loader.rs` (신규) | `<script src>` → 다운로드 → 파싱 → 실행, defer/async 속성 |
| 6.6.1.4 | 이미지 리소스 로딩 | `kpio-browser/src/loader/image_loader.rs` (신규) | `<img src>`, CSS background-image → 다운로드 → 디코딩 대기열 |
| 6.6.1.5 | 폰트 리소스 로딩 | `kpio-browser/src/loader/font_loader.rs` (신규) | @font-face URL → 다운로드 → 폰트 등록 → FOUT/FOIT 처리 |
| 6.6.1.6 | Preload / Prefetch | `kpio-browser/src/loader/preload.rs` (신규) | `<link rel="preload/prefetch">`, 리소스 힌트 |
| 6.6.1.7 | CSP 리소스 필터링 | `kpio-browser/src/loader/resource_loader.rs` | CSP 정책에 따라 리소스 차단/허용 |

### 6.6.2 이미지 디코딩

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.6.2.1 | PNG 디코더 | `kpio-browser/src/media/png.rs` (신규) | IHDR/IDAT/IEND 청크, DEFLATE 해제, 필터 역적용, 8/16비트 RGBA, 인터레이스 |
| 6.6.2.2 | JPEG 디코더 | `kpio-browser/src/media/jpeg.rs` (신규) | 허프만/양자화 테이블, DCT 역변환, YCbCr→RGB, 프로그레시브 |
| 6.6.2.3 | GIF 디코더 | `kpio-browser/src/media/gif.rs` (신규) | LZW 해제, 프레임 시퀀스, 투명도, 애니메이션 |
| 6.6.2.4 | WebP 디코더 | `kpio-browser/src/media/webp.rs` (신규) | VP8/VP8L 디코딩, 손실/무손실 |
| 6.6.2.5 | SVG 렌더러 | `kpio-browser/src/media/svg.rs` (신규) | SVG 파싱 (XML), 기본 도형 (rect/circle/path/text/line/polyline/polygon), viewBox, 변환 |
| 6.6.2.6 | ICO/Favicon | `kpio-browser/src/media/ico.rs` (신규) | 파비콘 디코딩, 탭 표시 |

### 6.6.3 폰트 렌더링

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.6.3.1 | TrueType 파서 | `kpio-browser/src/font/ttf.rs` (신규) | TTF 테이블 파싱 (head, cmap, glyf, loca, hhea, hmtx, maxp, name, post, kern) |
| 6.6.3.2 | OpenType/WOFF2 파서 | `kpio-browser/src/font/otf.rs` (신규) | OTF 테이블, WOFF2 Brotli 해제 |
| 6.6.3.3 | 글리프 래스터라이저 | `kpio-browser/src/font/rasterizer.rs` (신규) | 베지에 곡선 → 비트맵, 안티앨리어싱, 서브픽셀 렌더링 |
| 6.6.3.4 | 폰트 매칭 | `kpio-browser/src/font/matching.rs` (신규) | font-family 폴백 체인, 가중치/너비/스타일 매칭 알고리즘 |
| 6.6.3.5 | 시스템 폰트 번들 | `kpio-browser/src/font/system_fonts.rs` (신규) | Noto Sans (라틴+CJK) WOFF2 임베딩, Sans-Serif/Serif/Monospace 기본 폰트 |
| 6.6.3.6 | 글리프 캐시 | `kpio-browser/src/font/cache.rs` (신규) | LRU 글리프 비트맵 캐시, 크기별 캐시 |

### 6.6.4 입력·폼·인터랙션

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.6.4.1 | 텍스트 입력 필드 | `kpio-browser/src/input/text_input.rs` (신규) | `<input type="text/password/email/search/url/tel">`, 커서, 선택, 복사/붙여넣기 |
| 6.6.4.2 | 텍스트영역 | `kpio-browser/src/input/textarea.rs` (신규) | `<textarea>`, 멀티라인, 스크롤 |
| 6.6.4.3 | 체크박스/라디오 | `kpio-browser/src/input/checkbox.rs` (신규) | 상태 토글, name 그룹 |
| 6.6.4.4 | 셀렉트/드롭다운 | `kpio-browser/src/input/select.rs` (신규) | `<select><option>`, 드롭다운 표시, 선택 |
| 6.6.4.5 | 범위 입력 | `kpio-browser/src/input/range.rs` (신규) | `<input type="range">`, 슬라이더 |
| 6.6.4.6 | 날짜 입력 | `kpio-browser/src/input/date.rs` (신규) | `<input type="date/time/datetime-local">` |
| 6.6.4.7 | Form 제출 | `kpio-browser/src/input/form.rs` (신규) | `<form>` action/method, URL 인코딩, multipart/form-data, validation |
| 6.6.4.8 | contenteditable | `kpio-browser/src/input/editable.rs` (신규) | 인라인 편집, execCommand 기본 세트 |
| 6.6.4.9 | 포커스 관리 | `kpio-browser/src/input/focus.rs` (신규) | tabindex, focus/blur 이벤트, 포커스 순서 |
| 6.6.4.10 | 드래그 앤 드롭 | `kpio-browser/src/input/dnd.rs` (신규) | dragstart/dragenter/dragover/dragleave/drop/dragend, DataTransfer |

### 6.6.5 스크롤 & 리페인트

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.6.5.1 | 문서 스크롤 | `kpio-browser/src/scroll.rs` (신규) | 마우스 휠 → 스크롤 오프셋, 스크롤바 렌더링, smooth scroll |
| 6.6.5.2 | 오버플로우 스크롤 | `kpio-browser/src/scroll.rs` | overflow: auto/scroll 컨테이너 개별 스크롤 |
| 6.6.5.3 | 리페인트/리플로우 | `kpio-browser/src/render_pipeline.rs` | DOM/스타일 변경 → dirty 영역 → 부분 레이아웃 → 부분 페인트 |
| 6.6.5.4 | HitTest 정확도 | `kpio-browser/src/input/hit_test.rs` (신규) | 클릭 좌표 → 레이아웃 트리 → DOM 요소 매핑 (z-index, transform 고려) |
| 6.6.5.5 | 커서 변경 | `kpio-browser/src/input/cursor.rs` (신규) | CSS cursor 속성에 따른 커서 형태 변경 (pointer, text, default, move 등) |

### 6.6.6 네비게이션 & 탭

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.6.6.1 | 주소창 URL 반영 | `kpio-browser/src/ui/address_bar.rs` (신규) | URL 입력, 네비게이션, 자동완성 |
| 6.6.6.2 | 뒤로/앞으로 | `kpio-browser/src/navigation.rs` (신규) | 히스토리 스택 기반 네비게이션, DOM 상태 캐시 |
| 6.6.6.3 | 탭 관리 강화 | `kpio-browser/src/tabs.rs` | 탭 간 전환, 탭별 독립 레이아웃 트리/JS 환경 |
| 6.6.6.4 | 새 탭 페이지 | `kpio-browser/src/ui/new_tab.rs` (신규) | 빈 탭 → 검색 바, 자주 방문 사이트, 북마크 |
| 6.6.6.5 | 북마크 관리 | `kpio-browser/src/ui/bookmarks.rs` (신규) | 북마크 추가/삭제/폴더, VFS 영구 저장 |
| 6.6.6.6 | 다운로드 관리자 | `kpio-browser/src/ui/downloads.rs` (신규) | 파일 다운로드 → VFS 저장, 진행률, 일시정지/재개 |

## 퀄리티 게이트 (6.6)

> **아래 항목이 100% 충족되기 전까지 6.7로 진행 금지**

| # | 검증 항목 | 검증 방법 |
|---|----------|----------|
| QG-6.6.1 | 이미지 있는 웹페이지 렌더링 | PNG/JPEG 이미지가 포함된 HTML → 이미지 정상 표시 |
| QG-6.6.2 | 외부 CSS/JS 로딩 | `<link rel="stylesheet">` + `<script src>` → 정상 적용 |
| QG-6.6.3 | 폼 제출 | `<form method="POST">` → 서버에 데이터 전송 → 응답 표시 |
| QG-6.6.4 | 텍스트 입력 동작 | `<input type="text">` → 키 입력·커서·선택·삭제 정상 |
| QG-6.6.5 | 스크롤 동작 | 긴 문서 → 마우스 휠 스크롤 → 부드러운 화면 이동 |
| QG-6.6.6 | 리페인트 효율성 | DOM 변경 → 전체 레이아웃 아닌 부분 리플로우 확인 |
| QG-6.6.7 | TrueType 폰트 렌더링 | 임베디드 Noto Sans → 문자 정확 렌더링, CJK 포함 |
| QG-6.6.8 | 네비게이션 정상 | 링크 클릭 → 새 페이지 로드, 뒤로/앞으로 동작 |
| QG-6.6.9 | HitTest 정확도 | 겹치는 요소 클릭 → 올바른 최상위 요소에 이벤트 전달 |
| QG-6.6.10 | **실제 웹사이트 렌더링** | `https://example.com` → 이미지·스타일·텍스트 포함 정상 렌더링 확인 |
| QG-6.6.11 | SVG 기본 렌더링 | SVG 아이콘/로고 정상 표시 (rect, circle, path) |

---

# Sub-Phase 6.7: Media & Graphics Pipeline

## 목적
Canvas 2D API, 고급 렌더링 기능, 오디오/비디오 기본 지원을 구현하여 리치 미디어 웹앱을 지원한다.

## 선행 조건
- [x] QG-6.6 100% 충족
- [x] 이미지 디코딩 파이프라인 동작
- [x] 폰트 래스터라이저 동작

## 작업 목록

### 6.7.1 Canvas 2D API

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.7.1.1 | CanvasRenderingContext2D | `kpio-browser/src/canvas/context2d.rs` (신규) | getContext('2d'), 상태 스택 (save/restore) |
| 6.7.1.2 | 경로 그리기 | `kpio-browser/src/canvas/path.rs` (신규) | beginPath, moveTo, lineTo, arc, arcTo, quadraticCurveTo, bezierCurveTo, closePath, fill, stroke |
| 6.7.1.3 | 사각형 | `kpio-browser/src/canvas/context2d.rs` | fillRect, strokeRect, clearRect |
| 6.7.1.4 | 텍스트 | `kpio-browser/src/canvas/context2d.rs` | fillText, strokeText, measureText, font, textAlign, textBaseline |
| 6.7.1.5 | 이미지 | `kpio-browser/src/canvas/context2d.rs` | drawImage (3, 5, 9 인자 형태), createImageData, putImageData, getImageData |
| 6.7.1.6 | 변환 | `kpio-browser/src/canvas/context2d.rs` | translate, rotate, scale, setTransform, resetTransform, transform |
| 6.7.1.7 | 스타일 | `kpio-browser/src/canvas/context2d.rs` | fillStyle, strokeStyle (색상, 그라디언트, 패턴), globalAlpha, globalCompositeOperation |
| 6.7.1.8 | 그라디언트 | `kpio-browser/src/canvas/gradient.rs` (신규) | createLinearGradient, createRadialGradient, addColorStop |
| 6.7.1.9 | 라인 스타일 | `kpio-browser/src/canvas/context2d.rs` | lineWidth, lineCap, lineJoin, miterLimit, setLineDash, lineDashOffset |
| 6.7.1.10 | 클리핑 | `kpio-browser/src/canvas/context2d.rs` | clip() — 현재 경로로 클리핑 영역 설정 |
| 6.7.1.11 | toBlob / toDataURL | `kpio-browser/src/canvas/context2d.rs` | 캔버스 → PNG/JPEG 인코딩 |
| 6.7.1.12 | OffscreenCanvas | `kpio-browser/src/canvas/offscreen.rs` (신규) | 오프스크린 렌더링, transferToImageBitmap |

### 6.7.2 고급 렌더링

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.7.2.1 | border-radius 렌더링 | `graphics/src/render.rs` | 둥근 모서리 래스터라이징 (원호 세그먼트) |
| 6.7.2.2 | box-shadow 렌더링 | `graphics/src/effects.rs` (신규) | 가우시안 블러 기반 그림자, 오프셋/블러/스프레드 |
| 6.7.2.3 | 그라디언트 배경 렌더링 | `graphics/src/gradient.rs` (신규) | linear/radial 그라디언트 래스터라이징 |
| 6.7.2.4 | opacity / 합성 | `graphics/src/compositor.rs` | 알파 블렌딩, 레이어 합성 |
| 6.7.2.5 | CSS transform 렌더링 | `graphics/src/transform.rs` (신규) | 2D/3D 변환 행렬 적용, 보간 |
| 6.7.2.6 | CSS filter 렌더링 | `graphics/src/filter.rs` (신규) | blur, brightness, contrast, grayscale 등 픽셀 처리 |
| 6.7.2.7 | 텍스트 안티앨리어싱 개선 | `graphics/src/font.rs` | 서브픽셀 렌더링, 감마 보정 |

### 6.7.3 오디오 기본 지원

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.7.3.1 | PCM 오디오 출력 | `kernel/src/drivers/audio/hda.rs` (신규) | Intel HDA 컨트롤러 기본 드라이버, PCM 버퍼 재생 |
| 6.7.3.2 | Audio 요소 | `kpio-browser/src/media/audio.rs` (신규) | `<audio>` 요소, play/pause/currentTime/volume/duration |
| 6.7.3.3 | WAV 디코더 | `kpio-browser/src/media/wav.rs` (신규) | RIFF WAV 헤더 파싱, PCM 데이터 추출 |
| 6.7.3.4 | Web Audio API 기초 | `kpio-js/src/web_api/web_audio.rs` (신규) | AudioContext, OscillatorNode, GainNode, destination |

### 6.7.4 비디오 기본 지원

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.7.4.1 | Video 요소 프레임워크 | `kpio-browser/src/media/video.rs` (신규) | `<video>` 요소, play/pause/currentTime/controls |
| 6.7.4.2 | MP4 컨테이너 파싱 | `kpio-browser/src/media/mp4.rs` (신규) | MP4 atom/box 파싱, 트랙 추출 |

## 퀄리티 게이트 (6.7)

> **아래 항목이 100% 충족되기 전까지 6.8로 진행 금지**

| # | 검증 항목 | 검증 방법 |
|---|----------|----------|
| QG-6.7.1 | Canvas 도형 그리기 | fillRect + arc + bezierCurveTo → 정상 출력 |
| QG-6.7.2 | Canvas 텍스트 | fillText → 폰트·크기·정렬 정확 |
| QG-6.7.3 | Canvas 이미지 | drawImage → PNG/JPEG 이미지 정상 표시 |
| QG-6.7.4 | border-radius | `border-radius: 10px` → 부드러운 둥근 모서리 |
| QG-6.7.5 | box-shadow | 그림자 렌더링 → 블러·오프셋이 육안으로 확인 |
| QG-6.7.6 | CSS transform | `transform: rotate(45deg)` → 요소 45도 회전 |
| QG-6.7.7 | 그라디언트 배경 | `linear-gradient(red, blue)` → 부드러운 색상 전환 |
| QG-6.7.8 | 오디오 재생 | WAV 파일 재생 → 소리 출력 확인 (QEMU 오디오 백엔드) |
| QG-6.7.9 | Canvas 테스트 30개+ | 경로, 변환, 이미지, 텍스트 각 최소 5개 |

---

# Sub-Phase 6.8: Advanced Web Platform

## 목적
Service Worker, Web Worker, WebGL, 고급 스토리지, PWA 지원 등 현대 웹 플랫폼의 고급 기능을 구현한다.

## 선행 조건
- [x] QG-6.7 100% 충족
- [x] 모든 핵심 Web API 동작
- [x] Canvas 2D 동작

## 작업 목록

### 6.8.1 Web Workers

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.8.1.1 | Worker 스레드 모델 | `kpio-js/src/worker/mod.rs` (신규) | new Worker(url), postMessage/onmessage, 독립 JS 환경 (별도 힙+이벤트 루프) |
| 6.8.1.2 | Structured Clone 전송 | `kpio-js/src/worker/mod.rs` | postMessage 데이터 직렬화/역직렬화 |
| 6.8.1.3 | SharedWorker | `kpio-js/src/worker/shared.rs` (신규) | 다수 탭 간 공유 워커, MessagePort |
| 6.8.1.4 | Transferable Objects | `kpio-js/src/worker/mod.rs` | ArrayBuffer transfer (소유권 이동) |

### 6.8.2 Service Workers

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.8.2.1 | SW 등록/설치/활성화 | `kpio-browser/src/service_worker/lifecycle.rs` (신규) | register(), install/activate 이벤트, 스코프 매칭 |
| 6.8.2.2 | Fetch 이벤트 가로채기 | `kpio-browser/src/service_worker/fetch_handler.rs` (신규) | fetch 이벤트 → respondWith() → Cache/Network 전략 |
| 6.8.2.3 | SW 캐시 전략 | `kpio-browser/src/service_worker/strategy.rs` (신규) | Cache-First, Network-First, Stale-While-Revalidate |
| 6.8.2.4 | Push API 기초 | `kpio-browser/src/service_worker/push.rs` (신규) | PushManager, 구독, 알림 표시 |

### 6.8.3 WebGL 기초

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.8.3.1 | WebGLRenderingContext | `kpio-browser/src/webgl/context.rs` (신규) | getContext('webgl'), 상태 관리 |
| 6.8.3.2 | 셰이더 컴파일 | `kpio-browser/src/webgl/shader.rs` (신규) | GLSL ES 100 → 소프트웨어 래스터라이저 |
| 6.8.3.3 | 버퍼/텍스처 | `kpio-browser/src/webgl/buffer.rs` (신규) | createBuffer, bufferData, createTexture, texImage2D |
| 6.8.3.4 | Draw 호출 | `kpio-browser/src/webgl/draw.rs` (신규) | drawArrays, drawElements → CPU 래스터라이징 |
| 6.8.3.5 | 프레임버퍼 | `kpio-browser/src/webgl/framebuffer.rs` (신규) | FBO, 오프스크린 렌더링, readPixels |

### 6.8.4 PWA 지원

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.8.4.1 | Web App Manifest | `kpio-browser/src/pwa/manifest.rs` | manifest.json 파싱 (name, icons, start_url, display, theme_color) |
| 6.8.4.2 | 오프라인 지원 | `kpio-browser/src/pwa/offline.rs` (신규) | Service Worker + Cache API 기반 오프라인 |
| 6.8.4.3 | 설치 프롬프트 | `kpio-browser/src/pwa/install.rs` (신규) | beforeinstallprompt, 데스크톱 설치 |
| 6.8.4.4 | 알림 API | `kpio-js/src/web_api/notification.rs` (신규) | Notification.requestPermission, new Notification(title, options) |

### 6.8.5 확장 시스템 완성

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.8.5.1 | 콘텐츠 스크립트 실행 | `kpio-extensions/src/content.rs` | 매칭 URL → JS 인젝션 → 격리 월드 실행 |
| 6.8.5.2 | 백그라운드 스크립트 | `kpio-extensions/src/background.rs` | Service Worker 기반 백그라운드 런타임 |
| 6.8.5.3 | chrome.* API 서브셋 | `kpio-extensions/src/api/` | tabs, runtime, storage, webRequest, browserAction |
| 6.8.5.4 | 확장 UI (popup/options) | `kpio-extensions/src/ui.rs` | browserAction 팝업, 옵션 페이지 렌더링 |

## 퀄리티 게이트 (6.8)

> **아래 항목이 100% 충족되기 전까지 6.9로 진행 금지**

| # | 검증 항목 | 검증 방법 |
|---|----------|----------|
| QG-6.8.1 | Web Worker 동작 | Worker에서 연산 수행 → postMessage 결과 수신 |
| QG-6.8.2 | Service Worker 설치 | 등록 → install → activate 라이프사이클 정상 |
| QG-6.8.3 | SW 오프라인 캐시 | 네트워크 끊김 → 캐시된 응답 제공 |
| QG-6.8.4 | WebGL 삼각형 렌더링 | 셰이더 → 삼각형 → 화면 출력 |
| QG-6.8.5 | PWA 설치 | manifest.json 감지 → 설치 가능 표시 |
| QG-6.8.6 | 확장 콘텐츠 스크립트 | 매칭 URL 방문 시 콘텐츠 스크립트 실행 확인 |
| QG-6.8.7 | 고급 플랫폼 테스트 40개+ | Worker/SW/WebGL/PWA/Extension 각 최소 5개 |

---

# Sub-Phase 6.9: Framework Compatibility

## 목적
실제 웹 프레임워크(React, Vue, Angular, jQuery)와 주요 라이브러리가 정상 동작하도록 호환성을 보장하고, 실제 웹사이트 접속·렌더링을 검증한다.

## 선행 조건
- [x] QG-6.8 100% 충족
- [x] 모든 Web API + 고급 플랫폼 기능 동작

## 작업 목록

### 6.9.1 JS 엔진 호환성 보강

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.9.1.1 | ECMAScript Conformance 테스트 | 테스트 코드 | Test262 핵심 테스트 서브셋 (~500개) 실행·통과 |
| 6.9.1.2 | Polyfill 로딩 지원 | `kpio-js/src/interpreter.rs` | core-js 호환 (누락 빌트인 폴리필) |
| 6.9.1.3 | 엣지 케이스 수정 | 다수 파일 | Test262 실패 케이스 기반 버그 수정 |
| 6.9.1.4 | strict mode | `kpio-js/src/interpreter.rs` | 'use strict' 준수 (선언 필수, this undefined, arguments 제한 등) |
| 6.9.1.5 | Well-known Symbol 프로토콜 | `kpio-js/src/interpreter.rs` | Symbol.toPrimitive, Symbol.toStringTag, Symbol.hasInstance, Symbol.species |

### 6.9.2 React 호환성

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.9.2.1 | React 기본 렌더링 | 테스트 코드 | `ReactDOM.render(<h1>Hello</h1>, root)` 정상 |
| 6.9.2.2 | JSX (Babel 트랜스파일링) | 관련 없음 — 사전 빌드 | React 빌드 출력 (createElement 호출) 실행 |
| 6.9.2.3 | React Hooks 동작 | 테스트 코드 | useState, useEffect, useRef, useMemo 정상 |
| 6.9.2.4 | Virtual DOM diffing | 테스트 코드 | 상태 변경 → DOM 업데이트 → 렌더링 확인 |

### 6.9.3 Vue 호환성

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.9.3.1 | Vue 3 반응성 시스템 | 테스트 코드 | Proxy 기반 반응성 (Proxy/Reflect 필수) |
| 6.9.3.2 | 컴포넌트 렌더링 | 테스트 코드 | 기본 Vue 컴포넌트 마운트·렌더링 |

### 6.9.4 범용 라이브러리 호환성

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.9.4.1 | jQuery 3.x | 테스트 코드 | DOM 조작, 이벤트, Ajax(fetch 기반), 애니메이션 |
| 6.9.4.2 | Lodash | 테스트 코드 | 유틸리티 함수 실행 |
| 6.9.4.3 | Axios | 테스트 코드 | HTTP 요청 (XMLHttpRequest/fetch 기반) |

### 6.9.5 실제 웹사이트 접속 테스트

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.9.5.1 | example.com | 테스트 | 기초 HTML 렌더링 |
| 6.9.5.2 | Wikipedia 문서 | 테스트 | 복잡 레이아웃 + 이미지 + 링크 |
| 6.9.5.3 | GitHub 랜딩 페이지 | 테스트 | Grid/Flex + JS + 폰트 |
| 6.9.5.4 | Hacker News | 테스트 | 간단한 서버 렌더 HTML |
| 6.9.5.5 | 정적 블로그 (Hugo/Jekyll) | 테스트 | CSS + 반응형 레이아웃 |
| 6.9.5.6 | JSON REST API 테스트 | 테스트 | fetch → JSON 파싱 → DOM 업데이트 |

## 퀄리티 게이트 (6.9)

> **아래 항목이 100% 충족되기 전까지 6.10으로 진행 금지**

| # | 검증 항목 | 검증 방법 |
|---|----------|----------|
| QG-6.9.1 | Test262 핵심 500개 중 90%+ 통과 | 자동화 테스트 러너 |
| QG-6.9.2 | React Hello World 렌더링 | `React.createElement('h1', null, 'Hello')` → DOM 노드 생성·표시 |
| QG-6.9.3 | Vue 3 Proxy 반응성 | 상태 변경 → 자동 재렌더링 확인 |
| QG-6.9.4 | jQuery DOM 조작 | `$('#id').css('color', 'red').text('hello')` 정상 |
| QG-6.9.5 | example.com 완벽 렌더링 | 참조 브라우저와 유사한 출력 (텍스트·스타일·링크) |
| QG-6.9.6 | Wikipedia 렌더링 | 주요 콘텐츠 영역 텍스트·이미지·링크 정상 표시 |
| QG-6.9.7 | Hacker News 로딩 | 글 목록 표시, 링크 클릭 → 페이지 전환 |
| QG-6.9.8 | GitHub 랜딩 기본 구조 | 헤더·네비·주요 섹션 레이아웃 정상 |
| QG-6.9.9 | fetch + JSON | REST API → JSON 파싱 → 동적 DOM 업데이트 정상 |
| QG-6.9.10 | **인터넷 실제 접속 확인** | QEMU에서 최소 3개 HTTPS 사이트 정상 접속·렌더링 스크린샷 |

---

# Sub-Phase 6.10: Integration Verification & Polish

## 목적
모든 컴포넌트의 통합 무결성 검증, 성능 최적화, 보안 강화, 문서화를 완료하여 릴리스 품질을 보장한다.

## 선행 조건
- [x] QG-6.9 100% 충족
- [x] 모든 서브페이즈 퀄리티 게이트 통과

## 작업 목록

### 6.10.1 통합 테스트 스위트

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.10.1.1 | End-to-End 테스트 프레임워크 | `tests/e2e/web_tests.rs` (신규) | QEMU 자동 부팅, 직렬 포트 출력 파싱, 자동 검증 |
| 6.10.1.2 | 네트워크 E2E 테스트 | `tests/e2e/network_e2e.rs` (신규) | 부팅 → DHCP → DNS → HTTP → HTTPS → WebSocket 자동 검증 |
| 6.10.1.3 | 브라우저 E2E 테스트 | `tests/e2e/browser_e2e.rs` (신규) | 페이지 로드 → JS 실행 → DOM 검증 → 렌더링 확인 |
| 6.10.1.4 | Web Platform Tests 서브셋 | `tests/wpt/` | WPT 핵심 테스트 서브셋 (~200개) 실행 |
| 6.10.1.5 | 회귀 테스트 스위트 | `tests/regression/` (신규) | 발견된 버그에 대한 재발 방지 테스트 |

### 6.10.2 성능 최적화

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.10.2.1 | JS 엔진 성능 프로파일링 | `kpio-js/src/` 다수 | 핫스팟 식별, 인라인 캐시, 히든 클래스 |
| 6.10.2.2 | CSS 매칭 최적화 | `kpio-css/src/selector.rs` | 블룸 필터, 해시 인덱스, 셀렉터 캐시 |
| 6.10.2.3 | 레이아웃 캐시 | `kpio-layout/src/` | 레이아웃 결과 캐시, 증분 레이아웃 |
| 6.10.2.4 | 렌더링 최적화 | `graphics/src/` | 타일 캐시 활용, GPU 모듈 최적화, 배치 렌더링 |
| 6.10.2.5 | 메모리 사용량 최적화 | 다수 | 불필요한 복제 제거, 아레나 할당, 문자열 인터닝 확장 |
| 6.10.2.6 | 네트워크 성능 | `network/src/` | 연결 풀, 파이프라이닝, 압축 효율 |

### 6.10.3 보안 감사

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.10.3.1 | JS 샌드박스 검증 | `kpio-js/src/` | JS에서 커널 메모리 접근 불가 확인 |
| 6.10.3.2 | 네트워크 보안 | `network/src/` | TLS 다운그레이드 공격 방어, HSTS |
| 6.10.3.3 | CSP 검증 | `kpio-browser/src/` | XSS 시나리오 차단 확인 |
| 6.10.3.4 | 인증서 검증 엄격성 | `network/src/tls/` | 만료/자체서명/체인불완전 거부 확인 |
| 6.10.3.5 | 퍼징 테스트 | `fuzz/` | HTML/CSS/JS 파서 퍼징, 크래시 없음 확인 |
| 6.10.3.6 | Same-Origin Policy | `kpio-browser/src/` | 크로스 오리진 접근 차단 확인 |

### 6.10.4 문서화

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.10.4.1 | 아키텍처 문서 갱신 | `docs/architecture/` | 네트워크, 브라우저, JS 엔진 아키텍처 문서 |
| 6.10.4.2 | API 문서 | `docs/api/` (신규) | Web API 지원 목록, 호환성 표 |
| 6.10.4.3 | 빌드·실행 가이드 갱신 | `docs/guides/` | Phase 6 이후 빌드·실행 절차 |
| 6.10.4.4 | 릴리스 노트 | `RELEASE_NOTES.md` | Phase 6 변경 사항 요약 |

### 6.10.5 최종 인터넷 통신 검증

| ID | 작업 | 파일 | 설명 |
|----|------|------|------|
| 6.10.5.1 | QEMU 네트워크 통합 테스트 | 스크립트 | QEMU user-net에서 실제 외부 사이트 접속 자동화 |
| 6.10.5.2 | HTTPS 사이트 5개+ 접속 | 테스트 | google.com, github.com, wikipedia.org, news.ycombinator.com, httpbin.org |
| 6.10.5.3 | WebSocket 실시간 통신 | 테스트 | WebSocket 서버와 양방향 메시지 교환 |
| 6.10.5.4 | 대용량 파일 다운로드 | 테스트 | 1MB+ 파일 다운로드 → VFS 저장 → 무결성 확인 |
| 6.10.5.5 | 동시 다중 연결 | 테스트 | 3개+ 동시 HTTP 요청 처리 |

## 퀄리티 게이트 (6.10)

> **이 게이트가 100% 충족되면 Phase 6 완료**

| # | 검증 항목 | 검증 방법 |
|---|----------|----------|
| QG-6.10.1 | E2E 네트워크 테스트 전수 통과 | 자동화 테스트 실행 → 100% 통과 |
| QG-6.10.2 | WPT 서브셋 200개 중 70%+ 통과 | WPT 테스트 러너 |
| QG-6.10.3 | 퍼징 24시간 크래시 0 | HTML/CSS/JS 파서 퍼징 |
| QG-6.10.4 | 보안 체크리스트 100% | 샌드박스, SOP, CSP, TLS 검증 전수 통과 |
| QG-6.10.5 | 실제 HTTPS 사이트 5개 접속 성공 | QEMU에서 스크린샷/시리얼 로그 확인 |
| QG-6.10.6 | WebSocket 양방향 통신 | 실시간 메시지 교환 확인 |
| QG-6.10.7 | 페이지 로드 시간 | example.com 5초 이내 렌더링 완료 (QEMU 환경) |
| QG-6.10.8 | 메모리 누수 없음 | 10개 페이지 순차 로드 후 힙 크기 2배 이내 유지 |
| QG-6.10.9 | 문서화 완료 | 아키텍처, API, 가이드, 릴리스 노트 모두 갱신 |
| QG-6.10.10 | **최종 인터넷 통신 확인** | QEMU 터미널에서 `ping 8.8.8.8` + `curl https://httpbin.org/ip` 성공 로그 |

---

## 부록 A: 크레이트 의존성 그래프

```
kernel
  ├── network (TCP/UDP/DNS/TLS/HTTP)
  │     └── network::crypto (암호 프리미티브)
  ├── graphics (렌더링)
  │     └── kpio-layout (레이아웃 → DisplayList)
  ├── storage (VFS/블록 디바이스)
  └── runtime (WASM)

kpio-browser (브라우저 쉘)
  ├── kpio-html (HTML 파서)
  ├── kpio-css (CSS 파서)
  ├── kpio-dom (DOM 트리)
  ├── kpio-js (JavaScript 엔진)
  │     ├── kpio-js::web_api (Web Platform APIs)
  │     └── kpio-js::worker (Web Workers)
  ├── kpio-layout (레이아웃 엔진)
  ├── kpio-extensions (확장 시스템)
  ├── kpio-devtools (개발자 도구)
  ├── network (HTTP/HTTPS/WebSocket 클라이언트)
  └── graphics (프레임버퍼 렌더링)

servo-types (문자열 인터닝, 네임스페이스)
  └── used by: kpio-html, kpio-dom, kpio-css

userlib (유저스페이스 라이브러리)
  └── syscall 래퍼
```

## 부록 B: 추정 작업량

| 서브페이즈 | 추정 새 코드 | 추정 수정 코드 | 신규 파일 | 난이도 |
|-----------|------------|--------------|----------|--------|
| 6.1 Network Foundation | ~8,000줄 | ~2,000줄 | ~15 | ★★★★☆ |
| 6.2 Secure Communications | ~12,000줄 | ~1,500줄 | ~20 | ★★★★★ |
| 6.3 JS Engine Completion | ~15,000줄 | ~3,000줄 | ~25 | ★★★★★ |
| 6.4 CSS & Layout | ~10,000줄 | ~2,000줄 | ~20 | ★★★★☆ |
| 6.5 Web Platform APIs | ~8,000줄 | ~1,000줄 | ~25 | ★★★☆☆ |
| 6.6 Browser Core Features | ~12,000줄 | ~2,500줄 | ~30 | ★★★★☆ |
| 6.7 Media & Graphics | ~10,000줄 | ~1,500줄 | ~20 | ★★★★☆ |
| 6.8 Advanced Web Platform | ~8,000줄 | ~1,000줄 | ~15 | ★★★★☆ |
| 6.9 Framework Compatibility | ~3,000줄 | ~5,000줄 | ~10 | ★★★☆☆ |
| 6.10 Integration & Polish | ~5,000줄 | ~3,000줄 | ~15 | ★★★☆☆ |
| **합계** | **~91,000줄** | **~22,500줄** | **~195** | ★★★★☆ |

## 부록 C: 인터넷 통신 검증 절차

### QEMU 실행 환경

```bash
qemu-system-x86_64 \
  -drive format=raw,file=target/x86_64-kpio/release/boot.img \
  -netdev user,id=net0,hostfwd=tcp::8080-:80 \
  -device virtio-net-pci,netdev=net0 \
  -serial stdio \
  -m 512M
```

### 검증 단계

1. **부팅 확인**: 시리얼 포트에 `[NET] DHCP: Acquired IP x.x.x.x` 출력
2. **ICMP 검증**: 셸에서 `ping 10.0.2.2` (QEMU 게이트웨이) → 응답 수신
3. **DNS 검증**: `nslookup example.com` → IP 주소 반환
4. **HTTP 검증**: `curl http://example.com` → HTML 본문 출력
5. **HTTPS 검증**: `curl https://example.com` → TLS 핸드셰이크 성공 + HTML 출력
6. **브라우저 검증**: 주소창에 `https://example.com` 입력 → 페이지 렌더링

### 성공 기준

- 위 6단계 모두 오류 없이 완료
- 시리얼 로그에 패닉/오류 메시지 없음
- 렌더링 결과가 유의미한 내용 포함 (빈 화면 아님)

---

## 부록 D: 코드 품질 기준

### 필수 준수 사항

1. **no_std 호환**: 커널·네트워크·브라우저 핵심 모듈은 `#![no_std]` + `extern crate alloc`
2. **에러 처리**: `Result<T, E>` 반환, `unwrap()` 금지 (테스트 코드 제외)
3. **문서화**: 공개 API에 `///` 문서 주석, 모듈에 `//!` 모듈 문서
4. **테스트**: 각 모듈 최소 10개 단위 테스트, `#[cfg(test)]` 모듈
5. **의존성 제한**: 새 외부 크레이트 추가 시 no_std 호환 + 라이선스 확인 필수
6. **unsafe 최소화**: `unsafe` 블록 사용 시 `// SAFETY:` 주석 필수
7. **네이밍**: Rust 네이밍 컨벤션 준수 (snake_case 함수/변수, CamelCase 타입)
8. **모듈 크기**: 단일 파일 2,000줄 초과 시 분할 필수
9. **순환 의존 금지**: 크레이트 간 순환 의존 절대 금지
10. **clippy 경고 0**: `cargo clippy -- -W clippy::all` 경고 없음
