use super::*;

fn pace_of(due: u64, held_back: u64, fraction: f64) -> Pace {
    Pace {
        due,
        held_back,
        fraction,
    }
}

/// Literals worked out from the rule: owed = floor(elapsed * speed * 10)
/// less the ticks run, due = min(owed, 10), held = owed - due.
#[test]
fn supercom_pacing_cases() {
    assert_eq!(pace(0.25, 1.0, 0), pace_of(2, 0, 0.5));
    assert_eq!(pace(0.0, 1.0, 0), pace_of(0, 0, 0.0));
    assert_eq!(pace(1.0, 2.0, 0), pace_of(10, 10, 0.0));
    assert_eq!(pace(1.0, 2.0, 10), pace_of(10, 0, 0.0));
    assert_eq!(pace(10.0, 0.1, 0), pace_of(10, 0, 0.0));
    assert_eq!(pace(0.25, 1.0, 5), pace_of(0, 0, 0.5), "ahead owes nothing");
    assert_eq!(pace(1.05, 1.0, 0), pace_of(10, 0, 0.5), "ten whole ticks");
    assert_eq!(MAX_TICKS_PER_FRAME, 10);

    for (elapsed, speed) in [
        (f64::NAN, 1.0),
        (1.0, f64::NAN),
        (f64::INFINITY, 1.0),
        (1.0, f64::INFINITY),
        (f64::NEG_INFINITY, 1.0),
        (-1.0, 1.0),
        (1.0, -1.0),
    ] {
        assert_eq!(
            pace(elapsed, speed, 0),
            pace_of(0, 0, 0.0),
            "({elapsed}, {speed}) owes nothing"
        );
    }
}
