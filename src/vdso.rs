use crate::Range;
use goblin::elf::*;
use goblin::strtab::Strtab;
use std::error::Error;
use std::fs::{self, File};
use std::os::unix::prelude::FileExt;

#[derive(Debug, PartialEq)]
pub(crate) struct DynSym {
    pub(crate) name: String,
    pub(crate) address: u64,
    pub(crate) size: u64,
}
#[allow(non_camel_case_types)]
pub(crate) struct vDSO {}

impl vDSO {
    pub(crate) fn read(range: &Range) -> Vec<u8> {
        let mut buf = vec![0; range.end - range.start];
        let f = File::open("/proc/self/mem").unwrap();
        f.read_at(&mut buf, range.start as u64).unwrap();
        drop(f);
        return buf;
    }
    pub(crate) fn find(path: Option<&str>) -> Result<Range, Box<dyn Error>> {
        // could use getauxval(AT_SYSINFO_EHDR)
        // but calculating the length

        let data = fs::read_to_string(path.unwrap_or("/proc/self/maps"))?;

        for line in data.lines() {
            if !line.contains("[vdso]") {
                continue;
            }
            let (range, _) = line.split_once(" ").unwrap();
            let (start, end) = range.split_once("-").unwrap();
            let parts: Vec<&str> = line.split_whitespace().collect();
            let perms = parts[1];
            let r = Range {
                start: usize::from_str_radix(start, 16).unwrap(),
                end: usize::from_str_radix(end, 16).unwrap(),
                writable: perms.contains("w"),
            };
            return Ok(r);
        }
        println!("Map: {}", data);
        return Err("No vDSO mapped in memory range. Cannot continue".into());
    }
    pub(crate) fn dynsyms(buf: Vec<u8>) -> Vec<DynSym> {
        let r = Elf::parse(&buf).expect("bad elf");

        let mut align = 0;
        let mut base = 0;
        for h in r.section_headers {
            let name = get_str_til_nul(&r.shdr_strtab, h.sh_name);
            if h.sh_type == goblin::elf::section_header::SHT_PROGBITS && name == ".text" {
                align = h.sh_addralign;
                base = h.sh_addr - h.sh_offset;
            }
        }
        let mut ret = vec![];
        for ds in &r.dynsyms {
            if ds.st_value == 0 {
                continue;
            }
            let sym_name = get_str_til_nul(&r.dynstrtab, ds.st_name);
            let symsize = if (ds.st_size % align) == 0 {
                ds.st_size
            } else {
                ds.st_size + (align - (ds.st_size % align))
            };
            ret.push(DynSym {
                name: sym_name.as_str().to_string(),
                address: ds.st_value - base,
                size: symsize,
            });
        }
        return ret;
    }
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
        let addr_bytes = jmp_target.to_be_bytes();
        let addr_first_half = vec![addr_bytes[7], addr_bytes[6], addr_bytes[5], addr_bytes[4]];
        let addr_second_half = vec![addr_bytes[3], addr_bytes[2], addr_bytes[1], addr_bytes[0]];
        let nop = vec![0x13, 0x0, 0x0, 0x0];
        let mut opcodes = [
            auipc_t0,
            ld_t0_plus12,
            jr,
            addr_first_half,
            addr_second_half,
        ]
        .concat();
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
        let _a_bytes = jmp_target.to_be_bytes().to_vec();
        let addr_bytes = vec![
            _a_bytes[7],
            _a_bytes[6],
            _a_bytes[5],
            _a_bytes[4],
            _a_bytes[3],
            _a_bytes[2],
            _a_bytes[1],
            _a_bytes[0],
        ];

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
    fn generate_opcodes(jmp_target: usize, symbol_len: usize) -> Vec<u8> {
        Self::_generate_opcodes_riscv64(jmp_target, symbol_len)
    }

    #[cfg(target_arch = "aarch64")]
    fn generate_opcodes(jmp_target: usize, symbol_len: usize) -> Vec<u8> {
        Self::_generate_opcodes_aarch64(jmp_target, symbol_len)
    }

    #[cfg(target_arch = "x86_64")]
    fn generate_opcodes(jmp_target: usize, symbol_len: usize) -> Vec<u8> {
        Self::_generate_opcodes_x86_64(jmp_target, symbol_len)
    }

    /// Overwrites the process' memory at (`range.start + address`) with:
    /// ```asm
    /// mov rax, $jmp_address
    /// jmp rax
    /// nop
    /// nop
    /// nop
    /// nop
    /// ```
    pub(crate) fn overwrite(
        elf_offset: u64,
        symbol_address: u64,
        jmp_address: u64,
        symbol_size: usize,
    ) {
        let dst_addr = elf_offset + symbol_address;
        let opcodes = vDSO::generate_opcodes(jmp_address as usize, symbol_size);
        unsafe {
            for (i, b) in opcodes.iter().enumerate() {
                std::ptr::write_bytes((dst_addr as usize + i) as *mut u8, *b, 1);
            }
        }
    }
    pub(crate) fn restore(elf_offset: u64, symbol_address: u64, opcodes: &[u8]) {
        let dst_addr = elf_offset + symbol_address;
        unsafe {
            for (i, b) in opcodes.iter().enumerate() {
                std::ptr::write_bytes((dst_addr as usize + i) as *mut u8, *b, 1);
            }
        }
    }
    pub(crate) fn read_symbol(elf_offset: u64, symbol_address: u64, len: u64) -> Vec<u8> {
        vec![]
    }
}

fn get_str_til_nul(s: &Strtab, at: usize) -> String {
    let mut ret: String = "".to_string();
    for c in s.get_at(at).unwrap().bytes() {
        if c == 0 {
            break;
        }
        ret.push(c.into());
    }
    return ret;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClockController, TimeSpec};

    #[test]
    fn test_generate_riscv64_opcodes_with_padding() {
        let expected = std::fs::read("tests/files/riscv64_0x12ff34ff56ff78ff_pad_32.bin").unwrap();

        assert_eq!(
            expected,
            vDSO::_generate_opcodes_riscv64(0x12ff34ff56ff78ff, 32)
        );
    }

    #[test]
    fn test_generate_aarch64_opcodes_with_padding() {
        let expected = std::fs::read("tests/files/aarch64_0x12ff34ff56ff78ff_pad_32.bin").unwrap();

        assert_eq!(
            expected,
            vDSO::_generate_opcodes_aarch64(0x12ff34ff56ff78ff, 32)
        );
    }

    #[test]
    fn test_generate_x86_64_opcodes_with_padding() {
        let expected = std::fs::read("tests/files/x86_64_0x12ff34ff56ff78ff_pad_16.bin").unwrap();

        assert_eq!(
            expected,
            vDSO::_generate_opcodes_x86_64(0x12ff34ff56ff78ff, 16)
        );
    }

    #[test]
    fn test_generate_riscv64_opcodes_no_padding() {
        let expected = std::fs::read("tests/files/riscv64_0x12ff34ff56ff78ff.bin").unwrap();

        assert_eq!(
            expected,
            vDSO::_generate_opcodes_riscv64(0x12ff34ff56ff78ff, 12)
        );
    }

    #[test]
    fn test_generate_aarch64_opcodes_no_padding() {
        let expected = std::fs::read("tests/files/aarch64_0x12ff34ff56ff78ff.bin").unwrap();

        assert_eq!(
            expected,
            vDSO::_generate_opcodes_aarch64(0x12ff34ff56ff78ff, 12)
        );
    }

    #[test]
    fn test_generate_x86_64_opcodes_no_padding() {
        let expected = std::fs::read("tests/files/x86_64_0x12ff34ff56ff78ff.bin").unwrap();

        assert_eq!(
            expected,
            vDSO::_generate_opcodes_x86_64(0x12ff34ff56ff78ff, 12)
        );
    }
    #[test]
    fn test_parse_proc_self_maps() {
        let parsed = vDSO::find(Some("src/test_files/proc/self/maps"));
        let expected = Range {
            start: 0x7fff37953000,
            end: 0x7fff37955000,
            writable: false,
        };
        assert_eq!(parsed.is_ok(), true);
        assert_eq!(parsed.unwrap(), expected);
    }
    #[test]
    fn test_stuff() {
        ClockController::overwrite(
            Some(|_| TimeSpec {
                seconds: 1,
                nanos: 1,
            }),
            None,
            None,
            None,
        );
    }
    #[test]
    fn test_dynsyms() {
        let test_vdso =
            fs::read("src/test_files/test_vdso_elf_1").expect("Unable to read test file");
        let parsed = vDSO::dynsyms(test_vdso);
        let expected = vec![
            DynSym {
                name: "clock_gettime".to_string(),
                address: 3088,
                size: 16,
            },
            DynSym {
                name: "__vdso_gettimeofday".to_string(),
                address: 3024,
                size: 16,
            },
            DynSym {
                name: "clock_getres".to_string(),
                address: 3104,
                size: 96,
            },
            DynSym {
                name: "__vdso_clock_getres".to_string(),
                address: 3104,
                size: 96,
            },
            DynSym {
                name: "gettimeofday".to_string(),
                address: 3024,
                size: 16,
            },
            DynSym {
                name: "__vdso_time".to_string(),
                address: 3040,
                size: 48,
            },
            DynSym {
                name: "__vdso_sgx_enter_enclave".to_string(),
                address: 3248,
                size: 160,
            },
            DynSym {
                name: "time".to_string(),
                address: 3040,
                size: 48,
            },
            DynSym {
                name: "__vdso_clock_gettime".to_string(),
                address: 3088,
                size: 16,
            },
            DynSym {
                name: "__vdso_getcpu".to_string(),
                address: 3200,
                size: 48,
            },
            DynSym {
                name: "getcpu".to_string(),
                address: 3200,
                size: 48,
            },
        ];
        assert_eq!(parsed, expected);
    }
    #[test]
    fn test_dynsyms_riscv64() {
        let test_vdso =
            fs::read("src/test_files/test_vdso_elf_2").expect("Unable to read test file");
        let parsed = vDSO::dynsyms(test_vdso);
        let expected = vec![
            DynSym {
                name: "".to_string(),
                address: 1312,
                size: 0,
            },
            DynSym {
                name: "__vdso_gettimeofday".to_string(),
                address: 2330,
                size: 200,
            },
            DynSym {
                name: "__vdso_clock_getres".to_string(),
                address: 2530,
                size: 92,
            },
            DynSym {
                name: "__vdso_rt_sigreturn".to_string(),
                address: 2048,
                size: 8,
            },
            DynSym {
                name: "__vdso_clock_gettime".to_string(),
                address: 2058,
                size: 272,
            },
            DynSym {
                name: "__vdso_flush_icache".to_string(),
                address: 2632,
                size: 12,
            },
            DynSym {
                name: "__vdso_getcpu".to_string(),
                address: 2620,
                size: 12,
            },
        ];
        assert_eq!(parsed, expected);
    }
}
