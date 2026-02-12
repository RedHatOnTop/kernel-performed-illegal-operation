# KPIO Web App 개발자 가이드

KPIO OS에서 Progressive Web App (PWA)을 개발하고 설치하는 방법을 안내합니다.

## 목차

1. [PWA 기본 구조](#pwa-기본-구조)
2. [Web App Manifest 작성](#web-app-manifest-작성)
3. [Service Worker 등록](#service-worker-등록)
4. [오프라인 전략](#오프라인-전략)
5. [Web Storage API](#web-storage-api)
6. [IndexedDB](#indexeddb)
7. [Notifications API](#notifications-api)
8. [Background Sync](#background-sync)
9. [지원/미지원 API 목록](#지원미지원-api-목록)
10. [제한 사항](#제한-사항)
11. [데모 앱 참고](#데모-앱-참고)

---

## PWA 기본 구조

KPIO 호환 PWA는 최소 3개의 파일로 구성됩니다:

```
my-app/
├── index.html        ← 앱 진입점
├── manifest.json     ← Web App Manifest
└── sw.js             ← Service Worker
```

## Web App Manifest 작성

```json
{
  "name": "My KPIO App",
  "short_name": "MyApp",
  "start_url": "/my-app/",
  "display": "standalone",
  "theme_color": "#3F51B5",
  "background_color": "#FFFFFF",
  "scope": "/my-app/",
  "icons": [
    { "src": "icon-192.png", "sizes": "192x192", "type": "image/png" },
    { "src": "icon-512.png", "sizes": "512x512", "type": "image/png" }
  ]
}
```

### 필수 필드

| 필드 | 설명 |
|------|------|
| `name` | 앱 전체 이름 (설치 다이얼로그, 스플래시) |
| `short_name` | 짧은 이름 (데스크톱 아이콘 레이블) |
| `start_url` | 앱 시작 URL |
| `display` | `standalone` (권장) 또는 `minimal-ui` |
| `theme_color` | 타이틀 바 색상 (hex) |
| `icons` | 최소 192x192 1개 필수 |

### `display` 모드

- **`standalone`**: 주소 바 없음, 네이티브 앱처럼 표시. 타이틀 바에 `theme_color` 적용
- **`minimal-ui`**: 최소한의 네비게이션 바 (뒤로/앞으로/URL 표시)
- **`fullscreen`**: 전체 화면 (크롬 없음)

## Service Worker 등록

```html
<script>
  if ('serviceWorker' in navigator) {
    navigator.serviceWorker.register('sw.js')
      .then(reg => console.log('SW registered:', reg.scope))
      .catch(err => console.error('SW failed:', err));
  }
</script>
```

### 라이프사이클

1. **Parsed** → SW 스크립트 로드
2. **Installing** → `install` 이벤트 (리소스 프리캐시)
3. **Waiting** → 이전 SW 활성 대기
4. **Activated** → `activate` 이벤트 (이전 캐시 정리)

## 오프라인 전략

### Cache First (정적 콘텐츠용)

```javascript
self.addEventListener('fetch', event => {
  event.respondWith(
    caches.match(event.request).then(cached => {
      return cached || fetch(event.request);
    })
  );
});
```

### Network First (동적 데이터용)

```javascript
self.addEventListener('fetch', event => {
  event.respondWith(
    fetch(event.request)
      .then(response => {
        const clone = response.clone();
        caches.open('v1').then(c => c.put(event.request, clone));
        return response;
      })
      .catch(() => caches.match(event.request))
  );
});
```

## Web Storage API

### localStorage (영속 저장)

```javascript
localStorage.setItem('theme', 'dark');
const theme = localStorage.getItem('theme'); // "dark"
localStorage.removeItem('theme');
localStorage.clear();
```

### sessionStorage (세션 한정)

```javascript
sessionStorage.setItem('tab_state', 'active');
// 앱 종료 시 자동 삭제
```

**주의**: 쿼터 5MB (키 + 값 합산)

## IndexedDB

```javascript
const request = indexedDB.open('MyDB', 1);

request.onupgradeneeded = (event) => {
  const db = event.target.result;
  const store = db.createObjectStore('notes', { keyPath: 'id', autoIncrement: true });
  store.createIndex('by_title', 'title', { unique: false });
};

request.onsuccess = (event) => {
  const db = event.target.result;

  // 쓰기
  const tx = db.transaction('notes', 'readwrite');
  tx.objectStore('notes').put({ title: 'Hello', body: 'World' });

  // 읽기
  const rtx = db.transaction('notes', 'readonly');
  rtx.objectStore('notes').getAll().onsuccess = (e) => {
    console.log(e.target.result);
  };
};
```

**주의**: 앱 당 총 50MB 쿼터

## Notifications API

```javascript
// 1. 권한 요청
const permission = await Notification.requestPermission();

// 2. 알림 표시
if (permission === 'granted') {
  new Notification('새 메시지', {
    body: '홍길동님이 메시지를 보냈습니다.',
    icon: 'icon-192.png'
  });
}
```

KPIO에서의 동작:
- 토스트가 화면 우상단에 표시됨 (최대 3개 동시)
- 5초 후 자동 사라짐
- 클릭 시 앱 윈도우 포커스

## Background Sync

```javascript
// 등록
const reg = await navigator.serviceWorker.ready;
await reg.sync.register('outbox-sync');

// SW에서 처리
self.addEventListener('sync', event => {
  if (event.tag === 'outbox-sync') {
    event.waitUntil(sendPendingMessages());
  }
});
```

KPIO에서의 동작:
- 오프라인 → 온라인 전환 시 `sync` 이벤트 발생
- 실패 시 백오프 재시도 (30초 → 60초 → 300초)
- 최대 3회 시도 후 폐기

## 지원/미지원 API 목록

### ✅ 지원

| API | 상태 |
|-----|------|
| Web App Manifest | ✅ 완전 지원 |
| Service Worker (기본 라이프사이클) | ✅ 지원 |
| Cache API | ✅ 지원 (25MB 쿼터) |
| Fetch Interception | ✅ 지원 |
| localStorage | ✅ 지원 (5MB) |
| sessionStorage | ✅ 지원 |
| IndexedDB | ✅ 지원 (50MB) |
| Notifications API | ✅ 지원 |
| Background Sync | ✅ 지원 |
| `display: standalone` | ✅ 지원 |
| `display: minimal-ui` | ✅ 지원 |

### ❌ 미지원

| API | 비고 |
|-----|------|
| Push API (서버 푸시) | 로컬 알림만 지원 |
| Periodic Background Sync | 미구현 |
| Web Share API | 미구현 |
| Payment Request API | 미구현 |
| WebRTC | 미구현 |
| WebGL | 부분 지원 (소프트웨어 렌더링) |
| Web Bluetooth / USB | 하드웨어 미지원 |

## 제한 사항

| 항목 | 제한 |
|------|------|
| localStorage 쿼터 | 5 MB per origin |
| Cache API 쿼터 | 25 MB per app |
| IndexedDB 쿼터 | 50 MB per app |
| 알림 이력 | 최근 50건 |
| 동시 토스트 | 최대 3개 |
| Background Sync 재시도 | 최대 3회 |
| 앱 최대 인스턴스 | 제한 없음 (메모리 의존) |

## 데모 앱 참고

- **KPIO Notes**: `examples/pwa-notes/` — localStorage 기반 메모 앱 (Cache First)
- **KPIO Weather**: `examples/pwa-weather/` — 날씨 앱 (Network First + Background Sync + Notifications)
