//! The digest program of DET-024: prints the committed log's digest per
//! tick and the two cases, host-independent on stdout, with the raw NaN
//! bits and the measured depth on stderr.

use std::process::ExitCode;

use supercom_replay_harness::{HarnessError, report};

fn main() -> ExitCode {
    let Some(install) = supercom_testkit::game_dir() else {
        eprintln!("replay-digests: {}", HarnessError::NoInstall);
        return ExitCode::FAILURE;
    };
    match report(&install) {
        Ok(out) => {
            for line in &out.stderr {
                eprintln!("{line}");
            }
            for line in &out.stdout {
                println!("{line}");
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("replay-digests: {err}");
            ExitCode::FAILURE
        }
    }
}
