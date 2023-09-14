use std::{error::Error, time::SystemTime};

use tpom::{ClockController, Kind, Time, TimeSpec, TimeVal};

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

fn my_time() -> Time {
    666
}

pub fn main() -> Result<(), Box<dyn Error>> {
    println!("Now: {:?}", SystemTime::now());
    println!("Executing");
    let og = ClockController::get_time()
        .ok_or("Could not find clock")?
        .overwrite(myclock);
    println!("Done, Now: {:?}, restoring", SystemTime::now());
    og.restore();
    println!("Restored, Now: {:?}", SystemTime::now());
    Ok(())
}
