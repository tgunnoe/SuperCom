use super::*;

fn view(folder: &str, frames: Option<u32>) -> Command {
    Command::View(ViewArgs {
        folder: folder.to_owned(),
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
