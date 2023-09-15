use std::{error::Error, time::SystemTime};

use tpom::{get_time, TVDSOFun, Time, TimeSpec, TimeVal};

extern crate tpom;

fn my_other_clock(_clockid: i32) -> TimeSpec {
    TimeSpec {
        seconds: 222,
        nanos: 222,
    }
}

fn myclock(_clockid: i32) -> TimeSpec {
    TimeSpec {
        seconds: 111,
        nanos: 333,
    }
}

fn mygttod() -> TimeVal {
    TimeVal {
        seconds: 1,
        micros: 3,
    }
}

fn my_time() -> Time {
    666
}

pub fn main() -> Result<(), Box<dyn Error>> {
    println!("Now: {:?}", SystemTime::now());
    println!("Executing");
    let og = get_time().ok_or("Could not find clock")?.overwrite(myclock);
    println!("Done, Now: {:?}, restoring", SystemTime::now());
    og.restore();
    println!("Restored, Now: {:?}", SystemTime::now());
    Ok(())
}
