.global _start
_start:
    auipc x5, 0x0
    ld    x6, 12(x5)
    jalr  x0, x6
.dword 0x12ff34ff56ff78ff
    nop
    nop
    nop
