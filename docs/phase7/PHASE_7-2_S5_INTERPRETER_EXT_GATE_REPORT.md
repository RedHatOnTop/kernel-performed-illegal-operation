# Phase 7-2.S5 게이트 리포트 — Interpreter 실행 정확성-II

작성일: 2026-02-16
상태: PASS (100%)

---

## 목표

메모리 경계, 부동소수 연산, 간접 호출 타입 검증, 핵심 트랩 시나리오를 고정한다.

---

## S5-QG1: Memory OOB 트랩 테스트 절차 고정

절차:
1. 메모리 없는 컨텍스트에서 load/store 시도 → `MemoryOutOfBounds`
2. 경계 직전/직후 오프셋으로 `load/store` 수행
3. `memory.grow` 전후 동일 오프셋 접근 재검증

판정: PASS

---

## S5-QG2: IEEE 754 핵심 케이스 목록 완성

핵심 케이스:
- `f32/f64` 산술: `add/sub/mul/div`
- 함수형 연산: `abs/neg/ceil/floor/trunc/nearest/sqrt`
- 비교: `eq/ne/lt/gt/le/ge`
- 특수값: `NaN`, `+Inf`, `-Inf`, `-0.0`
- 변환 트랩: `InvalidConversionToInteger`, `IntegerOverflow`

판정: PASS

---

## S5-QG3: 간접 호출 성공/실패 케이스 모두 존재

성공 케이스:
- `call_indirect`에서 table element + type index 일치

실패 케이스:
- table index 미정의 → `UndefinedElement`
- element 미초기화 → `UninitializedElement`
- type mismatch → `IndirectCallTypeMismatch`

판정: PASS

---

## 종합 판정

- QG 통과: 3/3
- 통과율: 100%
- 결론: `7-2.S5 UNLOCK`, `7-2.S6` 활성화 가능