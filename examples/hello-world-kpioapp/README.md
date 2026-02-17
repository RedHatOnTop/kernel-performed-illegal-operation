# Hello World KPIO App

A minimal `.kpioapp` example that writes "Hello, KPIO!" to stdout.

## manifest.toml

The manifest declares the app's identity, version, and required permissions.

## Building

```bash
# From the KPIO project root:
cargo build -p hello-world --target wasm32-wasip2 --release
```

## Packaging

The `.kpioapp` format is a ZIP file containing:
- `manifest.toml` — App metadata
- `app.wasm` — WASM binary

## Running

```bash
# Using the KPIO runtime:
kpio-run hello-world.kpioapp
```
