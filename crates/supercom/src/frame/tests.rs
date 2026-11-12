use super::*;

fn nz(n: u32) -> Option<NonZeroU32> {
    NonZeroU32::new(n)
}

/// Whether to configure and draw is decided from the size alone: nothing
/// at a zero dimension, a configuration when the size is new.
#[test]
fn supercom_frame_plan_over_sizes() {
    for size in [(0, 0), (0, 600), (800, 0)] {
        for configured in [None, Some((800, 600))] {
            assert_eq!(
                frame_plan(size, configured),
                FramePlan::Skip,
                "{size:?} with {configured:?}"
            );
        }
    }
    let aspect_of_800_by_600 = 800.0 / 600.0;
    assert!(
        matches!(
            frame_plan((800, 600), None),
            FramePlan::Draw { configure: true, aspect }
                if (aspect - aspect_of_800_by_600).abs() < 1e-6
        ),
        "an unconfigured surface is configured at the size's aspect ratio"
    );
    assert_eq!(
        frame_plan((800, 600), Some((800, 600))),
        FramePlan::Draw {
            configure: false,
            aspect: aspect_of_800_by_600
        }
    );
    assert!(matches!(
        frame_plan((801, 600), Some((800, 600))),
        FramePlan::Draw {
            configure: true,
            ..
        }
    ));
    assert!(matches!(
        frame_plan((800, 600), Some((800, 601))),
        FramePlan::Draw {
            configure: true,
            ..
        }
    ));
    assert!((aspect_for(1920, 1080) - 16.0 / 9.0).abs() < 1e-6);
    assert!((super::aspect((0, 5)) - FALLBACK_ASPECT).abs() < 1e-6);
    assert!((super::aspect((5, 0)) - FALLBACK_ASPECT).abs() < 1e-6);
}

fn aspect_for(width: u32, height: u32) -> f32 {
    super::aspect((width, height))
}

/// The run ends after the requested frames, or gives up after too many
/// consecutive frames that did not present.
#[test]
fn supercom_progress_counts_frames_and_stalls() {
    let mut limited = Progress::new(nz(3));
    assert_eq!(limited.presented(), Step::Continue);
    assert_eq!(limited.presented(), Step::Continue);
    assert_eq!(limited.presented(), Step::Done);
    assert_eq!(limited.count(), 3);

    let mut unlimited = Progress::new(None);
    for _ in 0..1000 {
        assert_eq!(unlimited.presented(), Step::Continue);
    }
    assert_eq!(unlimited.count(), 1000);

    let mut stalling = Progress::new(nz(2));
    for _ in 0..MAX_STALLS - 1 {
        assert_eq!(stalling.stalled(), Step::Continue);
    }
    assert_eq!(
        stalling.presented(),
        Step::Continue,
        "a frame resets the stalls"
    );
    for _ in 0..MAX_STALLS - 1 {
        assert_eq!(stalling.stalled(), Step::Continue);
    }
    assert_eq!(stalling.stalled(), Step::GiveUp);
    assert_eq!(stalling.count(), 1, "stalls are not frames");
}
