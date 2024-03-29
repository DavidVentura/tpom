mod tests {
    use std::hint::black_box;
    use std::sync::Mutex;
    use std::thread;
    use std::time::{Duration, SystemTime};
    use tpom::{vdso, Kind, TVDSOFun, TimeSpec};

    static tm: Mutex<i32> = Mutex::new(0);

    fn myclock(_clockid: i32) -> TimeSpec {
        TimeSpec {
            seconds: 111,
            nanos: 333,
        }
    }

    #[test]
    fn regular_clock_produces_different_timestamps() {
        let _guard = tm.lock().unwrap();
        let time_a = SystemTime::now();
        thread::sleep(std::time::Duration::from_millis(1)); // clock in github actions is coarse
        let time_b = SystemTime::now();
        assert_ne!(time_a, time_b);
    }

    #[test]
    fn it_freezes_system_clock() {
        let _guard = tm.lock().unwrap();
        let v = vdso::vDSO::read().unwrap();
        let og = v
            .entry(Kind::GetTime)
            .ok_or("Could not find clock")
            .unwrap();
        let backup = og.overwrite(myclock);

        let time_a = SystemTime::now();
        thread::sleep(std::time::Duration::from_millis(1)); // clock in github actions is coarse
        let time_b = SystemTime::now();
        assert_eq!(time_a, time_b);
        backup.restore();
    }

    #[test]
    fn it_works_many_threads() {
        let _guard = tm.lock().unwrap();
        let v = vdso::vDSO::read().unwrap();
        let og = v
            .entry(Kind::GetTime)
            .ok_or("Could not find clock")
            .unwrap();
        let backup = og.overwrite(myclock);

        thread::scope(|s| {
            for _ in 0..10 {
                s.spawn(|| {
                    for _ in 0..100 {
                        black_box(SystemTime::now());
                        thread::sleep(Duration::from_millis(1));
                    }
                });
            }
        });
        backup.restore();
        black_box(SystemTime::now());
    }

    #[test]
    fn it_works_after_setenv() {
        let _guard = tm.lock().unwrap();
        std::env::set_var("SOMETHING", "VALUE");
        let v = vdso::vDSO::read().unwrap();
        let og = v
            .entry(Kind::GetTime)
            .ok_or("Could not find clock")
            .unwrap();
        let backup = og.overwrite(myclock);

        let time_a = SystemTime::now();
        thread::sleep(std::time::Duration::from_millis(1)); // clock in github actions is coarse
        let time_b = SystemTime::now();
        assert_eq!(time_a, time_b);
        backup.restore();
    }
}
