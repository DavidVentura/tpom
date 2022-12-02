use crate::{ClockGetResCb, ClockGetTimeCb, ClockGetTimeOfDayCb, TimeCb};
use libc::{self, c_void};
use std::sync::{Mutex, RwLock};

pub(crate) static CLOCK_GTOD_CB: RwLock<Option<ClockGetTimeOfDayCb>> = RwLock::new(None);
pub(crate) static CLOCK_GT_CB: RwLock<Option<ClockGetTimeCb>> = RwLock::new(None);
pub(crate) static CLOCK_RES_CB: RwLock<Option<ClockGetResCb>> = RwLock::new(None);
pub(crate) static TIME_CB: RwLock<Option<TimeCb>> = RwLock::new(None);
pub(crate) static BACKUP_VDSO: Mutex<Vec<u8>> = Mutex::new(vec![]);

/// Trampoline function between C and user's function. Panics if function was not set.
pub(crate) extern "C" fn my_time(t: *mut libc::time_t) -> libc::time_t {
    let res = TIME_CB.read().unwrap().unwrap()();
    if !t.is_null() {
        unsafe {
            *t = res;
        }
    }
    return res;
}

/// Trampoline function between C and user's function. Panics if function was not set.
pub(crate) extern "C" fn my_clockgettime(clockid: libc::clockid_t, ts: *mut libc::timespec) -> u32 {
    if !ts.is_null() {
        let res = CLOCK_GT_CB.read().unwrap().unwrap()(clockid);
        unsafe {
            (*ts).tv_sec = res.seconds;
            (*ts).tv_nsec = res.nanos;
        }
    }
    return 0;
}

/// Trampoline function between C and user's function. Panics if function was not set.
pub(crate) extern "C" fn my_clockgetres(clockid: libc::clockid_t, ts: *mut libc::timespec) -> u32 {
    if !ts.is_null() {
        let res = CLOCK_RES_CB.read().unwrap().unwrap()(clockid);
        unsafe {
            (*ts).tv_sec = res.seconds;
            (*ts).tv_nsec = res.nanos;
        }
    }
    return 0;
}

/// Trampoline function between C and user's function. Panics if function was not set.
/// Missing TZ support.
pub(crate) extern "C" fn my_gettimeofday(tp: *mut libc::timeval, _tz: *mut c_void) {
    // TODO: Support TZ
    if !tp.is_null() {
        let res = CLOCK_GTOD_CB.read().unwrap().unwrap()();
        unsafe {
            (*tp).tv_sec = res.seconds;
            (*tp).tv_usec = res.micros;
        }
    }
}
