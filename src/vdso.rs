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
        return Err("No vDSO mapped in memory range. Cannot continue".into());
    }
    pub(crate) fn dynsyms(buf: Vec<u8>) -> Vec<DynSym> {
        let r = Elf::parse(&buf).unwrap();

        let mut va = 0;
        for s in r.program_headers {
            if s.p_type == program_header::PT_DYNAMIC {
                va = s.p_vaddr;
            }
        }
        assert_ne!(va, 0);

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

    /// Overwrites the process' memory at (`range.start + address`) with:
    /// ```asm
    /// mov rax, $dst_address
    /// jmp rax
    /// nop
    /// nop
    /// nop
    /// nop
    /// ```
    pub(crate) fn overwrite(range: &Range, address: u64, dst_address: u64, size: usize) {
        let addr = (range.start as u64) + address;
        unsafe {
            /* These opcodes come from running `nasm -f elf64` on
              ```
                   global  _start
                   section .text
               _start:
                   mov		rax, 0x12ff34ff56ff78ff
                   jmp 		rax
              ```
              and copying them manually
            */
            // MOV RAX, <address>
            std::ptr::write_bytes((addr + 0) as *mut u8, 0x48, 1);
            std::ptr::write_bytes((addr + 1) as *mut u8, 0xB8, 1);
            std::ptr::write_bytes((addr + 2) as *mut u8, ((dst_address >> 0) & 0xFF) as u8, 1);
            std::ptr::write_bytes((addr + 3) as *mut u8, ((dst_address >> 8) & 0xFF) as u8, 1);
            std::ptr::write_bytes((addr + 4) as *mut u8, ((dst_address >> 16) & 0xFF) as u8, 1);
            std::ptr::write_bytes((addr + 5) as *mut u8, ((dst_address >> 24) & 0xFF) as u8, 1);
            std::ptr::write_bytes((addr + 6) as *mut u8, ((dst_address >> 32) & 0xFF) as u8, 1);
            std::ptr::write_bytes((addr + 7) as *mut u8, ((dst_address >> 40) & 0xFF) as u8, 1);
            std::ptr::write_bytes((addr + 8) as *mut u8, ((dst_address >> 48) & 0xFF) as u8, 1);
            std::ptr::write_bytes((addr + 9) as *mut u8, ((dst_address >> 56) & 0xFF) as u8, 1);
            // JMP
            std::ptr::write_bytes((addr + 10) as *mut u8, 0xFF, 1);
            std::ptr::write_bytes((addr + 11) as *mut u8, 0xE0, 1);
            // NOP the remaining space, unnecessary, but useful when debugging
            let padding_size = std::cmp::max(16, size) - 12;
            std::ptr::write_bytes((addr + 12) as *mut u8, 0x90, padding_size);
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
#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_dynsyms() {
        /*
        let r = vDSO::find().unwrap();
        let cur_vdso = vDSO::read(&r);
        fs::write("/tmp/foo", cur_vdso).expect("Unable to write file");
        */
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
