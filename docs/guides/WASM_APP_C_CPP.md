# Building WASM Apps with C/C++ for KPIO

This guide covers compiling C and C++ programs to WebAssembly targeting
the KPIO operating system using `wasi-sdk`.

## Prerequisites

### Install wasi-sdk

Download from https://github.com/WebAssembly/wasi-sdk/releases

```bash
# Linux / macOS
export WASI_SDK_PATH=/opt/wasi-sdk
wget https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-24/wasi-sdk-24.0-x86_64-linux.tar.gz
tar xf wasi-sdk-24.0-x86_64-linux.tar.gz -C /opt/
```

```powershell
# Windows 
# Download wasi-sdk-24.0-x86_64-windows.tar.gz and extract to C:\wasi-sdk
$env:WASI_SDK_PATH = "C:\wasi-sdk"
```

Verify installation:
```bash
$WASI_SDK_PATH/bin/clang --version
# Should show: clang version 18.x.x (wasi-sdk)
```

## Hello World (C)

### 1. Write the source

```c
// hello.c
#include <stdio.h>

int main(void) {
    printf("Hello, KPIO!\n");
    return 0;
}
```

### 2. Compile to WASM

```bash
$WASI_SDK_PATH/bin/clang \
    --target=wasm32-wasip2 \
    -o app.wasm \
    hello.c
```

### 3. Package and run

```bash
# Create manifest.toml (see Rust guide for format)
zip -0 hello.kpioapp manifest.toml app.wasm
kpio-run hello.kpioapp
```

## Hello World (C++)

```cpp
// hello.cpp
#include <iostream>

int main() {
    std::cout << "Hello, KPIO from C++!" << std::endl;
    return 0;
}
```

```bash
$WASI_SDK_PATH/bin/clang++ \
    --target=wasm32-wasip2 \
    -o app.wasm \
    hello.cpp
```

## CMake Toolchain File

For larger projects, use a CMake toolchain file:

```cmake
# wasi-toolchain.cmake
set(CMAKE_SYSTEM_NAME WASI)
set(CMAKE_SYSTEM_PROCESSOR wasm32)

set(WASI_SDK_PATH $ENV{WASI_SDK_PATH})
set(CMAKE_C_COMPILER "${WASI_SDK_PATH}/bin/clang")
set(CMAKE_CXX_COMPILER "${WASI_SDK_PATH}/bin/clang++")
set(CMAKE_AR "${WASI_SDK_PATH}/bin/llvm-ar")
set(CMAKE_RANLIB "${WASI_SDK_PATH}/bin/llvm-ranlib")

set(CMAKE_C_COMPILER_TARGET wasm32-wasip2)
set(CMAKE_CXX_COMPILER_TARGET wasm32-wasip2)

set(CMAKE_SYSROOT "${WASI_SDK_PATH}/share/wasi-sysroot")

set(CMAKE_C_FLAGS_INIT "-fno-exceptions")
set(CMAKE_CXX_FLAGS_INIT "-fno-exceptions -fno-rtti")

set(CMAKE_EXE_LINKER_FLAGS_INIT "-Wl,--export=_start")

set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_PACKAGE ONLY)
```

### Using the toolchain:

```bash
mkdir build && cd build
cmake -DCMAKE_TOOLCHAIN_FILE=../wasi-toolchain.cmake ..
make
```

## Linking with KPIO Host Functions

To call KPIO-specific host functions from C, declare them as imported:

```c
// kpio_host.h — declarations for KPIO host imports

// These are resolved by the KPIO runtime at instantiation time.
// They correspond to WIT interface functions.

// wasi:clocks/monotonic-clock
__attribute__((import_module("wasi:clocks/monotonic-clock@0.2.0")))
__attribute__((import_name("now")))
extern unsigned long long wasi_clock_now(void);

// wasi:random/random
__attribute__((import_module("wasi:random/random@0.2.0")))
__attribute__((import_name("get-random-u64")))
extern unsigned long long wasi_random_u64(void);
```

```c
// app.c
#include <stdio.h>
#include "kpio_host.h"

int main(void) {
    unsigned long long now = wasi_clock_now();
    unsigned long long rand = wasi_random_u64();
    printf("Clock: %llu, Random: %llu\n", now, rand);
    return 0;
}
```

## Known Limitations

1. **No dynamic linking**: WASM modules are statically linked. Shared
   libraries (`.so`/`.dll`) are not supported.

2. **No threads**: `pthread` is not available. Use single-threaded designs.
   WASI threads proposal is not yet supported.

3. **No signals**: Unix signals (`SIGINT`, `SIGTERM`, etc.) are not
   available in the WASM environment.

4. **Limited POSIX**: Only POSIX functions mapped through WASI are
   available. See the [POSIX Shim](../../runtime/src/posix_shim.rs) for
   the mapping table.

5. **Store-only ZIP**: The `.kpioapp` ZIP format currently requires
   Store compression (no Deflate). Use `zip -0` when packaging.

6. **No C++ exceptions**: Exception handling support in WASM is
   experimental. Use `-fno-exceptions` for now.

7. **Binary size**: C++ iostream can add ~100KB. Consider using `printf`
   for smaller binaries.

8. **Filesystem sandboxing**: File access is restricted to the app's data
   directory unless additional permissions are granted.

## Optimizing Binary Size

```bash
# Use -Oz for size optimization
$WASI_SDK_PATH/bin/clang \
    --target=wasm32-wasip2 \
    -Oz \
    -o app.wasm \
    hello.c

# Strip debug info
$WASI_SDK_PATH/bin/llvm-strip app.wasm

# Use wasm-opt for further optimization (install from binaryen)
wasm-opt -Oz -o app-opt.wasm app.wasm
```

## Next Steps

- See [Rust Guide](WASM_APP_RUST.md) for Rust-based development.
- See [API Reference](KPIO_APP_API_REFERENCE.md) for available interfaces.
