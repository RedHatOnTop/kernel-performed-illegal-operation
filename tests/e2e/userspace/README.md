# Phase 10-5 Test Userspace Programs

This directory contains minimal x86_64 Linux-ABI user-space programs used for
kernel end-to-end testing (Phase 10-5).

## Programs

| File | Description | Exit Code |
|------|-------------|-----------|
| `hello.S` | Writes "Hello from Ring 3\n" to stdout, exits 0 | 0 |
| `fork_test.S` | Forks, child exits 42, parent waitpid's then exits 0 | 0 |
| `spin.S` | Infinite loop — tests preemptive scheduling (killed by kernel) | N/A |

## Usage

These programs are **not assembled as part of the build**. Instead, their
machine code is hand-assembled and embedded as byte arrays in
`kernel/src/process/test_programs.rs`.

The assembly files here serve as source-of-truth documentation for what the
byte arrays represent.

## Encoding Reference

To verify or regenerate the machine code byte arrays:

```bash
# Assemble (requires nasm or GNU as)
nasm -f bin -o hello.bin hello.S
xxd -i hello.bin
```

## ABI

All programs use the **Linux x86_64 syscall ABI**:
- `RAX` = syscall number
- `RDI`, `RSI`, `RDX`, `R10`, `R8`, `R9` = arguments 1–6
- `SYSCALL` instruction invokes the kernel
- Return value in `RAX`
