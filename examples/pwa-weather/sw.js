// KPIO Weather — Service Worker (Network First strategy)
const CACHE_NAME = 'kpio-weather-v1';
const STATIC_ASSETS = [
  '/weather/',
  '/weather/index.html',
  '/weather/manifest.json',
  '/weather/icon-192.png',
  '/weather/icon-512.png'
];

// Install: pre-cache static assets
self.addEventListener('install', event => {
  event.waitUntil(
    caches.open(CACHE_NAME).then(cache => cache.addAll(STATIC_ASSETS))
  );
  self.skipWaiting();
});

// Activate: clean old caches
self.addEventListener('activate', event => {
  event.waitUntil(
    caches.keys().then(keys =>
      Promise.all(
        keys.filter(k => k !== CACHE_NAME).map(k => caches.delete(k))
      )
    )
  );
  self.clients.claim();
});

// Fetch: Network First — try network, fallback to cache
self.addEventListener('fetch', event => {
  event.respondWith(
    fetch(event.request)
      .then(response => {
        // Update cache with fresh response
        if (response.ok) {
          const clone = response.clone();
          caches.open(CACHE_NAME).then(cache => cache.put(event.request, clone));
        }
        return response;
      })
      .catch(() => {
        // Network failed — serve from cache
        return caches.match(event.request).then(cached => {
          if (cached) return cached;
          // Fallback for navigation
          if (event.request.mode === 'navigate') {
            return caches.match('/weather/index.html');
          }
        });
      })
  );
});

// Background Sync: weather-sync
self.addEventListener('sync', event => {
  if (event.tag === 'weather-sync') {
    event.waitUntil(syncWeatherData());
  }
});

async function syncWeatherData() {
  // In a real implementation, this would fetch fresh weather data
  // and notify the user of significant changes
  try {
    const clients = await self.clients.matchAll();
    for (const client of clients) {
      client.postMessage({ type: 'WEATHER_SYNCED', timestamp: Date.now() });
    }
  } catch (e) {
    // Retry will be handled by the sync manager
    throw e;
  }
}
