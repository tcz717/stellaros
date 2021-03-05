.globl _start
.extern LD_STACK_PTR

.section ".text._start"

_start:
    ldr     x30, =LD_STACK_PTR
    mov     sp, x30
    bl      start
