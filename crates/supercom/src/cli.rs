//! The command line: a pure function from the arguments to a command.

use std::ffi::OsString;
use std::fmt;
use std::num::NonZeroU32;

/// The map folder `view` opens when none is named.
#[expect(dead_code, reason = "the view subcommand uses it")]
pub const DEFAULT_MAP: &str = "SCMP_012";

/// What the binary prints, after a reason, when its arguments are wrong.
pub const USAGE: &str = "usage: supercom\n       supercom view [<map folder>] [--frames <n>]\n\n\
    With no argument, reports whether the game data is found.\n\
    view opens a window on an installed map (default folder SCMP_012);\n\
    --frames <n> presents n frames, prints the count, and exits.";

/// What the arguments ask for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    /// No argument: report whether the game data is found.
    Bare,
    /// Open a window on a map.
    #[cfg_attr(not(test), expect(dead_code, reason = "the parser builds it"))]
    View(ViewArgs),
}

/// The arguments of `view`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewArgs {
    /// The map folder under the install's `maps` directory.
    pub folder: String,
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
/// [`UsageError`] for any argument list other than none, or `view` with an
/// optional single-component map folder and an optional `--frames <n>`.
pub fn parse_args<I>(args: I) -> Result<Command, UsageError>
where
    I: IntoIterator,
    I::Item: Into<OsString>,
{
    if args.into_iter().next().is_none() {
        Err(UsageError::new("the seam parses nothing"))
    } else {
        Ok(Command::Bare)
    }
}

#[cfg(test)]
mod tests;
