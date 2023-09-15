use std::{error::Error, time::SystemTime};

use tpom::{vdso, Kind, TVDSOFun, Time, TimeSpec, TimeVal};

extern crate tpom;

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
    let v = vdso::vDSO::find(None)?;
    let og = v.entry(Kind::GetTime).ok_or("Could not find clock")?;
    let backup = og.overwrite(myclock);
    println!("Done, Now: {:?}, restoring", SystemTime::now());
    backup.restore();
    println!("Restored, Now: {:?}", SystemTime::now());
    Ok(())
}
