//! EXPERIMENTAL, undocumented: the whole-front-end PCH snapshot prototype
//! (WP-1383; spec/snap.md "Whole-compiler restore order"). Not a product
//! feature: no stable format, no user documentation, not in `--help`, off
//! unless `-fkiln-pch-snapshot=write|load` is given.
//!
//! - **Producer** (`-fkiln-pch-snapshot=write`): after the ordinary PCH
//!   job (record, image, diagnostics and exit status unchanged), a second
//!   front-end pass compiles an *empty* main file with the producer's
//!   `-include`s and then the header as `-include`s, parses to the end of
//!   that main file without the end-of-translation-unit work
//!   (`kiln_parse::parse_prefix`), closes it (`end_main_file`), and
//!   writes every root (source manager, interner, diagnostics engine,
//!   preprocessor, `Sema` with its AST and Oven) as one section
//!   (`bmi::SECTION_SNAPSHOT`) beside the record's others. Its
//!   diagnostics are discarded (the first pass printed them). A prefix
//!   with errors, or one whose state is not at rest, gets no section
//!   (`-v` note naming the field).
//! - **Consumer** (`-fkiln-pch-snapshot=load`): when the section is there
//!   and its identity matches ([`identity`]), the compile job restores
//!   the roots in order instead of replaying the header
//!   (`syntax_cmd`/`codegen_cmd` with a [`Restore`]); any mismatch or
//!   decode error replays, with a `-v` note.
//! - `KILN_SNAP_TIMING=1` prints the producer's encode time and the
//!   consumer's per-root load split on standard error.

use std::fmt::Write as _;
use std::sync::Arc;
use std::time::{Duration, Instant};

use kiln_diag::DiagEngine;
use kiln_lex::Interner;
use kiln_pp::Preprocessor;
use kiln_pp::bmi;
use kiln_sema::Sema;
use kiln_snap::{Reader, Schema, Snap, Writer};
use kiln_source::SourceManager;

use crate::args::{Args, PchSnapshotUse};
use crate::{SharedWriter, pch};

/// The schema hash of every root the section holds, in one schema: a
/// snapshot is read only by the build that wrote it.
pub(crate) fn schema_hash() -> u64 {
    let mut s = Schema::new();
    SourceManager::schema(&mut s);
    Arc::<Interner>::schema(&mut s);
    DiagEngine::snap_schema(&mut s);
    Preprocessor::snap_schema(&mut s);
    Sema::snap_schema(&mut s);
    s.hash()
}

/// Options left out of [`identity`] because they do not change the
/// front end's state at the end of the prefix: the inputs and outputs,
/// the action, dependency output, debug information, `-v`, the PCH
/// options themselves, dumps, and how diagnostics are shown. Each with
/// whether it takes a separate value.
const EXCLUDED: &[(&str, bool)] = &[
    ("-o", true),
    ("-x", true),
    ("-include", true),
    ("--include", true),
    ("-include-pch", true),
    ("-MF", true),
    ("-MT", true),
    ("-MQ", true),
    ("-MJ", true),
    ("-c", false),
    ("-S", false),
    ("-E", false),
    ("-fsyntax-only", false),
    ("-emit-llvm", false),
    ("-emit-pch", false),
    ("-v", false),
    ("-MD", false),
    ("-MMD", false),
    ("-MP", false),
    ("-MG", false),
    ("-fno-kiln-pch-image", false),
    ("-fno-kiln-pch-snapshot", false),
    ("-fcolor-diagnostics", false),
    ("-fno-color-diagnostics", false),
    // WP-1394: how the consumer's diagnostics are checked; a `-verify`
    // consumer restores (spec/driver.md "PCH snapshot: behaviour").
    ("-verify", false),
];

/// Prefixes of joined options left out of [`identity`] (see
/// [`EXCLUDED`]).
const EXCLUDED_PREFIXES: &[&str] = &[
    "-fkiln-pch-image",
    "-fkiln-pch-snapshot=",
    "-fdump-ast",
    "-ferror-limit=",
    "-fdiagnostics-color",
    "-verify=",
    "-verify-ignore-unexpected",
    "-g",
    "-include",
    "--include=",
    "-o",
    "-MF",
    "-MT",
    "-MQ",
    "-x",
];

/// Whether `arg` (not a value) is left out of [`identity`], and whether
/// it takes the next argument as its value.
fn excluded(arg: &str) -> Option<bool> {
    if let Some(&(_, sep)) = EXCLUDED.iter().find(|(o, _)| *o == arg) {
        return Some(sep);
    }
    EXCLUDED_PREFIXES.iter().any(|p| arg.starts_with(p)).then_some(false)
}

/// The prototype's identity of a compile (crude on purpose; the real
/// fingerprint is product work): the kiln build and version, the working
/// directory, and the full command line minus the input files and the
/// options of [`EXCLUDED`] / [`EXCLUDED_PREFIXES`] (also behind
/// `-Xclang`), in order. Producer and consumer must agree exactly.
pub(crate) fn identity(args: &Args) -> String {
    let mut out = format!(
        "kiln {} {}\ncwd {}\n",
        bmi::KILN_VERSION,
        bmi::kiln_sha(),
        std::env::current_dir().unwrap_or_default().display()
    );
    let words = &args.argv;
    let mut i = 0;
    while i < words.len() {
        let a = words[i].as_str();
        if a == "-Xclang"
            && let Some(inner) = words.get(i + 1)
            && let Some(sep) = excluded(inner)
        {
            // `-Xclang <opt>` and, for a separate value, `-Xclang <value>`.
            i += if sep { 4 } else { 2 };
            continue;
        }
        if let Some(sep) = excluded(a) {
            i += if sep { 2 } else { 1 };
            continue;
        }
        if args.inputs.iter().any(|inp| inp.path == a) {
            i += 1;
            continue;
        }
        out.push_str(a);
        out.push('\n');
        i += 1;
    }
    out
}

/// The section's head: the schema hash and the identity, in a stream of
/// their own (so the roots' sharing table starts with the roots).
fn head(args: &Args) -> Vec<u8> {
    let mut w = Writer::new();
    w.put(&schema_hash());
    w.put(&identity(args));
    w.into_bytes()
}

/// Whether `KILN_SNAP_TIMING` asks for the timing lines.
pub(crate) fn timing() -> bool {
    std::env::var_os("KILN_SNAP_TIMING").is_some_and(|v| !v.is_empty() && v != "0")
}

/// The producer's section: [`head`] and the roots `roots` (the stream
/// `syntax_cmd` wrote at the end of the prefix).
pub(crate) fn section(args: &Args, roots: &[u8]) -> Vec<u8> {
    let mut out = head(args);
    out.extend_from_slice(roots);
    out
}

/// `image` (the record's section block, if any) with the snapshot
/// section added.
pub(crate) fn add_section(image: Option<Vec<u8>>, snapshot: Vec<u8>) -> Result<Vec<u8>, String> {
    let mut sections = match image {
        Some(b) => bmi::read_sections(&b).map_err(|e| e.to_string())?,
        None => Vec::new(),
    };
    sections.push(bmi::Section { tag: bmi::SECTION_SNAPSHOT, bytes: snapshot });
    Ok(bmi::write_sections(&sections))
}

/// `-v`: a note about the snapshot of `pch` (producer or consumer).
pub(crate) fn note(args: &Args, stderr: &SharedWriter, pch: &str, what: &str) {
    if args.verbose {
        let mut err = stderr.clone();
        let _ = std::io::Write::write_fmt(
            &mut err,
            format_args!("note: precompiled header '{pch}': snapshot {what}\n"),
        );
    }
}

/// After the diagnostics engine `engine` is restored from `pch`'s
/// snapshot (WP-1394, spec/driver.md "PCH snapshot: behaviour"): the header's own
/// diagnostics are not shown again, nor counted; `-v` says how many
/// there were, and `KILN_SNAP_TIMING` prints the count for the
/// differential (`scripts/pch-snapshot-diff/consumer-only.awk`).
pub(crate) fn prefix_reported(
    args: &Args,
    stderr: &SharedWriter,
    pch: &str,
    engine: &DiagEngine<'_>,
) {
    let n = engine.prefix_reported();
    if timing() {
        eprintln!("snapshot-timing: the header reported {n} diagnostic(s) when it was built");
    }
    if n > 0 {
        let what = format!(
            "restored; the {n} warning(s) and error(s) its header reported when it was built are not shown again"
        );
        note(args, stderr, pch, &what);
    }
}

/// The consumer's snapshot of `pch` (whose record is `rec`), or why
/// there is none usable (the compile then replays).
pub(crate) fn find(
    args: &Args,
    pch: &str,
    rec: &pch::Record,
    includes: Vec<String>,
) -> Result<PchSnapshotUse, String> {
    let started = Instant::now();
    let Some(img) = &rec.image else { return Err("the PCH has no snapshot section".to_owned()) };
    let sections = bmi::read_sections(img).map_err(|e| format!("{e} in its section block"))?;
    let Some(s) = sections.into_iter().find(|s| s.tag == bmi::SECTION_SNAPSHOT) else {
        return Err("the PCH has no snapshot section".to_owned());
    };
    let sections_done = started.elapsed();
    let mut r = Reader::new(&s.bytes);
    let hash: u64 = r.get().map_err(|e| e.to_string())?;
    if hash != schema_hash() {
        return Err("written by another build (schema hash)".to_owned());
    }
    let id: String = r.get().map_err(|e| e.to_string())?;
    let mine = identity(args);
    if id != mine {
        let (a, b): (Vec<_>, Vec<_>) = (id.lines().collect(), mine.lines().collect());
        let at = a.iter().zip(&b).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
        return Err(format!(
            "the command line differs: '{}' (producer) vs '{}' (here)",
            a.get(at).unwrap_or(&"<end>"),
            b.get(at).unwrap_or(&"<end>")
        ));
    }
    let start = r.offset();
    let checked = started.elapsed();
    // Every file the producer read must still be the one it read: the
    // restored source manager re-reads them lazily.
    if let Some(f) = rec.files.iter().find(|f| {
        std::fs::read(&f.abs)
            .map_or(true, |b| b.len() as u64 != f.size || bmi::checksum(&b) != f.hash)
    }) {
        return Err(format!("'{}' changed", f.spelled));
    }
    if timing() {
        let validated = started.elapsed();
        eprintln!(
            "snapshot-timing: find sections {:.2} ms, identity {:.2} ms, file check {:.2} ms ({} files), section {} bytes",
            ms(sections_done),
            ms(checked.saturating_sub(sections_done)),
            ms(validated.saturating_sub(checked)),
            rec.files.len(),
            s.bytes.len()
        );
    }
    Ok(PchSnapshotUse { pch: pch.to_owned(), bytes: Arc::new(s.bytes), start, includes })
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// The per-root load timer of a restore (`KILN_SNAP_TIMING`).
#[derive(Debug)]
pub(crate) struct Timer {
    last: Instant,
    started: Instant,
    parts: Vec<(&'static str, Duration)>,
}

impl Timer {
    pub(crate) fn new() -> Timer {
        let now = Instant::now();
        let _ = kiln_ast::types::take_rebuild_time();
        Timer { last: now, started: now, parts: Vec::new() }
    }

    /// Ends the part `name` (the time since the previous mark).
    pub(crate) fn mark(&mut self, name: &'static str) {
        let now = Instant::now();
        self.parts.push((name, now - self.last));
        self.last = now;
    }

    /// Prints the split, with the hash-consing rebuild inside the AST.
    pub(crate) fn report(&self) {
        if !timing() {
            return;
        }
        let rebuild = kiln_ast::types::take_rebuild_time();
        let mut line = String::from("snapshot-timing: restore");
        for (n, d) in &self.parts {
            let _ = write!(line, " {n} {:.2} ms,", ms(*d));
        }
        let _ = write!(
            line,
            " (intern-table rebuild inside ast {:.2} ms) total {:.2} ms",
            ms(rebuild),
            ms(self.last.saturating_duration_since(self.started))
        );
        eprintln!("{line}");
    }
}

/// A consumer job's restore in progress: the roots' reader, positioned
/// after the source manager, interner and diagnostics engine, and what
/// the job still needs.
#[derive(Debug)]
pub(crate) struct Restore<'r> {
    pub reader: Reader<'r>,
    /// The `-include`s after the PCH.
    pub includes: &'r [String],
    /// Why the restore failed (the job then replays), set by the job.
    pub failed: Option<String>,
    pub timer: Timer,
}

/// Opens a snapshot: the source manager and the interner, restored
/// first (before anything interns or loads a file), and the reader
/// positioned at the diagnostics engine.
pub(crate) fn open(
    s: &PchSnapshotUse,
) -> Result<(SourceManager, Arc<Interner>, Restore<'_>), String> {
    let mut timer = Timer::new();
    let mut reader = Reader::new(&s.bytes[s.start..]);
    let sm: SourceManager = reader.get().map_err(|e| e.to_string())?;
    timer.mark("SourceManager");
    let interner: Arc<Interner> = reader.get().map_err(|e| e.to_string())?;
    timer.mark("Interner");
    Ok((sm, interner, Restore { reader, includes: &s.includes, failed: None, timer }))
}

/// The consumer's own `<built-in>` buffer reduced to its `-include`s
/// after the PCH (as [`kiln_pp::build_predefines`] writes them).
pub(crate) fn includes_buffer(includes: &[String]) -> String {
    let mut out = String::from("# 1 \"<built-in>\" 3\n# 1 \"<command line>\" 1\n");
    for f in includes {
        out.push_str("#include \"");
        for c in f.chars() {
            if matches!(c, '\\' | '"') {
                out.push('\\');
            }
            out.push(c);
        }
        out.push_str("\"\n");
    }
    out.push_str("# 1 \"<built-in>\" 2\n");
    out
}

impl crate::pp_cmd::Job<'_> {
    /// The front end of a consumer job rebuilt from a snapshot, in the
    /// restore order (spec/snap.md "Whole-compiler restore order"): the
    /// preprocessor configured from the consumer's command line, the
    /// snapshot applied, the main file entered (and the `-include`s after
    /// the PCH), then a fresh `Sema` with the AST, the Oven and its own
    /// fields restored. The source manager, interner and diagnostics
    /// engine are already restored. `None` with `restore.failed` set when
    /// the snapshot cannot be applied (nothing is reported then), or
    /// after reporting an error of the consumer's own inputs.
    #[allow(clippy::type_complexity, clippy::too_many_arguments)]
    pub(crate) fn restore_front<'a, 'd>(
        &self,
        sm: &'a SourceManager,
        engine: &'d DiagEngine<'a>,
        interner: &'a Arc<Interner>,
        inputs: crate::pp_cmd::PpInputs,
        restore: &mut Restore<'_>,
        target: &kiln_sema::target::Target,
        parse_options: impl FnOnce(Option<kiln_parse::PchLoad>) -> kiln_parse::ParseOptions,
    ) -> Option<(
        Sema<'d, 'a>,
        Preprocessor<'a, 'd>,
        Option<crate::pp_cmd::SharedDeps>,
        kiln_parse::ParseOptions,
    )> {
        let Some((mut pp, deps, main)) =
            self.configure_preprocessor(sm, engine, interner, inputs, true)
        else {
            restore.failed = Some("the preprocessor could not be configured".to_owned());
            return None;
        };
        let crate::pp_cmd::MainInput::File(main) = main else {
            restore.failed = Some("the main input is a module file".to_owned());
            return None;
        };
        if let Err(e) = pp.snap_restore(&mut restore.reader) {
            restore.failed = Some(format!("Preprocessor: {e}"));
            return None;
        }
        restore.timer.mark("Preprocessor");
        let entered = pp.enter_next_main_file(main).and_then(|()| {
            if restore.includes.is_empty() {
                Ok(())
            } else {
                pp.enter_builtin_includes(includes_buffer(restore.includes))
            }
        });
        if let Err(e) = entered {
            restore.failed = Some(format!("Preprocessor: {e}"));
            return None;
        }
        let popts = parse_options(None);
        let mut sema = kiln_parse::new_sema(
            Arc::clone(interner),
            engine,
            target.clone(),
            &popts,
            self.args.constexpr.limits(),
        );
        let timer = &mut restore.timer;
        let restored = sema.snap_restore_split(&mut restore.reader, &mut |part| {
            timer.mark(match part {
                "ast" => "Ast",
                "oven" => "Oven",
                _ => "Sema",
            });
        });
        if let Err(e) = restored {
            restore.failed = Some(format!("Sema: {e}"));
            return None;
        }
        if restore.reader.remaining() != 0 {
            restore.failed = Some(format!("{} trailing bytes", restore.reader.remaining()));
            return None;
        }
        Some((sema, pp, deps, popts))
    }
}

/// The producer's section for `header` (EXPERIMENTAL, WP-1383): a second
/// pass over an empty main file with the producer's `-include`s, then the
/// header, parsed to the end of the last one (`kiln_parse::parse_prefix`)
/// and written root by root. Its diagnostics are dropped: the header
/// itself was just checked. `Err`: why the PCH has no snapshot.
pub(crate) fn produce(
    args: &Args,
    tc: &crate::toolchain::Toolchain,
    std: crate::toolchain::LangStd,
    header: &str,
    diag_opts: &kiln_diag::DiagOptions,
) -> Result<Vec<u8>, String> {
    let mut a = args.clone();
    a.action = crate::args::Action::SyntaxOnly;
    a.includes.push(header.to_owned());
    a.deps.only = None;
    a.deps.side = None;
    a.deps.cdb_fragment = None;
    a.output = None;
    a.verbose = false;
    a.pch.header_main = false;
    a.pch.producing = false;
    a.pch.image = crate::args::PchImage::Off;
    let job = crate::pp_cmd::Job {
        args: &a,
        toolchain: tc,
        std,
        path: "-",
        stdin_text: Some(String::new()),
        diag_opts,
        stderr: SharedWriter::new(std::io::sink()),
        asm: false,
    };
    let roots = crate::syntax_cmd::SyntaxJob(job).snapshot()?;
    Ok(section(args, &roots))
}
