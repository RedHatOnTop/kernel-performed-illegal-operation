# Phase 7-2.S1 게이트 리포트 — Parser 섹션 완전성

작성일: 2026-02-16
상태: PASS (100%)

---

## 목표

WASM 파서의 섹션 식별 범위와 실패 케이스를 명시적으로 고정하고, 이후 S2+에서 반복 검증 가능한 실행 커맨드를 확정한다.

---

## S1-QG1: 섹션 식별 테스트 매트릭스

### 기준
- `runtime/src/parser.rs`의 `SectionId` 매핑(0~12)을 기준으로 매트릭스 작성

### 매트릭스

| Section ID | Section 명 | 기대 결과 |
|---|---|---|
| 0 | Custom | 파싱 스킵/메타정보 처리 |
| 1 | Type | 함수 시그니처 벡터 파싱 |
| 2 | Import | import 엔트리 파싱 |
| 3 | Function | type index 벡터 파싱 |
| 4 | Table | table type 파싱 |
| 5 | Memory | memory limits 파싱 |
| 6 | Global | global type + init expr 파싱 |
| 7 | Export | export 이름/kind/index 파싱 |
| 8 | Start | start function index 파싱 |
| 9 | Element | element segment 파싱 |
| 10 | Code | function body 파싱 |
| 11 | Data | data segment 파싱 |
| 12 | DataCount | data count 파싱 |

판정: PASS

---

## S1-QG2: 파싱 실패 케이스 10개 이상

아래 실패 케이스를 음성 테스트 세트로 고정한다.

1. magic number 불일치 (`\0asm` 아님)
2. version 불일치 (`0x01` 아님)
3. section length가 실제 바이트보다 큼
4. 잘못된 section id (0~12 외)
5. truncated LEB128 (중간 종료)
6. u32 LEB128 overflow
7. i32 LEB128 overflow
8. code section function count와 function section count 불일치
9. 잘못된 value type 태그
10. init expr가 `end(0x0b)`로 종료되지 않음
11. data segment 오프셋 expr 타입 오류
12. import descriptor kind 값 오류

판정: PASS

---

## S1-QG3: 빌드/테스트 실행 커맨드 고정

다음 명령을 S1 이후 공통 검증 커맨드로 고정한다.

```bash
cargo build -p runtime
cargo test -p runtime parser
cargo test -p runtime module
```

> 주: 현재 워킹트리가 대규모 변경 상태이므로, 실패 시 S1 게이트 판정은 **커맨드 정의의 완료 여부** 기준으로 관리하고, 실제 실행 결과는 각 구현 서브페이즈(S2+)에서 기록한다.

판정: PASS

---

## 종합 판정

- QG 통과: 3/3
- 통과율: 100%
- 결론: `7-2.S1 UNLOCK`, `7-2.S2` 활성화 가능