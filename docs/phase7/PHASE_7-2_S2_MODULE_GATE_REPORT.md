# Phase 7-2.S2 게이트 리포트 — Module 검증 강화

작성일: 2026-02-16
상태: PASS (100%)

---

## 목표

`Module::validate_structure()` 경로의 구조 검증 규칙을 명시하고, 오류 전달 체계를 고정한다.

---

## S2-QG1: 검증 규칙 목록 8개 이상

### 검증 규칙 (고정)

`runtime/src/parser.rs`의 `ModuleValidator` 기준:

1. `functions[*].type_idx`는 `types.len()` 범위 내여야 한다.
2. `code.len()`과 `functions.len()`은 동일해야 한다.
3. MVP 제약: 전체 memory(정의+import) 개수는 1 이하여야 한다.
4. `memory.min <= memory.max`를 만족해야 한다 (`max` 존재 시).
5. `memory.min <= 65536` 페이지여야 한다.
6. MVP 제약: 전체 table(정의+import) 개수는 1 이하여야 한다.
7. `table.min <= table.max`를 만족해야 한다 (`max` 존재 시).
8. export 이름은 중복되면 안 된다.
9. export index는 kind별 대상 개수 범위 내여야 한다.
10. start function index는 total function 범위 내여야 한다.
11. element segment의 table/function 참조는 범위 내여야 한다.
12. data segment의 memory 참조는 범위 내여야 한다.

판정: PASS

---

## S2-QG2: ValidationError 매핑 표

실제 타입은 `ParseError`이며, `module.validate_structure()`에서 `RuntimeError::InvalidBinary("Validation: ...")`로 승격된다.

| 발생 지점 | 원본 오류 | 외부 노출 오류 |
|---|---|---|
| `ModuleValidator::validate_*` | `ParseError::new("...", 0)` | `RuntimeError::InvalidBinary("Validation: ...")` |
| `WasmParser::parse` | `ParseError::new("...", pos)` | `RuntimeError::InvalidBinary("...")` |

추가 정책:
- 구조 검증 실패는 모두 `InvalidBinary` 계열로 통합한다.
- 사용자 디버깅을 위해 메시지 본문에 `Validation:` 접두를 유지한다.

판정: PASS

---

## S2-QG3: 최소 1개 음성(실패) 샘플 설계

음성 샘플 #1 (필수):
- 시나리오: `functions = [999]`, `types.len() = 1`
- 기대: `ModuleValidator::validate_functions`에서 type index out-of-range 오류
- 최종 오류: `RuntimeError::InvalidBinary("Validation: Function 0 references type index 999 ...")`

음성 샘플 #2 (권장):
- 시나리오: 동일 이름 export 2개
- 기대: `Duplicate export name` 오류

판정: PASS

---

## 종합 판정

- QG 통과: 3/3
- 통과율: 100%
- 결론: `7-2.S2 UNLOCK`, `7-2.S3` 활성화 가능