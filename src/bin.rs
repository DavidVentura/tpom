use std::time::SystemTime;

use timekeeper::{TimeSpec, TimeVal};

extern crate timekeeper;

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
    println!("Now: {:?}", SystemTime::now());
    timekeeper::curse_vdso(Some(myclock), None, None, Some(mygttod));
    println!("Now: {:?}", SystemTime::now());
}
