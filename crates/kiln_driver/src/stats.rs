//! The compile statistics log (`KILN_STATS_LOG=<path>`, spec/driver.md
//! "Compile statistics log", WP-1370): one JSON line per frontend job
//! (and one per link), appended when the job ends.
//!
//! **Off cost.** The driver reads the variable once per run
//! ([`log_path`]); unset, nothing else here runs: no phase sums, no walk
//! of the IR text.
//!
//! **Times** come from the clock readings the time trace takes (the
//! driver's `Frontend` / `EmitIR` / `PrintIR` phases and its children's
//! runs: [`kiln_source::trace::Phase`], `time_trace::InputTrace::run_child`),
//! summed per name by [`kiln_source::trace::phase_add`]; no trace file is
//! started. **Counts** are of the IR text the frontend produced (before
//! `opt`), read in one pass over its code part (the metadata section holds
//! no function or attribute group): exact for kiln's printer.
use crate::args::Args;
use crate::codegen_cmd::Phase;
use kiln_source::trace;
use std::ffi::{OsStr, OsString};
use std::fmt::Write as _;
use std::io::Write as _;
use std::time::Instant;

/// The variable naming the log file.
pub const ENV: &str = "KILN_STATS_LOG";

/// A line is never longer (a single `write` of at most this many bytes
/// to an `O_APPEND` file is not torn by concurrent writers).
pub const MAX_LINE: usize = 4096;

/// The log file, when `KILN_STATS_LOG` is set and not empty.
pub fn log_path() -> Option<OsString> {
    std::env::var_os(ENV).filter(|v| !v.is_empty())
}

/// The counts of the frontend's IR text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IrCounts {
    /// The text's length.
    pub ir_bytes: usize,
    /// `define` lines.
    pub functions_defined: usize,
    /// Definitions whose function attribute group holds `alwaysinline`.
    pub alwaysinline_defs: usize,
    /// `call` / `invoke` / `callbr` instructions whose attribute group
    /// holds `alwaysinline`.
    pub alwaysinline_callsites: usize,
    /// `define available_externally` lines.
    pub available_externally_defs: usize,
}

/// The `#N` attribute-group references of `line` outside quoted names.
fn group_refs(line: &str) -> impl Iterator<Item = u32> + '_ {
    let mut quoted = false;
    let mut out = Vec::new();
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'"' => quoted = !quoted,
            b'#' if !quoted && (i == 0 || b[i - 1].is_ascii_whitespace()) => {
                let digits = b[i + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
                let end = i + 1 + digits;
                if digits > 0
                    && b.get(end).is_none_or(|c| c.is_ascii_whitespace() || *c == b',')
                    && let Ok(n) = line[i + 1..end].parse()
                {
                    out.push(n);
                }
            }
            _ => {}
        }
        i += 1;
    }
    out.into_iter()
}

/// Whether the instruction line `l` (indented) is a call.
fn is_call(l: &str) -> bool {
    let l = l.trim_start();
    // `%x = ` before the opcode.
    let l = match l.split_once(" = ") {
        Some((lhs, rhs)) if lhs.starts_with('%') && !lhs.contains(' ') => rhs,
        _ => l,
    };
    let l = ["tail ", "musttail ", "notail "].iter().find_map(|p| l.strip_prefix(p)).unwrap_or(l);
    l.starts_with("call ") || l.starts_with("invoke ") || l.starts_with("callbr ")
}

impl IrCounts {
    /// Counts `ir` (the whole module text).
    pub fn of(ir: &str) -> IrCounts {
        let code = crate::codegen_cmd::code_part(ir);
        // Attribute groups follow the functions; read them first.
        let mut inline_groups: Vec<u32> = Vec::new();
        for l in code.lines().filter(|l| l.starts_with("attributes #")) {
            if l.split_ascii_whitespace().any(|w| w == "alwaysinline")
                && let Some(n) = group_refs(l).next()
            {
                inline_groups.push(n);
            }
        }
        let inline = |l: &str| group_refs(l).any(|n| inline_groups.contains(&n));
        let mut c = IrCounts { ir_bytes: ir.len(), ..IrCounts::default() };
        for l in code.lines() {
            if let Some(rest) = l.strip_prefix("define ") {
                c.functions_defined += 1;
                if rest.starts_with("available_externally ") {
                    c.available_externally_defs += 1;
                }
                if !inline_groups.is_empty() && inline(l) {
                    c.alwaysinline_defs += 1;
                }
            } else if !inline_groups.is_empty() && l.starts_with(' ') && is_call(l) && inline(l) {
                c.alwaysinline_callsites += 1;
            }
        }
        c
    }
}

/// One job's record while it runs: started with the job, written by
/// [`Job::finish`].
#[derive(Debug)]
pub struct Job {
    input: Option<String>,
    start: Instant,
    /// The `opt` triggers of the frontend's IR.
    pub triggers: Vec<&'static str>,
    /// The counts of the frontend's IR (`None`: no module).
    pub counts: Option<IrCounts>,
    /// A step of the job failed.
    pub failed: bool,
    /// The back end that made the job's code (spec/driver.md "Back-end
    /// selection"): `llvm` (`llc` ran), `hod`, `hod-declined`,
    /// `hod-failed`; `None` when none ran.
    pub backend: Option<&'static str>,
    /// hod's reason when it declined, or its status when it failed.
    pub hod_reason: Option<String>,
}

impl Job {
    /// Starts the job of `input` (`None`: the link) and the phase sums.
    pub fn start(input: Option<&str>) -> Job {
        trace::phases_start();
        Job {
            input: input.map(str::to_owned),
            start: Instant::now(),
            triggers: Vec::new(),
            counts: None,
            failed: false,
            backend: None,
            hod_reason: None,
        }
    }

    /// Records the frontend's IR text: its counts and `opt` triggers.
    pub fn module(&mut self, args: &Args, tc: &crate::toolchain::Toolchain, ir: &str) {
        self.counts = Some(IrCounts::of(ir));
        self.triggers = crate::codegen_cmd::opt_triggers(args, tc, ir);
    }

    /// Ends the job and appends its line to `log`. A log that cannot be
    /// written is ignored: the statistics never change a compile's output
    /// or exit status.
    pub fn finish(self, args: &Args, log: &OsStr) {
        let phases = trace::phases_finish();
        let total = self.start.elapsed();
        let line = self.line(args, &phases, u64::try_from(total.as_micros()).unwrap_or(u64::MAX));
        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).create(true).open(log) {
            // One `write` of the whole line (`write_all` loops only on a
            // short write, which a regular file does not do for 4 KiB).
            let _ = f.write_all(line.as_bytes());
        }
    }

    /// The JSON line (with its newline), at most [`MAX_LINE`] bytes.
    fn line(&self, args: &Args, phases: &[(&'static str, u64)], total_us: u64) -> String {
        let mut input = self.input.clone();
        let mut truncated = false;
        // Everything but the input path fits well within 1 KiB.
        if let Some(i) = &mut input
            && i.len() > MAX_LINE - 1024
        {
            let mut at = MAX_LINE - 1024;
            while !i.is_char_boundary(at) {
                at -= 1;
            }
            i.truncate(at);
            truncated = true;
        }
        let get = |n: &str| phases.iter().find(|(p, _)| *p == n).map_or(0, |(_, d)| *d);
        let mut o = String::with_capacity(512);
        o.push_str("{\"input\":");
        match &input {
            Some(i) => trace::json_str(&mut o, i),
            None => o.push_str("null"),
        }
        let kind = if self.input.is_none() { "link" } else { output_kind(args) };
        let _ = write!(o, ",\"output_kind\":\"{kind}\"");
        let _ = write!(o, ",\"opt_level\":\"{}\"", opt_level(args));
        let _ = write!(o, ",\"debug\":{}", args.debug.kind() != kiln_codegen::DebugInfoKind::None);
        let lto = match args.codegen.lto {
            None => "none",
            Some(false) => "full",
            Some(true) => "thin",
        };
        let _ = write!(o, ",\"lto\":\"{lto}\",\"triggers\":[");
        for (k, t) in self.triggers.iter().enumerate() {
            let sep = if k == 0 { "" } else { "," };
            let _ = write!(o, "{sep}\"{t}\"");
        }
        o.push(']');
        let count = |o: &mut String, name: &str, v: Option<usize>| {
            let _ = match v {
                Some(v) => write!(o, ",\"{name}\":{v}"),
                None => write!(o, ",\"{name}\":null"),
            };
        };
        let c = self.counts;
        count(&mut o, "alwaysinline_defs", c.map(|c| c.alwaysinline_defs));
        count(&mut o, "alwaysinline_callsites", c.map(|c| c.alwaysinline_callsites));
        count(&mut o, "available_externally_defs", c.map(|c| c.available_externally_defs));
        count(&mut o, "ir_bytes", c.map(|c| c.ir_bytes));
        count(&mut o, "functions_defined", c.map(|c| c.functions_defined));
        let (emit, print) = (get("EmitIR"), get("PrintIR"));
        let frontend = get("Frontend").saturating_sub(emit).saturating_sub(print);
        let _ = write!(o, ",\"frontend_ms\":{}", Ms(frontend));
        let _ = write!(o, ",\"emit_ir_ms\":{}", Ms(emit));
        let _ = write!(o, ",\"print_ir_ms\":{}", Ms(print));
        o.push_str(",\"children_ms\":{");
        let mut first = true;
        for (n, d) in
            phases.iter().filter(|(n, _)| !matches!(*n, "Frontend" | "EmitIR" | "PrintIR"))
        {
            let sep = if first { "" } else { "," };
            first = false;
            let _ = write!(o, "{sep}\"{n}\":{}", Ms(*d));
        }
        o.push('}');
        let _ = write!(o, ",\"total_ms\":{}", Ms(total_us));
        let _ = write!(o, ",\"exit\":{}", u8::from(self.failed));
        match self.backend {
            Some(b) => {
                let _ = write!(o, ",\"backend\":\"{b}\"");
            }
            None => o.push_str(",\"backend\":null"),
        }
        o.push_str(",\"hod_reason\":");
        match &self.hod_reason {
            // A reason is a short name; a failure's status text is too.
            Some(r) => {
                let mut at = r.len().min(128);
                while !r.is_char_boundary(at) {
                    at -= 1;
                }
                trace::json_str(&mut o, &r[..at]);
            }
            None => o.push_str("null"),
        }
        if truncated {
            o.push_str(",\"truncated\":true");
        }
        o.push_str("}\n");
        o
    }
}

/// µs printed as milliseconds with three decimals.
struct Ms(u64);

impl std::fmt::Display for Ms {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{:03}", self.0 / 1000, self.0 % 1000)
    }
}

/// The run's output kind: `obj`, `asm`, `llvm-ir`, `bitcode`, `link`
/// (the inputs of a linking run, and the link), `syntax-only`,
/// `precompile`.
fn output_kind(args: &Args) -> &'static str {
    match args.action {
        crate::args::Action::SyntaxOnly => return "syntax-only",
        crate::args::Action::Precompile => return "precompile",
        _ => {}
    }
    match Phase::of(args) {
        Phase::Object => "obj",
        Phase::Assembly => "asm",
        Phase::IrText => "llvm-ir",
        Phase::Bitcode => "bitcode",
        Phase::Link => "link",
    }
}

/// The `-O` level as written after `-O`: `0` when none is given, `1`
/// for a bare `-O`.
fn opt_level(args: &Args) -> &str {
    match args.opt_flag.as_deref().and_then(|f| f.strip_prefix("-O")) {
        None => "0",
        Some("") => "1",
        Some(l) => l,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IR: &str = "define available_externally void @a() #0 {\n  ret void\n}\n\
        define linkonce_odr void @b() #1 comdat {\n  call void @a() #2\n  \
        %3 = call i32 @\"q #1\"() #0, !dbg !4\n  \
        invoke void @a() #2\n          to label %4 unwind label %5\n  ret void\n}\n\
        define void @c() #0 {\n  tail call void @b() #1\n  ret void\n}\n\
        declare i32 @\"q #1\"() #3\n\
        attributes #0 = { mustprogress noinline optnone }\n\
        attributes #1 = { alwaysinline mustprogress }\n\
        attributes #2 = { alwaysinline }\n\
        attributes #3 = { nounwind }\n\
        !4 = !{}\n";

    #[test]
    fn counts_of_the_ir_text() {
        let c = IrCounts::of(IR);
        assert_eq!(c.ir_bytes, IR.len());
        assert_eq!(c.functions_defined, 3);
        assert_eq!(c.available_externally_defs, 1);
        assert_eq!(c.alwaysinline_defs, 1);
        // `call @a() #2`, `invoke @a() #2`, `tail call @b() #1`; the
        // quoted `#1` is a name, the call's group is `#0`.
        assert_eq!(c.alwaysinline_callsites, 3);
        assert_eq!(IrCounts::of("define void @f() {\n}\n").alwaysinline_defs, 0);
    }

    #[test]
    fn line_shape_and_bound() {
        let a = Args::parse(&["-c".to_owned(), "x.cpp".to_owned()]);
        let mut j = Job::start(Some("x.cpp"));
        j.triggers = vec!["alwaysinline", "coro"];
        j.counts = Some(IrCounts::of(IR));
        let l = j.line(&a, &[("Frontend", 5000), ("EmitIR", 1000), ("opt", 2500)], 9001);
        let _ = trace::phases_finish();
        assert!(l.starts_with("{\"input\":\"x.cpp\",\"output_kind\":\"obj\",\"opt_level\":\"0\""));
        assert!(l.contains("\"triggers\":[\"alwaysinline\",\"coro\"]"));
        assert!(l.contains("\"frontend_ms\":4.000,\"emit_ir_ms\":1.000,\"print_ir_ms\":0.000"));
        assert!(l.contains("\"children_ms\":{\"opt\":2.500},\"total_ms\":9.001,\"exit\":0,"));
        assert!(l.ends_with(",\"backend\":null,\"hod_reason\":null}\n"));
        assert!(trace::Json::parse(l.trim_end()).is_ok());
        let long = "d/".repeat(4000) + "x.cpp";
        let mut j = Job::start(Some(&long));
        j.failed = true;
        let l = j.line(&a, &[], 0);
        let _ = trace::phases_finish();
        assert!(
            l.len() <= MAX_LINE
                && l.ends_with(
                    ",\"exit\":1,\"backend\":null,\"hod_reason\":null,\"truncated\":true}\n"
                )
        );
        let mut j = Job::start(Some("x.cpp"));
        j.backend = Some("hod-declined");
        j.hod_reason = Some("alwaysinline:\"q\"".to_owned());
        let l = j.line(&a, &[("hod", 1)], 1);
        let _ = trace::phases_finish();
        assert!(l.ends_with(
            ",\"backend\":\"hod-declined\",\"hod_reason\":\"alwaysinline:\\\"q\\\"\"}\n"
        ));
        assert!(trace::Json::parse(l.trim_end()).is_ok());
        let link = Job::start(None).line(&a, &[("clang++", 1)], 1);
        let _ = trace::phases_finish();
        assert!(link.starts_with("{\"input\":null,\"output_kind\":\"link\""));
        assert!(link.contains("\"alwaysinline_defs\":null"));
    }
}
