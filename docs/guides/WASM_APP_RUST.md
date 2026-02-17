# Building WASM Apps with Rust for KPIO

This guide walks through building a WebAssembly application targeting the
KPIO operating system using Rust and WASI Preview 2.

## Prerequisites

- Rust nightly toolchain
- `wasm32-wasip2` target (or `wasm32-wasip1` for legacy support)

```bash
rustup toolchain install nightly
rustup target add wasm32-wasip2 --toolchain nightly
# or for WASI P1:
rustup target add wasm32-wasi --toolchain nightly
```

## Hello World

### 1. Create the project

```bash
cargo new --bin hello-kpio
cd hello-kpio
```

### 2. Write the source

```rust
// src/main.rs

fn main() {
    println!("Hello, KPIO!");
}
```

### 3. Build for WASM

```bash
cargo +nightly build --target wasm32-wasip2 --release
```

The compiled binary will be at:
```
target/wasm32-wasip2/release/hello-kpio.wasm
```

### 4. Create the manifest

Create `manifest.toml`:

```toml
[app]
id = "com.yourname.hello-kpio"
name = "Hello KPIO"
version = "1.0.0"
description = "My first KPIO app"
author = "Your Name"
entry = "app.wasm"

[permissions]
filesystem = "none"
network = false
gui = false
```

### 5. Package as `.kpioapp`

A `.kpioapp` file is a ZIP archive:

```bash
# Copy the WASM binary
cp target/wasm32-wasip2/release/hello-kpio.wasm app.wasm

# Create the package (Store compression, no deflate)
zip -0 hello-kpio.kpioapp manifest.toml app.wasm
```

### 6. Run on KPIO

```bash
# The KPIO runtime loads and executes the package:
kpio-run hello-kpio.kpioapp
# Output: Hello, KPIO!
```

## Using WASI Interfaces

KPIO supports WASI Preview 2 interfaces. Your Rust code can use
standard library functions that map to WASI:

```rust
use std::fs;
use std::io::{self, Read, Write};

fn main() {
    // stdout (wasi:cli/stdout)
    println!("Writing to stdout");

    // stderr (wasi:cli/stderr)
    eprintln!("Writing to stderr");

    // Environment (wasi:cli/environment)
    for (key, val) in std::env::vars() {
        println!("{}={}", key, val);
    }

    // Arguments (wasi:cli/environment)
    for arg in std::env::args() {
        println!("arg: {}", arg);
    }

    // Random (wasi:random/random)  
    // Use getrandom crate or std::collections::hash_map::RandomState
}
```

## Using kpio:gui Bindings

For GUI apps, declare a dependency on the KPIO GUI WIT bindings:

```rust
// These are stub declarations — the actual imports are resolved by the
// KPIO runtime at instantiation time.

// In your Cargo.toml, no extra dependency needed for WIT imports.
// The runtime provides these automatically when gui=true in manifest.

fn main() {
    // GUI apps typically use the kpio:gui/window interface.
    // Example (pseudo-code, actual API depends on WIT bindings):
    //
    // let window = kpio_gui::create_window("My App", 800, 600);
    // kpio_gui::set_title(&window, "Hello GUI");
    // loop {
    //     let event = kpio_gui::poll_event();
    //     match event {
    //         Event::Close => break,
    //         Event::Paint(ctx) => {
    //             ctx.fill_rect(0, 0, 100, 100, Color::RED);
    //         }
    //         _ => {}
    //     }
    // }

    println!("GUI app placeholder — actual GUI bindings TBD");
}
```

## Permissions

The `[permissions]` section in `manifest.toml` controls what WASI
interfaces are available to your app:

| Permission | WASI Interface | Description |
|-----------|----------------|-------------|
| `filesystem = "none"` | — | No filesystem access |
| `filesystem = "read-only"` | `wasi:filesystem` | Read user-selected files |
| `filesystem = "read-write"` | `wasi:filesystem` | Read/write user-selected files |
| `network = true` | `wasi:sockets`, `wasi:http` | TCP/UDP/HTTP access |
| `gui = true` | `kpio:gui` | Window creation and rendering |
| `clipboard = true` | `kpio:system` | Clipboard read/write |
| `notifications = true` | `kpio:system` | System notifications |

## Debugging Tips

1. **Check WASM validity**: Use `wasm-tools validate app.wasm` to verify
   the binary is well-formed.

2. **Print debugging**: `println!()` and `eprintln!()` work through WASI
   stdout/stderr streams captured by the KPIO runtime.

3. **Fuel limits**: By default, KPIO limits execution to 100M fuel units.
   If your app runs out, it terminates with a `ResourceLimit` error.
   Request more fuel via `LaunchConfig::fuel`.

4. **Binary size**: Use release builds with LTO and `opt-level = "z"`:
   ```toml
   [profile.release]
   lto = true
   opt-level = "z"
   strip = true
   ```

5. **Inspect exports**: Use `wasm-tools dump app.wasm | grep export` to
   verify that `_start` is exported (required for WASI apps).

## Project Structure

```
my-kpio-app/
├── Cargo.toml
├── manifest.toml         # KPIO app manifest
├── src/
│   └── main.rs           # Rust source
├── resources/            # Optional: icons, assets
│   └── icon-192.png
└── target/
    └── wasm32-wasip2/
        └── release/
            └── my-kpio-app.wasm
```

## Next Steps

- See [KPIO App API Reference](KPIO_APP_API_REFERENCE.md) for the full
  list of available interfaces and functions.
- See [C/C++ Guide](WASM_APP_C_CPP.md) for building apps with C/C++.
