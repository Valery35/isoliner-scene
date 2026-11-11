// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

// Service worker страницы: страница работает без сети. Образец -
// sw.js PlanetX Lite, без тайлов и поиска: их у сцены нет.
//
// - Своя страница (index.html, js, wasm, значки) - сначала сеть, без
//   сети - кэш. Новая сборка заменяет файлы прежней.
// - Пакеты по ссылке ?scene= - сначала сеть, без сети - кэш.

const SHELL = 'isoliner-scene-shell';
const FILES = 'isoliner-scene-files';
const ALL = [SHELL, FILES];
// Записей кэша пакетов, сверх этого стираются самые старые.
const FILE_LIMIT = 50;

// Файлы сборки Trunk с хэшем в имени.
const BUILT = /isoliner-scene-[0-9a-f]+(?:_bg\.wasm|\.js)/g;

self.addEventListener('install', (event) => {
  event.waitUntil(cacheShell().catch(() => undefined).then(() => self.skipWaiting()));
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((names) => Promise.all(names.filter((n) => n.startsWith('isoliner-scene-') && !ALL.includes(n)).map((n) => caches.delete(n))))
      .then(() => self.clients.claim()),
  );
});

self.addEventListener('fetch', (event) => {
  const request = event.request;
  if (request.method !== 'GET' || request.cache === 'only-if-cached') {
    return;
  }
  const url = request.url;
  if (request.mode === 'navigate') {
    event.respondWith(page(event));
  } else if (new URL(url).origin === self.location.origin) {
    event.respondWith(networkFirst(event, SHELL, 0));
  } else if (url.startsWith('https://')) {
    event.respondWith(networkFirst(event, FILES, FILE_LIMIT));
  }
});

// Страница и файлы сборки, на которые она ссылается. Файлы прежних
// сборок стираются.
async function cacheShell() {
  const index = new URL('./', self.registration.scope).href;
  const response = await fetch(index, { cache: 'no-cache' });
  if (!response.ok) {
    return;
  }
  const text = await response.clone().text();
  const cache = await caches.open(SHELL);
  await cache.put(index, response);
  const built = new Set((text.match(BUILT) || []).map((name) => new URL(name, index).href));
  for (const href of built) {
    if (!(await cache.match(href))) {
      const file = await fetch(href);
      if (file.ok) {
        await cache.put(href, file);
      }
    }
  }
  for (const key of await cache.keys()) {
    if (key.url.match(BUILT) && !built.has(key.url)) {
      await cache.delete(key);
    }
  }
}

// Страница по любому адресу с параметрами (?scene=..., ?pose=...) -
// одна запись index.html.
async function page(event) {
  try {
    const response = await fetch(event.request);
    if (response.ok) {
      event.waitUntil(cacheShell().catch(() => undefined));
    }
    return response;
  } catch (error) {
    const cache = await caches.open(SHELL);
    const hit = await cache.match(new URL('./', self.registration.scope).href);
    if (hit) {
      return hit;
    }
    throw error;
  }
}

// Ответ уходит странице сразу, запись в кэш идёт следом.
async function networkFirst(event, name, limit) {
  const request = event.request;
  const cache = await caches.open(name);
  try {
    const response = await fetch(request);
    if (response.ok && response.type !== 'opaque') {
      event.waitUntil(store(cache, request, response.clone(), limit).catch(() => undefined));
    }
    return response;
  } catch (error) {
    const hit = await cache.match(request);
    if (hit) {
      return hit;
    }
    throw error;
  }
}

async function store(cache, request, response, limit) {
  await cache.put(request, response);
  if (limit > 0) {
    // записи кэша идут в порядке записи - стираются самые старые
    const keys = await cache.keys();
    for (let i = 0; i < keys.length - limit; i += 1) {
      await cache.delete(keys[i]);
    }
  }
}
