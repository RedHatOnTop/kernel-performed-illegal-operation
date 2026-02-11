# Sub-Phase 6.7 Checklist: Media & Graphics Pipeline

## Overview
Canvas 2D API, 고급 렌더링, 오디오/비디오 기본 지원을 구현하여 리치 미디어 웹앱을 지원한다.

## Pre-requisites
- [ ] QG-6.6 100% 충족
- [ ] 이미지 디코딩 파이프라인 동작
- [ ] 폰트 래스터라이저 동작

---

## 6.7.1 Canvas 2D API

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.7.1.1 | CanvasRenderingContext2D (getContext) | ⬜ | |
| 6.7.1.2 | 경로 그리기 (arc, bezier 등) | ⬜ | |
| 6.7.1.3 | 사각형 (fillRect/strokeRect/clearRect) | ⬜ | |
| 6.7.1.4 | 텍스트 (fillText/measureText) | ⬜ | |
| 6.7.1.5 | 이미지 (drawImage/putImageData) | ⬜ | |
| 6.7.1.6 | 변환 (translate/rotate/scale) | ⬜ | |
| 6.7.1.7 | 스타일 (fillStyle/strokeStyle/alpha) | ⬜ | |
| 6.7.1.8 | 그라디언트 | ⬜ | |
| 6.7.1.9 | 라인 스타일 (lineWidth/dash 등) | ⬜ | |
| 6.7.1.10 | 클리핑 | ⬜ | |
| 6.7.1.11 | toBlob / toDataURL | ⬜ | |
| 6.7.1.12 | OffscreenCanvas | ⬜ | |

## 6.7.2 고급 렌더링

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.7.2.1 | border-radius 렌더링 | ⬜ | |
| 6.7.2.2 | box-shadow 렌더링 (가우시안 블러) | ⬜ | |
| 6.7.2.3 | 그라디언트 배경 렌더링 | ⬜ | |
| 6.7.2.4 | opacity / 합성 (알파 블렌딩) | ⬜ | |
| 6.7.2.5 | CSS transform 렌더링 | ⬜ | |
| 6.7.2.6 | CSS filter 렌더링 | ⬜ | |
| 6.7.2.7 | 텍스트 안티앨리어싱 개선 | ⬜ | |

## 6.7.3 오디오 기본 지원

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.7.3.1 | PCM 오디오 출력 (HDA 드라이버) | ⬜ | |
| 6.7.3.2 | `<audio>` 요소 | ⬜ | |
| 6.7.3.3 | WAV 디코더 | ⬜ | |
| 6.7.3.4 | Web Audio API 기초 | ⬜ | |

## 6.7.4 비디오 기본 지원

| ID | Task | Status | Notes |
|----|------|--------|-------|
| 6.7.4.1 | `<video>` 요소 프레임워크 | ⬜ | |
| 6.7.4.2 | MP4 컨테이너 파싱 | ⬜ | |

---

## Quality Gate

| # | Criterion | Method | Status |
|---|-----------|--------|--------|
| QG-6.7.1 | Canvas 도형 그리기 | fillRect + arc + bezier 정상 | ⬜ |
| QG-6.7.2 | Canvas 텍스트 | fillText 폰트·크기·정렬 정확 | ⬜ |
| QG-6.7.3 | Canvas 이미지 | drawImage PNG/JPEG 정상 | ⬜ |
| QG-6.7.4 | border-radius | 부드러운 둥근 모서리 | ⬜ |
| QG-6.7.5 | box-shadow | 블러·오프셋 육안 확인 | ⬜ |
| QG-6.7.6 | CSS transform | rotate(45deg) 정확 | ⬜ |
| QG-6.7.7 | 그라디언트 배경 | linear-gradient 부드러운 전환 | ⬜ |
| QG-6.7.8 | 오디오 재생 | WAV 재생 확인 (QEMU) | ⬜ |
| QG-6.7.9 | Canvas 테스트 30개+ | 경로/변환/이미지/텍스트 각 ≥5개 | ⬜ |
