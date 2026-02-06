# Phase: Core Infrastructure & Application Layer

## 전체 목표
픽셀 그래픽 GUI 뒤에 숨어있던 커널/터미널/브라우저의 실질적 기능을 완성하여,
KPIO OS를 "데모용 렌더링 셸"에서 "실제 동작하는 운영체제"로 전환한다.

## 성취 기준 (Phase 완료 조건)
1. **터미널**: 50+ 리눅스 호환 명령어, 인메모리 파일시스템, 환경변수, 히스토리, 파이프라인 개념 → 터미널만으로 OS 조작 가능
2. **커널**: 작동하는 syscall dispatch, 프레임 해제, 스케줄러 yield, VFS 연동
3. **브라우저**: HTML 파싱→스타일→레이아웃→렌더 파이프라인 연결, 웹앱 형태의 내부 페이지 구동
4. **드라이버**: VirtIO 블록/넷 초기화 시퀀스 완성, 키보드 레이아웃 확장, ACPI 테이블 파싱

## 서브페이즈 목록

| SP | 이름 | 핵심 산출물 |
|----|------|------------|
| 1 | Terminal: Linux-Compatible Shell | 전용 터미널 모듈 + 인메모리 FS + 50+ 명령어 |
| 2 | Kernel Core Hardening | syscall 정비, 메모리 개선, 스케줄러 강화 |
| 3 | VFS & In-Memory Filesystem | 커널 VFS 레이어 + ramfs + fd 테이블 |
| 4 | Network Stack Foundation | TCP 상태머신, DNS, HTTP 클라이언트 실제 구현 |
| 5 | Browser Engine Enhancement | 렌더 파이프라인 연결, 웹앱 페이지 렌더링 |
| 6 | Driver & Hardware Support | VirtIO I/O, 키보드 레이아웃, ACPI 개선 |

---
*각 서브페이즈의 상세 기획은 실행 직전에 별도 문서로 작성 후, 완료 시 삭제+커밋*
