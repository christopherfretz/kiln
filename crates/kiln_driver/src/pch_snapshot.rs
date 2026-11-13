//! The whole-front-end PCH snapshot (WP-1383, WP-1392; spec/driver.md
//! "Snapshots", spec/snap.md "Whole-compiler restore order"). Not in
//! `--help` and off unless `-fkiln-pch-snapshot[=write|load|check]` is
//! given, until it becomes the default.
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
//! - **Consumer** (`-fkiln-pch-snapshot=load`): when the section is there,
//!   its header ([`MAGIC`], [`FORMAT_VERSION`]) and schema hash are this
//!   build's, its configuration fingerprint matches ([`identity`]) and
//!   the files it read are unchanged ([`find`]), the compile job restores
//!   the roots in order instead of replaying the header
//!   (`syntax_cmd`/`codegen_cmd` with a [`Restore`]); any mismatch or
//!   decode error replays, with a `-v` note naming the first difference.
//!   `=check` also replays and compares ([`check_difference`]).
//! - `KILN_SNAP_TIMING=1` prints the producer's encode time and the
//!   consumer's per-root load split on standard error.

use std::fmt::Write as _;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use kiln_diag::DiagEngine;
use kiln_lex::Interner;
use kiln_pp::Preprocessor;
use kiln_pp::bmi;
use kiln_sema::Sema;
use kiln_snap::{Reader, Schema, Snap, Writer};
use kiln_source::SourceManager;

use crate::args::{Args, PchSnapshotUse, PchValidate};
use crate::{SharedWriter, pch};

/// The schema hash of every root the section holds, in one schema: a
/// snapshot is read only by the build that wrote it. Computed once per
/// process (it walks every type the roots reach).
pub(crate) fn schema_hash() -> u64 {
    static HASH: OnceLock<u64> = OnceLock::new();
    *HASH.get_or_init(|| {
        let mut s = Schema::new();
        SourceManager::schema(&mut s);
        Arc::<Interner>::schema(&mut s);
        DiagEngine::snap_schema(&mut s);
        Preprocessor::snap_schema(&mut s);
        Sema::snap_schema(&mut s);
        s.hash()
    })
}

/// The section's magic: its first 8 bytes.
pub(crate) const MAGIC: [u8; 8] = *b"KILNSNAP";
/// The section's format version (bytes 8..12, little-endian): the layout
/// of the head ([`head`]). It is independent of kiln's version; the
/// roots' layout is the schema hash's business.
pub(crate) const FORMAT_VERSION: u32 = 1;
/// The length of the magic and the format version.
const HEADER_LEN: usize = 12;

/// The fields of [`Args`] the fingerprint leaves out, reset to their
/// defaults before the rest is compared (spec/driver.md "Snapshots", the
/// identity table, gives the argument for each): they cannot change the
/// front end's state at the end of the prefix, or they are compared by
/// their effect in a component of their own ([`identity`]).
fn excluded(args: &Args) -> Args {
    let d = Args::default();
    Args {
        // By effect: the feature queries, and every parsed field below.
        argv: d.argv,
        // Inputs, outputs, the action and its reports.
        inputs: d.inputs,
        output: d.output,
        action: d.action,
        compile: d.compile,
        precompile: d.precompile,
        module_output: d.module_output,
        program: d.program,
        version: d.version,
        verbose: d.verbose,
        help: d.help,
        print_resource_dir: d.print_resource_dir,
        probe: d.probe,
        print_commands: d.print_commands,
        errors: d.errors,
        // Dumps and reports of the main file's compile.
        dump_ast: d.dump_ast,
        dump_ast_filter: d.dump_ast_filter,
        dump_record_layouts: d.dump_record_layouts,
        dump_vtable_layouts: d.dump_vtable_layouts,
        dump_cfg: d.dump_cfg,
        cfg_stats: d.cfg_stats,
        memory_report: d.memory_report,
        time_trace: d.time_trace,
        bmi_image_census: d.bmi_image_census,
        bmi_image_census_all: d.bmi_image_census_all,
        verify: d.verify,
        no_crash_diagnostics: d.no_crash_diagnostics,
        crash_diagnostics_dir: d.crash_diagnostics_dir,
        crash_for_testing: d.crash_for_testing,
        // Dependency output, the PCH options themselves, and the
        // `-include`s (the PCH's own are matched by the record; the
        // consumer's later ones are entered after the restore).
        deps: d.deps,
        pch: d.pch,
        includes: d.includes,
        // After the front end: debug information, the back end, the
        // pass pipeline, the assembler and the linker.
        debug: d.debug,
        backend: d.backend,
        hod: d.hod,
        disable_llvm_passes: d.disable_llvm_passes,
        asm: d.asm,
        link: d.link,
        quiet_unused_arguments: d.quiet_unused_arguments,
        modules_prune_interval: d.modules_prune_interval,
        modules_prune_after: d.modules_prune_after,
        // How diagnostics are shown, and the warning policy (compared by
        // effect).
        diag_args: d.diag_args,
        // Compared in components of their own, by effect where the
        // effect is computed.
        std: d.std,
        ansi: d.ansi,
        target: d.target,
        macros: d.macros,
        macro_prefix_map: d.macro_prefix_map,
        source_path_style: d.source_path_style,
        reproducible_time: d.reproducible_time,
        dirs: d.dirs,
        sysroot: d.sysroot,
        isysroot: d.isysroot,
        resource_dir: d.resource_dir,
        gcc_toolchain: d.gcc_toolchain,
        gcc_install_dir: d.gcc_install_dir,
        llvm_dir: d.llvm_dir,
        ..args.clone()
    }
}

/// The fingerprint of a compile (spec/driver.md "Snapshots"): named
/// components, compared in order, the first difference refusing the
/// snapshot. Each is derived from the parsed configuration (the
/// toolchain resolved, the predefines built, the warning policy built),
/// so options that reach the same state agree whatever their spelling;
/// every option not left out by [`excluded`] is in the last component.
pub(crate) fn identity(
    args: &Args,
    tc: &crate::toolchain::Toolchain,
    std: crate::toolchain::LangStd,
    diag_opts: &kiln_diag::DiagOptions,
) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut add = |name: &str, text: String| out.push((name.to_owned(), text));
    add("kiln build", format!("kiln {} {}", bmi::KILN_VERSION, bmi::kiln_sha()));
    add("working directory", std::env::current_dir().unwrap_or_default().display().to_string());
    add("language", format!("{std:?}"));
    let (features, _) = crate::features::resolve(args, &tc.target);
    add("target", format!("{}\n{features:?}", tc.target.triple));
    let rest = excluded(args);
    // The search list and the feature queries from the options as given
    // (`argv`, the directories).
    let full = probe_job(args, tc, std, diag_opts);
    add("include search path", full.search_list().verbose_text());
    let macros: String = args
        .macros
        .iter()
        .map(|m| match m {
            kiln_pp::CmdMacro::Define(d) => format!("-D{d}\n"),
            kiln_pp::CmdMacro::Undef(u) => format!("-U{u}\n"),
        })
        .collect();
    add("-D/-U list", macros);
    // The predefines without the `-D`/`-U`s (the component above), the
    // `-include`s and the PCH's lines.
    let predefines = {
        let sm = SourceManager::new();
        let e = crate::make_engine(&sm, diag_opts, SharedWriter::new(std::io::sink()), None);
        probe_job(&rest, tc, std, diag_opts)
            .predefines(&e)
            .unwrap_or_else(|| "<unreadable>".to_owned())
    };
    add("predefined macros", predefines);
    let fq = full.clang_features().map(|f| format!("{:?} {:?}", f.flags(), f.compat()));
    add("feature queries", fq.unwrap_or_default());
    add("warning policy", diag_opts.policy_fingerprint());
    let mut maps = String::new();
    for (old, new) in &args.macro_prefix_map {
        let _ = writeln!(maps, "{old}={new}");
    }
    add("prefix maps", maps);
    add("source path style", format!("{:?}", args.source_path_style));
    add(
        "reproducible time",
        format!("{:?}", crate::time::reproducible_epoch(args.reproducible_time)),
    );
    add("options", format!("{rest:#?}"));
    out
}

/// A job of `args` with no input, for what [`identity`] computes.
fn probe_job<'j>(
    args: &'j Args,
    tc: &'j crate::toolchain::Toolchain,
    std: crate::toolchain::LangStd,
    diag_opts: &'j kiln_diag::DiagOptions,
) -> crate::pp_cmd::Job<'j> {
    crate::pp_cmd::Job {
        args,
        toolchain: tc,
        std,
        path: "-",
        stdin_text: None,
        diag_opts,
        stderr: SharedWriter::new(std::io::sink()),
        asm: false,
    }
}

/// The first difference of the producer's identity `old` and this
/// compile's `new`: the component and, for a component of more than one
/// line, the first differing line ("why was my PCH not used").
pub(crate) fn first_difference(
    old: &[(String, String)],
    new: &[(String, String)],
) -> Option<String> {
    let clip = |s: &str| -> String {
        if s.chars().count() > 160 {
            format!("{}...", s.chars().take(160).collect::<String>())
        } else {
            s.to_owned()
        }
    };
    for i in 0..old.len().max(new.len()) {
        let (Some((on, ot)), Some((nn, nt))) = (old.get(i), new.get(i)) else {
            return Some("the identity has another number of components".to_owned());
        };
        if on != nn {
            return Some(format!(
                "the identity's components differ: '{on}' (producer) vs '{nn}' (here)"
            ));
        }
        if ot == nt {
            continue;
        }
        // The plural names ("predefined macros", "options") end in `s`.
        let verb = if on.ends_with('s') { "differ" } else { "differs" };
        let (a, b): (Vec<&str>, Vec<&str>) = (ot.lines().collect(), nt.lines().collect());
        // A list (each line ending in a newline) says where it differs.
        let list = ot.ends_with('\n') || nt.ends_with('\n');
        if !list && a.len() <= 1 && b.len() <= 1 {
            return Some(format!(
                "{on} {verb}: '{}' (producer) vs '{}' (here)",
                clip(a.first().unwrap_or(&"")),
                clip(b.first().unwrap_or(&""))
            ));
        }
        let at = a.iter().zip(&b).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
        return Some(format!(
            "{on} {verb} at line {}: '{}' (producer) vs '{}' (here)",
            at + 1,
            clip(a.get(at).unwrap_or(&"<end>")),
            clip(b.get(at).unwrap_or(&"<end>"))
        ));
    }
    None
}

/// The section's head: the magic and format version, then, in a stream
/// of their own (so the roots' sharing table starts with the roots), the
/// schema hash and the identity.
fn head(id: &[(String, String)]) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    let mut w = Writer::new();
    w.put(&schema_hash());
    w.put(&id.to_vec());
    out.extend_from_slice(&w.into_bytes());
    out
}

/// Whether `KILN_SNAP_TIMING` asks for the timing lines.
pub(crate) fn timing() -> bool {
    std::env::var_os("KILN_SNAP_TIMING").is_some_and(|v| !v.is_empty() && v != "0")
}

/// The producer's section: [`head`] with the producer's identity, and
/// the roots `roots` (the stream `syntax_cmd` wrote at the end of the
/// prefix).
pub(crate) fn section(
    args: &Args,
    tc: &crate::toolchain::Toolchain,
    std: crate::toolchain::LangStd,
    diag_opts: &kiln_diag::DiagOptions,
    roots: &[u8],
) -> Vec<u8> {
    let mut out = head(&identity(args, tc, std, diag_opts));
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

/// The section's header: the offset of its stream, or why it is not a
/// section this kiln reads (checked before anything is decoded).
fn check_header(bytes: &[u8]) -> Result<usize, String> {
    let Some(header) = bytes.get(..HEADER_LEN) else {
        return Err("its section is truncated (no header)".to_owned());
    };
    if header[..8] != MAGIC {
        return Err("its section is not a kiln snapshot (bad magic)".to_owned());
    }
    let mut v = [0u8; 4];
    v.copy_from_slice(&header[8..]);
    let v = u32::from_le_bytes(v);
    if v != FORMAT_VERSION {
        return Err(format!("its section has format version {v}, expected {FORMAT_VERSION}"));
    }
    Ok(HEADER_LEN)
}

/// Why the recorded file `f` is not the one the producer read, if it is
/// not: its size, then (with `mtime`) its modification time decides; a
/// file with another time is hashed and accepted when its text is the
/// same (clang's check, with the content check as the fallback rather
/// than an error).
fn file_changed(f: &pch::FileEntry, validate: PchValidate) -> Option<&'static str> {
    let Ok(meta) = std::fs::metadata(&f.abs) else { return Some("is gone") };
    if meta.len() != f.size {
        return Some("changed size");
    }
    if validate == PchValidate::Mtime && f.mtime != 0 && pch::mtime_ns(&meta) == f.mtime {
        return None;
    }
    match std::fs::read(&f.abs) {
        Ok(b) if b.len() as u64 == f.size && bmi::checksum(&b) == f.hash => None,
        Ok(_) => Some("changed"),
        Err(_) => Some("cannot be read"),
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
#[allow(clippy::too_many_arguments)]
pub(crate) fn find(
    args: &Args,
    tc: &crate::toolchain::Toolchain,
    std: crate::toolchain::LangStd,
    diag_opts: &kiln_diag::DiagOptions,
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
    let at = check_header(&s.bytes)?;
    let mut r = Reader::new(&s.bytes[at..]);
    let hash: u64 = r.get().map_err(|e| e.to_string())?;
    let ours = schema_hash();
    let hashed = started.elapsed();
    if hash != ours {
        return Err("written by another build (schema hash)".to_owned());
    }
    let id: Vec<(String, String)> = r.get().map_err(|e| e.to_string())?;
    let mine = identity(args, tc, std, diag_opts);
    if let Some(why) = first_difference(&id, &mine) {
        return Err(why);
    }
    let start = at + r.offset();
    let checked = started.elapsed();
    // Every file the producer read must still be the one it read: the
    // restored source manager re-reads them lazily.
    if let Some((f, why)) =
        rec.files.iter().find_map(|f| file_changed(f, args.pch.validate).map(|w| (f, w)))
    {
        return Err(format!("'{}' {why}", f.spelled));
    }
    if timing() {
        let validated = started.elapsed();
        eprintln!(
            "snapshot-timing: find sections {:.2} ms, schema hash {:.2} ms, identity {:.2} ms, file check {:.2} ms ({} files), section {} bytes",
            ms(sections_done),
            ms(hashed.saturating_sub(sections_done)),
            ms(checked.saturating_sub(hashed)),
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
    Ok(section(args, tc, std, diag_opts, &roots))
}

/// A standard error that keeps what is written (`=check`'s runs).
#[derive(Clone, Default)]
struct Capture(Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for Capture {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Capture {
    fn text(&self) -> String {
        let b = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        String::from_utf8_lossy(&b).into_owned()
    }
}

/// `-fkiln-pch-snapshot=check` (spec/driver.md "Snapshots"): the replay
/// arguments of a compile whose snapshot was found, which the job runs
/// with after [`check_difference`]; `None` for any other compile.
pub(crate) fn check_replay(args: &Args) -> Option<Args> {
    if args.pch.snapshot != crate::args::PchSnapshot::Check || args.pch.snapshot_use.is_none() {
        return None;
    }
    let mut a = args.clone();
    a.pch.snapshot_use = None;
    Some(a)
}

/// `=check`: the input compiled to its `-fdump-ast=locs` twice, once
/// with the snapshot restored and once replaying the header, and the
/// first difference: a snapshot that cannot be restored, then the
/// diagnostics (replay's may first repeat the header's own warnings,
/// which a restored compile does not show, as clang's PCH does not),
/// then the dump.
pub(crate) fn check_difference(
    args: &Args,
    tc: &crate::toolchain::Toolchain,
    std: crate::toolchain::LangStd,
    path: &str,
    stdin_text: Option<&str>,
    diag_opts: &kiln_diag::DiagOptions,
) -> Option<String> {
    let Some(snap) = &args.pch.snapshot_use else { return None };
    let mut a = args.clone();
    a.action = crate::args::Action::SyntaxOnly;
    a.dump_ast = Some("locs".to_owned());
    a.dump_ast_filter = None;
    a.verify = None;
    a.verbose = false;
    a.deps = crate::args::DepArgs::default();
    a.output = None;
    a.time_trace = crate::args::TimeTraceArgs::default();
    a.memory_report = false;
    let job = |a, cap: &Capture| crate::pp_cmd::Job {
        args: a,
        toolchain: tc,
        std,
        path,
        stdin_text: stdin_text.map(str::to_owned),
        diag_opts,
        stderr: SharedWriter::new(cap.clone()),
        asm: false,
    };
    let mut replay = a.clone();
    replay.pch.snapshot_use = None;
    let lcap = Capture::default();
    let restored = match crate::syntax_cmd::SyntaxJob(job(&a, &lcap)).run_restored(snap) {
        Ok(r) => r,
        Err(why) => return Some(format!("the snapshot cannot be restored ({why})")),
    };
    let rcap = Capture::default();
    let replayed = crate::syntax_cmd::SyntaxJob(job(&replay, &rcap)).run();
    if let Some(d) = diags_differ(&lcap.text(), &rcap.text()) {
        return Some(d);
    }
    first_line_difference(&restored.output, &replayed.output)
        .map(|d| format!("the AST dump differs at {d}"))
}

/// The first difference of the restored compile's diagnostics `load`
/// and the replay's `replay`, the `N warnings generated.` lines left
/// out: replay's must be the restored ones, preceded only by lines with
/// no error (the header's own warnings).
fn diags_differ(load: &str, replay: &str) -> Option<String> {
    let body = |t: &str| -> String {
        t.lines().filter(|l| !is_summary(l)).fold(String::new(), |mut s, l| {
            s.push_str(l);
            s.push('\n');
            s
        })
    };
    let (l, r) = (body(load), body(replay));
    if let Some(head) = r.strip_suffix(l.as_str())
        && !head.lines().any(|x| x.contains("error: "))
    {
        return None;
    }
    first_line_difference(&l, &r).map(|d| format!("the diagnostics differ at {d}"))
}

/// A `N warnings generated.` / `N warnings and M errors generated.` line.
fn is_summary(line: &str) -> bool {
    line.strip_suffix(" generated.").is_some_and(|s| {
        s.split(' ').all(|w| {
            w.chars().all(|c| c.is_ascii_digit())
                || matches!(w, "warning" | "warnings" | "error" | "errors" | "and")
        })
    })
}

/// `line N: '<snapshot>' (snapshot) vs '<replay>' (replay)` for the first
/// line where `a` and `b` differ.
fn first_line_difference(a: &str, b: &str) -> Option<String> {
    if a == b {
        return None;
    }
    let (x, y): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    let at = x.iter().zip(&y).position(|(p, q)| p != q).unwrap_or(x.len().min(y.len()));
    Some(format!(
        "line {}: '{}' (snapshot) vs '{}' (replay)",
        at + 1,
        x.get(at).unwrap_or(&"<end>"),
        y.get(at).unwrap_or(&"<end>")
    ))
}

/// Reports `=check`'s difference `what` for the PCH of `args`.
pub(crate) fn report_mismatch(
    args: &Args,
    diag_opts: &kiln_diag::DiagOptions,
    stderr: &SharedWriter,
    what: &str,
) {
    let pch = args.pch.snapshot_use.as_ref().map_or("", |s| s.pch.as_str());
    let sm = SourceManager::new();
    let opts = crate::codegen_cmd::driver_diag_options(diag_opts);
    let e = crate::make_engine(&sm, &opts, stderr.clone(), Some(&args.program));
    kiln_diag::Diag::new(kiln_diag::DiagId::drv_pch_snapshot_mismatch, kiln_source::Span::INVALID)
        .arg(pch)
        .arg(what)
        .emit(&e);
    e.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The identity of `kiln++ <argv>` for a C++ input.
    fn id(argv: &[&str]) -> Vec<(String, String)> {
        id_as(argv, crate::args::Lang::Cxx)
    }

    /// The identity of `kiln++ <argv>` for an input of language `lang`.
    fn id_as(argv: &[&str], lang: crate::args::Lang) -> Vec<(String, String)> {
        let argv: Vec<String> = argv.iter().map(|&a| a.to_owned()).collect();
        let a = Args::parse_as("kiln++", &argv);
        let mut d = kiln_diag::DiagOptions::from_args(&a.diag_args);
        d.clang_compat = a.predef.clang_compat.major();
        let sm = SourceManager::new();
        let e = crate::make_engine(&sm, &d, SharedWriter::new(std::io::sink()), None);
        let tc = crate::toolchain_of(&a, &e);
        let std = crate::input_std(&a, lang)
            .unwrap_or_else(|_| crate::toolchain::LangStd::default_for(true));
        identity(&a, &tc, std, &d)
    }

    /// The first difference between the identities of two command lines.
    fn diff(producer: &[&str], here: &[&str]) -> Option<String> {
        first_difference(&id(producer), &id(here))
    }

    fn named(producer: &[&str], here: &[&str], what: &str) {
        let d = diff(producer, here);
        assert!(
            d.as_deref().is_some_and(|d| d.starts_with(what)),
            "{producer:?} vs {here:?}: expected '{what}', got {d:?}"
        );
    }

    const BASE: &[&str] = &["-std=c++20", "-c", "t.cpp"];

    fn with<'a>(extra: &[&'a str]) -> Vec<&'a str> {
        [BASE, extra].concat()
    }

    /// WP-1392: each component refuses, named (spec/driver.md
    /// "Snapshots", the identity table).
    #[test]
    fn each_component_is_named() {
        named(BASE, &["-std=c++23", "-c", "t.cpp"], "language differs");
        named(
            &with(&["--target=aarch64-linux-gnu"]),
            &with(&["--target=x86_64-linux-gnu"]),
            "target differs",
        );
        named(BASE, &with(&["-I/nonexistent-wp1392", "-I/"]), "include search path differs");
        named(BASE, &with(&["-DA=1"]), "-D/-U list differs");
        named(
            &with(&["-DA=1"]),
            &with(&["-DA=2"]),
            "-D/-U list differs at line 1: '-DA=1' (producer) vs '-DA=2' (here)",
        );
        named(&with(&["-DA", "-UA"]), &with(&["-UA", "-DA"]), "-D/-U list differs at line 1");
        named(BASE, &with(&["-O2"]), "predefined macros differ");
        named(BASE, &with(&["-fno-rtti"]), "predefined macros differ");
        named(BASE, &with(&["-fexperimental-library"]), "feature queries differ");
        named(BASE, &with(&["-Wshadow"]), "warning policy differs");
        named(BASE, &with(&["-Werror"]), "warning policy differs");
        named(BASE, &with(&["-fmacro-prefix-map=/a=/b"]), "prefix maps differ");
        named(BASE, &with(&["-fkiln-source-path-style=gcc"]), "source path style differs");
        named(BASE, &with(&["-fkiln-reproducible-time"]), "reproducible time differs");
        named(BASE, &with(&["-fconstexpr-steps=1000"]), "options differ at line");
        named(BASE, &with(&["-fpermissive"]), "options differ at line");
    }

    /// The language component tells C from C++ and one C standard from
    /// another (WP-1394: C headers get snapshots).
    #[test]
    fn c_language_is_named() {
        use crate::args::Lang;
        let c = |argv: &[&str]| id_as(argv, Lang::C);
        let first = |a: Vec<(String, String)>, b: Vec<(String, String)>| {
            first_difference(&a, &b).unwrap_or_default()
        };
        assert!(
            first(c(&["-std=c17", "t.c"]), id(&["-std=c++17", "t.cpp"]))
                .starts_with("language differs")
        );
        assert!(
            first(c(&["-std=c11", "t.c"]), c(&["-std=c17", "t.c"])).starts_with("language differs")
        );
        assert!(
            first(c(&["-std=c17", "t.c"]), c(&["-std=gnu17", "t.c"]))
                .starts_with("language differs")
        );
        assert_eq!(first_difference(&c(&["-std=c17", "t.c"]), &c(&["-std=c17", "u.c"])), None);
    }

    /// Options that cannot change the prefix's state do not refuse, and
    /// neither does an order that cannot matter.
    #[test]
    fn irrelevant_options_match() {
        let same = |p: &[&str], h: &[&str]| assert_eq!(diff(p, h), None, "{p:?} vs {h:?}");
        same(BASE, &["-std=c++20", "-fsyntax-only", "u.cpp"]);
        same(BASE, &["-std=c++20", "-S", "-emit-llvm", "t.cpp", "-o", "x.ll"]);
        same(
            BASE,
            &[
                "-std=c++20",
                "-c",
                "t.cpp",
                "-g",
                "-v",
                "-MD",
                "-MF",
                "t.d",
                "-fdump-ast=locs",
                "-ferror-limit=3",
                "-fcolor-diagnostics",
                "-include",
                "extra.h",
                "-fkiln-pch-snapshot=load",
                "-fkiln-pch-validate=content",
                "-Qunused-arguments",
                "-verify=v1",
                "-verify-ignore-unexpected",
            ],
        );
        same(BASE, &with(&["-verify"]));
        same(
            &["-std=c++20", "-Wshadow", "-Wunused-variable", "t.cpp"],
            &["-std=c++20", "-Wunused-variable", "-Wshadow", "t.cpp"],
        );
        // The same standard, spelled two ways.
        same(&["-std=c++2c", "t.cpp"], &["-std=c++26", "t.cpp"]);
        named(
            &["-std=c++20", "-Wall", "-Wno-unused-variable", "t.cpp"],
            &["-std=c++20", "-Wno-unused-variable", "-Wall", "t.cpp"],
            "warning policy differs",
        );
    }

    /// The `-v` wording of a one-line component.
    #[test]
    fn one_line_wording() {
        let old = vec![("target".to_owned(), "a".to_owned())];
        let new = vec![("target".to_owned(), "b".to_owned())];
        assert_eq!(
            first_difference(&old, &new).as_deref(),
            Some("target differs: 'a' (producer) vs 'b' (here)")
        );
        assert_eq!(first_difference(&old, &old), None);
        assert!(first_difference(&old, &[]).is_some());
    }

    /// The section header is checked before anything is decoded.
    #[test]
    fn section_header() {
        let mut ok = MAGIC.to_vec();
        ok.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        assert_eq!(check_header(&ok), Ok(HEADER_LEN));
        assert!(check_header(&ok[..7]).unwrap_err().contains("truncated"));
        let mut foreign = ok.clone();
        foreign[..8].copy_from_slice(b"KILNBMI\0");
        assert!(check_header(&foreign).unwrap_err().contains("bad magic"));
        let mut old = MAGIC.to_vec();
        old.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(check_header(&old).unwrap_err(), "its section has format version 0, expected 1");
    }
}
