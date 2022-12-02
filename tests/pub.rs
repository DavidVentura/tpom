mod tests {
    use serial_test::serial;
    use std::time::SystemTime;
    use tpom::{ClockController, TimeSpec};

    #[test]
    #[serial]
    fn regular_clock_produces_different_timestamps() {
        let time_a = SystemTime::now();
        let time_b = SystemTime::now();
        assert_ne!(time_a, time_b);
    }
    #[test]
    #[serial]
    fn it_freezes_system_clock() {
        ClockController::overwrite(
            Some(|_| TimeSpec {
                seconds: 1,
                nanos: 1,
            }),
            None,
            None,
            None,
        );
        let time_a = SystemTime::now();
        let time_b = SystemTime::now();
        ClockController::restore();
        assert_eq!(time_a, time_b);
    }
    #[test]
    #[serial]
    fn it_does_not_freeze_system_clock_if_unset() {
        ClockController::overwrite(None, None, None, None);
        let time_a = SystemTime::now();
        let time_b = SystemTime::now();
        assert_ne!(time_a, time_b);
    }
}
