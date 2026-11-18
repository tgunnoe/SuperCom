use super::*;
use std::path::PathBuf;

fn view(folder: &str, frames: Option<u32>) -> Command {
    Command::View(ViewArgs {
        folder: folder.to_owned(),
        frames: frames.and_then(NonZeroU32::new),
    })
}

fn replay(file: &str, speed: f64, frames: Option<u32>) -> Command {
    Command::Replay(ReplayArgs {
        file: PathBuf::from(file),
        speed,
        frames: frames.and_then(NonZeroU32::new),
    })
}

fn is_usage(args: &[&str]) -> bool {
    parse_args(args.iter().copied()).is_err()
}

/// The accepted forms, with the empty list parsing to the bare run.
#[test]
fn supercom_parse_args_cases() {
    assert_eq!(parse_args(Vec::<&str>::new()), Ok(Command::Bare));
    assert_eq!(parse_args(["view"]), Ok(view("SCMP_012", None)));
    assert_eq!(parse_args(["view", "SCMP_016"]), Ok(view("SCMP_016", None)));
    assert_eq!(
        parse_args(["view", "--frames", "3"]),
        Ok(view("SCMP_012", Some(3)))
    );
    assert_eq!(
        parse_args(["view", "SCMP_016", "--frames", "12"]),
        Ok(view("SCMP_016", Some(12)))
    );
    assert_eq!(
        parse_args(["view", "--frames", "1", "SCMP_016"]),
        Ok(view("SCMP_016", Some(1)))
    );
    assert_eq!(
        parse_args(["view", "--frames", "007"]),
        Ok(view("SCMP_012", Some(7)))
    );
    replay_cases();
}

/// `replay`: the accepted forms in every order, the bounds of the speed,
/// and the lists it rejects; REN-032's test runs it.
fn replay_cases() {
    assert_eq!(
        parse_args(["replay", "a.replay"]),
        Ok(replay("a.replay", 1.0, None))
    );
    assert_eq!(
        parse_args(["replay", "dir/b.replay", "--speed", "2.5"]),
        Ok(replay("dir/b.replay", 2.5, None))
    );
    assert_eq!(
        parse_args(["replay", "--speed", "0.1", "a.replay"]),
        Ok(replay("a.replay", 0.1, None))
    );
    assert_eq!(
        parse_args(["replay", "--speed", "10", "--frames", "3", "a"]),
        Ok(replay("a", 10.0, Some(3)))
    );
    assert_eq!(
        parse_args(["replay", "--frames", "12", "a", "--speed", "007.50"]),
        Ok(replay("a", 7.5, Some(12)))
    );
    assert_eq!(
        parse_args(["replay", "a", "--frames", "1"]),
        Ok(replay("a", 1.0, Some(1)))
    );
    for args in [
        &["replay"][..],
        &["replay", "--speed", "2"],
        &["replay", "-x"],
        &["replay", "a", "b"],
        &["replay", ""],
        &["replay", "a", "--speed"],
        &["replay", "a", "--frames"],
        &["replay", "a", "--speed", "0.09"],
        &["replay", "a", "--speed", "10.01"],
        &["replay", "a", "--speed", "0"],
        &["replay", "a", "--speed", "-1"],
        &["replay", "a", "--speed", "+1"],
        &["replay", "a", "--speed", "1e1"],
        &["replay", "a", "--speed", "1.2.3"],
        &["replay", "a", "--speed", ""],
        &["replay", "a", "--speed", "."],
        &["replay", "a", "--speed", ".5"],
        &["replay", "a", "--speed", "5."],
        &["replay", "a", "--speed", "NaN"],
        &["replay", "a", "--speed", "inf"],
        &["replay", "a", "--speed", "1", "--speed", "2"],
        &["replay", "a", "--frames", "0"],
        &["replay", "a", "--frames", "2", "--frames", "3"],
        &["replay", "a", "--unknown"],
        &["replay", "--speed", "-", "a"],
    ] {
        assert!(is_usage(args), "{args:?} was accepted");
    }
    assert!(is_usage(&["REPLAY", "a"]));
    assert!(is_usage(&["view", "--speed", "2"]));
}

/// Every other list is a usage error, with the usage in its text.
#[test]
fn supercom_parse_args_rejects_everything_else() {
    for args in [
        &["--help"][..],
        &["-h"],
        &["help"],
        &["VIEW"],
        &["view", "--frames"],
        &["view", "--frames", "0"],
        &["view", "--frames", "-1"],
        &["view", "--frames", "+3"],
        &["view", "--frames", "3.5"],
        &["view", "--frames", "abc"],
        &["view", "--frames", ""],
        &["view", "--frames", "99999999999999999999"],
        &["view", "--frames", "2", "--frames", "3"],
        &["view", "--unknown"],
        &["view", "-x"],
        &["view", "SCMP_012", "-x"],
        &["view", "A", "B"],
        &["view", "--frames", "2", "A", "B"],
        &["SCMP_012"],
        &["view", ""],
        &["bare"],
    ] {
        assert!(is_usage(args), "{args:?} was accepted");
    }
    let message = parse_args(["view", "--frames", "0"])
        .err()
        .map(|error| error.to_string());
    assert!(
        message.as_deref().is_some_and(|text| text.contains(USAGE)),
        "the usage error carries the usage text: {message:?}"
    );
}

/// The map argument is one path component.
#[test]
fn supercom_parse_args_folder_is_one_component() {
    assert_eq!(parse_args(["view", "SCMP_012"]), Ok(view("SCMP_012", None)));
    assert_eq!(parse_args(["view", "my map"]), Ok(view("my map", None)));
    assert_eq!(parse_args(["view", "a.b"]), Ok(view("a.b", None)));
    for folder in [
        ".", "..", "a/b", "/abs", "../x", "a/", "./a", "a\\b", "..\\x", "a/../b", "",
    ] {
        assert!(
            is_usage(&["view", folder]),
            "folder {folder:?} was accepted"
        );
    }
    assert_eq!(
        parse_args(["view", "SCMP_012", "--frames", "2"]),
        Ok(view("SCMP_012", Some(2)))
    );
}
