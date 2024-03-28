.text

.globl _start
_start:
    LDR    x0, .+16
    BR     x0
    NOP
    NOP
.dword 0x12ff34ff56ff78ff
