// App-shell cache: stale-while-revalidate for same-origin GETs (API excluded).
const CACHE = 'outliner-shell-v1';
self.addEventListener('install', (e) => {
  e.waitUntil(caches.open(CACHE).then((c) => c.addAll(['./', './index.html'])).then(() => self.skipWaiting()));
});
self.addEventListener('activate', (e) => {
  e.waitUntil(
    caches.keys().then((keys) => Promise.all(keys.filter((k) => k !== CACHE).map((k) => caches.delete(k)))).then(() => self.clients.claim()),
  );
});
self.addEventListener('fetch', (e) => {
  const url = new URL(e.request.url);
  if (e.request.method !== 'GET' || url.origin !== location.origin || url.pathname.includes('/api/')) return;
  e.respondWith(
    caches.open(CACHE).then(async (c) => {
      const hit = await c.match(e.request, { ignoreSearch: url.pathname.endsWith('/') || url.pathname.endsWith('.html') });
      const net = fetch(e.request)
        .then((res) => {
          if (res.ok) c.put(e.request, res.clone());
          return res;
        })
        .catch(() => hit ?? c.match('./index.html'));
      return hit ?? net;
    }),
  );
});
