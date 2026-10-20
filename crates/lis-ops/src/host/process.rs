//! The one way probes run OS tools (M4 design brief §6.1, E4): an absolute binary path,
//! fixed arguments, no input, a 10-second timeout and bounded output. Any failure is
//! `None`, which the probes turn into an unknown fact, never into "ok".

use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// How long a probe may run.
const TIMEOUT: Duration = Duration::from_secs(10);
/// The most output read from a probe.
const MAX_OUTPUT_BYTES: u64 = 64 * 1024;
const POLL: Duration = Duration::from_millis(25);

/// The standard output of `program args`, when it exits successfully within the timeout
/// with UTF-8 output. `program` must be an absolute path and `args` fixed by the caller.
pub(crate) fn run(program: &'static str, args: &[&str]) -> Option<String> {
    run_with_timeout(program, args, TIMEOUT)
}

fn run_with_timeout(program: &'static str, args: &[&str], timeout: Duration) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let stdout = child.stdout.take()?;
    // Read on a separate thread so a chatty probe cannot block on a full pipe.
    let reader = thread::spawn(move || {
        let mut output = Vec::new();
        stdout
            .take(MAX_OUTPUT_BYTES)
            .read_to_end(&mut output)
            .map(|_| output)
    });
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < timeout => thread::sleep(POLL),
            Ok(None) | Err(_) => {
                // Killing an already exited child fails harmlessly; either way it is reaped.
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    let output = reader.join().ok()?.ok()?;
    if !status.success() {
        return None;
    }
    String::from_utf8(output).ok()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn a_successful_probe_returns_its_output() {
        assert_eq!(
            run("/bin/echo", &["synthetic"]).as_deref(),
            Some("synthetic\n")
        );
    }

    #[test]
    fn failures_missing_binaries_and_timeouts_are_none() {
        assert_eq!(run("/usr/bin/false", &[]), None);
        assert_eq!(run("/nonexistent/probe", &[]), None);
        assert_eq!(
            run_with_timeout("/bin/sleep", &["5"], Duration::from_millis(100)),
            None
        );
    }
}
