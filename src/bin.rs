use std::{error::Error, time::SystemTime};
use std::fs::File;
use std::io::prelude::*;

use tpom::{vdso, Kind, Time, TimeSpec, TimeVal, TVDSOFun};

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
    let v = vdso::vDSO::read()?;
    let mut f = File::create("original_vdso.elf").unwrap();
    let mut f1 = File::create("overwritten_vdso.elf").unwrap();
    let mut f2 = File::create("restored_vdso.elf").unwrap();
    f.write_all(&v.data).unwrap();
    f1.set_len(0);
    f2.set_len(0);

    let og = v.entry(Kind::GetTime).ok_or("Could not find clock")?;
    let backup = og.overwrite(myclock);
    let overwritten = vdso::vDSO::read()?;

    f1.write_all(&overwritten.data).unwrap();

    println!("Done");
    println!("Now: {:?}, restoring", SystemTime::now());
    backup.restore();
    let restored = vdso::vDSO::read()?;
    f2.write_all(&restored.data).unwrap();
    println!("Restored");

    println!("Now: {:?}", SystemTime::now());
    Ok(())
}
