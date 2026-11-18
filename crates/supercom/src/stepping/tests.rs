use super::*;

fn digest(n: u8) -> Digest {
    [n; 32]
}

fn log() -> [(Tick, Digest); 3] {
    [
        (Tick(99), digest(1)),
        (Tick(199), digest(2)),
        (Tick(299), digest(3)),
    ]
}

/// Each answer of the comparison, with the tick named by a mismatch.
#[test]
fn supercom_checkpoint_verdict_cases() {
    let log = log();
    assert_eq!(verdict(Tick(99), &digest(1), &log), Verdict::Match);
    assert_eq!(verdict(Tick(199), &digest(2), &log), Verdict::Match);
    assert_eq!(verdict(Tick(299), &digest(3), &log), Verdict::Match);

    let mut last_byte = digest(2);
    last_byte[31] = 9;
    assert_eq!(
        verdict(Tick(199), &last_byte, &log),
        Verdict::Mismatch { tick: Tick(199) }
    );
    assert_eq!(
        verdict(Tick(99), &digest(2), &log),
        Verdict::Mismatch { tick: Tick(99) },
        "another checkpoint's digest is not this one's"
    );

    for tick in [0, 98, 100, 198, 200, 300, u64::MAX] {
        assert_eq!(
            verdict(Tick(tick), &digest(1), &log),
            Verdict::NoCheckpoint,
            "tick {tick}"
        );
    }
    assert_eq!(
        verdict(Tick(99), &digest(1), &[]),
        Verdict::NoCheckpoint,
        "an empty log has none"
    );
}

/// A step is taken up to and including the last checkpoint's tick.
#[test]
fn supercom_stop_rule_ends_at_the_last_checkpoint() {
    let log = log();
    assert!(may_step(Tick(0), &log));
    assert!(may_step(Tick(298), &log));
    assert!(may_step(Tick(299), &log));
    assert!(!may_step(Tick(300), &log));
    assert!(!may_step(Tick(u64::MAX), &log));
    assert!(!may_step(Tick(0), &[]));
}

/// The text of the last line, for the committed log's end and a cut run, with
/// the count of kinds drawn as markers.
#[test]
fn supercom_summary_line_text() {
    assert_eq!(
        summary_line(4600, 46, 46, 0, 0),
        "replayed 4600 ticks, 46 of 46 checkpoints matched, 0 frames held back, 0 kinds drawn as markers"
    );
    assert_eq!(
        summary_line(150, 1, 46, 3, 2),
        "replayed 150 ticks, 1 of 46 checkpoints matched, 3 frames held back, 2 kinds drawn as markers"
    );
    assert_eq!(
        summary_line(1, 0, 46, 0, 1),
        "replayed 1 ticks, 0 of 46 checkpoints matched, 0 frames held back, 1 kinds drawn as markers",
        "the count is not given a singular"
    );
}
