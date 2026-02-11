# Sub-Phase 6.3 Checklist: JavaScript Engine Completion

## Overview
ES2020+ JavaScript 기능을 완전히 구현하여 React/Vue/Angular 등 현대 프레임워크 코드를 실행 가능하게 한다.

## Pre-requisites
- [ ] QG-6.2 100% 충족
- [x] 기본 JS 인터프리터 동작
- [x] GC 기본 프레임워크 존재

---

## 6.3.1 핵심 런타임 기반

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.3.1.1 | 이벤트 루프 (매크로/마이크로태스크) | ⬜ | |
| 6.3.1.2 | setTimeout / setInterval | ⬜ | |
| 6.3.1.3 | requestAnimationFrame | ⬜ | |
| 6.3.1.4 | Promise 완전 구현 | ⬜ | resolve/reject/then/catch/finally/all/race/any/allSettled |
| 6.3.1.5 | async/await 실행 | ⬜ | |
| 6.3.1.6 | Generator / Iterator | ⬜ | |
| 6.3.1.7 | async Generator | ⬜ | |
| 6.3.1.8 | 모듈 시스템 (import/export) | ⬜ | |

## 6.3.2 내장 객체 완성

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.3.2.1 | String.prototype 전체 (25+ 메서드) | ⬜ | |
| 6.3.2.2 | Array.prototype 전체 (30+ 메서드) | ⬜ | |
| 6.3.2.3 | Object 정적 메서드 (15+ 메서드) | ⬜ | |
| 6.3.2.4 | JSON 완전 구현 (parse/stringify) | ⬜ | |
| 6.3.2.5 | Math 객체 전체 | ⬜ | |
| 6.3.2.6 | Date 객체 | ⬜ | |
| 6.3.2.7 | RegExp 엔진 (NFA) | ⬜ | |
| 6.3.2.8 | Map / Set | ⬜ | |
| 6.3.2.9 | WeakMap / WeakSet | ⬜ | |
| 6.3.2.10 | Symbol (well-known symbols) | ⬜ | |
| 6.3.2.11 | ArrayBuffer / TypedArray / DataView | ⬜ | |
| 6.3.2.12 | Error 계층 (스택 트레이스) | ⬜ | |
| 6.3.2.13 | Proxy / Reflect | ⬜ | |
| 6.3.2.14 | Number/Boolean 메서드 | ⬜ | |

## 6.3.3 언어 기능 보완

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.3.3.1 | 프로토타입 체인 완성 | ⬜ | |
| 6.3.3.2 | this 바인딩 (call/apply/bind) | ⬜ | |
| 6.3.3.3 | Getter/Setter 동작 | ⬜ | |
| 6.3.3.4 | 구조 분해 기본값 | ⬜ | |
| 6.3.3.5 | Optional chaining (?.) | ⬜ | |
| 6.3.3.6 | Nullish coalescing (??) | ⬜ | |
| 6.3.3.7 | Computed property names | ⬜ | |
| 6.3.3.8 | for...in / for...of | ⬜ | |
| 6.3.3.9 | 태그드 템플릿 리터럴 | ⬜ | |
| 6.3.3.10 | 클래스 상속 완성 (프라이빗 필드 #) | ⬜ | |
| 6.3.3.11 | Logical assignment (??=, &&=, ||=) | ⬜ | |
| 6.3.3.12 | 동적 import() | ⬜ | |

## 6.3.4 GC 강화

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.3.4.1 | 세대별 GC (Young/Old) | ⬜ | |
| 6.3.4.2 | 증분 마킹 | ⬜ | |
| 6.3.4.3 | WeakRef / FinalizationRegistry | ⬜ | |

---

## Quality Gate

| # | Criterion | Method | Status |
|---|-----------|--------|--------|
| QG-6.3.1 | Promise 체이닝 | Promise.resolve(1).then(x=>x+1).then(console.log) → 2 | ⬜ |
| QG-6.3.2 | async/await | async function + await Promise → 결과 | ⬜ |
| QG-6.3.3 | 이벤트 루프 순서 보장 | 마이크로→매크로 순서 | ⬜ |
| QG-6.3.4 | Array map/filter/reduce | [1,2,3].map(x=>x*2).filter(x=>x>2).reduce((a,b)=>a+b,0) → 10 | ⬜ |
| QG-6.3.5 | JSON.parse 복잡 객체 | 중첩 객체/배열 정상 파싱 | ⬜ |
| QG-6.3.6 | JSON.stringify 객체/배열 | 정상 직렬화 | ⬜ |
| QG-6.3.7 | RegExp 기본 동작 | 캡처 그룹 포함 | ⬜ |
| QG-6.3.8 | 프로토타입 상속 | class extends + super() | ⬜ |
| QG-6.3.9 | Map/Set 동작 | 기본 CRUD | ⬜ |
| QG-6.3.10 | Generator 동작 | yield + spread [...g()] | ⬜ |
| QG-6.3.11 | 모듈 import/export | 정적 import, 순환 참조 | ⬜ |
| QG-6.3.12 | GC 메모리 누수 없음 | 10000회 생성/파기 후 힙 안정 | ⬜ |
| QG-6.3.13 | Date.now() 동작 | 커널 타이머 연동 | ⬜ |
| QG-6.3.14 | 내장 객체 테스트 100개+ | 카테고리별 ≥10개 | ⬜ |
