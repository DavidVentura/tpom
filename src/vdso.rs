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
        let data = fs::read_to_string(path.unwrap_or("/proc/self/maps"))?;

        for line in data.lines() {
            if !line.contains("vdso") {
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
        let r = Elf::parse(&buf).unwrap();

        let mut ret = vec![];
        for ds in &r.dynsyms {
            if ds.st_value == 0 {
                continue;
            }
            let sym_name = get_str_til_nul(&r.dynstrtab, ds.st_name);
            ret.push(DynSym {
                name: sym_name.as_str().to_string(),
                address: ds.st_value,
                size: ds.st_size,
            });
        }
        return ret;
    }

    #[cfg(target_arch = "aarch64")]
    fn generate_opcodes(jmp_target: usize, symbol_len: usize) -> Vec<u8> {
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

        [
            ldr_x0_8,
            br_x0,
            addr_bytes,
            nop.clone(),
            nop.clone(),
            nop.clone(),
        ]
        .concat()
    }

    #[cfg(target_arch = "x86_64")]
    fn generate_opcodes(jmp_target: usize, symbol_len: usize) -> Vec<u8> {
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
        let padding_size = std::cmp::max(16, symbol_len) - opcodes.len();
        let mut nops = vec![0x90u8; padding_size];
        opcodes.append(&mut nops);

        opcodes
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

#[allow(dead_code)]
fn dump_vdso(suffix: Option<&str>) {
    println!("Dumping vDSO");
    let r = vDSO::find(None).unwrap();
    let cur_vdso = vDSO::read(&r);
    let fname = format!("/tmp/vdso{}", suffix.unwrap_or(""));
    fs::write(&fname, cur_vdso).expect(&format!("Unable to write file {}", fname));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClockController, TimeSpec};

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
                size: 5,
            },
            DynSym {
                name: "__vdso_gettimeofday".to_string(),
                address: 3024,
                size: 5,
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
                size: 5,
            },
            DynSym {
                name: "__vdso_time".to_string(),
                address: 3040,
                size: 41,
            },
            DynSym {
                name: "__vdso_sgx_enter_enclave".to_string(),
                address: 3248,
                size: 156,
            },
            DynSym {
                name: "time".to_string(),
                address: 3040,
                size: 41,
            },
            DynSym {
                name: "__vdso_clock_gettime".to_string(),
                address: 3088,
                size: 5,
            },
            DynSym {
                name: "__vdso_getcpu".to_string(),
                address: 3200,
                size: 37,
            },
            DynSym {
                name: "getcpu".to_string(),
                address: 3200,
                size: 37,
            },
        ];
        assert_eq!(parsed, expected);
    }
}
