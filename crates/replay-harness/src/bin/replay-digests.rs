//! The digest program of DET-024: prints the committed log's digest per
//! tick and the two cases, host-independent on stdout, with the raw NaN
//! bits and the measured depth on stderr.

use std::process::ExitCode;

fn main() -> ExitCode {
    eprintln!("replay-digests: not implemented");
    ExitCode::FAILURE
}
