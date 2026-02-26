# tests/e2e/userspace/hello.S
#
# Minimal x86_64 Linux user-space program: writes "Hello from Ring 3\n"
# to stdout (fd 1) via SYS_WRITE, then exits with code 0 via SYS_EXIT.
#
# Assembled machine code is embedded in the kernel as a byte array
# (see kernel/src/process/test_programs.rs).
#
# Build (reference only — not part of kernel build):
#   nasm -f bin -o hello.bin hello.S
#
# Encoding:
#   mov rax, 1          ; SYS_WRITE = 1
#   mov rdi, 1          ; fd = stdout
#   lea rsi, [rip+msg]  ; buffer = msg
#   mov rdx, 18         ; length = 18
#   syscall             ; invoke kernel
#   mov rax, 60         ; SYS_EXIT = 60
#   xor rdi, rdi        ; exit code = 0
#   syscall             ; invoke kernel
# msg: .ascii "Hello from Ring 3\n"

.global _start
_start:
    mov rax, 1          # SYS_WRITE
    mov rdi, 1          # stdout
    lea rsi, [rip+msg]  # buffer
    mov rdx, 18         # length
    syscall
    mov rax, 60         # SYS_EXIT
    xor rdi, rdi        # exit code 0
    syscall
msg:
    .ascii "Hello from Ring 3\n"
