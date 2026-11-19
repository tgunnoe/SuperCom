//! The command line: a pure function from the arguments to a command.

use std::ffi::OsString;
use std::fmt;
use std::num::NonZeroU32;
use std::path::PathBuf;

/// The map folder `view` opens when none is named.
pub const DEFAULT_MAP: &str = "SCMP_012";

/// The slowest speed `replay` accepts, inclusive.
pub const MIN_SPEED: f64 = 0.1;

/// The fastest speed `replay` accepts, inclusive.
pub const MAX_SPEED: f64 = 10.0;

/// The speed of a replay when none is given.
pub const DEFAULT_SPEED: f64 = 1.0;

/// What the binary prints, after a reason, when its arguments are wrong.
pub const USAGE: &str = "usage: supercom\n       supercom view [<map folder>] [--frames <n>]\n       \
    supercom replay <file> [--speed <x>] [--frames <n>]\n\n\
    With no argument, reports whether the game data is found.\n\
    view opens a window on an installed map (default folder SCMP_012);\n\
    replay plays a recorded match in a window, at <x> times real time\n\
    (0.1 to 10, default 1);\n\
    --frames <n> presents n frames, prints the count, and exits.";

/// What the arguments ask for.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// No argument: report whether the game data is found.
    Bare,
    /// Open a window on a map.
    View(ViewArgs),
    /// Play a recorded match in a window.
    Replay(ReplayArgs),
}

/// The arguments of `view`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewArgs {
    /// The map folder under the install's `maps` directory.
    pub folder: String,
    /// The number of frames to present before exiting, if limited.
    pub frames: Option<NonZeroU32>,
}

/// The arguments of `replay`.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplayArgs {
    /// The recorded match.
    pub file: PathBuf,
    /// Times real time, from [`MIN_SPEED`] to [`MAX_SPEED`].
    pub speed: f64,
    /// The number of frames to present before exiting, if limited.
    pub frames: Option<NonZeroU32>,
}

/// The arguments are not a valid command line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageError {
    reason: String,
}

impl UsageError {
    fn new(reason: &str) -> Self {
        Self {
            reason: reason.to_owned(),
        }
    }
}

impl fmt::Display for UsageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "supercom: {}\n{USAGE}", self.reason)
    }
}

impl std::error::Error for UsageError {}

/// Parses the arguments after the program name.
///
/// # Errors
/// [`UsageError`] for any argument list other than none, `view` with an
/// optional single-component map folder and an optional `--frames <n>`, or
/// `replay` with one file and an optional `--speed <x>` and `--frames <n>`.
pub fn parse_args<I>(args: I) -> Result<Command, UsageError>
where
    I: IntoIterator,
    I::Item: Into<OsString>,
{
    let mut args = args.into_iter().map(Into::into);
    let Some(first) = args.next() else {
        return Ok(Command::Bare);
    };
    if first == "view" {
        parse_view(args)
    } else if first == "replay" {
        parse_replay(args)
    } else {
        Err(UsageError::new(&format!(
            "unknown argument {}",
            first.to_string_lossy()
        )))
    }
}

/// The value of `--frames`: the next argument as a positive count.
fn frames_value(
    args: &mut impl Iterator<Item = OsString>,
    frames: &mut Option<NonZeroU32>,
) -> Result<(), UsageError> {
    if frames.is_some() {
        return Err(UsageError::new("--frames is given twice"));
    }
    let count = args
        .next()
        .and_then(|value| value.into_string().ok())
        .and_then(|value| parse_count(&value));
    *frames = Some(count.ok_or_else(|| UsageError::new("--frames takes a positive integer"))?);
    Ok(())
}

fn parse_view(mut args: impl Iterator<Item = OsString>) -> Result<Command, UsageError> {
    let mut folder: Option<String> = None;
    let mut frames: Option<NonZeroU32> = None;
    while let Some(arg) = args.next() {
        let Ok(text) = arg.into_string() else {
            return Err(UsageError::new("an argument is not valid UTF-8"));
        };
        if text == "--frames" {
            frames_value(&mut args, &mut frames)?;
        } else if text.starts_with('-') {
            return Err(UsageError::new(&format!("unknown option {text}")));
        } else if folder.is_some() {
            return Err(UsageError::new("view takes at most one map folder"));
        } else if is_single_component(&text) {
            folder = Some(text);
        } else {
            return Err(UsageError::new(&format!(
                "{text:?} is not a single map folder name"
            )));
        }
    }
    Ok(Command::View(ViewArgs {
        folder: folder.unwrap_or_else(|| DEFAULT_MAP.to_owned()),
        frames,
    }))
}

fn parse_replay(mut args: impl Iterator<Item = OsString>) -> Result<Command, UsageError> {
    let mut file: Option<PathBuf> = None;
    let mut speed: Option<f64> = None;
    let mut frames: Option<NonZeroU32> = None;
    while let Some(arg) = args.next() {
        if arg == "--frames" {
            frames_value(&mut args, &mut frames)?;
        } else if arg == "--speed" {
            if speed.is_some() {
                return Err(UsageError::new("--speed is given twice"));
            }
            let value = args
                .next()
                .and_then(|value| value.into_string().ok())
                .and_then(|value| parse_speed(&value));
            speed = Some(value.ok_or_else(|| {
                UsageError::new("--speed takes a decimal from 0.1 to 10 in ASCII digits")
            })?);
        } else if arg.as_encoded_bytes().first() == Some(&b'-') {
            return Err(UsageError::new(&format!(
                "unknown option {}",
                arg.to_string_lossy()
            )));
        } else if file.is_some() {
            return Err(UsageError::new("replay takes exactly one file"));
        } else if arg.is_empty() {
            return Err(UsageError::new("the replay file is an empty argument"));
        } else {
            file = Some(PathBuf::from(arg));
        }
    }
    let file = file.ok_or_else(|| UsageError::new("replay takes a file"))?;
    Ok(Command::Replay(ReplayArgs {
        file,
        speed: speed.unwrap_or(DEFAULT_SPEED),
        frames,
    }))
}

/// A speed written as ASCII digits, optionally a point and more digits, from
/// [`MIN_SPEED`] to [`MAX_SPEED`] inclusive.
fn parse_speed(text: &str) -> Option<f64> {
    let (whole, fraction) = match text.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (text, None),
    };
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    if !digits(whole) || fraction.is_some_and(|part| !digits(part)) {
        return None;
    }
    text.parse::<f64>()
        .ok()
        .filter(|speed| (MIN_SPEED..=MAX_SPEED).contains(speed))
}

/// A positive integer of ASCII digits that fits 32 bits.
fn parse_count(text: &str) -> Option<NonZeroU32> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse::<u32>().ok().and_then(NonZeroU32::new)
}

/// Whether `text` is one non-empty path component: no separator, and
/// neither `.` nor `..`. A backslash is refused as well, so the rule is the
/// same on every platform.
pub fn is_single_component(text: &str) -> bool {
    !text.is_empty() && text != "." && text != ".." && !text.contains(['/', '\\', '\0'])
}

#[cfg(test)]
mod tests;
