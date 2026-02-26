# tests/e2e/userspace/spin.S
#
# Infinite loop (CPU-bound) user-space program to test preemption.
# This program should never prevent other tasks from running because
# the APIC timer drives preemptive scheduling.
#
# The kernel should be able to kill this process with SIGKILL after
# verifying that preemption works (other tasks continue running).
#
# Assembled machine code is embedded in the kernel as a byte array
# (see kernel/src/process/test_programs.rs).
#
# Build (reference only):
#   nasm -f bin -o spin.bin spin.S

.global _start
_start:
    jmp _start
