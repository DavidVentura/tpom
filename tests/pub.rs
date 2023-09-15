mod tests {
    use serial_test::serial;
    use std::time::SystemTime;
    use tpom::{vdso, Kind, TVDSOFun, TimeSpec};

    fn myclock(_clockid: i32) -> TimeSpec {
        TimeSpec {
            seconds: 111,
            nanos: 333,
        }
    }

    #[test]
    #[serial]
    fn regular_clock_produces_different_timestamps() {
        let time_a = SystemTime::now();
        std::thread::sleep(std::time::Duration::from_millis(1)); // clock in github actions is coarse
        let time_b = SystemTime::now();
        assert_ne!(time_a, time_b);
    }
    #[test]
    #[serial]
    fn it_freezes_system_clock() {
        let v = vdso::vDSO::open().unwrap();
        let og = v
            .entry(Kind::GetTime)
            .ok_or("Could not find clock")
            .unwrap();
        let backup = og.overwrite(myclock);

        let time_a = SystemTime::now();
        std::thread::sleep(std::time::Duration::from_millis(1)); // clock in github actions is coarse
        let time_b = SystemTime::now();
        backup.restore();
        assert_eq!(time_a, time_b);
    }
}
