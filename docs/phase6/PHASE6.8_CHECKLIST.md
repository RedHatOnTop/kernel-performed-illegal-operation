# Sub-Phase 6.8 Checklist: Advanced Web Platform

## Overview
Service Worker, Web Worker, WebGL, PWA, 확장 시스템 등 현대 웹 플랫폼의 고급 기능을 구현한다.

## Pre-requisites
- [ ] QG-6.7 100% 충족
- [ ] 모든 핵심 Web API 동작
- [ ] Canvas 2D 동작

---

## 6.8.1 Web Workers

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.8.1.1 | Worker 스레드 모델 | ⬜ | |
| 6.8.1.2 | Structured Clone 전송 | ⬜ | |
| 6.8.1.3 | SharedWorker | ⬜ | |
| 6.8.1.4 | Transferable Objects | ⬜ | |

## 6.8.2 Service Workers

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.8.2.1 | SW 등록/설치/활성화 | ⬜ | |
| 6.8.2.2 | Fetch 이벤트 가로채기 | ⬜ | |
| 6.8.2.3 | SW 캐시 전략 | ⬜ | |
| 6.8.2.4 | Push API 기초 | ⬜ | |

## 6.8.3 WebGL 기초

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.8.3.1 | WebGLRenderingContext | ⬜ | |
| 6.8.3.2 | 셰이더 컴파일 (GLSL ES 100) | ⬜ | |
| 6.8.3.3 | 버퍼/텍스처 | ⬜ | |
| 6.8.3.4 | Draw 호출 (CPU 래스터라이징) | ⬜ | |
| 6.8.3.5 | 프레임버퍼 | ⬜ | |

## 6.8.4 PWA 지원

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.8.4.1 | Web App Manifest 파싱 | ⬜ | |
| 6.8.4.2 | 오프라인 지원 (SW + Cache) | ⬜ | |
| 6.8.4.3 | 설치 프롬프트 | ⬜ | |
| 6.8.4.4 | 알림 API | ⬜ | |

## 6.8.5 확장 시스템 완성

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.8.5.1 | 콘텐츠 스크립트 실행 | ⬜ | |
| 6.8.5.2 | 백그라운드 스크립트 | ⬜ | |
| 6.8.5.3 | chrome.* API 서브셋 | ⬜ | |
| 6.8.5.4 | 확장 UI (popup/options) | ⬜ | |

---

## Quality Gate

| # | Criterion | Method | Status |
|---|-----------|--------|--------|
| QG-6.8.1 | Web Worker 동작 | Worker 연산 → postMessage 결과 수신 | ⬜ |
| QG-6.8.2 | SW 설치 | register → install → activate | ⬜ |
| QG-6.8.3 | SW 오프라인 캐시 | 네트워크 끊김 → 캐시 응답 | ⬜ |
| QG-6.8.4 | WebGL 삼각형 렌더링 | 셰이더 → 삼각형 → 출력 | ⬜ |
| QG-6.8.5 | PWA 설치 | manifest 감지 → 설치 가능 | ⬜ |
| QG-6.8.6 | 확장 콘텐츠 스크립트 | URL 매칭 → JS 인젝션 확인 | ⬜ |
| QG-6.8.7 | 고급 플랫폼 테스트 40개+ | 각 모듈 ≥5개 | ⬜ |
