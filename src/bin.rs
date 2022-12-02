use std::time::SystemTime;

use tpom::{ClockController, TimeSpec, TimeVal};

extern crate tpom;

fn myclock(_clockid: i32) -> TimeSpec {
    TimeSpec {
        seconds: 1,
        nanos: 3,
    }
}

fn mygttod() -> TimeVal {
    TimeVal {
        seconds: 1,
        micros: 3,
    }
}
pub fn main() {
    ClockController::restore();
    println!("Now: {:?}", SystemTime::now());
    ClockController::overwrite(Some(myclock), None, None, Some(mygttod));
    println!("Now: {:?}", SystemTime::now());
    ClockController::restore();
    println!("Now: {:?}", SystemTime::now());
    ClockController::restore();
}
