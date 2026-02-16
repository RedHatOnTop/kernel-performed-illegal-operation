# Phase 7-2.S3 게이트 리포트 — Opcode 디코딩 커버리지

작성일: 2026-02-16
상태: PASS (100%)

---

## 목표

`Instruction` 정의와 `decode_instruction` 매핑을 기준으로 디코딩 커버리지를 측정 가능한 단위로 고정한다.

---

## S3-QG1: 명령군별 테스트 수량 목표 정의

### 명령군 분류

1. 제어 흐름 (`block/loop/if/br/call/...`)
2. 참조 (`ref.null/ref.is_null/ref.func`)
3. 변수 (`local/global get/set/tee`)
4. 테이블 (`table.get/set/init/copy/grow/...`)
5. 메모리 (`load/store/size/grow/init/copy/fill`)
6. 상수/비교/산술 (`i32/i64/f32/f64`)
7. 변환/재해석 (`wrap/extend/trunc/convert/reinterpret`)
8. 확장(`0xFC` sub-opcode 계열)

### 목표 수량 (최소)

| 명령군 | 최소 테스트 케이스 |
|---|---:|
| 제어 흐름 | 15 |
| 참조 | 5 |
| 변수 | 8 |
| 테이블 | 8 |
| 메모리 | 20 |
| 상수/비교/산술 | 40 |
| 변환/재해석 | 20 |
| 확장(0xFC) | 12 |
| **총계** | **128** |

판정: PASS

---

## S3-QG2: 미지원 opcode 처리 정책

정책:
- 1바이트 opcode 미지원 시: `ParseError("Unknown opcode", pos)` 반환
- `0xFC` 확장 sub-opcode 미지원 시: `ParseError("Unknown 0xFC sub-opcode", pos)` 반환
- decoder는 실패 즉시 중단(fail-fast)하고 부분 결과를 사용하지 않는다.

판정: PASS

---

## S3-QG3: 디코더 회귀 테스트 실행 절차

고정 절차:

```bash
cargo test -p runtime parser::tests
cargo test -p runtime module::tests
cargo test -p runtime -- --nocapture
```

회귀 체크 포인트:
1. `decode_instructions()`가 정상 경로에서 EOF까지 소비되는지
2. 잘못된 바이트 스트림에서 즉시 오류를 반환하는지
3. `Instruction` 추가 시 매핑 누락이 없는지(Unknown opcode 회귀)

판정: PASS

---

## 종합 판정

- QG 통과: 3/3
- 통과율: 100%
- 결론: `7-2.S3 UNLOCK`, `7-2.S4` 활성화 가능