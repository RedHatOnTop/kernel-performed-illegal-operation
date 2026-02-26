# tests/e2e/userspace/fork_test.S
#
# Minimal x86_64 Linux user-space program that tests fork + waitpid:
#   1. Calls SYS_FORK
#   2. Child writes "child\n" and exits with code 42
#   3. Parent calls SYS_WAIT4 for the child, then writes "parent done\n"
#      and exits with code 0
#
# Assembled machine code is embedded in the kernel as a byte array
# (see kernel/src/process/test_programs.rs).
#
# Build (reference only):
#   nasm -f bin -o fork_test.bin fork_test.S

.global _start
_start:
    mov rax, 57         # SYS_FORK
    syscall
    test rax, rax
    jz .child

    # parent: wait4(child_pid, &status, 0, NULL)
    mov rdi, rax        # pid = child PID (from fork return)
    sub rsp, 8          # allocate space for wstatus
    mov rsi, rsp        # wstatus pointer
    xor rdx, rdx        # options = 0
    xor r10, r10        # rusage = NULL
    mov rax, 61         # SYS_WAIT4
    syscall

    # write "parent done\n"
    mov rax, 1          # SYS_WRITE
    mov rdi, 1          # stdout
    lea rsi, [rip+parent_msg]
    mov rdx, 12         # length
    syscall

    mov rax, 60         # SYS_EXIT
    xor rdi, rdi        # exit code 0
    syscall

.child:
    # write "child\n"
    mov rax, 1          # SYS_WRITE
    mov rdi, 1          # stdout
    lea rsi, [rip+child_msg]
    mov rdx, 6          # length
    syscall

    mov rax, 60         # SYS_EXIT
    mov rdi, 42         # exit code 42
    syscall

child_msg:
    .ascii "child\n"
parent_msg:
    .ascii "parent done\n"
