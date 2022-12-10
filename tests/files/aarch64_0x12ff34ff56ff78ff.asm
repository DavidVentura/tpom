.text

.globl _start
_start:
    LDR    x0, .+8
    BR     x0
.dword 0x12ff34ff56ff78ff
