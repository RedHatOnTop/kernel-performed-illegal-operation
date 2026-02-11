# Sub-Phase 6.5 Checklist: Web Platform APIs

## Overview
fetch, localStorage, MutationObserver, History API 등 핵심 Web API를 구현하여 JavaScript에서 브라우저 기능에 접근 가능하게 한다.

## Pre-requisites
- [ ] QG-6.4 100% 충족
- [ ] JS 엔진 Promise/async/await 동작
- [ ] HTTPS 통신 동작

---

## 6.5.1 네트워크 API

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.5.1.1 | fetch() API (Request/Response/Headers) | ⬜ | |
| 6.5.1.2 | XMLHttpRequest | ⬜ | |
| 6.5.1.3 | WebSocket JS 바인딩 | ⬜ | |
| 6.5.1.4 | AbortController / AbortSignal | ⬜ | |
| 6.5.1.5 | EventSource (SSE) | ⬜ | |
| 6.5.1.6 | navigator.onLine | ⬜ | |

## 6.5.2 스토리지 API

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.5.2.1 | localStorage | ⬜ | |
| 6.5.2.2 | sessionStorage | ⬜ | |
| 6.5.2.3 | IndexedDB | ⬜ | |
| 6.5.2.4 | Cache API | ⬜ | |
| 6.5.2.5 | Cookie API (document.cookie) | ⬜ | |

## 6.5.3 DOM 확장 API

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.5.3.1 | MutationObserver | ⬜ | |
| 6.5.3.2 | IntersectionObserver | ⬜ | |
| 6.5.3.3 | ResizeObserver | ⬜ | |
| 6.5.3.4 | DOMParser / innerHTML | ⬜ | |
| 6.5.3.5 | Range / Selection API | ⬜ | |
| 6.5.3.6 | Element.classList | ⬜ | |
| 6.5.3.7 | Element.dataset | ⬜ | |
| 6.5.3.8 | Element.style (CSSOM) | ⬜ | |
| 6.5.3.9 | Element.getBoundingClientRect | ⬜ | |
| 6.5.3.10 | Element.scrollIntoView | ⬜ | |

## 6.5.4 Window/Navigator API

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.5.4.1 | window.location | ⬜ | |
| 6.5.4.2 | History API (pushState/popState) | ⬜ | |
| 6.5.4.3 | navigator.userAgent | ⬜ | |
| 6.5.4.4 | navigator.clipboard | ⬜ | |
| 6.5.4.5 | window.matchMedia | ⬜ | |
| 6.5.4.6 | Performance API | ⬜ | |
| 6.5.4.7 | console 확장 | ⬜ | |

## 6.5.5 기타 웹 API

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.5.5.1 | URL / URLSearchParams | ⬜ | |
| 6.5.5.2 | FormData | ⬜ | |
| 6.5.5.3 | Blob / File / FileReader | ⬜ | |
| 6.5.5.4 | TextEncoder / TextDecoder | ⬜ | |
| 6.5.5.5 | Crypto (Web Crypto API) | ⬜ | |
| 6.5.5.6 | structuredClone | ⬜ | |
| 6.5.5.7 | queueMicrotask | ⬜ | |

---

## Quality Gate

| # | Criterion | Method | Status |
|---|-----------|--------|--------|
| QG-6.5.1 | fetch JSON | fetch → r.json() → console.log | ⬜ |
| QG-6.5.2 | localStorage 영속성 | 새로고침 후 값 유지 | ⬜ |
| QG-6.5.3 | MutationObserver 동작 | DOM 변경 시 콜백 호출 | ⬜ |
| QG-6.5.4 | History pushState | popstate 이벤트 발화 | ⬜ |
| QG-6.5.5 | FormData + fetch POST | 폼 데이터 전송 확인 | ⬜ |
| QG-6.5.6 | URL 파싱 | 모든 컴포넌트 정상 분석 | ⬜ |
| QG-6.5.7 | TextEncoder/Decoder 왕복 | UTF-8 정확도 100% | ⬜ |
| QG-6.5.8 | Performance.now() 정밀도 | 마이크로초, 단조 증가 | ⬜ |
| QG-6.5.9 | WebSocket JS 메시지 교환 | 양방향 확인 | ⬜ |
| QG-6.5.10 | Web API 테스트 60개+ | 카테고리별 ≥5개 | ⬜ |
