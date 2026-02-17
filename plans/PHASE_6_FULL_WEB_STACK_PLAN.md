# Phase 6: Full Web Stack Implementation Plan

> **Goal:** Extend KPIO OS with a complete web rendering and execution stack, enabling real-world web page display and interaction.  
> **Dependencies:** Phase 5 (System Integration)  
> **Estimated Duration:** 16-24 weeks  
> **Status:** Planning Stage

---

## Current State Summary

### Completed
- **Phase 5 (System Integration):** Display system, framebuffer, window manager, graphics pipeline
- **kpio-html:** HTML5 tokenizer/parser, DOM tree construction
- **kpio-css:** CSS3 parser, value types, specificity/cascade computation
- **kpio-layout:** Box model, block/inline/flex layout, position system
- **kpio-dom:** DOM API, events (capture/bubble), mutation observer stubs
- **kpio-js:** JS interpreter (boa-engine), DOM bindings, console API

### Remaining Gaps
- ❌ Real-world network stack (TCP/IP, DNS, HTTP/1.1+)
- ❌ TLS/HTTPS support
- ❌ JS engine completeness (ES2020+, async/await, Promises)
- ❌ CSS layout accuracy (table, grid, multi-column)
- ❌ Web APIs (Fetch, Web Storage, Canvas, Worker, WebSocket)
- ❌ Cookie/session management
- ❌ Actual page rendering (full pipeline: network → parse → layout → paint → composite)

---

## Architecture Principles

### Layered Architecture

```
┌─────────────────────────────────────────────────────────┐
│                  Browser Chrome (UI)                     │
│  ┌──────┐ ┌───────┐ ┌────────┐ ┌─────────────────────┐ │
│  │ Tabs │ │Address│ │DevTools│ │    Extension API     │ │
│  └──────┘ └───────┘ └────────┘ └─────────────────────┘ │
├─────────────────────────────────────────────────────────┤
│              Web Platform Engine                         │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐  │
│  │   HTML   │ │   CSS    │ │    JS    │ │ Web APIs │  │
│  │  Parser  │ │  Engine  │ │  Engine  │ │          │  │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘  │
│       │            │            │             │         │
│  ┌────┴────────────┴────────────┴─────────────┴──────┐ │
│  │              DOM / CSSOM Tree                      │ │
│  └─────────────────────┬─────────────────────────────┘ │
│                        │                                │
│  ┌─────────────────────┴─────────────────────────────┐ │
│  │         Layout Engine (Flow, Flex, Grid)           │ │
│  └─────────────────────┬─────────────────────────────┘ │
│                        │                                │
│  ┌─────────────────────┴─────────────────────────────┐ │
│  │      Rendering Pipeline (Paint + Composite)        │ │
│  └───────────────────────────────────────────────────┘ │
├─────────────────────────────────────────────────────────┤
│           System Layer (KPIO Kernel)                     │
│   Network │ Storage │ Graphics │ Input │ Audio          │
└─────────────────────────────────────────────────────────┘
```

### Design Constraints
1. **`no_std` first:** Kernel-level and platform crates must work in `no_std` environments.
2. **Layered isolation:** Each component has a clear interface. Network ↔ Engine ↔ Renderer are separated.
3. **Incremental implementation:** Start from static HTML display and progressively extend to dynamic/interactive pages.
4. **Spec compliance:** Not full W3C/WHATWG compliance. Prioritize the "practically used" subset.

---

## Sub-Phase 6.1: Network Foundation

> **Goal:** Implement TCP/IP based HTTP/1.1 communication, enabling actual web page downloads.  
> **Duration:** 2-3 weeks  
> **Dependencies:** Phase 5.3 (Network stack basics), `network/` crate

### 6.1.1 TCP Stack Enhancement

**Current State:** The `network/` crate has basic TCP/IP types and packet structures.  
**Target:** Full TCP connection lifecycle management.

**Checklist:**
- [ ] TCP state machine: CLOSED → LISTEN → SYN_SENT → SYN_RECEIVED → ESTABLISHED → FIN_WAIT → CLOSED
- [ ] 3-way handshake (SYN → SYN-ACK → ACK)
- [ ] Sliding window flow control
- [ ] Retransmission timer (exponential backoff)
- [ ] Congestion control (simplified Reno)
- [ ] Out-of-order packet reordering
- [ ] MSS (Maximum Segment Size) negotiation
- [ ] FIN/RST handling
- [ ] Keep-alive support

### 6.1.2 DNS Resolver

**Checklist:**
- [ ] DNS query builder (A, AAAA, CNAME records)
- [ ] UDP-based DNS query/response (port 53)
- [ ] DNS response parser (name decompression)
- [ ] DNS cache (TTL-based expiry)
- [ ] `/etc/hosts` style local override
- [ ] Multi-server fallback (Google DNS 8.8.8.8, Cloudflare 1.1.1.1)

### 6.1.3 HTTP/1.1 Client

**Checklist:**
- [ ] Request builder: Method + URL + Headers + Body
- [ ] Response parser: Status line, headers, body
- [ ] Chunked Transfer-Encoding support
- [ ] Content-Length based body reading
- [ ] Connection: Keep-Alive (connection reuse)
- [ ] Redirect handling (301, 302, 307, 308) — max 10 redirects
- [ ] Host header auto-generation
- [ ] User-Agent: `KPIO/x.y.z`
- [ ] Cookie header sending / Set-Cookie receiving

### 6.1.4 URL Parser

**Checklist:**
- [ ] WHATWG URL standard parsing
- [ ] Scheme, host, port, path, query, fragment extraction
- [ ] Percent encoding/decoding
- [ ] Relative URL resolution
- [ ] Punycode/IDNA (international domain names) — basic support
- [ ] `data:` URL scheme
- [ ] `blob:` URL scheme (stub)

### Quality Gate 6.1
- [ ] `http://httpbin.org/get` request → receive JSON response
- [ ] `http://example.com` → render HTML page
- [ ] DNS resolution testable
- [ ] TCP connection state machine is complete

---

## Sub-Phase 6.2: TLS/HTTPS

> **Goal:** Enable secure communication with TLS 1.2/1.3, allowing HTTPS page access.  
> **Duration:** 2-3 weeks  
> **Dependencies:** Phase 6.1

### 6.2.1 TLS Implementation

Options: Implement on top of `rustls` (no_std-compatible fork) or implement a custom TLS stack.

**Checklist:**
- [ ] TLS 1.2 handshake (RSA key exchange)
- [ ] TLS 1.3 handshake (ECDHE + AEAD)
- [ ] Certificate chain validation:
  - Root CA bundle (Mozilla CA bundle, embedded)
  - Chain: leaf → intermediate → root
  - Validity period check
  - Common Name / SAN (Subject Alternative Name) matching
- [ ] Cipher suites:
  - TLS_AES_128_GCM_SHA256
  - TLS_AES_256_GCM_SHA384
  - TLS_CHACHA20_POLY1305_SHA256
- [ ] SNI (Server Name Indication) extension
- [ ] ALPN (Application-Layer Protocol Negotiation)
- [ ] Session resumption (session ticket / PSK)

### 6.2.2 Crypto Primitives

Using the `ring` or `RustCrypto` crate family:

**Checklist:**
- [ ] SHA-256, SHA-384 hash
- [ ] HMAC-SHA256
- [ ] AES-128-GCM, AES-256-GCM
- [ ] ChaCha20-Poly1305
- [ ] X25519 ECDH key exchange
- [ ] RSA-PKCS1v15 verification (for legacy certificates)
- [ ] ECDSA (P-256) verification
- [ ] Ed25519 (for KPIO app signing)

### 6.2.3 HTTPS Integration

**Checklist:**
- [ ] `https://` URL scheme handling
- [ ] Port 443 default
- [ ] Certificate error UI (warning page, proceed, reject)
- [ ] HSTS (HTTP Strict Transport Security) cache
- [ ] Mixed content blocking (HTTP inside HTTPS page)

### Quality Gate 6.2
- [ ] `https://example.com` → successful load
- [ ] `https://httpbin.org/get` → JSON response over HTTPS
- [ ] Invalid certificate → warning page display
- [ ] Certificate chain validation passes for major CAs

---

## Sub-Phase 6.3: JavaScript Engine Enhancement

> **Goal:** Upgrade the JS engine to cover ES2020+ syntax and Web API integration.  
> **Duration:** 3-4 weeks  
> **Dependencies:** Phase 5 (`kpio-js` crate)

### 6.3.1 ES2020+ Language Features

Based on `boa-engine`, extend or replace:

**Checklist:**
- [ ] `async`/`await` (async function, for-await-of)
- [ ] `Promise` (constructor, then/catch/finally, Promise.all/race/allSettled)
- [ ] `Symbol` (Symbol.iterator, Symbol.toPrimitive)
- [ ] `Proxy` / `Reflect`
- [ ] `WeakRef` / `FinalizationRegistry`
- [ ] Optional chaining (`?.`) and nullish coalescing (`??`)
- [ ] `BigInt`
- [ ] `globalThis`
- [ ] Dynamic `import()`
- [ ] `for...of`, iterators, generators (`function*`)
- [ ] Template literals (tagged templates)
- [ ] Destructuring (object/array, default values)
- [ ] Spread operator (`...`)
- [ ] Module system (`import`/`export`) — basic support

### 6.3.2 Built-in Objects Enhancement

**Checklist:**
- [ ] `Array`: `flat`, `flatMap`, `at`, `findLast`, `findLastIndex`
- [ ] `Object`: `fromEntries`, `hasOwn`
- [ ] `String`: `replaceAll`, `trimStart`/`trimEnd`, `matchAll`
- [ ] `Map`, `Set` (full implementation)
- [ ] `WeakMap`, `WeakSet`
- [ ] `RegExp` (named groups, lookbehind, dotAll flag)
- [ ] `Date` (ISO 8601 parsing, UTC methods)
- [ ] `JSON` (full spec — reviver, replacer, space)
- [ ] `Intl` (basic — `NumberFormat`, `DateTimeFormat`)
- [ ] `TextEncoder`/`TextDecoder` (UTF-8)
- [ ] `URL`/`URLSearchParams`

### 6.3.3 Event Loop and Async Runtime

**Checklist:**
- [ ] Microtask queue (Promise resolution, queueMicrotask)
- [ ] Macrotask queue (setTimeout, setInterval, I/O callbacks)
- [ ] `requestAnimationFrame` (linked to display refresh)
- [ ] Event loop phases: script execution → microtasks → render → macrotasks
- [ ] Async I/O integration (network fetch → Promise resolution)

### 6.3.4 Memory Management

**Checklist:**
- [ ] Mark-and-sweep GC (existing boa-engine GC review)
- [ ] Memory limits per context (configurable, default 64MB)
- [ ] GC pressure monitoring (logging, metrics)
- [ ] WeakRef integration with GC cycles

### Quality Gate 6.3
- [ ] `async function` + `await fetch()` end-to-end operation
- [ ] `Promise.all()` + multiple async tasks
- [ ] `setTimeout`/`setInterval` executing correctly
- [ ] Test262 pass rate >= 70% (ES2020 subset)

---

## Sub-Phase 6.4: CSS Engine and Layout Enhancement

> **Goal:** Support CSS features needed by real-world websites.  
> **Duration:** 2-3 weeks  
> **Dependencies:** `kpio-css`, `kpio-layout`

### 6.4.1 CSS Selectors (Level 3+)

**Checklist:**
- [ ] Attribute selectors: `[attr=val]`, `[attr~=val]`, `[attr|=val]`, `[attr^=val]`, `[attr$=val]`, `[attr*=val]`
- [ ] Pseudo-classes: `:nth-child(n)`, `:nth-of-type(n)`, `:first-child`, `:last-child`, `:not(sel)`, `:is(sel)`
- [ ] Pseudo-elements: `::before`, `::after`, `::first-line`, `::first-letter`
- [ ] Combinators: descendant (` `), child (`>`), adjacent (`+`), general sibling (`~`)
- [ ] Universal selector `*`
- [ ] Specificity calculation accuracy improvement
- [ ] `@media` queries (width, height, orientation, prefers-color-scheme)

### 6.4.2 CSS Box Model and Visual Properties

**Checklist:**
- [ ] `box-sizing: border-box`
- [ ] `overflow: hidden/scroll/auto` + scrollbar rendering
- [ ] `box-shadow` (offset, blur, spread, color)
- [ ] `border-radius` (rounded corners, per-corner)
- [ ] `opacity` and `rgba()`/`hsla()` colors
- [ ] `background`: linear-gradient, radial-gradient
- [ ] `background-image: url()` — image loading and rendering
- [ ] `background-size`, `background-position`, `background-repeat`
- [ ] `transform`: translate, rotate, scale (2D)
- [ ] `transition` (property, duration, timing)
- [ ] `animation` / `@keyframes` (basic loop/alternate)
- [ ] `filter`: blur, brightness, contrast, grayscale
- [ ] `z-index` stacking context

### 6.4.3 Layout Engine Improvements

**Checklist:**
- [ ] `display: grid`:
  - `grid-template-rows/columns` (explicit tracks)
  - `grid-gap` / `gap`
  - `grid-area`, `grid-row`, `grid-column` (placement)
  - `fr` unit, `auto`, `minmax()`
  - Auto-placement algorithm
- [ ] `display: table`:
  - `table`, `table-row`, `table-cell`, `table-header-group`
  - Column width calculation (auto, fixed, percentage)
  - `colspan`, `rowspan`
  - `border-collapse`, `border-spacing`
- [ ] Multi-column layout (`column-count`, `column-width`, `column-gap`)
- [ ] Float reflow accuracy improvement
- [ ] `position: sticky`
- [ ] `min-content`, `max-content`, `fit-content` intrinsic sizes
- [ ] `calc()` in layout properties

### 6.4.4 Text Rendering Enhancement

**Checklist:**
- [ ] `font-family` fallback chain (system fonts → generic)
- [ ] `font-weight` (100-900, bold)
- [ ] `font-style` (normal, italic)
- [ ] `text-decoration` (underline, overline, line-through)
- [ ] `text-transform` (uppercase, lowercase, capitalize)
- [ ] `letter-spacing`, `word-spacing`
- [ ] `text-overflow: ellipsis` + `overflow: hidden` + `white-space: nowrap`
- [ ] `line-height` calculation (normal = 1.2)
- [ ] CJK (Chinese/Japanese/Korean) line-breaking rules (basic)
- [ ] Bi-directional text (minimal RTL)

### Quality Gate 6.4
- [ ] CSS Grid: 3-column responsive layout renders correctly
- [ ] Border-radius + box-shadow renders correctly
- [ ] `position: sticky` sticks at scroll threshold
- [ ] `@media (max-width: 768px)` responsive design applies
- [ ] CSS animations run at 30fps+ simple loop

---

## Sub-Phase 6.5: Web Platform APIs

> **Goal:** Implement essential Web APIs for dynamic web app operation.  
> **Duration:** 3-4 weeks  
> **Dependencies:** Phase 6.1-6.3

### 6.5.1 Fetch API

**Checklist:**
- [ ] `fetch(url, options)` → `Promise<Response>`
- [ ] Request options: method, headers, body, mode, credentials
- [ ] Response: `status`, `statusText`, `ok`, `headers`
- [ ] Body methods: `text()`, `json()`, `arrayBuffer()`, `blob()`
- [ ] `AbortController` / `AbortSignal`
- [ ] CORS (blocked in kernel context — everything allowed, but set headers)
- [ ] Error handling (network error, timeout)
- [ ] Streaming response (ReadableStream — basic)

### 6.5.2 DOM APIs (Extension)

Extending existing `kpio-dom`:

**Checklist:**
- [ ] `document.querySelector()` / `querySelectorAll()` (CSS selector matching)
- [ ] `element.classList` (add, remove, toggle, contains)
- [ ] `element.dataset` (data-* attributes)
- [ ] `element.style` (inline style manipulation)
- [ ] `element.getBoundingClientRect()` (layout rectangle query)
- [ ] `element.scrollIntoView()`
- [ ] `document.createDocumentFragment()`
- [ ] `element.insertAdjacentHTML()`
- [ ] `IntersectionObserver` (basic — visible/hidden)
- [ ] `ResizeObserver` (basic — element resized)
- [ ] `MutationObserver` (full implementation — from existing stub)

### 6.5.3 Canvas API

**Checklist:**
- [ ] `<canvas>` element + `getContext('2d')`
- [ ] Drawing operations:
  - `fillRect`, `strokeRect`, `clearRect`
  - `fillText`, `strokeText`, `measureText`
  - `beginPath`, `moveTo`, `lineTo`, `arc`, `bezierCurveTo`
  - `fill`, `stroke`, `clip`
- [ ] Style: `fillStyle`, `strokeStyle` (color, gradient)
- [ ] Transform: `translate`, `rotate`, `scale`, `setTransform`
- [ ] Image: `drawImage()` (from `<img>` or another `<canvas>`)
- [ ] Pixel data: `getImageData()`, `putImageData()`
- [ ] `toDataURL()` / `toBlob()`

### 6.5.4 Web Storage

**Checklist:**
- [ ] `localStorage`:
  - `getItem`, `setItem`, `removeItem`, `clear`
  - Persistent storage (VFS-backed)
  - 5MB per-origin limit
  - `storage` event
- [ ] `sessionStorage`:
  - Same API as localStorage
  - Cleared on tab/window close
- [ ] `IndexedDB`:
  - `open` + `onupgradeneeded`
  - Object store: `add`, `put`, `get`, `delete`
  - Index: `createIndex`, `get`
  - Cursor: `openCursor`, iterate
  - Transaction: `readonly`, `readwrite`

### 6.5.5 Other Essential Web APIs

**Checklist:**
- [ ] `FormData` (multipart/form-data)
- [ ] `Headers` (HTTP header map)
- [ ] `WebSocket` (ws:// protocol — basic)
- [ ] `history` API (`pushState`, `popState`, `replaceState`)
- [ ] `location` object (`href`, `origin`, `pathname`, `search`, `hash`)
- [ ] `Performance` API (`performance.now()`, `performance.mark()`)
- [ ] `Crypto.getRandomValues()`
- [ ] `structuredClone()`
- [ ] `Blob` / `File` / `FileReader`
- [ ] `console`: `log`, `error`, `warn`, `info`, `table`, `time`/`timeEnd`

### Quality Gate 6.5
- [ ] `fetch()` → JSON API call → display data on page
- [ ] Canvas: line chart drawing
- [ ] localStorage: save/load persistent data
- [ ] `querySelector` + classList manipulation → dynamic style change

---

## Sub-Phase 6.6: Browser Core Enhancement

> **Goal:** Enhance the browser application with tabbed browsing, history, bookmarks, etc.  
> **Duration:** 2 weeks  
> **Dependencies:** Phase 6.1-6.5

### 6.6.1 Multi-Tab Support

**Checklist:**
- [ ] Tab infrastructure: create, close, switch, reorder
- [ ] Per-tab independent DOM/CSSOM/JS context
- [ ] Tab bar UI (title, favicon, close button)
- [ ] Active tab highlighting
- [ ] "New Tab" page
- [ ] Ctrl+T (new tab), Ctrl+W (close tab), Ctrl+Tab (switch)

### 6.6.2 Navigation

**Checklist:**
- [ ] Address bar (URL input + autocomplete)
- [ ] Back/Forward (history-based)
- [ ] Refresh (re-request + re-render)
- [ ] Stop (abort in-progress requests)
- [ ] Navigation progress bar (loading indicator)

### 6.6.3 History and Bookmarks

**Checklist:**
- [ ] History storage (URL, title, timestamp, visit count)
- [ ] History UI page (chronological list, search)
- [ ] Bookmark add/remove/edit
- [ ] Bookmark bar (frequent bookmarks)
- [ ] Bookmark manager page

### 6.6.4 Cookie Manager

**Checklist:**
- [ ] Cookie jar: set, get, delete
- [ ] Cookie attributes: Domain, Path, Expires, Max-Age, Secure, HttpOnly, SameSite
- [ ] Cookie scope enforcement (domain + path matching)
- [ ] Cookie send on request (Cookie header)
- [ ] Cookie receive from response (Set-Cookie header)
- [ ] Cookie UI (view/delete per-site)

### Quality Gate 6.6
- [ ] Open 3+ tabs, each with different pages
- [ ] Back/Forward navigation works correctly
- [ ] Bookmarks persist across browser restart
- [ ] Cookies persisted, sent in subsequent requests

---

## Sub-Phase 6.7: Media and Graphics

> **Goal:** Display images and play media on web pages.  
> **Duration:** 2-3 weeks  
> **Dependencies:** Phase 6.4 (layout), Phase 5 (graphics)

### 6.7.1 Image Support

**Checklist:**
- [ ] `<img>` element rendering:
  - PNG decoding (DEFLATE / interlaced)
  - JPEG decoding (baseline / progressive)
  - GIF decoding (static only initially, then animated)
  - SVG rendering (basic shapes, paths, text)
  - WebP decoding (lossy, lossless)
- [ ] Image sizing: width/height attributes, CSS size, aspect-ratio preservation
- [ ] `<picture>` / `<source>` (srcset media queries)
- [ ] Background image rendering (`background-image: url()`)
- [ ] Image caching (memory + disk)
- [ ] Loading states: placeholder → loaded → error
- [ ] Lazy loading (`loading="lazy"`)

### 6.7.2 SVG Rendering

**Checklist:**
- [ ] SVG elements: `<svg>`, `<rect>`, `<circle>`, `<ellipse>`, `<line>`, `<polyline>`, `<polygon>`
- [ ] `<path>` (M, L, C, Q, A, Z commands)
- [ ] `<text>` / `<tspan>`
- [ ] `<g>` (grouping) + `transform`
- [ ] `fill`, `stroke`, `stroke-width`, `opacity`
- [ ] `viewBox` and `preserveAspectRatio`
- [ ] CSS styling of SVG elements
- [ ] Inline SVG (within HTML)

### 6.7.3 Audio/Video (Future Optimization)

> **Note:** Full media playback is low priority. Basic support only.

**Checklist:**
- [ ] `<audio>` element:
  - WAV playback (PCM)
  - MP3 decoding (via minimp3-like library)
  - Play, pause, stop controls
  - Volume control
- [ ] `<video>` element — stub only (poster image display)
- [ ] MediaSource Extensions — not supported

### Quality Gate 6.7
- [ ] Web page with mixed PNG/JPEG/SVG images displays correctly
- [ ] SVG icons render properly
- [ ] Image lazy loading works (load on scroll)
- [ ] WAV audio plays with controls

---

## Sub-Phase 6.8: Advanced Web Platform Features

> **Goal:** Provide advanced features needed for modern web apps.  
> **Duration:** 2 weeks  
> **Dependencies:** Phase 6.5

### 6.8.1 Service Worker (Basic)

**Checklist:**
- [ ] `navigator.serviceWorker.register(scriptURL)`
- [ ] SW lifecycle: installing → waiting → active
- [ ] `install` event (cache static resources)
- [ ] `fetch` event (intercept requests)
- [ ] Cache-first strategy
- [ ] SW update detection

### 6.8.2 Web Workers

**Checklist:**
- [ ] `new Worker(scriptURL)` — separate JS context
- [ ] `postMessage` / `onmessage` inter-thread communication
- [ ] `SharedWorker` (stub)
- [ ] Structured clone for message serialization
- [ ] Worker termination (`worker.terminate()`)

### 6.8.3 Notification API

**Checklist:**
- [ ] `Notification.requestPermission()`
- [ ] `new Notification(title, options)`
- [ ] Notification rendering (OS notification center integration)
- [ ] `onclick`, `onclose`, `onerror` events
- [ ] Icon and body text display

### 6.8.4 PWA Foundation

**Checklist:**
- [ ] Web App Manifest parsing (`manifest.json`)
- [ ] `beforeinstallprompt` event
- [ ] App install: create home screen/launcher icon
- [ ] `display: standalone` mode (no browser chrome)
- [ ] Offline capability detection

### Quality Gate 6.8
- [ ] Service Worker-cached page loads offline
- [ ] Web Worker background computation
- [ ] Notification displayed on screen
- [ ] PWA install → standalone launch

---

## Sub-Phase 6.9: Framework Compatibility

> **Goal:** Ensure compatibility with major frontend frameworks' output.  
> **Duration:** 2-3 weeks  
> **Dependencies:** Phase 6.3-6.5

### Target Frameworks
1. **React** (v18+ build output): JSX → createElement, hooks, virtual DOM diffing
2. **Vue 3** (build output): template compilation → virtual DOM
3. **Svelte** (build output): direct DOM manipulation (no virtual DOM)
4. **Alpine.js** (CDN): declarative DOM manipulation
5. **HTMX** (CDN): HTML attribute-driven AJAX (simplest)

### Compatibility Priority Strategy

```
Level 1: HTMX / Alpine.js
  → These are the simplest. Only leverage DOM manipulation and fetch.
  → Supporting these validates the base web API layer.

Level 2: Svelte (build output)
  → Direct DOM manipulation, no virtual DOM overhead.
  → Validates DOM API completeness.

Level 3: Vue 3 (build output)
  → Proxy-based reactivity → need Proxy/Reflect support.
  → Template compiled output (render functions).

Level 4: React 18 (build output)
  → Most complex: concurrent rendering, hooks, fiber architecture.
  → Requires extensive Symbol, WeakRef, queueMicrotask support.
```

### 6.9.1 DOM API Compatibility Matrix

**Checklist:**
- [ ] HTMX test app — attribute-based AJAX
- [ ] Alpine.js test app — x-data, x-on, x-show, x-for
- [ ] Svelte compiled output — DOM operations direct mapping
- [ ] Vue 3 compiled output — createApp, ref, reactive, computed
- [ ] React 18 compiled output — createElement, useState, useEffect, render

### Quality Gate 6.9
- [ ] HTMX TODO app: add/edit/delete tasks
- [ ] Alpine.js counter app: click → number change
- [ ] Svelte build: static page display
- [ ] React build: component rendering (may have warnings)

---

## Sub-Phase 6.10: Integration and Optimization

> **Goal:** Combine all sub-phases and optimize the end-to-end web pipeline.  
> **Duration:** 2 weeks  
> **Dependencies:** Phase 6.1-6.9

### 6.10.1 End-to-End Pipeline

```
URL input → DNS → TCP → TLS → HTTP → HTML parse → CSS parse
→ DOM tree → CSSOM → Style resolution → Layout → Paint → Composite → Display
→ JS execution → DOM mutation → Relayout → Repaint
```

**Checklist:**
- [ ] Full pipeline integration test (URL → visible page)
- [ ] Incremental rendering (display content as it arrives, not after full load)
- [ ] Error handling at each stage (DNS failure, TLS error, parse error, etc.)
- [ ] Loading indicator (progress bar, spinner)

### 6.10.2 Performance Optimization

**Checklist:**
- [ ] CSS selector matching optimization (right-to-left, bloom filter)
- [ ] Layout caching (skip recalc for unchanged subtrees)
- [ ] Paint layer management (reduce full redraw)
- [ ] JS JIT warm-up optimization (boa-engine tuning)
- [ ] Image decode caching
- [ ] DOM access optimization (avoid linear scans)

### 6.10.3 Developer Tools (DevTools)

Already started in `kpio-devtools`:

**Checklist:**
- [ ] Element inspector (DOM tree + computed style)
- [ ] Console (JS console.log output)
- [ ] Network panel (request list, timing, headers, response body)
- [ ] Performance panel (frame timing, layout/paint duration)
- [ ] Source panel (JS source view, breakpoint — stretch)
- [ ] DevTools panel toggle (F12)

### Quality Gate 6.10
- [ ] Static HTML/CSS site: full render (blog template)
- [ ] Dynamic JS page: fetch → DOM update (API client)
- [ ] Page load time < 3s for simple page (on QEMU)
- [ ] DevTools element inspector shows DOM tree

---

## Appendix A: Test Sites by Priority Tier

| Tier | Site | Focus Features |
|------|------|----------------|
| **T0** | Custom static HTML/CSS | Parser/layout basic validation |
| **T1** | `http://example.com` | Basic HTTP + HTML |
| **T1** | `http://info.cern.ch` | World's first website (basic HTML) |
| **T2** | Custom HTMX TODO App | Dynamic AJAX |
| **T2** | Custom Alpine.js App | Declarative reactivity |
| **T3** | Static blog template | CSS Grid + responsive |
| **T3** | Simple canvas game | Canvas API + JS |
| **T4** | Svelte build output | Framework compatibility |
| **T5** | React/Vue build output | Advanced framework |

---

## Appendix B: Crate Dependency Map

```
kpio-browser
  ├── kpio-html       (HTML parser, DOM tree construction)
  ├── kpio-css        (CSS parser, cascade, specificity)
  ├── kpio-dom        (DOM API, events, mutation)
  ├── kpio-js         (JS engine, DOM bindings)
  ├── kpio-layout     (layout engine)
  ├── kpio-devtools   (developer tools)
  ├── kpio-extensions (extension API)
  ├── network         (TCP/IP, DNS, HTTP, TLS)
  ├── graphics        (rendering, framebuffer, compositor)
  └── storage         (VFS, persistent storage)
```

---

## Appendix C: Estimated LOC by Sub-Phase

| Sub-Phase | Estimated LOC | Complexity |
|-----------|--------------|-----------|
| 6.1 Network Foundation | 3,000-4,000 | High |
| 6.2 TLS/HTTPS | 2,000-3,000 | Very High |
| 6.3 JS Engine Enhancement | 3,000-5,000 | Very High |
| 6.4 CSS & Layout | 4,000-6,000 | High |
| 6.5 Web Platform APIs | 5,000-7,000 | Medium-High |
| 6.6 Browser Core | 2,000-3,000 | Medium |
| 6.7 Media & Graphics | 3,000-4,000 | Medium-High |
| 6.8 Advanced Web Platform | 2,000-3,000 | Medium |
| 6.9 Framework Compat | 1,000-2,000 | Medium |
| 6.10 Integration | 1,000-2,000 | Medium |
| **Total** | **26,000-39,000** | |

---

## Appendix D: Available `no_std` Crate Candidates

| Purpose | Crate | Note |
|---------|-------|------|
| TLS | `rustls` | no_std fork available |
| Crypto | `ring` / `RustCrypto` | ring may need std; RustCrypto is no_std |
| HTTP parse | `httparse` | no_std, zero-copy |
| URL parse | `url` | needs alloc |
| JSON parse | `serde_json` | needs alloc |
| PNG decode | `png` | needs alloc |
| JPEG decode | `jpeg-decoder` | needs alloc |
| GIF decode | `gif` | needs alloc |
| SVG parse | `svg` / `usvg`† | † needs std |
| DNS | Custom | Must implement |
| MP3 decode | `minimp3` | no_std, C FFI |

---

*This document defines the full scope of web stack implementation for KPIO OS Phase 6. Each sub-phase can be worked on independently, but QA test coverage should increase as phases accumulate.*
