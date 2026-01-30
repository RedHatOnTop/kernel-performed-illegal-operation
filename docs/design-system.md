# KPIO Browser Design System

Version 1.0.0 | 2026-01-30

## 개요

KPIO Design System은 KPIO Browser OS를 위한 통합 디자인 시스템입니다. 현대적이고 접근성 높은 UI/UX를 제공하며, 일관된 사용자 경험을 보장합니다.

## 디자인 원칙

### 1. 단순함 (Simplicity)
- 불필요한 요소 제거
- 명확한 시각적 계층 구조
- 직관적인 인터랙션

### 2. 접근성 (Accessibility)
- WCAG 2.1 AA 기준 준수
- 키보드 네비게이션 지원
- 스크린 리더 호환성
- 충분한 색상 대비

### 3. 반응성 (Responsiveness)
- 유동적인 레이아웃
- 다양한 해상도 지원
- 터치 및 마우스 인터랙션

### 4. 일관성 (Consistency)
- 통일된 컴포넌트
- 예측 가능한 동작
- 재사용 가능한 패턴

---

## 색상 팔레트

### Primary (브랜드 색상)
| Token | Hex | 용도 |
|-------|-----|------|
| Primary-50 | #EFF6FF | 배경 틴트 |
| Primary-100 | #DBEAFE | 호버 배경 |
| Primary-200 | #BFDBFE | 활성 배경 |
| Primary-500 | #3B82F6 | 기본 버튼, 링크 |
| Primary-600 | #2563EB | 호버 상태 |
| Primary-700 | #1D4ED8 | 활성/클릭 상태 |

### Neutral (그레이스케일)
| Token | Hex | 용도 |
|-------|-----|------|
| Gray-50 | #F9FAFB | 배경 |
| Gray-100 | #F3F4F6 | 카드 배경 |
| Gray-200 | #E5E7EB | 테두리 |
| Gray-500 | #6B7280 | 보조 텍스트 |
| Gray-900 | #111827 | 기본 텍스트 |

### Semantic (의미 색상)
| 상태 | 색상 | 용도 |
|------|------|------|
| Success | #22C55E | 완료, 성공 |
| Warning | #F59E0B | 경고 |
| Error | #EF4444 | 오류, 위험 |
| Info | #3B82F6 | 정보 |

---

## 타이포그래피

### 폰트 스택
```
Primary: Pretendard, -apple-system, BlinkMacSystemFont, sans-serif
Mono: JetBrains Mono, Consolas, monospace
```

### 텍스트 스케일
| 이름 | 크기 | 줄높이 | 용도 |
|------|------|--------|------|
| Display LG | 48px | 56px | 대형 헤드라인 |
| Display MD | 36px | 44px | 중형 헤드라인 |
| Heading 1 | 24px | 32px | 페이지 제목 |
| Heading 2 | 20px | 28px | 섹션 제목 |
| Heading 3 | 18px | 24px | 카드 제목 |
| Body LG | 16px | 24px | 본문 (크게) |
| Body MD | 14px | 20px | 기본 본문 |
| Body SM | 12px | 16px | 작은 본문 |
| Label | 12px | 16px | 라벨, 태그 |
| Caption | 11px | 14px | 캡션, 힌트 |

---

## 간격 시스템

8px 기반 그리드 시스템:

| Token | 값 | 용도 |
|-------|-----|------|
| xs | 4px | 최소 간격 |
| sm | 8px | 인라인 요소 간격 |
| md | 12px | 컴포넌트 내부 |
| lg | 16px | 컴포넌트 간격 |
| xl | 24px | 섹션 간격 |
| xxl | 32px | 대형 섹션 |
| xxxl | 48px | 페이지 패딩 |

---

## 컴포넌트

### Button

```
┌────────────────────┐
│  [Icon]  Label     │ ← 40px 높이 (Medium)
└────────────────────┘
```

**Variants:**
- **Primary**: 채워진 파란색 배경
- **Secondary**: 테두리만 있는 버튼
- **Ghost**: 배경 없음, 호버 시 배경
- **Danger**: 빨간색, 삭제 등 위험 작업

**Sizes:**
- XSmall: 24px
- Small: 32px
- Medium: 40px (기본)
- Large: 48px
- XLarge: 56px

### Input

```
┌─────────────────────────────────┐
│ Label                           │
├─────────────────────────────────┤
│ Placeholder...             [x]  │ ← 40px 높이
├─────────────────────────────────┤
│ Helper text or error message    │
└─────────────────────────────────┘
```

### Card

```
┌─────────────────────────────────┐
│                                 │
│   Title                         │
│   Subtitle                      │
│                                 │
│   Content area                  │
│                                 │
│                     [Actions]   │
└─────────────────────────────────┘
    ↑ 8px radius, 그림자
```

### Tabs

```
┌────┬────┬────┬────────────────────┬───┐
│ ⊕  │ 🔵 │ 📄 │ Tab Title...      │ x │
└────┴────┴────┴────────────────────┴───┘
  │     │    │           │           │
  │     │    │           │           └─ 닫기 버튼
  │     │    │           └─ 제목 (최대 240px)
  │     │    └─ 파비콘
  │     └─ 로딩 인디케이터
  └─ 새 탭 버튼
```

---

## 레이아웃

### Browser Chrome

```
┌─────────────────────────────────────────────────────────────┐
│ [←][→][↻][🏠]  │ 🔒 example.com                    │ [⋮]   │ ← 탭바 + 툴바
├─────────────────────────────────────────────────────────────┤
│ [북마크바]                                                   │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│                                                             │
│                    Content Area                             │
│                                                             │
│                                                             │
├─────────────────────────────────────────────────────────────┤
│ Status: Ready                                   Zoom: 100%  │ ← 상태바
└─────────────────────────────────────────────────────────────┘
```

### Flex Layout

```rust
Flex::row()
    .gap(spacing::MD)
    .justify(JustifyContent::SpaceBetween)
    .align(AlignItems::Center)
```

### Grid Layout

```rust
Grid::new(3)
    .gap(spacing::LG)
    .padding(EdgeInsets::all(spacing::XL))
```

---

## 테마

### Light Theme
- 배경: Gray-50
- 표면: White
- 텍스트: Gray-900
- 테두리: Gray-200

### Dark Theme
- 배경: Gray-950
- 표면: Gray-900
- 텍스트: Gray-50
- 테두리: Gray-700

### 테마 전환
```rust
let theme = match preference {
    ThemePreference::Light => Theme::light(),
    ThemePreference::Dark => Theme::dark(),
    ThemePreference::System => detect_system_theme(),
};
```

---

## 애니메이션

### 지속 시간
| Token | 값 | 용도 |
|-------|-----|------|
| Instant | 0ms | 즉시 |
| Fast | 100ms | 마이크로 인터랙션 |
| Normal | 200ms | 일반 트랜지션 |
| Slow | 300ms | 모달, 패널 |
| Slower | 500ms | 페이지 전환 |

### Easing
- `ease-out`: 요소 등장
- `ease-in`: 요소 사라짐
- `ease-in-out`: 상태 변경
- `bounce-out`: 강조 효과

### 접근성 고려
- `prefers-reduced-motion` 지원
- 모션 감소 시 즉시 전환

---

## 아이콘

### 크기
- 16px: 인라인, 버튼 내부
- 20px: 기본 크기
- 24px: 툴바, 네비게이션
- 32px: 빈 상태, 강조

### 스타일
- Stroke width: 2px
- Round line cap
- Round line join

### 카테고리
- **Navigation**: 화살표, 홈, 새로고침
- **Browser**: 탭, 북마크, 다운로드
- **Actions**: 추가, 삭제, 편집
- **UI**: 설정, 사용자, 알림

---

## 반응형 브레이크포인트

| 이름 | 너비 | 용도 |
|------|------|------|
| Mobile | < 640px | 스마트폰 |
| Tablet | 640px - 1024px | 태블릿 |
| Desktop | 1024px - 1440px | 데스크톱 |
| Wide | > 1440px | 와이드 모니터 |

---

## 접근성 가이드라인

### 색상 대비
- 일반 텍스트: 4.5:1 이상
- 대형 텍스트: 3:1 이상
- UI 컴포넌트: 3:1 이상

### 포커스 표시
- 모든 인터랙티브 요소에 포커스 링
- 키보드 탐색 순서 논리적
- Skip link 제공

### ARIA
- 적절한 role 속성
- aria-label 필수 제공
- 라이브 리전 사용

---

## 파일 구조

```
design/
├── mod.rs          # 모듈 진입점
├── tokens.rs       # 디자인 토큰 (색상, 간격, 타이포)
├── theme.rs        # 라이트/다크 테마
├── components.rs   # 기본 UI 컴포넌트
├── layout.rs       # 레이아웃 시스템
├── icons.rs        # 아이콘 정의
├── animation.rs    # 애니메이션 시스템
├── browser.rs      # 브라우저 크롬 컴포넌트
├── dialogs.rs      # 대화상자, 토스트
└── pages.rs        # 페이지 컴포넌트
```

---

## 사용 예시

### 버튼 생성
```rust
use kpio_browser::design::*;

let button = Button::new("저장")
    .variant(ButtonVariant::Primary)
    .size(Size::Medium)
    .icon("save");
```

### 테마 적용
```rust
let design = DesignSystem::new()
    .with_theme(Theme::dark())
    .with_scale(1.0);
```

### 레이아웃 구성
```rust
let layout = Flex::row()
    .gap(spacing::MD)
    .justify(JustifyContent::SpaceBetween)
    .padding(EdgeInsets::all(spacing::LG));
```

---

## 변경 이력

| 버전 | 날짜 | 변경 사항 |
|------|------|----------|
| 1.0.0 | 2026-01-30 | 초기 디자인 시스템 릴리스 |
