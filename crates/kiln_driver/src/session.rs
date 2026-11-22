//! Evaluation sessions opened from a driver command line
//! (spec/eval-session.md §2): the translation unit is parsed as
//! `-fsyntax-only` parses it (include paths, predefines, the resource
//! directory, `-std=`, `--target=`, `-W` options and constexpr limits
//! apply), with `__builtin_kiln_host_call` visible, and the finished
//! front end is handed to a closure as a [`SessionFrontEnd`].

use std::io::Write as _;
use std::sync::Arc;

use kiln_diag::{Diag, DiagId, DiagOptions, Format};
use kiln_lex::Interner;
use kiln_parse::{EvalSession, HostCalls, HostError, HostOut, OpenError};
use kiln_source::{FileId, SourceManager, Span};

use crate::args::{Args, Lang};
use crate::pp_cmd::Job;
use crate::syntax_cmd::SyntaxJob;
use crate::{
    Io, codegen_cmd, emit_arg_error, existing_inputs, input_std, make_engine, rsp, toolchain_of,
};

/// The finished front end of a translation unit with its session
/// (spec/eval-session.md §2). Fields may be added.
#[derive(Debug)]
#[non_exhaustive]
pub struct SessionFrontEnd<'a, 'sm> {
    /// The session.
    pub session: EvalSession<'a, 'sm>,
    /// The source manager (for diagnostics' locations).
    pub sources: &'sm SourceManager,
    /// The main file.
    pub main: Option<FileId>,
}

/// Why [`with_eval_session`] opened no session.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SessionRefused {
    /// The command line does not name exactly one C++ source input, or
    /// the driver rejected it (reported on standard error).
    CommandLine,
    /// The translation unit reported errors (reported on standard error,
    /// then `drv_eval_session_tu_errors`).
    TuHasErrors {
        /// How many.
        errors: u32,
    },
}

/// The translation unit's handler of a session opened here: every call
/// fails (runs get their own handler).
#[derive(Debug, Default)]
struct Placeholder;

impl HostCalls for Placeholder {
    fn call(
        &mut self,
        _op: u32,
        _ints: &[u64],
        _bytes: &[u8],
        _out: &mut HostOut<'_>,
    ) -> Result<u64, HostError> {
        Err(HostError::new("host calls are served only in evaluation-session runs"))
    }
}

/// Parses the one C++ source input of `command_line` (the driver's
/// arguments after `argv0`) as `-fsyntax-only` would, opens an
/// evaluation session on it and calls `f` with the front end, or with
/// why there is none. The translation unit's diagnostics go to `io`'s
/// standard error as the driver prints them; runs' diagnostics are
/// returned in their reports.
pub fn with_eval_session<R>(
    argv0: &str,
    command_line: &[String],
    io: Io,
    f: impl for<'a, 'sm> FnOnce(Result<SessionFrontEnd<'a, 'sm>, SessionRefused>) -> R,
) -> R {
    let stderr = io.stderr;
    let command_line = rsp::expand(command_line);
    let mut args = Args::parse_as(argv0, &command_line);
    args.host_call_test_handler = false;
    args.host_calls_visible = true;
    let mut diag_opts = DiagOptions::from_args(&args.diag_args);
    diag_opts.clang_compat = args.predef.clang_compat.major();
    let dsm = SourceManager::new();
    let drv = make_engine(
        &dsm,
        &codegen_cmd::driver_diag_options(&diag_opts),
        stderr.clone(),
        Some(&args.program),
    );
    for e in &args.errors {
        emit_arg_error(e, &drv);
    }
    let tc = toolchain_of(&args, &drv);
    let inputs = existing_inputs(&args, &drv);
    let input = match inputs.as_slice() {
        [i] if i.path != "-" && codegen_cmd::is_source(i) && args.input_lang(i) == Lang::Cxx => {
            Some(*i)
        }
        _ => None,
    };
    let std = match input.map(|_| input_std(&args, Lang::Cxx)) {
        Some(Ok(s)) => Some(s),
        Some(Err(d)) => {
            (*d).emit(&drv);
            None
        }
        None => None,
    };
    drv.flush();
    let (Some(input), Some(std)) = (input, std) else { return f(Err(SessionRefused::CommandLine)) };
    if drv.error_count() != 0 {
        return f(Err(SessionRefused::CommandLine));
    }
    let job = Job {
        args: &args,
        toolchain: &tc,
        std,
        path: &input.path,
        stdin_text: None,
        diag_opts: &diag_opts,
        stderr: stderr.clone(),
        asm: false,
    };
    let sm = SourceManager::new();
    let interner = Arc::new(Interner::new());
    let engine = make_engine(&sm, &diag_opts, stderr.clone(), None);
    let parsed = crate::crash::catching(&input.path, None, &sm, || {
        SyntaxJob(job).parse_retained(&sm, &engine, &interner, Box::new(Placeholder))
    });
    let summary = |engine: &kiln_diag::DiagEngine<'_>| {
        engine.flush();
        if let Some(s) = engine.summary()
            && diag_opts.format == Format::Text
        {
            let mut err = stderr.clone();
            let _ = writeln!(err, "{s}");
        }
    };
    let Some((sema, main)) = parsed else {
        summary(&engine);
        return f(Err(SessionRefused::CommandLine));
    };
    match EvalSession::open(sema) {
        Ok(session) => {
            engine.flush();
            f(Ok(SessionFrontEnd { session, sources: &sm, main }))
        }
        Err(e) => {
            summary(&engine);
            let errors = match e {
                OpenError::TuHasErrors { errors } => errors,
                _ => 1,
            };
            Diag::new(DiagId::drv_eval_session_tu_errors, Span::INVALID)
                .arg(errors.to_string())
                .emit(&drv);
            drv.flush();
            f(Err(SessionRefused::TuHasErrors { errors }))
        }
    }
}
