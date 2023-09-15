//! # TPOM
//! Allows replacing time-related functions in the vDSO ([1](https://man7.org/linux/man-pages/man7/vdso.7.html), [2](https://en.wikipedia.org/wiki/VDSO)) with user-provided functions.  
//!
//! Only works on Linux. Is currently limited to x86_64, aarch64 and riscv, though it could be extended for other architectures.
//!
//! Replaces these functions, if provided:
//!
//! |User Function| vDSO|
//! |-------------|-----|
//! |ClockGetTime|[clock_gettime](https://linux.die.net/man/3/clock_gettime)|
//! |ClockGetTimeOfDay|[gettimeofday](https://linux.die.net/man/2/gettimeofday)|
//! |ClockGetRes|[clock_getres](https://man7.org/linux/man-pages/man2/clock_getres.2.html)|
//! |ClockGetTime|[time](https://linux.die.net/man/2/time)|
//!
//! # Examples
//! ```
//! use tpom::*;
//! use std::time::SystemTime;
//!
//! ClockController::overwrite(
//!     Some(|_| TimeSpec {
//!         seconds: 1,
//!         nanos: 1,
//!     }),
//!     None,
//!     None,
//!     None,
//! );
//! // Clock is frozen; all calls to time return the same values
//! let time_a = SystemTime::now();
//! let time_b = SystemTime::now();
//! assert_eq!(time_a, time_b);
//!
//! // Restore clock; all calls to time return unique values
//! ClockController::restore();
//! let time_c = SystemTime::now();
//! let time_d = SystemTime::now();
//! assert_ne!(time_c, time_d);
//! ```

pub(crate) mod trampolines;
pub(crate) mod vdso;

use libc;
use std::fs;

use crate::trampolines::*;
use crate::vdso::vDSO;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Range {
    start: usize,
    end: usize,
    writable: bool,
}

pub type Time = libc::time_t; // as libc::time_t

/// Return type for `ClockGetTime` and `ClockGetRes`; maps to
/// [libc::timespec](https://docs.rs/libc/0.2.56/libc/struct.timespec.html).
pub struct TimeSpec {
    pub seconds: Time,
    pub nanos: i64, // as libc::c_long
}

/// Return type for `ClockGetTimeOfDay`; maps to
/// [libc::timeval](https://docs.rs/libc/0.2.56/libc/struct.timeval.html).
pub struct TimeVal {
    pub seconds: Time,
    pub micros: i64, // as libc::suseconds_t
}

pub type TimeCb = fn() -> Time;

/// Considered infallible
pub type ClockGetTimeCb = fn(clockid: i32) -> TimeSpec;

/// Considered infallible
pub type ClockGetResCb = fn(i32) -> TimeSpec;

/// Considered infallible
pub type ClockGetTimeOfDayCb = fn() -> TimeVal; // FIXME: Needs to take a TZ

#[derive(Clone)]
pub struct VDSOFun {
    pub name: String,
    //pub kind: Kind,
    addr: u64,
    size: u64,
}

pub struct BackupEntry {
    v: VDSOFun,
    data: Vec<u8>,
}

pub struct GTVdso {
    v: VDSOFun,
}

#[derive(PartialEq)]
enum Kind {
    GetTime,
    Time,
    ClockGetRes,
    GetTimeOfDay,
}

impl BackupEntry {
    pub fn restore(&self) {
        let r = vDSO::find(None).unwrap();
        vDSO::restore(r.start as u64, self.v.addr, &self.data)
    }
}

pub trait TVDSOFun {
    fn trampoline(&self) -> u64;
    fn overwrite(&self, cb: ClockGetTimeCb) -> BackupEntry;
}

fn _overwrite(v: &VDSOFun, trampoline: u64) -> BackupEntry {
    let r = vDSO::find(None).unwrap();
    unsafe {
        libc::mprotect(
            r.start as *mut libc::c_void,
            r.end - r.start,
            libc::PROT_EXEC | libc::PROT_WRITE | libc::PROT_READ,
        );
    }
    //let backup = vDSO::read_symbol(r.start as u64, v.addr, v.size as usize);
    let buf = vDSO::read(&r);
    let backup = &buf[(v.addr as usize)..(v.addr + v.size) as usize];
    vDSO::overwrite(r.start as u64, v.addr, trampoline, v.size as usize);
    // vDSO::overwrite(elf_offset, ds.address, *dst_addr, ds.size as usize);
    BackupEntry {
        v: v.clone(),
        data: backup.to_owned(),
    }
}
impl TVDSOFun for GTVdso {
    fn trampoline(&self) -> u64 {
        my_clockgettime as *const () as u64
    }
    fn overwrite(&self, cb: ClockGetTimeCb) -> BackupEntry {
        let mut w = CLOCK_GT_CB.write().unwrap();
        *w = Some(cb);
        _overwrite(&self.v, self.trampoline())
    }
}

pub fn get_time() -> Option<GTVdso> {
    match entry(Kind::GetTime) {
        None => None,
        Some(v) => Some(GTVdso { v }),
    }
}

fn entry(wanted: Kind) -> Option<VDSOFun> {
    let r = vDSO::find(None).unwrap();
    let buf = vDSO::read(&r);
    for ds in vDSO::dynsyms(buf) {
        let kind = match ds.name.as_str() {
            "clock_gettime" => Some(Kind::GetTime),
            &_ => None,
        };
        if kind.is_none() {
            continue;
        }
        if kind.as_ref() != Some(&wanted) {
            continue;
        }
        let v = VDSOFun {
            name: ds.name,
            addr: ds.address,
            size: ds.size,
        };
        match kind {
            None => {}
            Some(Kind::GetTime) => return Some(v),
            Some(_) => {}
        }
    }
    None
}

#[allow(dead_code)]
pub fn dump_vdso(suffix: Option<&str>) {
    println!("Dumping vDSO");
    let r = vDSO::find(None).unwrap();
    let cur_vdso = vDSO::read(&r);
    let fname = format!("/tmp/vdso{}", suffix.unwrap_or(""));
    fs::write(&fname, cur_vdso).expect(&format!("Unable to write file {}", fname));
}
