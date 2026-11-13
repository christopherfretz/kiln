//! The external commands of the codegen path (`spec/driver.md` "Code
//! generation"): `llc` (assembly, objects), `llvm-as` (bitcode) and
//! `clang++` (linking). Every subprocess the driver starts is built and run
//! here.
//!
//! A command that cannot start, exits non-zero or dies of a signal is a
//! driver diagnostic (clang's wording: `linker command failed with exit
//! code 1 (use -v to see invocation)`), never a panic.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command as Process, Stdio};

use kiln_diag::{Diag, DiagEngine, DiagId};
use kiln_source::Span;

use crate::SharedWriter;
use crate::llvm::LlvmError;
use crate::toolchain::{Tool, Toolchain};

/// One external command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    /// Which tool.
    pub tool: Tool,
    /// The executable, as found ([`Toolchain::find_tool`]); `None` if it
    /// does not exist (running it reports `unable to execute command`).
    pub program: Option<PathBuf>,
    /// The arguments.
    pub args: Vec<String>,
    /// Why there is no program when no LLVM installation was found:
    /// running the command reports that instead of `unable to execute`
    /// (spec/driver.md "LLVM discovery").
    pub missing_llvm: Option<LlvmError>,
}

impl Command {
    /// `tool` with no arguments yet.
    pub fn new(tc: &Toolchain, tool: Tool) -> Command {
        let program = tc.find_tool(tool);
        let missing_llvm = match &tc.llvm {
            Err(e) if program.is_none() => Some(e.clone()),
            _ => None,
        };
        Command { tool, program, args: Vec::new(), missing_llvm }
    }

    /// Appends an argument.
    pub fn arg(&mut self, a: impl Into<String>) -> &mut Command {
        self.args.push(a.into());
        self
    }

    /// The program as printed: its path, or its name when it was not found.
    pub fn program_text(&self) -> String {
        self.program
            .as_ref()
            .map_or_else(|| self.tool.name().to_owned(), |p| p.display().to_string())
    }

    /// The command line as clang prints it: a leading space, the program
    /// quoted, then the arguments, each quoted when `quote_all` (`-###`)
    /// or when it contains a space, `"`, `\` or `$` (`-v`); a newline ends
    /// it.
    pub fn render(&self, quote_all: bool) -> String {
        let mut s = String::from(" ");
        print_arg(&mut s, &self.program_text(), true);
        for a in &self.args {
            s.push(' ');
            print_arg(&mut s, a, quote_all);
        }
        s.push('\n');
        s
    }

    /// Runs the command with `input` on its standard input (or none).
    /// Its standard output goes to `stdout`, its standard error to
    /// `stderr`. Failures are reported on `drv`; returns success.
    pub fn run(
        &self,
        input: Option<&[u8]>,
        stdout: &mut dyn Write,
        stderr: &SharedWriter,
        drv: &DiagEngine<'_>,
    ) -> bool {
        let Some(program) = &self.program else {
            if let Some(e) = &self.missing_llvm {
                e.emit(drv);
                return false;
            }
            Diag::new(DiagId::drv_unable_to_execute, Span::INVALID).arg(self.tool.name()).emit(drv);
            return false;
        };
        let (status, out, err) = match spawn_and_wait(program, &self.args, input) {
            Ok(r) => r,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Diag::new(DiagId::drv_unable_to_execute, Span::INVALID)
                    .arg(program.display())
                    .emit(drv);
                return false;
            }
            Err(e) => {
                Diag::new(DiagId::drv_cannot_execute, Span::INVALID)
                    .arg(program.display())
                    .arg(e)
                    .emit(drv);
                return false;
            }
        };
        let _ = stdout.write_all(&out);
        let _ = stdout.flush();
        if !err.is_empty() {
            let mut e = stderr.clone();
            let _ = e.write_all(&err);
            let _ = e.flush();
        }
        if status.success() {
            return true;
        }
        match status.code() {
            Some(code) => Diag::new(DiagId::drv_command_failed, Span::INVALID)
                .arg(self.tool.stage())
                .arg(code)
                .emit(drv),
            None => Diag::new(DiagId::drv_command_signalled, Span::INVALID)
                .arg(self.tool.stage())
                .emit(drv),
        }
        false
    }
}

/// Runs `program` with `args` and `input` on its standard input (or
/// none), collecting its standard output and standard error; returns
/// its exit status and both. An error: it could not be started or
/// waited for.
pub(crate) fn spawn_and_wait(
    program: &std::path::Path,
    args: &[String],
    input: Option<&[u8]>,
) -> std::io::Result<(std::process::ExitStatus, Vec<u8>, Vec<u8>)> {
    let mut child = Process::new(program)
        .args(args)
        .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    // Feed standard input and drain standard error on their own
    // threads so that no pipe can fill up and deadlock the child.
    let child_in = child.stdin.take();
    let child_err = child.stderr.take();
    let mut child_out = child.stdout.take();
    let (out, err) = std::thread::scope(|s| {
        if let (Some(mut w), Some(text)) = (child_in, input) {
            s.spawn(move || {
                // A child that exits early closes the pipe; its exit
                // status says what went wrong.
                let _ = w.write_all(text);
            });
        }
        let err = s.spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut e) = child_err {
                let _ = e.read_to_end(&mut buf);
            }
            buf
        });
        let mut out = Vec::new();
        if let Some(o) = child_out.as_mut() {
            let _ = o.read_to_end(&mut out);
        }
        (out, err.join().unwrap_or_default())
    });
    let status = child.wait()?;
    Ok((status, out, err))
}

/// Appends `a` as `llvm::sys::printArg` does.
fn print_arg(s: &mut String, a: &str, quote: bool) {
    if !quote && !a.contains([' ', '"', '\\', '$']) {
        s.push_str(a);
        return;
    }
    s.push('"');
    for c in a.chars() {
        if matches!(c, '"' | '\\' | '$') {
            s.push('\\');
        }
        s.push(c);
    }
    s.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(program: Option<&str>, args: &[&str]) -> Command {
        Command {
            tool: Tool::Llc,
            program: program.map(PathBuf::from),
            args: args.iter().map(|a| (*a).to_owned()).collect(),
            missing_llvm: None,
        }
    }

    #[test]
    fn rendering() {
        let c = cmd(Some("/usr/bin/llc-19"), &["-O0", "a b", "x\"$\\"]);
        assert_eq!(c.render(true), " \"/usr/bin/llc-19\" \"-O0\" \"a b\" \"x\\\"\\$\\\\\"\n");
        assert_eq!(c.render(false), " \"/usr/bin/llc-19\" -O0 \"a b\" \"x\\\"\\$\\\\\"\n");
        assert_eq!(cmd(None, &[]).render(true), " \"llc\"\n");
    }

    #[test]
    fn missing_tool_is_a_diagnostic() {
        let sm = kiln_source::SourceManager::new();
        let buf = kiln_diag::SharedBuffer::new();
        let err = SharedWriter::new(buf.clone());
        let drv =
            crate::make_engine(&sm, &kiln_diag::DiagOptions::default(), err.clone(), Some("kiln"));
        let mut out = Vec::new();
        assert!(!cmd(None, &["-O0"]).run(Some(b"x"), &mut out, &err, &drv));
        drv.flush();
        assert_eq!(
            buf.contents(),
            "kiln: error: unable to execute command: Executable \"llc\" doesn't exist!\n"
        );
    }
}
