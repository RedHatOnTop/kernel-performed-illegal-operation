# KPIO App API Reference

Complete reference of available interfaces for KPIO WASM applications.

## WASI Preview 2 Interfaces

### wasi:clocks/monotonic-clock@0.2.0

Monotonic clock for measuring elapsed time.

| Function | Params | Returns | Description |
|----------|--------|---------|-------------|
| `now` | — | `u64` | Current monotonic timestamp in nanoseconds |
| `resolution` | — | `u64` | Clock resolution in nanoseconds |
| `subscribe-instant` | `instant: u64` | `pollable` | Subscribe to a specific instant |
| `subscribe-duration` | `duration: u64` | `pollable` | Subscribe to a duration from now |

### wasi:clocks/wall-clock@0.2.0

Wall clock for real-world time.

| Function | Params | Returns | Description |
|----------|--------|---------|-------------|
| `now` | — | `datetime {seconds: u64, nanoseconds: u32}` | Current wall clock time |
| `resolution` | — | `datetime` | Clock resolution |

### wasi:random/random@0.2.0

Cryptographically secure random number generation.

| Function | Params | Returns | Description |
|----------|--------|---------|-------------|
| `get-random-bytes` | `len: u64` | `list<u8>` | Generate `len` random bytes |
| `get-random-u64` | — | `u64` | Generate a random 64-bit integer |
| `insecure-random` | — | `(u64, u64)` | Non-crypto PRNG seed pair |

### wasi:io/streams@0.2.0

Byte stream I/O.

| Function | Params | Returns | Description |
|----------|--------|---------|-------------|
| `read` | `stream: input-stream, len: u64` | `result<list<u8>, stream-error>` | Read up to `len` bytes |
| `write` | `stream: output-stream, data: list<u8>` | `result<u64, stream-error>` | Write bytes, return count written |
| `flush` | `stream: output-stream` | `result<_, stream-error>` | Flush buffered output |
| `subscribe` | `stream: input-stream` | `pollable` | Subscribe to stream readiness |

### wasi:io/poll@0.2.0

Async polling mechanism.

| Function | Params | Returns | Description |
|----------|--------|---------|-------------|
| `poll` | `pollables: list<pollable>` | `list<u32>` | Wait for any pollable to be ready |

### wasi:filesystem/types@0.2.0

Filesystem types and operations.

| Function | Params | Returns | Description |
|----------|--------|---------|-------------|
| `read-via-stream` | `descriptor, offset: u64` | `result<input-stream, error-code>` | Open file for reading |
| `write-via-stream` | `descriptor, offset: u64` | `result<output-stream, error-code>` | Open file for writing |
| `stat` | `descriptor` | `result<descriptor-stat, error-code>` | Get file metadata |
| `read-directory` | `descriptor` | `result<directory-entry-stream, error-code>` | List directory |
| `open-at` | `descriptor, path, flags` | `result<descriptor, error-code>` | Open relative path |
| `create-directory-at` | `descriptor, path` | `result<_, error-code>` | Create directory |
| `unlink-file-at` | `descriptor, path` | `result<_, error-code>` | Delete file |

### wasi:cli/environment@0.2.0

Command-line environment.

| Function | Params | Returns | Description |
|----------|--------|---------|-------------|
| `get-environment` | — | `list<(string, string)>` | Environment variables |
| `get-arguments` | — | `list<string>` | Command-line arguments |

### wasi:cli/stdout@0.2.0 / stderr@0.2.0

Standard output/error streams.

| Function | Params | Returns | Description |
|----------|--------|---------|-------------|
| `get-stdout` | — | `output-stream` | Get stdout stream handle |
| `get-stderr` | — | `output-stream` | Get stderr stream handle |

### wasi:sockets/tcp@0.2.0

TCP networking.

| Function | Params | Returns | Description |
|----------|--------|---------|-------------|
| `create-tcp-socket` | `address-family` | `result<tcp-socket, error-code>` | Create TCP socket |
| `bind` | `socket, address` | `result<_, error-code>` | Bind to address |
| `connect` | `socket, address` | `result<(input-stream, output-stream), error-code>` | Connect to remote |
| `listen` | `socket` | `result<_, error-code>` | Start listening |
| `accept` | `socket` | `result<(tcp-socket, input-stream, output-stream), error-code>` | Accept connection |

### wasi:sockets/udp@0.2.0

UDP networking.

| Function | Params | Returns | Description |
|----------|--------|---------|-------------|
| `create-udp-socket` | `address-family` | `result<udp-socket, error-code>` | Create UDP socket |
| `bind` | `socket, address` | `result<_, error-code>` | Bind to address |
| `send` | `socket, datagrams` | `result<u64, error-code>` | Send datagrams |
| `receive` | `socket, max-results` | `result<list<datagram>, error-code>` | Receive datagrams |

### wasi:http/outgoing-handler@0.2.0

HTTP client requests.

| Function | Params | Returns | Description |
|----------|--------|---------|-------------|
| `handle` | `outgoing-request, options` | `result<future-incoming-response, error-code>` | Send HTTP request |

## KPIO-Specific Interfaces

### kpio:gui/window

Window management (requires `gui = true` permission).

| Function | Params | Returns | Description |
|----------|--------|---------|-------------|
| `create-window` | `title: string, width: u32, height: u32` | `window-handle` | Create a window |
| `set-title` | `handle, title: string` | — | Set window title |
| `close-window` | `handle` | — | Close window |
| `poll-event` | `handle` | `option<event>` | Poll for input events |
| `request-redraw` | `handle` | — | Request repaint |

### kpio:gui/canvas

2D drawing (requires `gui = true` permission).

| Function | Params | Returns | Description |
|----------|--------|---------|-------------|
| `fill-rect` | `ctx, x, y, w, h, color` | — | Fill rectangle |
| `stroke-rect` | `ctx, x, y, w, h, color, width` | — | Stroke rectangle |
| `draw-text` | `ctx, text, x, y, size, color` | — | Draw text |
| `draw-line` | `ctx, x1, y1, x2, y2, color, width` | — | Draw line |
| `draw-image` | `ctx, data, x, y, w, h` | — | Draw image data |

### kpio:system/clipboard

Clipboard access (requires `clipboard = true` permission).

| Function | Params | Returns | Description |
|----------|--------|---------|-------------|
| `read-text` | — | `option<string>` | Read clipboard text |
| `write-text` | `text: string` | — | Write text to clipboard |

### kpio:system/notifications

System notifications (requires `notifications = true` permission).

| Function | Params | Returns | Description |
|----------|--------|---------|-------------|
| `show-notification` | `title: string, body: string` | `notification-id` | Show notification |
| `dismiss-notification` | `id: notification-id` | — | Dismiss notification |

### kpio:net/http

HTTP client (requires `network = true` permission).

| Function | Params | Returns | Description |
|----------|--------|---------|-------------|
| `fetch` | `url: string, options: request-options` | `result<response, error>` | HTTP fetch |

## Permission Model

Apps declare required permissions in `manifest.toml`. The KPIO runtime
enforces these at instantiation time — if an app tries to import an
interface it doesn't have permission for, instantiation fails.

| Permission | Grants Access To |
|-----------|-----------------|
| `filesystem = "none"` | App-private data directory only |
| `filesystem = "read-only"` | + User-selected files (read) |
| `filesystem = "read-write"` | + User-selected files (read/write) |
| `network = true` | TCP, UDP, HTTP interfaces |
| `gui = true` | Window, Canvas, Input interfaces |
| `clipboard = true` | Clipboard read/write |
| `notifications = true` | System notifications |

## Resource Limits

| Resource | Default Limit | Configuration |
|----------|---------------|---------------|
| Execution fuel | 100,000,000 | `LaunchConfig.fuel` |
| Memory pages | 256 (16 MB) | `RuntimeConfig.max_memory_pages` |
| Table size | 10,000 | `RuntimeConfig.max_table_size` |
| Stack size | 1 MB | `RuntimeConfig.stack_size` |
| Package size | 50 MB | `MAX_PACKAGE_SIZE` |

When a resource limit is exceeded, the runtime returns a `ResourceLimit`
error and the app is terminated.
