use std::time::SystemTime;

use tpom::{TimeSpec, TimeVal};

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
    tpom::lift_curse_vdso();
    println!("Now: {:?}", SystemTime::now());
    tpom::curse_vdso(Some(myclock), None, None, Some(mygttod));
    println!("Now: {:?}", SystemTime::now());
    tpom::lift_curse_vdso();
    println!("Now: {:?}", SystemTime::now());
    tpom::lift_curse_vdso();
}
