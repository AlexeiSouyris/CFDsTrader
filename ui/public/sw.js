/*
 * Service worker de CFDsTrader (#307). Guarda SOLO el armazón de la app (HTML, JS, CSS,
 * íconos y manifest) para que abra rápido y se pueda instalar en el celular.
 *
 * NUNCA guarda ni responde desde la caché nada de /api: precios, posiciones, cuenta, órdenes y
 * corridas van siempre a la red. Sin red, esos pedidos fallan y la UI lo muestra, en vez de
 * enseñar datos viejos. Tampoco toca pedidos que no sean GET ni de otro origen.
 *
 * Lo registra `src/main.tsx` solo en el build de producción.
 */
const CACHE = 'cfds-shell-v1'
const SHELL = [
  '/',
  '/index.html',
  '/manifest.webmanifest',
  '/favicon.svg',
  '/icon-192.png',
  '/icon-512.png',
  '/apple-touch-icon.png',
]

/** ¿Es del gateway? `/api` y todo lo que cuelga de él. */
function isApi(url) {
  return url.pathname === '/api' || url.pathname.startsWith('/api/')
}

/** Solo GET del mismo origen y fuera de /api. */
function shellRequest(request) {
  if (request.method !== 'GET') return false
  const url = new URL(request.url)
  return url.origin === self.location.origin && !isApi(url)
}

/** Una respuesta que se puede guardar: completa, del mismo origen y sin `no-store`. */
function storable(response) {
  return (
    response &&
    response.ok &&
    response.type === 'basic' &&
    !/no-store/i.test(response.headers.get('Cache-Control') || '')
  )
}

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches
      .open(CACHE)
      .then((cache) => cache.addAll(SHELL))
      .then(() => self.skipWaiting()),
  )
})

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) => Promise.all(keys.filter((k) => k !== CACHE).map((k) => caches.delete(k))))
      .then(() => self.clients.claim()),
  )
})

self.addEventListener('fetch', (event) => {
  const request = event.request
  // /api, POST y otros orígenes: el navegador los hace como siempre, sin caché.
  if (!shellRequest(request)) return

  const url = new URL(request.url)
  if (request.mode === 'navigate') {
    // La página: primero la red (versión nueva), y sin red, la guardada.
    event.respondWith(
      fetch(request)
        .then((response) => {
          if (storable(response)) {
            const copy = response.clone()
            caches.open(CACHE).then((cache) => cache.put('/index.html', copy))
          }
          return response
        })
        .catch(() => caches.match('/index.html')),
    )
    return
  }

  // Archivos del build (con hash en el nombre) e íconos: primero la caché.
  event.respondWith(
    caches.match(request).then(
      (hit) =>
        hit ||
        fetch(request).then((response) => {
          if (
            storable(response) &&
            (url.pathname.startsWith('/assets/') || SHELL.includes(url.pathname))
          ) {
            const copy = response.clone()
            caches.open(CACHE).then((cache) => cache.put(request, copy))
          }
          return response
        }),
    ),
  )
})
