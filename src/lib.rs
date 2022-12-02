use goblin::elf::*;
use goblin::strtab::Strtab;
use lazy_static::lazy_static;
use libc::{self, c_void};
use std::collections::HashMap;
use std::error::Error;
use std::fs::{self, File};
use std::io::prelude::*;
use std::os::unix::prelude::FileExt;
use std::sync::{Mutex, RwLock};

#[derive(Debug)]
struct Range {
    start: usize,
    end: usize,
    writable: bool,
}

pub type Time = libc::time_t; // as libc::time_t

pub struct TimeSpec {
    pub seconds: Time,
    pub nanos: i64, // as libc::c_long
}

pub struct TimeVal {
    pub seconds: Time,
    pub micros: i64, // as libc::suseconds_t
}

type TimeCb = fn() -> Time;

/// Considered infallible
type ClockGetTimeCb = fn(clockid: i32) -> TimeSpec;

/// Considered infallible
type ClockGetResCb = fn(i32) -> TimeSpec;

/// Considered infallible
type ClockGetTimeOfDayCb = fn() -> TimeVal; // FIXME: Needs to take a TZ

lazy_static! {
    static ref CLOCK_GTOD_CB: RwLock<Option<ClockGetTimeOfDayCb>> = RwLock::new(None);
    static ref CLOCK_GT_CB: RwLock<Option<ClockGetTimeCb>> = RwLock::new(None);
    static ref CLOCK_RES_CB: RwLock<Option<ClockGetResCb>> = RwLock::new(None);
    static ref TIME_CB: RwLock<Option<TimeCb>> = RwLock::new(None);
    static ref BACKUP_VDSO: Mutex<Vec<u8>> = Mutex::new(vec![]);
}

extern "C" fn my_time(t: *mut libc::time_t) -> libc::time_t {
    let res = TIME_CB.read().unwrap().unwrap()();
    if !t.is_null() {
        unsafe {
            *t = res;
        }
    }
    return res;
}

extern "C" fn my_clockgettime(clockid: libc::clockid_t, ts: *mut libc::timespec) -> u32 {
    if !ts.is_null() {
        let res = CLOCK_GT_CB.read().unwrap().unwrap()(clockid);
        unsafe {
            (*ts).tv_sec = res.seconds;
            (*ts).tv_nsec = res.nanos;
        }
    }
    return 0;
}

extern "C" fn my_clockgetres(clockid: libc::clockid_t, ts: *mut libc::timespec) -> u32 {
    if !ts.is_null() {
        let res = CLOCK_RES_CB.read().unwrap().unwrap()(clockid);
        unsafe {
            (*ts).tv_sec = res.seconds;
            (*ts).tv_nsec = res.nanos;
        }
    }
    return 0;
}

extern "C" fn my_gettimeofday(tp: *mut libc::timeval, _tz: *mut c_void) {
    // TODO: Support TZ
    if !tp.is_null() {
        let res = CLOCK_GTOD_CB.read().unwrap().unwrap()();
        unsafe {
            (*tp).tv_sec = res.seconds;
            (*tp).tv_usec = res.micros;
        }
    }
}

fn vdso_mem_range() -> Result<Range, Box<dyn Error>> {
    let data = fs::read_to_string("/proc/self/maps")?;
    for line in data.lines() {
        if line.contains("vdso") {
            let (range, _) = line.split_once(" ").unwrap();
            let (start, end) = range.split_once("-").unwrap();
            let parts: Vec<&str> = line.split_whitespace().collect();
            let perms = parts[1];
            return Ok(Range {
                start: usize::from_str_radix(start, 16).unwrap(),
                end: usize::from_str_radix(end, 16).unwrap(),
                writable: perms.contains("w"),
            });
        }
    }
    return Err("No vDSO mapped in memory range. Cannot continue".into());
}

pub fn is_cursed() -> bool {
    let r = vdso_mem_range().unwrap();
    r.writable
}
pub fn lift_curse_vdso() {
    let r = vdso_mem_range().unwrap();
    if !r.writable {
        return;
    }
    if let Ok(b) = BACKUP_VDSO.lock() {
        if b.len() == 0 {
            return;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(b.as_ptr(), r.start as *mut u8, b.len());
            /*
            libc::mprotect(
                r.start as *mut libc::c_void,
                r.end - r.start,
                libc::PROT_EXEC | libc::PROT_READ,
            );
            */
        }
    }
}
pub fn curse_vdso(
    clockgettime_cb: Option<ClockGetTimeCb>,
    time_cb: Option<TimeCb>,
    clock_getres: Option<ClockGetResCb>,
    gettimeofday: Option<ClockGetTimeOfDayCb>,
) {
    let r = vdso_mem_range().unwrap();
    unsafe {
        libc::mprotect(
            r.start as *mut libc::c_void,
            r.end - r.start,
            libc::PROT_EXEC | libc::PROT_WRITE | libc::PROT_READ,
        );
    }
    let mut mapping: HashMap<&'static str, u64> = HashMap::new();
    if let Some(g) = clockgettime_cb {
        let mut w = CLOCK_GT_CB.write().unwrap();
        *w = Some(g);
        let addr = my_clockgettime as *const () as u64;
        mapping.insert("clock_gettime", addr);
        mapping.insert("__vdso_clock_gettime", addr);
    }
    if let Some(g) = time_cb {
        let mut w = TIME_CB.write().unwrap();
        *w = Some(g);
        let addr = my_time as *const () as u64;
        mapping.insert("time", addr);
        mapping.insert("__vdso_time", addr);
    }
    if let Some(g) = clock_getres {
        let mut w = CLOCK_RES_CB.write().unwrap();
        *w = Some(g);
        let addr = my_clockgetres as *const () as u64;
        mapping.insert("clock_getres", addr);
        mapping.insert("__vdso_clock_getres", addr);
    }
    if let Some(g) = gettimeofday {
        let mut w = CLOCK_GTOD_CB.write().unwrap();
        *w = Some(g);
        let addr = my_gettimeofday as *const () as u64;
        mapping.insert("gettimeofday", addr);
        mapping.insert("__vdso_gettimeofday", addr);
    }

    let b = read_vdso(&r);
    BACKUP_VDSO.lock().unwrap().clear();
    BACKUP_VDSO.lock().unwrap().append(&mut b.clone());
    mess_vdso(b, &r, mapping);
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
fn read_vdso(range: &Range) -> Vec<u8> {
    let mut buf = vec![0; range.end - range.start];
    let f = File::open("/proc/self/mem").unwrap();
    f.read_at(&mut buf, range.start as u64).unwrap();
    drop(f);
    return buf;
}

fn write_vdso(buf: &Vec<u8>) {
    let mut tmp = File::create("out").unwrap();
    tmp.write_all(&buf).unwrap();
}

fn overwrite(range: &Range, address: u64, dst_address: u64, size: u32) {
    let addr = (range.start as u64) + address;
    unsafe {
        /*
        std::ptr::write_bytes((addr + 0) as *mut u8, 0xC3, 1); // RET
        std::ptr::write_bytes((addr + 1) as *mut u8, 0x90, (size - 1) as usize);
        */
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
        // Size is 2**(size -1)
        std::ptr::write_bytes(
            (addr + 12) as *mut u8,
            0x90,
            ((u64::pow(2, size - 1)) - 12) as usize,
        );
    }
}
fn mess_vdso(buf: Vec<u8>, range: &Range, mapping: HashMap<&'static str, u64>) {
    let r = Elf::parse(&buf).unwrap();

    let mut va = 0;
    for s in r.program_headers {
        if s.p_type == 2 {
            // "PT_DYNAMIC"
            va = s.p_vaddr;
        }
    }
    assert_ne!(va, 0);

    for ds in &r.dynsyms {
        let sym_name = get_str_til_nul(&r.dynstrtab, ds.st_name);
        if let Some(dst_addr) = mapping.get(sym_name.as_str()) {
            // println!("Overriding dyn sym {}", sym_name);
            overwrite(range, ds.st_value, *dst_addr, ds.st_size as u32);
        }
    }
    // write_vdso(&read_vdso(range));
}
pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
