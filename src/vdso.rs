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
