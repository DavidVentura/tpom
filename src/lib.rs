//! # TPOM
//! Allows replacing time-related functions in the vDSO ([1](https://man7.org/linux/man-pages/man7/vdso.7.html), [2](https://en.wikipedia.org/wiki/VDSO)) with user-provided functions.  
//!
//! Only works on Linux. Is currently limited to x86_64, though it could be extended for other architectures.
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
//! let time_a = SystemTime::now();
//! let time_b = SystemTime::now();
//! // Clock is frozen; all calls to time return the same values
//! assert_eq!(time_a, time_b);
//! // Unfreeze clock; all calls to time return unique values
//! ClockController::restore();
//! let time_c = SystemTime::now();
//! let time_d = SystemTime::now();
//! assert_ne!(time_c, time_d);
//! ```

pub(crate) mod trampolines;
pub(crate) mod vdso;

use libc;
use std::collections::HashMap;

use crate::trampolines::*;
use crate::vdso::vDSO;

#[derive(Debug, Clone, Copy)]
struct Range {
    start: usize,
    end: usize,
    writable: bool,
}

pub(crate) type Time = libc::time_t; // as libc::time_t

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

pub struct ClockController {}

impl ClockController {
    pub fn is_overwritten() -> bool {
        //! Whether the vDSO is currently overwritten
        let r = vDSO::find().unwrap();
        r.writable
    }
    pub fn restore() {
        //! Restore the vDSO to its original state, if it is currently overwritten
        let r = vDSO::find().unwrap();
        if !r.writable {
            return;
        }
        if let Ok(b) = BACKUP_VDSO.lock() {
            if b.len() == 0 {
                return;
            }
            unsafe {
                std::ptr::copy_nonoverlapping(b.as_ptr(), r.start as *mut u8, b.len());
                libc::mprotect(
                    r.start as *mut libc::c_void,
                    r.end - r.start,
                    libc::PROT_EXEC | libc::PROT_READ,
                );
            }
        }
    }

    pub fn overwrite(
        clockgettime_cb: Option<ClockGetTimeCb>,
        time_cb: Option<TimeCb>,
        clock_getres: Option<ClockGetResCb>,
        gettimeofday: Option<ClockGetTimeOfDayCb>,
    ) {
        //! Overwrite the vDSO with the user-provided functions.
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

        let r = vDSO::find().unwrap();
        unsafe {
            libc::mprotect(
                r.start as *mut libc::c_void,
                r.end - r.start,
                libc::PROT_EXEC | libc::PROT_WRITE | libc::PROT_READ,
            );
        }
        let b = vDSO::read(&r);
        BACKUP_VDSO.lock().unwrap().clear();
        BACKUP_VDSO.lock().unwrap().append(&mut b.clone());
        ClockController::mess_vdso(b, &r, mapping);
    }
    fn mess_vdso(buf: Vec<u8>, range: &Range, mapping: HashMap<&'static str, u64>) {
        for ds in vDSO::dynsyms(buf) {
            if let Some(dst_addr) = mapping.get(ds.name.as_str()) {
                // println!("Overriding dyn sym {} at {:x}", sym_name, dst_addr);
                overwrite(range, ds.address, *dst_addr, ds.size as usize);
            }
        }
    }
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
fn overwrite(range: &Range, address: u64, dst_address: u64, size: usize) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::time::SystemTime;

    #[test]
    #[serial]
    fn regular_clock_produces_different_timestamps() {
        let time_a = SystemTime::now();
        let time_b = SystemTime::now();
        assert_ne!(time_a, time_b);
    }
    #[test]
    #[serial]
    fn it_freezes_system_clock() {
        ClockController::overwrite(
            Some(|_| TimeSpec {
                seconds: 1,
                nanos: 1,
            }),
            None,
            None,
            None,
        );
        let time_a = SystemTime::now();
        let time_b = SystemTime::now();
        ClockController::restore();
        assert_eq!(time_a, time_b);
    }
    #[test]
    #[serial]
    fn it_does_not_freeze_system_clock_if_unset() {
        ClockController::overwrite(None, None, None, None);
        let time_a = SystemTime::now();
        let time_b = SystemTime::now();
        assert_ne!(time_a, time_b);
    }
}
