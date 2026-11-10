//! The GCC oracle of `oracle: gcc` tests (`spec/harness.md` "GCC oracle").
//!
//! GCC (`KILN_GCC`, default `g++-16`) compiles the test once, with the
//! test's flags mapped by [`crate::oracle::gcc_args`]: `-fsyntax-only` for
//! `compile-ok`/`compile-fail` (its accept/reject verdict must be the
//! test's), a link and one run of the binary for `run` (its stdout and exit
//! status must be kiln's binary's). IR and diagnostic wording are never
//! compared. A GCC crash, internal compiler error, timeout, memory kill or
//! `sorry, unimplemented` never fails kiln (WP-102): the comparison is
//! dropped and the result carries `advisory: oracle crashed (...)`.

use super::*;

/// What GCC made of the test.
pub(super) enum Verdict {
    Accepted,
    /// GCC rejected it cleanly, with this stderr.
    Rejected(String),
    /// GCC crashed, hung or gave up: no verdict ([`Test::advisory`] is set).
    Advisory,
}

impl Test<'_> {
    /// The GCC `spec` is compared with in this run: it says `oracle: gcc`,
    /// kiln is the compiler under test, a GCC is configured, and the test
    /// has no `module:`/`unit:`/`header-unit:`/`pch:` lines (GCC's module
    /// and PCH flags differ).
    pub(super) fn gcc_applies(&self, spec: &TestSpec) -> Option<PathBuf> {
        if spec.gcc
            && self.cfg.compiler_kind == CompilerKind::Kiln
            && spec.modules.is_empty()
            && spec.units.is_empty()
            && spec.header_units.is_empty()
            && spec.pch.is_none()
        {
            self.cfg.gcc.clone()
        } else {
            None
        }
    }

    /// `<gcc> [-x lang] -std=<std> <mode...> <mapped args...> <file> [-o out]`.
    fn gcc_command(
        &self,
        gcc: &Path,
        spec: &TestSpec,
        mode: &[&str],
        out: Option<&Path>,
    ) -> Command {
        let mut c = Command::new(gcc);
        c.current_dir(&self.cfg.root);
        match (self.case.lang, self.case.is_header) {
            (Lang::C, _) => {
                c.args(["-x", "c"]);
            }
            (Lang::Cxx, true) => {
                c.args(["-x", "c++"]);
            }
            (Lang::Cxx, false) => {}
        }
        c.arg(format!("-std={}", spec.std));
        c.args(mode);
        c.args(self.meta_header_flags(false));
        let args: Vec<String> =
            spec.args.iter().map(|a| directives::substitute(a, |n| self.placeholder(n))).collect();
        c.args(crate::oracle::gcc_args(&args));
        c.arg(&self.case.name);
        if let Some(o) = out {
            c.arg("-o").arg(o);
        }
        c
    }

    /// Compiles the test with `gcc` under the test's cap. Failing to start
    /// GCC makes the test broken (a configured GCC must run).
    fn gcc_compile(
        &mut self,
        gcc: &Path,
        spec: &TestSpec,
        mode: &[&str],
        out: Option<&Path>,
    ) -> Result<Verdict, Problem> {
        let cmd = self.gcc_command(gcc, spec, mode, out);
        let shown = Shown(&cmd).to_string();
        let cap = self.mem_limit;
        let o = self
            .run_capped(&shown, &cmd, None, cap, self.cfg.compile_timeout)
            .map_err(|e| Problem::Broken(format!("could not run the GCC oracle `{shown}`: {e}")))?;
        if let Some(why) = advisory_cause(&o) {
            self.log.push(format!("# the GCC oracle: {why}; its comparison is dropped"));
            self.add_advisory(format!("advisory: oracle crashed ({why})"));
            return Ok(Verdict::Advisory);
        }
        self.check_capped("the GCC oracle", &o, cap)?;
        self.gcc_compared = true;
        Ok(if o.success() { Verdict::Accepted } else { Verdict::Rejected(o.stderr_str()) })
    }

    /// The GCC oracle judges a compile test's verdict whether or not kiln
    /// met it (so an `xfail(kiln)` test's expectation is still checked).
    pub(super) fn with_gcc_verdict(&mut self, spec: &TestSpec, outcome: Check) -> Check {
        if matches!(outcome, Ok(()) | Err(Problem::Fail(_)))
            && matches!(spec.expect, Expect::CompileOk | Expect::CompileFail)
        {
            self.check_gcc_verdict(spec).and(outcome)
        } else {
            outcome
        }
    }

    /// `compile-ok`/`compile-fail`: GCC's verdict must be the test's; if it
    /// is not, the expectation is in doubt and the test is broken (ERROR,
    /// never masked by `xfail`; `skip(gcc)` is not the fix, a wrong test is).
    fn check_gcc_verdict(&mut self, spec: &TestSpec) -> Check {
        let Some(gcc) = self.gcc_applies(spec) else { return Ok(()) };
        let verdict = self.gcc_compile(&gcc, spec, &["-fsyntax-only"], None)?;
        match (spec.expect, verdict) {
            (Expect::CompileOk, Verdict::Rejected(stderr)) => Err(Problem::Broken(format!(
                "the GCC oracle ({}) rejects this compile-ok test\n{}",
                gcc.display(),
                indent(&tail(&stderr, 20))
            ))),
            (Expect::CompileFail, Verdict::Accepted) => Err(Problem::Broken(format!(
                "the GCC oracle ({}) accepts this compile-fail test",
                gcc.display()
            ))),
            _ => Ok(()),
        }
    }

    /// `run`: GCC builds and runs the test once; its binary's stdout and exit
    /// status must be kiln's (`got`). Differences are appended to `failures`.
    pub(super) fn gcc_run(
        &mut self,
        gcc: &Path,
        spec: &TestSpec,
        stdin: Option<&[u8]>,
        got: &Output,
        failures: &mut String,
    ) -> Check {
        let exe = self.scratch()?.join("gcc.out");
        match self.gcc_compile(gcc, spec, &[], Some(&exe))? {
            Verdict::Advisory => return Ok(()),
            Verdict::Rejected(stderr) => {
                return Err(Problem::Broken(format!(
                    "the GCC oracle ({}) rejects this run test\n{}",
                    gcc.display(),
                    indent(&tail(&stderr, 20))
                )));
            }
            Verdict::Accepted => {}
        }
        let theirs = self.execute_binary(&exe, spec, stdin, true)?;
        if theirs.code().is_none() {
            return Err(Problem::Broken(format!(
                "the GCC oracle's binary {}\nstdout:\n{}stderr:\n{}",
                theirs.describe_end(),
                indent(&theirs.stdout_str()),
                indent(&theirs.stderr_str())
            )));
        }
        if theirs.stdout != got.stdout {
            let _ = write!(
                failures,
                "stdout differs from the GCC oracle (-gcc +actual):\n{}",
                indent(&line_diff(&theirs.stdout_str(), &got.stdout_str()))
            );
        }
        if theirs.code() != got.code() {
            let _ = writeln!(
                failures,
                "exit differs from the GCC oracle: gcc {}, actual {}",
                theirs.describe_end(),
                got.describe_end()
            );
        }
        Ok(())
    }
}

/// Why a GCC compile has no verdict, if it has none: it timed out, ran out
/// of memory, died of a signal, reported an internal compiler error, or
/// gave up on an unimplemented feature (`sorry, unimplemented`).
pub(super) fn advisory_cause(o: &Output) -> Option<String> {
    if o.timed_out() {
        return Some("timeout".into());
    }
    if o.out_of_memory() {
        return Some("out of memory".into());
    }
    if o.code().is_none() {
        return Some(o.describe_end());
    }
    let stderr = o.stderr_str();
    if stderr.contains("internal compiler error") {
        return Some("internal compiler error".into());
    }
    if stderr.contains("sorry, unimplemented") {
        return Some("sorry, unimplemented".into());
    }
    None
}
