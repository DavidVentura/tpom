//! # TPOM
//! Allows replacing time-related functions in the vDSO<sup>[1](https://man7.org/linux/man-pages/man7/vdso.7.html), [2](https://en.wikipedia.org/wiki/VDSO)</sup> with user-provided functions.  
//!
//! Only works on Linux. Is currently limited to x86_64, AArch64 and RISC-V, though it could be extended for other architectures.
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
//! fn myclock(_clockid: i32) -> TimeSpec {
//!     TimeSpec {
//!         seconds: 111,
//!         nanos: 333,
//!     }
//! }
//!
//! let v = vdso::vDSO::read().unwrap();
//! let og = v.entry(Kind::GetTime).ok_or("Could not find clock").unwrap();
//! let backup = og.overwrite(myclock);
//!
//! // Clock is frozen; all calls to time return the same values
//! let time_a = SystemTime::now();
//! let time_b = SystemTime::now();
//! assert_eq!(time_a, time_b);
//!
//! // Restore clock; all calls to time return unique values
//! backup.restore();
//! let time_c = SystemTime::now();
//! let time_d = SystemTime::now();
//! assert_ne!(time_c, time_d);
//! ```

mod opcodes;
pub(crate) mod trampolines;
pub mod vdso;
pub mod auxv;

use crate::trampolines::*;
use crate::vdso::vDSO;

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
pub struct VDSOFun<'a> {
    pub name: String,
    addr: usize,
    size: usize,
    v: &'a vDSO,
}

pub struct BackupEntry<'a> {
    v: &'a VDSOFun<'a>,
    data: Vec<u8>,
}

pub struct GTVdso<'a> {
    v: VDSOFun<'a>,
}

#[derive(PartialEq)]
pub enum Kind {
    GetTime,
    Time,
    ClockGetRes,
    GetTimeOfDay,
}

impl<'a> BackupEntry<'a> {
    pub fn restore(&self) {
        self.v.v.overwrite(self.v.addr, &self.data)
    }
}

pub trait TVDSOFun {
    fn overwrite(&self, cb: ClockGetTimeCb) -> BackupEntry;
}

fn _overwrite<'a>(v: &'a VDSOFun, trampoline: usize) -> BackupEntry<'a> {
    let opcodes = opcodes::generate_opcodes(trampoline, v.size);
    let backup = v.v.symbol_code(&v.name);
    v.v.overwrite(v.addr, &opcodes);
    BackupEntry {
        v,
        data: backup.to_owned(),
    }
}
impl<'a> TVDSOFun for GTVdso<'a> {
    fn overwrite(&self, cb: ClockGetTimeCb) -> BackupEntry {
        let mut w = CLOCK_GT_CB.write().unwrap();
        *w = Some(cb);
        _overwrite(&self.v, my_clockgettime as *const () as usize)
    }
}
