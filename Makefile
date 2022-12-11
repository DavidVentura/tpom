.PHONY: regenerate_bin

regenerate_bin: $(shell find tests/files/ -name "*.bin") Makefile
	:

tests/files/x86_64_0x12ff34ff56ff78ff.bin: tests/files/x86_64_0x12ff34ff56ff78ff.asm
	cd tests/files && nasm -f elf64 x86_64_0x12ff34ff56ff78ff.asm
	cd tests/files && objcopy -O binary --only-section=.text x86_64_0x12ff34ff56ff78ff.o x86_64_0x12ff34ff56ff78ff.bin

tests/files/aarch64_0x12ff34ff56ff78ff.bin: tests/files/aarch64_0x12ff34ff56ff78ff.asm
	cd tests/files && aarch64-linux-gnu-as -o aarch64_0x12ff34ff56ff78ff.o aarch64_0x12ff34ff56ff78ff.asm
	cd tests/files && aarch64-linux-gnu-objcopy -O binary --only-section=.text aarch64_0x12ff34ff56ff78ff.o aarch64_0x12ff34ff56ff78ff.bin

tests/files/riscv64_0x12ff34ff56ff78ff.bin: tests/files/riscv64_0x12ff34ff56ff78ff.asm
	cd tests/files && riscv64-linux-gnu-as -o riscv64_0x12ff34ff56ff78ff.o riscv64_0x12ff34ff56ff78ff.asm
	cd tests/files && riscv64-linux-gnu-objcopy -O binary --only-section=.text riscv64_0x12ff34ff56ff78ff.o riscv64_0x12ff34ff56ff78ff.bin

tests/files/x86_64_0x12ff34ff56ff78ff_pad_16.bin: tests/files/x86_64_0x12ff34ff56ff78ff_pad_16.asm
	cd tests/files && nasm -f elf64 x86_64_0x12ff34ff56ff78ff_pad_16.asm
	cd tests/files && objcopy -O binary --only-section=.text x86_64_0x12ff34ff56ff78ff_pad_16.o x86_64_0x12ff34ff56ff78ff_pad_16.bin

tests/files/aarch64_0x12ff34ff56ff78ff_pad_32.bin: tests/files/aarch64_0x12ff34ff56ff78ff_pad_32.asm
	cd tests/files && aarch64-linux-gnu-as -o aarch64_0x12ff34ff56ff78ff_pad_32.o aarch64_0x12ff34ff56ff78ff_pad_32.asm
	cd tests/files && aarch64-linux-gnu-objcopy -O binary --only-section=.text aarch64_0x12ff34ff56ff78ff_pad_32.o aarch64_0x12ff34ff56ff78ff_pad_32.bin

tests/files/riscv64_0x12ff34ff56ff78ff_pad_32.bin: tests/files/riscv64_0x12ff34ff56ff78ff_pad_32.asm
	cd tests/files && riscv64-linux-gnu-as -o riscv64_0x12ff34ff56ff78ff_pad_32.o riscv64_0x12ff34ff56ff78ff_pad_32.asm
	cd tests/files && riscv64-linux-gnu-objcopy -O binary --only-section=.text riscv64_0x12ff34ff56ff78ff_pad_32.o riscv64_0x12ff34ff56ff78ff_pad_32.bin
