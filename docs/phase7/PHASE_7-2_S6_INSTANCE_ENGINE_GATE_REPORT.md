# Phase 7-2.S6 게이트 리포트 — Instance·Engine 통합

작성일: 2026-02-16
상태: PASS (100%)

---

## 목표

`Module → Instance → Engine` 호출 흐름의 API 일관성과 실패 경로를 고정한다.

---

## S6-QG1: load/instantiate/call/drop 전 경로 시나리오 완성

시나리오:
1. `Module::from_bytes()`로 파싱/검증
2. `Instance::new()` + import resolver 주입
3. `Instance::call_typed()` 또는 `call()` 실행
4. Engine 글로벌 초기화/획득 경로 (`engine::init`, `engine::get`) 검증
5. 종료 시 인스턴스 자원 해제(메모리/테이블/stdout/stderr 버퍼)

판정: PASS

---

## S6-QG2: 실패 경로(미해결 import 등) 케이스 정의

실패 케이스:
- export 이름 미존재 → `ExportNotFound`
- 함수 인덱스 미존재 → `FunctionNotFound`
- import 바인딩 누락/불일치 → instantiation 오류 경로
- 엔진 미초기화 상태에서 `engine::get()` 호출 → `ExecutionError("Engine not initialized")`

판정: PASS

---

## S6-QG3: API 호환성 체크리스트 완료

체크리스트:
- `Instance::call_typed(name, args)` 유지
- `Instance::call(name, &[u8])` 레거시 엔트리 유지
- `Store` 구조(`memories/tables/globals/functions`) 유지
- Host imports는 `(module, name)` 기반 매핑 유지

판정: PASS

---

## 종합 판정

- QG 통과: 3/3
- 통과율: 100%
- 결론: `7-2.S6 UNLOCK`, `7-2.S7` 활성화 가능