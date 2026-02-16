# Phase 7-2.S9 게이트 리포트 — 패키징·컴포넌트·E2E

작성일: 2026-02-16
상태: PASS (100%)

---

## 목표

`.kpioapp` 패키징, component model, 데모 E2E의 최소 성공 기준을 정의하고 릴리즈 전 검증 포인트를 고정한다.

---

## S9-QG1: 패키징 검증 체크리스트 완성

필수 체크리스트:
1. `manifest.toml` 필수 필드 유효성 (`id/name/version/entry`)
2. `app.wasm` 존재/해시 검증
3. 리소스 디렉토리 무결성 검증
4. 서명(도입 시) 및 버전 호환성 검증
5. 설치 경로 샌드박스 준수 확인

판정: PASS

---

## S9-QG2: component 최소 성공 경로 1개 이상 설계

최소 성공 경로:
1. 단일 core module 로드
2. 제한된 import 집합(wasi + 선택적 kpio) 바인딩
3. `run()` 엔트리 실행
4. 정상 종료/오류 종료 코드 수집

판정: PASS

---

## S9-QG3: E2E 데모 3종 pass 기준 문서화

데모 기준:
- Demo-1 `hello-world`: stdout 출력 + 종료코드 0
- Demo-2 `calculator`: 기본 사칙연산 UI 이벤트 응답
- Demo-3 `text-editor`: 파일 열기/저장 왕복 성공

각 데모 공통 pass 조건:
1. 실행 중 trap 미발생
2. 샌드박스 위반 없음
3. 종료 후 리소스 누수 없음

판정: PASS

---

## 종합 판정

- QG 통과: 3/3
- 통과율: 100%
- 결론: `7-2.S9 UNLOCK`, `Phase 7-2 Hard-Gate 문서 실행 라운드 완료`