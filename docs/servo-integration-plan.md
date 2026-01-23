# Servo Integration Plan for KPIO

## Overview

Servo는 std 환경에서 동작하는 대규모 브라우저 엔진입니다.
KPIO는 no_std 커널이므로, Servo를 직접 커널에서 실행할 수 없습니다.

## 통합 전략

### Phase 3.1: Platform Abstraction Layer ✅
- `servo-platform` 크레이트 생성 완료
- net, gpu, fs, thread, time, window, ipc 모듈 구현

### Phase 3.2: KPIO Userspace Runtime (현재 단계)
Servo는 KPIO **userspace**에서 실행됩니다:

```
┌─────────────────────────────────────────────────────┐
│                 KPIO Userspace                       │
│  ┌─────────────────────────────────────────────┐    │
│  │              Servo Process                   │    │
│  │  ┌─────────────────────────────────────┐    │    │
│  │  │         Servo Components            │    │    │
│  │  │  (script, layout, compositor, etc)  │    │    │
│  │  └─────────────┬───────────────────────┘    │    │
│  │                │                            │    │
│  │  ┌─────────────▼───────────────────────┐    │    │
│  │  │     kpio-servo-platform (shim)      │    │    │
│  │  │  (replaces std::net, std::fs, etc)  │    │    │
│  │  └─────────────┬───────────────────────┘    │    │
│  └────────────────┼────────────────────────────┘    │
│                   │ syscalls                        │
├───────────────────┼─────────────────────────────────┤
│                   ▼                                 │
│              KPIO Kernel                            │
│  (networking, GPU, FS services via IPC)             │
└─────────────────────────────────────────────────────┘
```

### Phase 3.3: Minimal Servo Build

목표: 최소 Servo 컴포넌트만 빌드하여 HTML 렌더링 테스트

**필수 컴포넌트:**
1. `components/shared/base` - 기본 타입
2. `components/url` - URL 파싱
3. `components/geometry` - 기하학 연산
4. `components/pixels` - 픽셀 조작
5. `components/fonts` - 폰트 처리

**선택적 컴포넌트 (후순위):**
- `components/script` - JavaScript (SpiderMonkey 필요)
- `components/net` - 네트워킹
- `components/layout` - CSS 레이아웃

### Phase 3.4: Custom Servo Port

`ports/kpio/` 디렉토리 생성하여 KPIO 전용 포트 개발

## 현실적 MVP 목표

### 단계 1: 정적 HTML 렌더링
- JavaScript 없이 정적 HTML 파싱
- CSS 레이아웃 계산
- 화면에 렌더링

### 단계 2: 기본 상호작용
- 마우스 클릭
- 스크롤
- 링크 탐색

### 단계 3: JavaScript 지원
- SpiderMonkey 통합 (가장 복잡)

## 대안 전략

Servo 전체 통합이 너무 복잡하면:

### Option A: HTML5ever + WebRender만 사용
- `html5ever`: HTML 파서 (no_std 가능)
- `webrender`: GPU 렌더러 (std 필요하지만 단순화 가능)

### Option B: 경량 브라우저 엔진 직접 구현
- 커스텀 HTML 파서
- 커스텀 CSS 엔진
- 소프트웨어 렌더러

### Option C: 웹 렌더링 서버 방식
- 별도 프로세스에서 Servo 실행
- 렌더링된 이미지만 KPIO로 전송

## 다음 단계

1. Servo의 `html5ever` 크레이트 단독 빌드 테스트
2. KPIO userspace에서 std 지원 여부 확인
3. 최소 렌더링 파이프라인 프로토타입
