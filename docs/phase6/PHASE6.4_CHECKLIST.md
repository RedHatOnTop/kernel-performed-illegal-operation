# Sub-Phase 6.4 Checklist: CSS & Layout Engine Completion

## Overview
CSS 파싱, 캐스케이딩, 레이아웃 알고리즘을 완성하여 현대 웹사이트의 레이아웃을 정확하게 렌더링한다.

## Pre-requisites
- [ ] QG-6.3 100% 충족
- [x] CSS 기본 파싱 동작
- [x] Block/Inline/Flex 레이아웃 기초 존재

---

## 6.4.1 CSS 파서 확장

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.4.1.1 | @media 쿼리 파싱·평가 | ⬜ | |
| 6.4.1.2 | @keyframes 파싱 | ⬜ | |
| 6.4.1.3 | @font-face 파싱 | ⬜ | |
| 6.4.1.4 | @import 처리 | ⬜ | |
| 6.4.1.5 | @supports 평가 | ⬜ | |
| 6.4.1.6 | calc() / min() / max() / clamp() | ⬜ | |
| 6.4.1.7 | var() / CSS Custom Properties | ⬜ | |
| 6.4.1.8 | Shorthand 속성 확장 | ⬜ | |
| 6.4.1.9 | !important 우선순위 | ⬜ | |
| 6.4.1.10 | transform 함수 파싱 | ⬜ | |
| 6.4.1.11 | gradient 파싱 | ⬜ | |
| 6.4.1.12 | box-shadow / text-shadow 파싱 | ⬜ | |
| 6.4.1.13 | filter / backdrop-filter 파싱 | ⬜ | |

## 6.4.2 CSS 캐스케이드·상속 완성

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.4.2.1 | 완전한 상속 모델 | ⬜ | |
| 6.4.2.2 | 사용자 에이전트 스타일시트 | ⬜ | |
| 6.4.2.3 | 셀렉터 매칭 최적화 | ⬜ | |
| 6.4.2.4 | 고급 셀렉터 (:not,:is,:has 등) | ⬜ | |
| 6.4.2.5 | 스타일 무효화 (Dirty 체크) | ⬜ | |

## 6.4.3 레이아웃 엔진 확장

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.4.3.1 | position: absolute/relative | ⬜ | |
| 6.4.3.2 | position: fixed | ⬜ | |
| 6.4.3.3 | position: sticky | ⬜ | |
| 6.4.3.4 | z-index / stacking context | ⬜ | |
| 6.4.3.5 | float / clear | ⬜ | |
| 6.4.3.6 | Grid 레이아웃 | ⬜ | |
| 6.4.3.7 | Table 레이아웃 | ⬜ | |
| 6.4.3.8 | overflow: scroll/hidden/auto | ⬜ | |
| 6.4.3.9 | min/max-width/height | ⬜ | |
| 6.4.3.10 | 텍스트 줄바꿈 (Unicode UAX #14) | ⬜ | |
| 6.4.3.11 | 실제 폰트 메트릭 | ⬜ | |
| 6.4.3.12 | Replaced elements (img) | ⬜ | |
| 6.4.3.13 | Flexbox 완성 | ⬜ | |
| 6.4.3.14 | 마진 상쇄 완성 | ⬜ | |

## 6.4.4 CSS 애니메이션 & 트랜지션

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.4.4.1 | CSS Transitions | ⬜ | |
| 6.4.4.2 | CSS Animations | ⬜ | |
| 6.4.4.3 | 타이밍 함수 (cubic-bezier 등) | ⬜ | |
| 6.4.4.4 | 프로퍼티 보간 (숫자/색상/변환) | ⬜ | |
| 6.4.4.5 | 애니메이션 ↔ JS 이벤트 | ⬜ | |

---

## Quality Gate

| # | Criterion | Method | Status |
|---|-----------|--------|--------|
| QG-6.4.1 | position: absolute 동작 | 정확한 containing block 좌표 | ⬜ |
| QG-6.4.2 | Flexbox 테스트 10개+ 통과 | CSS 사양 테스트 | ⬜ |
| QG-6.4.3 | Grid 기본 동작 | 1fr 2fr 1fr 열 분배 | ⬜ |
| QG-6.4.4 | float 레이아웃 | 텍스트 감싸기 정확 | ⬜ |
| QG-6.4.5 | @media 쿼리 반응 | 뷰포트 변경 시 스타일 전환 | ⬜ |
| QG-6.4.6 | calc() 혼합 단위 | calc(100% - 20px) 정확 | ⬜ |
| QG-6.4.7 | CSS Custom Properties | var(--color) 상속 정상 | ⬜ |
| QG-6.4.8 | CSS Transitions | opacity 전환 부드러움 | ⬜ |
| QG-6.4.9 | UA 스타일시트 | 스타일 없는 HTML 합리적 렌더링 | ⬜ |
| QG-6.4.10 | 레이아웃 테스트 80개+ | 각 모듈 ≥10개 | ⬜ |
| QG-6.4.11 | 마진 상쇄 정확도 | CSS2.1 명세 테스트 | ⬜ |
