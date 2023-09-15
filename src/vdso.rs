use crate::Range;
use crate::*;
use goblin::elf::*;
use goblin::strtab::Strtab;
use std::error::Error;
use std::fs::{self, File};
use std::os::unix::prelude::FileExt;

#[derive(Debug, PartialEq)]
pub(crate) struct DynSym {
    pub(crate) name: String,
    pub(crate) address: usize,
    pub(crate) size: usize,
}
#[allow(non_camel_case_types)]
#[derive(PartialEq, Clone, Debug)]
pub struct vDSO {
    range: Range,
    data: Vec<u8>,
}

impl vDSO {
    pub(crate) fn read(range: &Range) -> Vec<u8> {
        let mut buf = vec![0; range.end - range.start];
        let f = File::open("/proc/self/mem").unwrap();
        f.read_at(&mut buf, range.start as u64).unwrap();
        drop(f);
        buf
    }

    pub(crate) fn change_mode(&self, write: bool) {
        let mode = if write {
            libc::PROT_EXEC | libc::PROT_WRITE | libc::PROT_READ
        } else {
            libc::PROT_EXEC | libc::PROT_READ
        };
        unsafe {
            libc::mprotect(
                self.range.start as *mut libc::c_void,
                self.range.end - self.range.start,
                mode,
            );
        }
    }
    fn parse_mem_map(path: Option<&str>) -> Result<Range, Box<dyn Error>> {
        // could use getauxval(AT_SYSINFO_EHDR)
        // but calculating the length is complicated, and i'm not really sure how to
        // pass a pointer to memory as a &[u8], without specifying length

        let data = fs::read_to_string(path.unwrap_or("/proc/self/maps"))?;

        for line in data.lines() {
            if !line.contains("[vdso]") {
                continue;
            }
            let (range, _) = line.split_once(' ').unwrap();
            let (start, end) = range.split_once('-').unwrap();
            let parts: Vec<&str> = line.split_whitespace().collect();
            let perms = parts[1];
            return Ok(Range {
                start: usize::from_str_radix(start, 16).unwrap(),
                end: usize::from_str_radix(end, 16).unwrap(),
                writable: perms.contains('w'),
            });
        }
        Err("No vDSO mapped in memory range. Cannot continue".into())
    }

    pub fn open() -> Result<Self, Box<dyn Error>> {
        vDSO::open_at(None)
    }

    pub fn open_at(path: Option<&str>) -> Result<Self, Box<dyn Error>> {
        let r = vDSO::parse_mem_map(path)?;
        Ok(vDSO {
            range: r,
            data: vDSO::read(&r),
        })
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
        let dst_addr = self.range.start + symbol_address;
        self.change_mode(true);
        for (i, b) in opcodes.iter().enumerate() {
            unsafe {
                std::ptr::write_bytes((dst_addr + i) as *mut u8, *b, 1);
            }
        }
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
                "__vdso_clock_gettime" => Some(Kind::GetTime),
                "__vdso_gettimeofday" => Some(Kind::GetTimeOfDay),
                "__vdso_clock_getres" => Some(Kind::ClockGetRes),
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
    fn test_parse_proc_self_maps() {
        let parsed = vDSO::parse_mem_map(Some("src/test_files/proc/self/maps"));
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
        let test_vdso =
            fs::read("src/test_files/test_vdso_elf_1").expect("Unable to read test file");
        let a = vDSO {
            range: Range {
                start: 0,
                end: 0,
                writable: true,
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
            range: Range {
                start: 0,
                end: 0,
                writable: true,
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
