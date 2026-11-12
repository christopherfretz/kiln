//! `-fkiln-pch-image=check` (WP-1036, spec/driver.md "Precompiled
//! headers"): a compile whose PCH carries a declaration image runs twice,
//! once as a `load` consumer (the record installed, the image built) and
//! once replaying the header, and the two are compared: the declarations
//! the translation unit ends with (its `-fdump-ast=all-files` declaration
//! nodes), the diagnostics, and for a compile the IR. The first
//! difference is an error; the replay's results are the job's.
//!
//! `-E`/`-M` with a PCH whose producer had `-include`s replay only the
//! header (as clang's `-E` does), so the record is checked by a separate
//! replay of the `-include`s and the header with no output.

use std::fmt::Write as _;
use std::io::Write;
use std::sync::{Arc, Mutex, PoisonError};

use kiln_diag::{Diag, DiagId, DiagOptions};
use kiln_source::{SourceManager, Span};

use crate::args::{Action, Args, PchImageUse};
use crate::pp_cmd::{Job, JobResult, SharedDeps};
use crate::syntax_cmd::SyntaxJob;
use crate::toolchain::{LangStd, Toolchain};
use crate::{SharedWriter, make_engine};

/// A standard error that keeps what is written.
#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Capture {
    fn writer(&self) -> SharedWriter {
        SharedWriter::new(self.clone())
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap_or_else(PoisonError::into_inner)).into_owned()
    }
}

/// One input's job, for both runs.
pub(crate) struct Input<'a> {
    pub tc: &'a Toolchain,
    pub std: LangStd,
    pub path: &'a str,
    pub stdin_text: Option<String>,
    pub diag_opts: &'a DiagOptions,
    pub stderr: &'a SharedWriter,
}

impl Input<'_> {
    fn job<'j>(&'j self, args: &'j Args, stderr: SharedWriter) -> Job<'j> {
        Job {
            args,
            toolchain: self.tc,
            std: self.std,
            path: self.path,
            stdin_text: self.stdin_text.clone(),
            diag_opts: self.diag_opts,
            stderr,
            asm: false,
        }
    }

    /// Reports `what` differs (the job then fails).
    fn mismatch(&self, args: &Args, what: &str) {
        let pch = args.pch.check_load.as_ref().map_or("", |c| c.image_use.pch.as_str());
        let sm = SourceManager::new();
        let opts = crate::codegen_cmd::driver_diag_options(self.diag_opts);
        let e = make_engine(&sm, &opts, self.stderr.clone(), Some(&args.program));
        Diag::new(DiagId::drv_pch_decls_mismatch, Span::INVALID).arg(pch).arg(what).emit(&e);
        e.flush();
    }

    /// The declarations each run ends with, or the first difference.
    fn decls(&self, args: &Args, load: &Args) -> Result<(), String> {
        let dump = |a: &Args| {
            let mut a = a.clone();
            a.dump_ast = Some("all-files".to_owned());
            a.dump_ast_filter = None;
            a.verify = None;
            let cap = Capture::default();
            let r = SyntaxJob(self.job(&a, cap.writer())).run();
            decl_lines(&r.output)
        };
        match first_difference(&dump(load), &dump(args)) {
            Some(d) => Err(format!("the declarations differ at {d}")),
            None => Ok(()),
        }
    }
}

/// `-fsyntax-only` with `check`: `None` when the job is not checked.
pub(crate) fn syntax_only(args: &Args, input: &Input<'_>) -> Option<JobResult> {
    let load = args.pch_load_variant()?;
    let decls = input.decls(args, &load);
    let lcap = Capture::default();
    let lr = SyntaxJob(input.job(&load, lcap.writer())).run();
    let rcap = Capture::default();
    let mut r = SyntaxJob(input.job(args, rcap.writer())).run();
    let (ltext, rtext) = (lcap.text(), rcap.text());
    let _ = input.stderr.clone().write_all(rtext.as_bytes());
    let diff = decls.err().or_else(|| diags(&ltext, &rtext)).or_else(|| {
        (lr.output != r.output).then(|| {
            first_difference(&lr.output, &r.output).map_or_else(
                || "the output differs".to_owned(),
                |d| format!("the output differs at {d}"),
            )
        })
    });
    if let Some(d) = diff {
        input.mismatch(args, &d);
        r.errors += 1;
    }
    Some(r)
}

/// A compile's front end with `check`: `None` when the job is not
/// checked, else the replay's IR (`None` after an error) and dependency
/// list.
pub(crate) fn codegen(
    args: &Args,
    input: &Input<'_>,
) -> Option<(Option<String>, Option<SharedDeps>)> {
    let load = args.pch_load_variant()?;
    let decls = input.decls(args, &load);
    let lcap = Capture::default();
    let (lir, _, _) = crate::codegen_cmd::CodegenJob(input.job(&load, lcap.writer())).run();
    let rcap = Capture::default();
    let (ir, _, deps) = crate::codegen_cmd::CodegenJob(input.job(args, rcap.writer())).run();
    let (ltext, rtext) = (lcap.text(), rcap.text());
    let _ = input.stderr.clone().write_all(rtext.as_bytes());
    let diff = decls.err().or_else(|| diags(&ltext, &rtext)).or_else(|| {
        let (l, r) = (lir.as_deref().unwrap_or(""), ir.as_deref().unwrap_or(""));
        first_difference(l, r).map(|d| format!("the IR differs at {d}"))
    });
    match diff {
        Some(d) => {
            input.mismatch(args, &d);
            Some((None, deps))
        }
        None => Some((ir, deps)),
    }
}

/// `-E`/`-M` with `check` and a producer with `-include`s: replays the
/// `-include`s and the header with the record compared at their end
/// (`checkpoint`), the output dropped; `false` after reporting a
/// difference.
pub(crate) fn preprocess_record(
    args: &Args,
    input: &Input<'_>,
    includes: Vec<String>,
    record: PchImageUse,
) -> bool {
    let mut a = args.clone();
    a.action = Action::Preprocess;
    a.includes = includes;
    a.pch.include = None;
    a.pch.image_use = Some(record);
    a.deps = crate::args::DepArgs::default();
    a.output = None;
    a.verbose = false;
    a.time_trace = crate::args::TimeTraceArgs::default();
    let cap = Capture::default();
    let job = Job { path: "-", stdin_text: Some(String::new()), ..input.job(&a, cap.writer()) };
    let _ = job.run();
    // The header's own diagnostics come again from the real job: only
    // the record's are passed on.
    let text = cap.text();
    let mut ok = true;
    let mut err = input.stderr.clone();
    for line in text.lines().filter(|l| l.contains("preprocessor-state record of precompiled")) {
        let _ = writeln!(err, "{line}");
        ok = false;
    }
    ok
}

/// The diagnostics' first difference.
fn diags(load: &str, replay: &str) -> Option<String> {
    first_difference(load, replay).map(|d| format!("the diagnostics differ at {d}"))
}

/// The declaration nodes of an AST dump, each with its depth (the tree
/// drawing dropped: a declaration whose last child the image does not
/// keep, an enumerator's written value, draws differently). A
/// `static_assert` has no effect once checked and is not carried; a
/// constant's value (` value=`) is what replay evaluated while parsing
/// the header, which the image's consumer evaluates when it is used.
fn decl_lines(dump: &str) -> String {
    let mut out = String::new();
    for line in dump.lines() {
        let body = line.trim_start_matches(['|', '`', '-', ' ']);
        let depth = (line.len() - body.len()) / 2;
        let node = body.split(' ').next().unwrap_or("");
        if node.ends_with("Decl") && node != "StaticAssertDecl" {
            let body = body.split_once(" value=").map_or(body, |(b, _)| b);
            let _ = writeln!(out, "{depth} {body}");
        }
    }
    out
}

/// Where `load`'s text first differs from `replay`'s: `line N: '<load>'
/// (image) vs '<replay>' (replay)`.
fn first_difference(load: &str, replay: &str) -> Option<String> {
    if load == replay {
        return None;
    }
    let cut = |s: &str| {
        let s = s.trim_end();
        match s.char_indices().nth(160) {
            Some((i, _)) => format!("{}...", &s[..i]),
            None => s.to_owned(),
        }
    };
    let (mut image, mut replayed) = (load.lines(), replay.lines());
    let mut line = 1;
    loop {
        match (image.next(), replayed.next()) {
            (Some(x), Some(y)) if x == y => line += 1,
            (None, None) => return Some(format!("line {line}: line endings")),
            (x, y) => {
                let show = |t: Option<&str>| {
                    t.map_or_else(|| "end of text".to_owned(), |t| format!("'{}'", cut(t)))
                };
                return Some(format!("line {line}: {} (image) vs {} (replay)", show(x), show(y)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn differences() {
        assert_eq!(first_difference("a\nb\n", "a\nb\n"), None);
        assert_eq!(
            first_difference("a\nb\n", "a\nc\n").as_deref(),
            Some("line 2: 'b' (image) vs 'c' (replay)")
        );
        assert_eq!(
            first_difference("a\n", "a\nc\n").as_deref(),
            Some("line 2: end of text (image) vs 'c' (replay)")
        );
    }

    #[test]
    fn decl_nodes() {
        let dump = "TranslationUnitDecl\n|-EnumDecl E\n| `-EnumConstantDecl A 'E'\n|   `-ConstantExpr 'int' value=5\n`-StaticAssertDecl\n";
        assert_eq!(
            decl_lines(dump),
            "0 TranslationUnitDecl\n1 EnumDecl E\n2 EnumConstantDecl A 'E'\n"
        );
    }
}
