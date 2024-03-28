use crate::*;
use goblin::elf::*;
use goblin::strtab::Strtab;
use core::slice;
use std::error::Error;
use std::fs;

#[derive(Debug, PartialEq)]
pub(crate) struct DynSym {
    pub(crate) name: String,
    pub(crate) address: usize,
    pub(crate) size: usize,
}

#[allow(non_camel_case_types)]
#[derive(Debug)]
pub struct vDSO {
    avv: auxv::AuxVecValues,
    pub data: Vec<u8>,
}

#[cfg(target_pointer_width="32")]
const ELF_HDR_SIZE: usize = 52;

#[cfg(target_pointer_width="64")]
const ELF_HDR_SIZE: usize = 64;

impl vDSO {
    pub fn read() -> Result<vDSO, Box<dyn Error>> {
        let auxvec = auxv::read_aux_vec()?;

        // As the size of the vDSO is unknown, read first only the header which has constant size
        let header_bytes: &[u8] = unsafe { slice::from_raw_parts(&*(auxvec.vdso_base as *const u8), ELF_HDR_SIZE) };
        let bare_header = Elf::parse_header(&header_bytes).unwrap();
        // Having parsed the header, we can now calculate the len of the vDSO
        let vdso_len = usize::from(bare_header.e_shnum * bare_header.e_shentsize) + (bare_header.e_shoff as usize);
        // And with the len, we can read the right amount
        let vdso_bytes = unsafe { slice::from_raw_parts(&*(auxvec.vdso_base as *const u8), vdso_len) };

        Ok(vDSO {data: vdso_bytes.into(), avv: auxvec })
    }

    pub(crate) fn change_mode(&self, write: bool) {
        let mode = if write {
            libc::PROT_EXEC | libc::PROT_WRITE | libc::PROT_READ
        } else {
            libc::PROT_EXEC | libc::PROT_READ
        };
        // As we need to mprotect() the vDSO and that can only be done in full pages, we need
        // to bump the vDSO length to the next page
        let vdso_size_page_aligned = (self.data.len() + self.avv.page_size-1) & !(self.avv.page_size-1);
        unsafe {

            libc::mprotect(
                self.avv.vdso_base as *mut libc::c_void,
                vdso_size_page_aligned,
                mode,
            );
        }
    }

    pub(crate) fn dynsyms(&self) -> Vec<DynSym> {
        let r = Elf::parse(&self.data).expect("bad elf");

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
                address: (ds.st_value - base) as usize,
                size: symsize as usize,
            });
        }
        ret
    }

    pub fn restore(&self) {
        self.overwrite(0, &self.data)
    }
    pub(crate) fn symbol_code(&self, symbol_name: &str) -> &[u8] {
        for sym in self.dynsyms() {
            if sym.name == symbol_name {
                return &self.data[sym.address..(sym.address + sym.size)];
            }
        }
        unreachable!("Got illegal symbol name");
    }
    /// Overwrites the process' vDSO memory at offset `symbol_address` with `opcodes`.
    /// It is the caller's responsibility to provide the correct amount of data.
    pub(crate) fn overwrite(&self, symbol_address: usize, opcodes: &[u8]) {
        let dst_addr = self.avv.vdso_base + symbol_address;
        println!("writing 0x{:x} bytes to 0x{dst_addr:x}", opcodes.len());
        self.change_mode(true);
        unsafe {
            std::ptr::copy_nonoverlapping(opcodes.as_ptr(), dst_addr as *mut u8, opcodes.len())
        };
        self.change_mode(false);
    }

    pub fn entry(&self, wanted: Kind) -> Option<impl TVDSOFun + '_> {
        for ds in self.dynsyms() {
            let v = VDSOFun {
                name: ds.name.clone(),
                addr: ds.address,
                size: ds.size,
                v: self,
            };
            let kind = match ds.name.as_str() {
                // Per the man page:
                // > "All of these symbols are also available without the "__vdso_" prefix, but you should ignore those."

                #[cfg(target_arch = "aarch64")]
                "__kernel_clock_gettime" => Some(Kind::GetTime),
                #[cfg(target_arch = "aarch64")]
                "__kernel_gettimeofday" => Some(Kind::GetTimeOfDay),
                #[cfg(target_arch = "aarch64")]
                "__kernel_clock_getres" => Some(Kind::ClockGetRes),

                #[cfg(any(target_arch = "x86_64", target_arch = "riscv64"))]
                "__vdso_clock_gettime" => Some(Kind::GetTime),
                #[cfg(any(target_arch = "x86_64", target_arch = "riscv64"))]
                "__vdso_gettimeofday" => Some(Kind::GetTimeOfDay),
                #[cfg(any(target_arch = "x86_64", target_arch = "riscv64"))]
                "__vdso_clock_getres" => Some(Kind::ClockGetRes),
                #[cfg(any(target_arch = "x86_64", target_arch = "riscv64"))]
                "__vdso_time" => Some(Kind::Time),

                &_ => None,
            };
            if kind.is_none() {
                continue;
            }
            if kind.as_ref() != Some(&wanted) {
                continue;
            }

            return Some(match kind {
                None => unreachable!(),
                Some(Kind::GetTime) => GTVdso { v },
                Some(_) => todo!(),
            });
        }
        None
    }

    pub fn dump(&self, suffix: Option<&str>) {
        let fname = format!("/tmp/vdso{}", suffix.unwrap_or(""));
        fs::write(&fname, &self.data).unwrap_or_else(|_| panic!("Unable to write file {}", fname));
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
    ret
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dynsyms() {
        let test_vdso =
            fs::read("src/test_files/test_vdso_elf_1").expect("Unable to read test file");
        let a = vDSO {
            avv: auxv::AuxVecValues {
                vdso_base: 0,
                page_size: 0x1000,
            },
            data: test_vdso,
        };
        let parsed = a.dynsyms();
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
        let a = vDSO {
            avv: auxv::AuxVecValues {
                vdso_base: 0,
                page_size: 0x1000,
            },
            data: test_vdso,
        };
        let parsed = a.dynsyms();
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
