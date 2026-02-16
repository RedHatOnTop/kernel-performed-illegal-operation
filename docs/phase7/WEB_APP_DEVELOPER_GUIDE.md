# KPIO Web App Developer Guide

A guide for developing and installing Progressive Web Apps (PWAs) on KPIO OS.

## Table of Contents

1. [PWA Basic Structure](#pwa-basic-structure)
2. [Writing a Web App Manifest](#writing-a-web-app-manifest)
3. [Registering a Service Worker](#registering-a-service-worker)
4. [Offline Strategies](#offline-strategies)
5. [Web Storage API](#web-storage-api)
6. [IndexedDB](#indexeddb)
7. [Notifications API](#notifications-api)
8. [Background Sync](#background-sync)
9. [Supported / Unsupported API List](#supported--unsupported-api-list)
10. [Limitations](#limitations)
11. [Demo App References](#demo-app-references)

---

## PWA Basic Structure

A KPIO-compatible PWA consists of at least 3 files:

```
my-app/
├── index.html        ← App entry point
├── manifest.json     ← Web App Manifest
└── sw.js             ← Service Worker
```

## Writing a Web App Manifest

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

### Required Fields

| Field | Description |
|-------|-------------|
| `name` | Full app name (install dialog, splash screen) |
| `short_name` | Short name (desktop icon label) |
| `start_url` | App start URL |
| `display` | `standalone` (recommended) or `minimal-ui` |
| `theme_color` | Title bar color (hex) |
| `icons` | At least one 192x192 icon required |

### `display` Modes

- **`standalone`**: No address bar, displayed like a native app. `theme_color` applied to title bar
- **`minimal-ui`**: Minimal navigation bar (back/forward/URL display)
- **`fullscreen`**: Full screen (no chrome)

## Registering a Service Worker

```html
<script>
  if ('serviceWorker' in navigator) {
    navigator.serviceWorker.register('sw.js')
      .then(reg => console.log('SW registered:', reg.scope))
      .catch(err => console.error('SW failed:', err));
  }
</script>
```

### Lifecycle

1. **Parsed** → SW script loaded
2. **Installing** → `install` event (pre-cache resources)
3. **Waiting** → Waiting for previous SW to deactivate
4. **Activated** → `activate` event (clean up old caches)

## Offline Strategies

### Cache First (for static content)

```javascript
self.addEventListener('fetch', event => {
  event.respondWith(
    caches.match(event.request).then(cached => {
      return cached || fetch(event.request);
    })
  );
});
```

### Network First (for dynamic data)

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

### localStorage (persistent storage)

```javascript
localStorage.setItem('theme', 'dark');
const theme = localStorage.getItem('theme'); // "dark"
localStorage.removeItem('theme');
localStorage.clear();
```

### sessionStorage (session-scoped)

```javascript
sessionStorage.setItem('tab_state', 'active');
// Automatically deleted when the app closes
```

**Note**: 5MB quota (total of keys + values)

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

  // Write
  const tx = db.transaction('notes', 'readwrite');
  tx.objectStore('notes').put({ title: 'Hello', body: 'World' });

  // Read
  const rtx = db.transaction('notes', 'readonly');
  rtx.objectStore('notes').getAll().onsuccess = (e) => {
    console.log(e.target.result);
  };
};
```

**Note**: 50MB total quota per app

## Notifications API

```javascript
// 1. Request permission
const permission = await Notification.requestPermission();

// 2. Show notification
if (permission === 'granted') {
  new Notification('New Message', {
    body: 'You have received a new message.',
    icon: 'icon-192.png'
  });
}
```

Behavior on KPIO:
- Toast displayed in top-right corner (max 3 simultaneous)
- Auto-dismissed after 5 seconds
- Clicking focuses the app window

## Background Sync

```javascript
// Register
const reg = await navigator.serviceWorker.ready;
await reg.sync.register('outbox-sync');

// Handle in SW
self.addEventListener('sync', event => {
  if (event.tag === 'outbox-sync') {
    event.waitUntil(sendPendingMessages());
  }
});
```

Behavior on KPIO:
- `sync` event fired on offline → online transition
- Retry with backoff on failure (30s → 60s → 300s)
- Discarded after max 3 attempts

## Supported / Unsupported API List

### ✅ Supported

| API | Status |
|-----|--------|
| Web App Manifest | ✅ Fully supported |
| Service Worker (basic lifecycle) | ✅ Supported |
| Cache API | ✅ Supported (25MB quota) |
| Fetch Interception | ✅ Supported |
| localStorage | ✅ Supported (5MB) |
| sessionStorage | ✅ Supported |
| IndexedDB | ✅ Supported (50MB) |
| Notifications API | ✅ Supported |
| Background Sync | ✅ Supported |
| `display: standalone` | ✅ Supported |
| `display: minimal-ui` | ✅ Supported |

### ❌ Unsupported

| API | Notes |
|-----|-------|
| Push API (server push) | Local notifications only |
| Periodic Background Sync | Not implemented |
| Web Share API | Not implemented |
| Payment Request API | Not implemented |
| WebRTC | Not implemented |
| WebGL | Partial support (software rendering) |
| Web Bluetooth / USB | Hardware not supported |

## Limitations

| Item | Limit |
|------|-------|
| localStorage quota | 5 MB per origin |
| Cache API quota | 25 MB per app |
| IndexedDB quota | 50 MB per app |
| Notification history | Last 50 entries |
| Simultaneous toasts | Max 3 |
| Background Sync retries | Max 3 attempts |
| Max app instances | Unlimited (memory dependent) |

## Demo App References

- **KPIO Notes**: `examples/pwa-notes/` — localStorage-based memo app (Cache First)
- **KPIO Weather**: `examples/pwa-weather/` — Weather app (Network First + Background Sync + Notifications)
