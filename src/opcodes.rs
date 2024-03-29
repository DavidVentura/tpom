fn _generate_opcodes_riscv64(jmp_target: usize, symbol_len: usize) -> Vec<u8> {
    /*
          0:   00000297                auipc   t0,0x0
          4:   00c2b303                ld      t1,12(t0) # c <_start+0xc>
          8:   00030067                jr      t1
          c:   56ff78ff                .word   0x56ff78ff
         10:   12ff34ff                .word   0x12ff34ff
         14:   00000013                nop
         18:   00000013                nop
         1c:   00000013                nop
    */
    let auipc_t0 = vec![0x97, 0x02, 0x00, 0x00]; // store PC at t0
    let ld_t0_plus12 = vec![0x03, 0xb3, 0xc2, 0x00]; // load PC+12 into t1
    let jr = vec![0x67, 0x00, 0x03, 0x00]; // jump to T1
    let addr_bytes = jmp_target.to_le_bytes().to_vec();

    let nop = vec![0x13, 0x0, 0x0, 0x0];
    let mut opcodes = [auipc_t0, ld_t0_plus12, jr, addr_bytes].concat();
    while symbol_len > opcodes.len() {
        opcodes.append(&mut nop.clone());
    }
    opcodes
}
fn _generate_opcodes_aarch64(jmp_target: usize, symbol_len: usize) -> Vec<u8> {
    /* These opcodes come from running `nasm -f elf64` on
    ```
    .text

    .globl _start
    _start:
        LDR    x0, .+8
        BR     x0
    .dword 0x12ff34ff56ff78ff
        NOP
        NOP
        NOP
    ```
    which becomes
    ```
    0000000000000000 <_start>:
       0:	58000040 	ldr	x0, 8 <_start+0x8>
       4:	d61f0000 	br	x0
       8:	56ff78ff 	.word	0x56ff78ff
       c:	12ff34ff 	.word	0x12ff34ff
      10:	d503201f 	nop
      14:	d503201f 	nop
      18:	d503201f 	nop
    ```
    */
    let addr_bytes = jmp_target.to_le_bytes().to_vec();

    let ldr_x0_8 = vec![0x40, 0x00, 0x00, 0x58];
    let br_x0 = vec![0x00, 0x00, 0x1f, 0xd6];
    let nop = vec![0x1f, 0x20, 0x03, 0xd5];

    let mut opcodes = [ldr_x0_8, br_x0, addr_bytes].concat();
    while symbol_len > opcodes.len() {
        opcodes.append(&mut nop.clone());
    }
    opcodes
}
fn _generate_opcodes_x86_64(jmp_target: usize, symbol_len: usize) -> Vec<u8> {
    /* These opcodes come from running `nasm -f elf64` on
      ```
           global  _start
           section .text
       _start:
           mov		rax, 0x12ff34ff56ff78ff
           jmp 		rax
      ```
      and copying them
    */
    let mut addr_bytes = jmp_target.to_le_bytes().to_vec();

    // MOV RAX, <addr>
    let mut opcodes: Vec<u8> = vec![0x48, 0xB8];
    opcodes.append(&mut addr_bytes);
    // JMP
    opcodes.append(&mut vec![0xFF, 0xE0]);
    // NOP
    assert!(symbol_len >= opcodes.len());
    let padding_size = symbol_len - opcodes.len();
    let mut nops = vec![0x90u8; padding_size];
    opcodes.append(&mut nops);

    opcodes
}
#[cfg(target_arch = "riscv64")]
pub(crate) fn generate_opcodes(jmp_target: usize, symbol_len: usize) -> Vec<u8> {
    _generate_opcodes_riscv64(jmp_target, symbol_len)
}

#[cfg(target_arch = "aarch64")]
pub(crate) fn generate_opcodes(jmp_target: usize, symbol_len: usize) -> Vec<u8> {
    _generate_opcodes_aarch64(jmp_target, symbol_len)
}

#[cfg(target_arch = "x86_64")]
pub(crate) fn generate_opcodes(jmp_target: usize, symbol_len: usize) -> Vec<u8> {
    _generate_opcodes_x86_64(jmp_target, symbol_len)
}
#[cfg(test)]
mod tests {
    use crate::opcodes::*;

    #[test]
    fn test_generate_riscv64_opcodes_with_padding() {
        let expected = std::fs::read("tests/files/riscv64_0x12ff34ff56ff78ff_pad_32.bin").unwrap();

        assert_eq!(expected, _generate_opcodes_riscv64(0x12ff34ff56ff78ff, 32));
    }

    #[test]
    fn test_generate_aarch64_opcodes_with_padding() {
        let expected = std::fs::read("tests/files/aarch64_0x12ff34ff56ff78ff_pad_32.bin").unwrap();

        assert_eq!(expected, _generate_opcodes_aarch64(0x12ff34ff56ff78ff, 32));
    }

    #[test]
    fn test_generate_x86_64_opcodes_with_padding() {
        let expected = std::fs::read("tests/files/x86_64_0x12ff34ff56ff78ff_pad_16.bin").unwrap();

        assert_eq!(expected, _generate_opcodes_x86_64(0x12ff34ff56ff78ff, 16));
    }

    #[test]
    fn test_generate_riscv64_opcodes_no_padding() {
        let expected = std::fs::read("tests/files/riscv64_0x12ff34ff56ff78ff.bin").unwrap();

        assert_eq!(expected, _generate_opcodes_riscv64(0x12ff34ff56ff78ff, 12));
    }

    #[test]
    fn test_generate_aarch64_opcodes_no_padding() {
        let expected = std::fs::read("tests/files/aarch64_0x12ff34ff56ff78ff.bin").unwrap();

        assert_eq!(expected, _generate_opcodes_aarch64(0x12ff34ff56ff78ff, 12));
    }

    #[test]
    fn test_generate_x86_64_opcodes_no_padding() {
        let expected = std::fs::read("tests/files/x86_64_0x12ff34ff56ff78ff.bin").unwrap();

        assert_eq!(expected, _generate_opcodes_x86_64(0x12ff34ff56ff78ff, 12));
    }
}
