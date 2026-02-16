# Phase 7-2.S7 게이트 리포트 — WASI P1 파일·프로세스

작성일: 2026-02-16
상태: PASS (100%)

---

## 목표

WASI Preview1의 파일/디렉토리/시간/난수/프로세스 제어 경로를 테스트 가능한 체크포인트로 고정한다.

---

## S7-QG1: stdout 출력 검증 시나리오 존재

시나리오:
1. `fd_write(1, ...)`로 stdout에 바이트 기록
2. 반환값(쓰기 길이) 검증
3. stdout 캡처 버퍼 내용 검증

판정: PASS

---

## S7-QG2: preopened dir sandbox 위반 케이스 존재

시나리오:
1. preopened dir 기준 외부 경로 접근 시도
2. `path_open`에서 접근 거부(`Access`/권한 오류) 확인
3. 허용 경로 접근은 정상 통과하는지 대비 검증

판정: PASS

---

## S7-QG3: proc_exit 종료 코드 전달 검증 존재

시나리오:
1. `proc_exit(42)` 호출
2. runtime exit 상태가 `42`로 보존되는지 확인
3. 후속 실행 중단(trap/exit 전달) 확인

추가 고정 검증 항목:
- `fd_read/fd_seek/fd_close`
- `path_create_directory/path_unlink_file/path_rename/fd_readdir`
- `args_get/environ_get`
- `clock_time_get/random_get`

판정: PASS

---

## 종합 판정

- QG 통과: 3/3
- 통과율: 100%
- 결론: `7-2.S7 UNLOCK`, `7-2.S8` 활성화 가능