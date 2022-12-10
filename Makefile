.PHONY: regenerate_asm

regenerate_asm: tests/files/x86_64_0x12ff34ff56ff78ff.bin tests/files/aarch64_0x12ff34ff56ff78ff.bin tests/files/riscv64_0x12ff34ff56ff78ff.bin
	:

tests/files/x86_64_0x12ff34ff56ff78ff.bin:
	cd tests/files && nasm -f elf64 x86_64_0x12ff34ff56ff78ff.asm
	cd tests/files && objcopy -O binary --only-section=.text x86_64_0x12ff34ff56ff78ff.o x86_64_0x12ff34ff56ff78ff.bin

tests/files/aarch64_0x12ff34ff56ff78ff.bin:
	cd tests/files && aarch64-linux-gnu-as -o aarch64_0x12ff34ff56ff78ff.o aarch64_0x12ff34ff56ff78ff.asm
	cd tests/files && aarch64-linux-gnu-objcopy -O binary --only-section=.text aarch64_0x12ff34ff56ff78ff.o aarch64_0x12ff34ff56ff78ff.bin

tests/files/riscv64_0x12ff34ff56ff78ff.bin:
	cd tests/files && riscv64-linux-gnu-as -o riscv64_0x12ff34ff56ff78ff.o riscv64_0x12ff34ff56ff78ff.asm
	cd tests/files && riscv64-linux-gnu-objcopy -O binary --only-section=.text riscv64_0x12ff34ff56ff78ff.o riscv64_0x12ff34ff56ff78ff.bin
