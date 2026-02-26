//! Embedded test programs for Phase 10-5 integration testing.
//!
//! Each constant is a minimal x86_64 flat binary (no ELF headers) that can
//! be written directly to a user-space code page.  The programs use the
//! Linux x86_64 syscall ABI.
//!
//! Source assembly is in `tests/e2e/userspace/*.S`.

/// hello.bin — SYS_WRITE("Hello from Ring 3\n") then SYS_EXIT(0).
///
/// ```asm
/// _start:
///     mov rax, 1            ; SYS_WRITE
///     mov rdi, 1            ; fd = stdout
///     lea rsi, [rip+msg]    ; buffer
///     mov rdx, 18           ; length
///     syscall
///     mov rax, 60           ; SYS_EXIT
///     xor rdi, rdi          ; exit code 0
///     syscall
/// msg: .ascii "Hello from Ring 3\n"
/// ```
///
/// Hand-assembled for x86_64 (AT&T → Intel encoding):
pub const HELLO_PROGRAM: &[u8] = &[
    // mov rax, 1            ; 48 c7 c0 01 00 00 00
    0x48, 0xc7, 0xc0, 0x01, 0x00, 0x00, 0x00,
    // mov rdi, 1            ; 48 c7 c7 01 00 00 00
    0x48, 0xc7, 0xc7, 0x01, 0x00, 0x00, 0x00,
    // lea rsi, [rip+0x15]   ; 48 8d 35 15 00 00 00  (RIP+21 → msg)
    0x48, 0x8d, 0x35, 0x15, 0x00, 0x00, 0x00,
    // mov rdx, 18           ; 48 c7 c2 12 00 00 00
    0x48, 0xc7, 0xc2, 0x12, 0x00, 0x00, 0x00,
    // syscall               ; 0f 05
    0x0f, 0x05,
    // mov rax, 60           ; 48 c7 c0 3c 00 00 00
    0x48, 0xc7, 0xc0, 0x3c, 0x00, 0x00, 0x00,
    // xor rdi, rdi          ; 48 31 ff
    0x48, 0x31, 0xff,
    // syscall               ; 0f 05
    0x0f, 0x05,
    // msg: "Hello from Ring 3\n"
    b'H', b'e', b'l', b'l', b'o', b' ', b'f', b'r', b'o', b'm',
    b' ', b'R', b'i', b'n', b'g', b' ', b'3', b'\n',
];

/// spin.bin — Infinite loop (jmp _start) to test preemption.
///
/// ```asm
/// _start:
///     jmp _start   ; eb fe  (short jump -2)
/// ```
pub const SPIN_PROGRAM: &[u8] = &[
    // jmp $  (short jump to self = EB FE)
    0xeb, 0xfe,
];

/// exit42.bin — Immediately exit with code 42.
///
/// ```asm
/// _start:
///     mov rax, 60   ; SYS_EXIT
///     mov rdi, 42   ; exit code
///     syscall
/// ```
pub const EXIT42_PROGRAM: &[u8] = &[
    // mov rax, 60           ; 48 c7 c0 3c 00 00 00
    0x48, 0xc7, 0xc0, 0x3c, 0x00, 0x00, 0x00,
    // mov rdi, 42           ; 48 c7 c7 2a 00 00 00
    0x48, 0xc7, 0xc7, 0x2a, 0x00, 0x00, 0x00,
    // syscall               ; 0f 05
    0x0f, 0x05,
];
