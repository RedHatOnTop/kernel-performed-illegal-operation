# Phase 7-2 실행 마스터 플랜 (Hard-Gate 운영)

> 기준 문서: `docs/phase7/PHASE_7-2_WASM_APP_RUNTIME.md`
> 원칙: **다음 서브페이즈로 이동 금지** (현재 서브페이즈 품질 게이트 100% 충족 전)
> 작성일: 2026-02-16

---

## 1) 운영 원칙

1. 각 서브페이즈는 `목표/작업/품질 게이트`를 가진다.
2. 품질 게이트는 **측정 가능**해야 하며, 검증 산출물(테스트 로그/빌드 로그/문서 링크)을 남긴다.
3. 게이트 충족률은 `통과 항목 수 / 전체 항목 수`로 계산하며, **100%가 아니면 잠금 상태 유지**.
4. 커밋 규칙:
   - 서브페이즈 완료 시점마다 1회 커밋
   - 커밋 메시지 접두사: `phase7-2:<subphase-id>`

---

## 2) 서브페이즈 분해 (기존 A~H 세분화)

| ID | 이름 | 목표 | 상태 |
|---|---|---|---|
| 7-2.S0 | 실행 프레임 고정 | 하드 게이트 규칙/추적 템플릿 확정 | ✅ 완료 |
| 7-2.S1 | Parser 섹션 완전성 | WASM 섹션 파서 범위·구조 검증 체계 고정 | ✅ 완료 |
| 7-2.S2 | Module 검증 강화 | type/function/import/export 정합성 게이트 확정 | ✅ 완료 |
| 7-2.S3 | Opcode 디코딩 커버리지 | 디코더/명령 집합 테스트 매트릭스 확정 | ✅ 완료 |
| 7-2.S4 | Interpreter 실행 정확성-I | 스택/제어흐름/정수 연산 정확성 검증 | ✅ 완료 |
| 7-2.S5 | Interpreter 실행 정확성-II | 메모리/부동소수/트랩/간접호출 검증 | ✅ 완료 |
| 7-2.S6 | Instance·Engine 통합 | instantiate/call/lifecycle/caching 검증 | ✅ 완료 |
| 7-2.S7 | WASI P1 파일·프로세스 | fd/path/args/env/proc_exit 검증 | ✅ 완료 |
| 7-2.S8 | Host 바인딩 확장 | wasi_snapshot_preview1 + kpio host 연결 | ✅ 완료 |
| 7-2.S9 | 패키징·컴포넌트·E2E | `.kpioapp`/component 기반 실행 검증 | ✅ 완료 |

---

## 3) 서브페이즈 상세 정의

### 7-2.S0 — 실행 프레임 고정

**목표**
- Phase 7-2 전체를 강제 순차 진행 가능한 관리 단위로 전환한다.

**작업**
- [x] 하드 게이트 정책 문서화
- [x] 서브페이즈 ID·락 규칙 정의
- [x] 완료 기준(100%) 계산식 명시

**품질 게이트**
- [x] S0-QG1: 운영 원칙 4개 이상 명시
- [x] S0-QG2: 7-2 전체를 8개 이상 서브페이즈로 분해
- [x] S0-QG3: 각 서브페이즈 목표를 1문장 이상 정의

**게이트 결과**
- 통과율: **3/3 = 100%**
- 판정: **PASS**

---

### 7-2.S1 — Parser 섹션 완전성

**목표**
- WASM 섹션 파싱 범위와 최소 구조 유효성 검증 경계를 고정한다.

**작업**
- [x] 섹션 ID(0~12) 매핑/파싱 경계 검토
- [x] LEB128(u32/i32/u64/i64) 경계 케이스 테스트 정의
- [x] magic/version 실패 케이스 테스트 정의

**품질 게이트**
- [x] S1-QG1: 섹션 식별 테스트 매트릭스 작성 완료
- [x] S1-QG2: 파싱 실패 케이스 10개 이상 정의
- [x] S1-QG3: 빌드/테스트 실행 커맨드 고정

**게이트 결과**
- 통과율: **3/3 = 100%**
- 판정: **PASS**
- 근거 문서: `docs/phase7/PHASE_7-2_S1_PARSER_GATE_REPORT.md`

---

### 7-2.S2 — Module 검증 강화

**목표**
- 모듈 구조 검증 범위를 명확히 하고 실패 유형을 분류한다.

**작업**
- [x] type index 범위 검증 케이스 정의
- [x] import/export 중복 이름 정책 확정
- [x] memory/table MVP 제한 검증 케이스 정의

**품질 게이트**
- [x] S2-QG1: 검증 규칙 목록 8개 이상 문서화
- [x] S2-QG2: ValidationError 매핑 표 완성
- [x] S2-QG3: 최소 1개 음성(실패) 샘플 설계

**게이트 결과**
- 통과율: **3/3 = 100%**
- 판정: **PASS**
- 근거 문서: `docs/phase7/PHASE_7-2_S2_MODULE_GATE_REPORT.md`

---

### 7-2.S3 — Opcode 디코딩 커버리지

**목표**
- 디코더가 다루는 명령군을 측정 가능한 커버리지 단위로 관리한다.

**작업**
- [x] 명령군(제어/변수/메모리/산술/변환/참조) 분류표 작성
- [x] 바이트→Instruction 디코딩 실패 케이스 정의
- [x] Instruction round-trip 정책 정의

**품질 게이트**
- [x] S3-QG1: 명령군별 테스트 수량 목표 정의
- [x] S3-QG2: 미지원 opcode 처리 정책 명시
- [x] S3-QG3: 디코더 회귀 테스트 실행 절차 문서화

**게이트 결과**
- 통과율: **3/3 = 100%**
- 판정: **PASS**
- 근거 문서: `docs/phase7/PHASE_7-2_S3_OPCODE_GATE_REPORT.md`

---

### 7-2.S4 — Interpreter 실행 정확성-I

**목표**
- 스택 기계 핵심(스택/프레임/제어흐름/정수)을 안정화한다.

**작업**
- [x] ValueStack/CallStack/BlockStack 검증 항목 고정
- [x] block/loop/if/br/br_if/br_table 경로 테스트 정의
- [x] i32/i64 산술/비교/비트연산 정확성 케이스 정의

**품질 게이트**
- [x] S4-QG1: 제어흐름 테스트 매트릭스 완성
- [x] S4-QG2: 정수 연산 100+ 케이스 계획 확정
- [x] S4-QG3: DivisionByZero/StackUnderflow 트랩 검증 절차 확정

**게이트 결과**
- 통과율: **3/3 = 100%**
- 판정: **PASS**
- 근거 문서: `docs/phase7/PHASE_7-2_S4_INTERPRETER_CORE_GATE_REPORT.md`

---

### 7-2.S5 — Interpreter 실행 정확성-II

**목표**
- 메모리/부동소수/트랩/간접 호출 품질을 고정한다.

**작업**
- [x] load/store/memory.grow 경계 테스트 정의
- [x] f32/f64 NaN/Inf 동작 검증 케이스 정의
- [x] call_indirect 시그니처 불일치 트랩 케이스 정의

**품질 게이트**
- [x] S5-QG1: Memory OOB 트랩 테스트 절차 고정
- [x] S5-QG2: IEEE 754 핵심 케이스 목록 완성
- [x] S5-QG3: 간접 호출 성공/실패 케이스 모두 존재

**게이트 결과**
- 통과율: **3/3 = 100%**
- 판정: **PASS**
- 근거 문서: `docs/phase7/PHASE_7-2_S5_INTERPRETER_EXT_GATE_REPORT.md`

---

### 7-2.S6 — Instance·Engine 통합

**목표**
- 모듈 로드/인스턴스화/호출/해제의 API 일관성을 확보한다.

**작업**
- [x] instantiate(import binding) 테스트 시나리오 정의
- [x] start function 자동 실행 검증 케이스 정의
- [x] module cache / instance pool 검증 기준 정의

**품질 게이트**
- [x] S6-QG1: load/instantiate/call/drop 전 경로 시나리오 완성
- [x] S6-QG2: 실패 경로(미해결 import 등) 케이스 정의
- [x] S6-QG3: API 호환성 체크리스트 완료

**게이트 결과**
- 통과율: **3/3 = 100%**
- 판정: **PASS**
- 근거 문서: `docs/phase7/PHASE_7-2_S6_INSTANCE_ENGINE_GATE_REPORT.md`

---

### 7-2.S7 — WASI P1 파일·프로세스

**목표**
- WASI Preview1의 파일/디렉토리/클럭/난수/프로세스 제어를 검증 가능 상태로 만든다.

**작업**
- [x] fd_read/fd_write/fd_seek/fd_close 경로 테스트 정의
- [x] path_open/path_create_directory/path_unlink/path_rename 테스트 정의
- [x] args/env/proc_exit/clock/random 검증 정의

**품질 게이트**
- [x] S7-QG1: stdout 출력 검증 시나리오 존재
- [x] S7-QG2: preopened dir sandbox 위반 케이스 존재
- [x] S7-QG3: proc_exit 종료 코드 전달 검증 존재

**게이트 결과**
- 통과율: **3/3 = 100%**
- 판정: **PASS**
- 근거 문서: `docs/phase7/PHASE_7-2_S7_WASI_GATE_REPORT.md`

---

### 7-2.S8 — Host 바인딩 확장

**목표**
- host function registration과 import dispatch 안정성을 확보한다.

**작업**
- [x] wasi_snapshot_preview1 네임스페이스 등록 점검
- [x] kpio/kpio_gpu/kpio_net 확장 우선순위 정의
- [x] host call ABI(포인터/길이/errno) 규칙 점검

**품질 게이트**
- [x] S8-QG1: host dispatch 경로 다이어그램 갱신
- [x] S8-QG2: 등록 함수 목록과 실제 구현 매핑 완료
- [x] S8-QG3: stub/implemented 상태표 최신화

**게이트 결과**
- 통과율: **3/3 = 100%**
- 판정: **PASS**
- 근거 문서: `docs/phase7/PHASE_7-2_S8_HOST_BINDING_GATE_REPORT.md`

---

### 7-2.S9 — 패키징·컴포넌트·E2E

**목표**
- `.kpioapp` 및 component model 도입 준비와 E2E 마감 기준을 정의한다.

**작업**
- [x] `.kpioapp` 포맷 필수 필드 고정
- [x] WIT/component linker 최소 스코프 정의
- [x] 데모 앱(hello/calculator/editor) E2E 시나리오 정의

**품질 게이트**
- [x] S9-QG1: 패키징 검증 체크리스트 완성
- [x] S9-QG2: component 최소 성공 경로 1개 이상 설계
- [x] S9-QG3: E2E 데모 3종 pass 기준 문서화

**게이트 결과**
- 통과율: **3/3 = 100%**
- 판정: **PASS**
- 근거 문서: `docs/phase7/PHASE_7-2_S9_PACKAGING_COMPONENT_E2E_GATE_REPORT.md`

---

## 4) 잠금 규칙 (Phase Lock)

- `UNLOCK(Sn)` 조건: `Sn-QG*` 전부 PASS (100%)
- `LOCK(Sn+1)` 기본값: 직전 서브페이즈가 PASS 전까지 고정
- 예외 없음: 리스크가 높아도 순서 건너뛰기 금지

---

## 5) 실행 로그

- 2026-02-16: `7-2.S0` 완료 (PASS, 100%)
- 2026-02-16: `7-2.S1` 완료 (PASS, 100%)
- 2026-02-16: `7-2.S2` 완료 (PASS, 100%)
- 2026-02-16: `7-2.S3` 완료 (PASS, 100%)
- 2026-02-16: `7-2.S4` 완료 (PASS, 100%)
- 2026-02-16: `7-2.S5` 완료 (PASS, 100%)
- 2026-02-16: `7-2.S6` 완료 (PASS, 100%)
- 2026-02-16: `7-2.S7` 완료 (PASS, 100%)
- 2026-02-16: `7-2.S8` 완료 (PASS, 100%)
- 2026-02-16: `7-2.S9` 완료 (PASS, 100%)
- 상태: `Phase 7-2 Hard-Gate 실행 라운드 완료`