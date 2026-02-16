# Phase 7-2.S4 게이트 리포트 — Interpreter 실행 정확성-I

작성일: 2026-02-16
상태: PASS (100%)

---

## 목표

인터프리터 코어(스택/프레임/제어흐름/정수 연산) 품질 기준을 고정하고, 트랩 검증 절차를 명문화한다.

---

## S4-QG1: 제어흐름 테스트 매트릭스 완성

### 대상 제어 명령
- `block`, `loop`, `if`, `else`, `br`, `br_if`, `br_table`, `return`, `call`, `call_indirect`

### 매트릭스

| 영역 | 정상 경로 | 실패 경로 |
|---|---|---|
| block/loop | 진입/탈출 스택 높이 보존 | 잘못된 label depth 트랩 |
| if/else | 조건 0/1 분기 정상 | 타입 불일치 조건 트랩 |
| br/br_if | depth 기반 점프 정상 | 존재하지 않는 블록 참조 트랩 |
| br_table | index별 분기 + default | 대상 depth 범위 오류 트랩 |
| call/return | frame push/pop 정상 | call stack overflow |
| call_indirect | type match 성공 | type mismatch / uninitialized element |

판정: PASS

---

## S4-QG2: 정수 연산 100+ 케이스 계획 확정

### 커버 범위
- `i32`: `add/sub/mul/div_s/div_u/rem_s/rem_u`, bitwise, shift/rotate, compare
- `i64`: 동일 범주

### 최소 케이스 수량
- i32 산술/비트/비교: 60
- i64 산술/비트/비교: 60
- 총합: 120

판정: PASS

---

## S4-QG3: DivisionByZero/StackUnderflow 트랩 검증 절차 확정

### 트랩 검증 절차

1. `ValueStack::pop()` 빈 스택 호출 → `TrapError::StackUnderflow` 확인
2. `i32.div_s`/`i32.div_u` 분모 0 입력 → `TrapError::DivisionByZero` 확인
3. `i64.div_s`/`i64.div_u` 분모 0 입력 → `TrapError::DivisionByZero` 확인
4. 회귀 검증 시 trap variant가 정확히 매칭되는지 assert

### 고정 커맨드

```bash
cargo test -p runtime interpreter
cargo test -p runtime executor
```

판정: PASS

---

## 종합 판정

- QG 통과: 3/3
- 통과율: 100%
- 결론: `7-2.S4 UNLOCK`, `7-2.S5` 활성화 가능