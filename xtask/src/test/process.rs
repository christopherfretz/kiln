//! Running a child process with captured output and a wall-clock timeout,
//! using only `std` (reader threads + `try_wait` polling).

use std::fmt;
use std::io::{self, Read};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// The result of a finished (or killed) child.
#[derive(Debug)]
pub struct Output {
    /// `None` if the process was killed for exceeding the timeout.
    pub status: Option<ExitStatus>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub elapsed: Duration,
}

impl Output {
    pub fn timed_out(&self) -> bool {
        self.status.is_none()
    }

    pub fn success(&self) -> bool {
        self.status.is_some_and(|s| s.success())
    }

    pub fn stdout_str(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    pub fn stderr_str(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }

    /// Exit code, or `None` if killed (by timeout or a signal).
    pub fn code(&self) -> Option<i32> {
        self.status.and_then(|s| s.code())
    }

    /// Describes how the process ended: `exit code 3`, `signal 11`, `timed out`.
    pub fn describe_end(&self) -> String {
        match self.status {
            None => format!("timed out after {:.1}s", self.elapsed.as_secs_f64()),
            Some(s) => match s.code() {
                Some(c) => format!("exit code {c}"),
                None => signal_of(s).map_or_else(
                    || "terminated abnormally".to_owned(),
                    |sig| format!("killed by signal {sig}"),
                ),
            },
        }
    }
}

#[cfg(unix)]
fn signal_of(s: ExitStatus) -> Option<i32> {
    std::os::unix::process::ExitStatusExt::signal(&s)
}

#[cfg(not(unix))]
fn signal_of(_: ExitStatus) -> Option<i32> {
    None
}

/// Renders a command for logs: program and arguments, space-separated, with
/// arguments containing whitespace or quotes single-quoted.
pub struct Shown<'a>(pub &'a Command);

impl fmt::Display for Shown<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let quote = |s: &str| {
            if s.is_empty() || s.contains(|c: char| c.is_whitespace() || "'\"\\$".contains(c)) {
                format!("'{}'", s.replace('\'', r"'\''"))
            } else {
                s.to_owned()
            }
        };
        write!(f, "{}", quote(&self.0.get_program().to_string_lossy()))?;
        for a in self.0.get_args() {
            write!(f, " {}", quote(&a.to_string_lossy()))?;
        }
        Ok(())
    }
}

/// Runs `cmd` with stdin closed, capturing stdout and stderr, killing it
/// after `timeout`.
pub fn run(cmd: &mut Command, timeout: Duration) -> io::Result<Output> {
    let start = Instant::now();
    let mut child =
        cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let out_reader = spawn_reader(&mut child, true);
    let err_reader = spawn_reader(&mut child, false);
    let mut delay = Duration::from_millis(1);
    let status = loop {
        if let Some(s) = child.try_wait()? {
            break Some(s);
        }
        if start.elapsed() >= timeout {
            // The child may exit between try_wait and kill; both are fine.
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        thread::sleep(delay);
        delay = (delay * 2).min(Duration::from_millis(25));
    };
    let stdout = out_reader.join().unwrap_or_default();
    let stderr = err_reader.join().unwrap_or_default();
    Ok(Output { status, stdout, stderr, elapsed: start.elapsed() })
}

fn spawn_reader(child: &mut Child, stdout: bool) -> thread::JoinHandle<Vec<u8>> {
    let mut pipe: Box<dyn Read + Send> = if stdout {
        Box::new(child.stdout.take().expect("stdout is piped"))
    } else {
        Box::new(child.stderr.take().expect("stderr is piped"))
    };
    thread::spawn(move || {
        let mut buf = Vec::new();
        // A read error (e.g. the child was killed) just truncates the capture.
        let _ = pipe.read_to_end(&mut buf);
        buf
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_and_reports_exit() {
        let o = run(
            Command::new("sh").args(["-c", "echo out; echo err >&2; exit 3"]),
            Duration::from_secs(10),
        )
        .unwrap();
        assert_eq!(o.stdout_str(), "out\n");
        assert_eq!(o.stderr_str(), "err\n");
        assert_eq!(o.code(), Some(3));
        assert_eq!(o.describe_end(), "exit code 3");
        assert!(!o.success() && !o.timed_out());
    }

    #[test]
    fn times_out() {
        let o = run(Command::new("sleep").arg("5"), Duration::from_millis(100)).unwrap();
        assert!(o.timed_out());
        assert!(o.elapsed < Duration::from_secs(4));
        assert!(o.describe_end().starts_with("timed out"));
    }

    #[test]
    fn shown_quotes() {
        let mut c = Command::new("cc");
        c.args(["-DX=a b", "plain", "it's"]);
        assert_eq!(Shown(&c).to_string(), r"cc '-DX=a b' plain 'it'\''s'");
    }
}
