use crate::Range;
use goblin::elf::*;
use goblin::strtab::Strtab;
use std::error::Error;
use std::fs::{self, File};
use std::os::unix::prelude::FileExt;

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
    pub(crate) fn find() -> Result<Range, Box<dyn Error>> {
        let data = fs::read_to_string("/proc/self/maps")?;

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
