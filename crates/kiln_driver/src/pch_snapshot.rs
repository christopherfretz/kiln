//! The whole-front-end PCH snapshot (WP-1383, WP-1392; spec/pch.md,
//! spec/snap.md "Whole-compiler restore order"). On by
//! default since WP-1404: every PCH build writes it and every consumer
//! restores it, replaying the header on any refusal;
//! `-fno-kiln-pch-snapshot` turns it off on either side.
//!
//! - **Producer** (the default, `-fkiln-pch-snapshot[=write|check]`): a
//!   single pass that stands in for the header pass where it may
//!   ([`single_pass_blocker`], [`stands_in`]); otherwise, after the
//!   ordinary PCH job (record, diagnostics and exit status
//!   unchanged), a second front-end pass compiles an *empty* main file
//!   with the producer's `-include`s and then the header as `-include`s,
//!   parses to the end of that main file without the
//!   end-of-translation-unit work (`kiln_parse::parse_prefix`), closes it (`end_main_file`), and
//!   writes every root (source manager, interner, diagnostics engine,
//!   preprocessor, `Sema` with its AST and Oven) as one section
//!   (`bmi::SECTION_SNAPSHOT`) beside the record's others. Its
//!   diagnostics are discarded (the first pass printed them). A prefix
//!   with errors gets no PCH; one whose state is not at rest gets no
//!   section, a `-v` note naming the field and one
//!   `-Wpch-snapshot-unused` warning (WP-1404).
//! - **Consumer** (the default, `-fkiln-pch-snapshot[=load]`): when the section is there,
//!   its header ([`MAGIC`], [`FORMAT_VERSION`]) and schema hash are this
//!   build's, its configuration fingerprint matches ([`identity`]) and
//!   the files it read are unchanged ([`find`]), the compile job restores
//!   the roots in order instead of replaying the header
//!   (`syntax_cmd`/`codegen_cmd` with a [`Restore`]); any mismatch or
//!   decode error replays, with a `-v` note naming the first difference.
//!   `=check` also replays and compares ([`check_difference`]).
//! - `KILN_SNAP_TIMING=1` prints the producer's encode time and the
//!   consumer's per-root load split on standard error.

use std::cell::RefCell;
use std::fmt::Write as _;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use kiln_diag::DiagEngine;
use kiln_lex::Interner;
use kiln_pp::Preprocessor;
use kiln_pp::bmi;
use kiln_sema::Sema;
use kiln_snap::{Reader, Writer};
use kiln_source::SourceManager;

use crate::args::{Args, PchSnapshotUse, PchValidate, SnapshotRoots};
use crate::{SharedWriter, pch};

/// The schema hash of every root the section holds: a snapshot is read
/// only by the build that wrote it. A constant (WP-1401): each root's
/// [`kiln_snap::SchemaHash`], folded at compile time from its fields'
/// constants, so a load walks nothing.
pub(crate) const SCHEMA_HASH: u64 = kiln_snap::SchemaHash::new("snapshot", "roots")
    .field::<SourceManager>("source manager")
    .field::<Arc<Interner>>("interner")
    .root_field("diagnostics engine", DiagEngine::SNAP_SCHEMA_HASH)
    .root_field("preprocessor", Preprocessor::SNAP_SCHEMA_HASH)
    .root_field("sema", Sema::SNAP_SCHEMA_HASH)
    .finish();

/// The descriptor walk of the same roots ([`SCHEMA_HASH`]'s readable
/// form): the descriptors and the constants their types declare, for
/// the cross-check of the two (`schema_constants_cover_every_type`).
#[cfg(test)]
pub(crate) fn schema() -> kiln_snap::Schema {
    use kiln_snap::Snap as _;
    let mut s = kiln_snap::Schema::new();
    SourceManager::schema(&mut s);
    Arc::<Interner>::schema(&mut s);
    DiagEngine::snap_schema(&mut s);
    Preprocessor::snap_schema(&mut s);
    Sema::snap_schema(&mut s);
    s
}

/// The process's phase marks for `KILN_SNAP_TIMING` (WP-1401): each
/// [`phase`] call while the timer is on adds the time since the previous
/// mark under its name; [`phase_report`] prints the line.
static PHASES: Mutex<Vec<(&'static str, Instant)>> = Mutex::new(Vec::new());

/// Marks the start of the process for the phase timer (`main`, before
/// anything else runs). A no-op unless `KILN_SNAP_TIMING` is set.
pub fn process_started() {
    if timing() {
        phase("start");
    }
}

/// Ends the phase `name` of the process (`KILN_SNAP_TIMING`).
pub(crate) fn phase(name: &'static str) {
    if timing() {
        PHASES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((name, Instant::now()));
    }
}

/// Prints the process's phases (`snapshot-timing: process ...`), each the
/// time since the previous mark, then the total since the first. Called
/// once, just before the process exits.
pub fn phase_report() {
    if !timing() {
        return;
    }
    phase("exit");
    let marks =
        std::mem::take(&mut *PHASES.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
    let Some(&(_, first)) = marks.first() else { return };
    let mut line = String::from("snapshot-timing: process");
    for w in marks.windows(2) {
        let _ = write!(line, " {} {:.2} ms,", w[1].0, ms(w[1].1 - w[0].1));
    }
    let last = marks.last().map_or(first, |m| m.1);
    let _ = write!(line, " total {:.2} ms", ms(last - first));
    eprintln!("{line}");
}

/// The section's magic: its first 8 bytes.
pub(crate) const MAGIC: [u8; 8] = *b"KILNSNAP";
/// The section's format version (bytes 8..12, little-endian): the layout
/// of the head ([`head`]) and, since WP-1397, the schema: a change of
/// [`SCHEMA_HASH`] bumps it, and `crates/kiln_driver/pch-snapshot-format.txt`
/// records every (version, schema hash) pair (spec/pch.md "Format
/// versioning"; the `format_record` test). It is independent of kiln's
/// version. Version 2 (WP-1403) added the lookup log to the head;
/// version 3 (WP-1411) is a new diagnostic id (`DiagId`'s schema hash).
pub(crate) const FORMAT_VERSION: u32 = 3;
/// The length of the magic and the format version.
pub(crate) const HEADER_LEN: usize = 12;

/// The fields of [`Args`] the fingerprint leaves out, reset to their
/// defaults before the rest is compared (spec/pch.md, the
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
        // WP-1403: these only drop directories from the search list
        // (and the `<meta>` hint after a lookup that failed, which a
        // prefix with a snapshot has not had); `-stdlib=` also reaches
        // sema and stays in `options`.
        nostdinc: d.nostdinc,
        nostdincxx: d.nostdincxx,
        nostdlibinc: d.nostdlibinc,
        nobuiltininc: d.nobuiltininc,
        // WP-1403: compared by the prefix's `#embed` / `__has_embed`
        // lookups, which are repeated with the consumer's list.
        embed_dirs: d.embed_dirs,
        resource_dir: d.resource_dir,
        gcc_toolchain: d.gcc_toolchain,
        gcc_install_dir: d.gcc_install_dir,
        llvm_dir: d.llvm_dir,
        // WP-1404: `-O0` is what no `-O` gives, to every reader of the
        // field (the predefines, the sanitizers, code generation), so a
        // PCH built without it matches a consumer with it. Other levels
        // stay as spelled (their predefines are a component too).
        opt_flag: args.opt_flag.clone().filter(|f| f != "-O0"),
        ..args.clone()
    }
}

/// The fingerprint of a compile (spec/pch.md): named
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
    identity_and_list(args, tc, std, diag_opts).0
}

/// The name of the identity component of the search list, compared by
/// the prefix's lookups when its text differs (WP-1403).
pub(crate) const SEARCH_PATH: &str = "include search path";
/// The name of the identity component of the working directory, left
/// out when the prefix does not depend on it (WP-1403).
pub(crate) const WORKING_DIRECTORY: &str = "working directory";

/// [`identity`] and the search list it describes.
fn identity_and_list(
    args: &Args,
    tc: &crate::toolchain::Toolchain,
    std: crate::toolchain::LangStd,
    diag_opts: &kiln_diag::DiagOptions,
) -> (Vec<(String, String)>, kiln_pp::SearchList) {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut add = |name: &str, text: String| out.push((name.to_owned(), text));
    add("kiln build", format!("kiln {} {}", bmi::KILN_VERSION, bmi::kiln_sha()));
    add(WORKING_DIRECTORY, std::env::current_dir().unwrap_or_default().display().to_string());
    add("language", format!("{std:?}"));
    let (features, _) = crate::features::resolve(args, &tc.target);
    add("target", format!("{}\n{features:?}", tc.target.triple));
    let rest = excluded(args);
    // The search list and the feature queries from the options as given
    // (`argv`, the directories).
    let full = probe_job(args, tc, std, diag_opts);
    let list = full.search_list();
    // The `--embed-dir` list is part of it (the `#embed` lookups are
    // repeated with it), so equal text is still the fast path.
    let mut search = list.identity_text();
    for d in &args.embed_dirs {
        let _ = writeln!(search, "--embed-dir={d}");
    }
    add(SEARCH_PATH, search);
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
    (out, list)
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
fn head(id: &[(String, String)], lookups: &kiln_pp::LookupLog) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    let mut w = Writer::new();
    w.put(&SCHEMA_HASH);
    w.put(&id.to_vec());
    w.put(lookups);
    out.extend_from_slice(&w.into_bytes());
    out
}

/// Whether `KILN_SNAP_TIMING` asks for the timing lines.
pub(crate) fn timing() -> bool {
    std::env::var_os("KILN_SNAP_TIMING").is_some_and(|v| !v.is_empty() && v != "0")
}

/// What the producer's snapshot pass wrote at the end of the prefix: the
/// roots' stream and the prefix's header lookups (WP-1403).
#[derive(Debug)]
pub(crate) struct Roots {
    pub bytes: Vec<u8>,
    pub lookups: kiln_pp::LookupLog,
}

/// The producer's section: [`head`] with the producer's identity and
/// lookup log, and the roots (the stream `syntax_cmd` wrote at the end
/// of the prefix).
pub(crate) fn section(
    args: &Args,
    tc: &crate::toolchain::Toolchain,
    std: crate::toolchain::LangStd,
    diag_opts: &kiln_diag::DiagOptions,
    roots: &Roots,
) -> Vec<u8> {
    let mut out = head(&identity(args, tc, std, diag_opts), &roots.lookups);
    if timing() {
        eprintln!(
            "snapshot-timing: producer lookup log: {} lookups, {} directories, {} bytes",
            roots.lookups.lookups.len(),
            roots.lookups.dirs.len(),
            {
                let mut w = Writer::new();
                w.put(&roots.lookups);
                w.into_bytes().len()
            }
        );
    }
    out.extend_from_slice(&roots.bytes);
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
/// `KILN_SNAP_TIMING` also prints it as `snapshot-timing: note: <what>`,
/// so the differential can tell why a consumer replayed without `-v`'s
/// banner (WP-1395, spec/harness.md "PCH differential").
pub(crate) fn note(args: &Args, stderr: &SharedWriter, pch: &str, what: &str) {
    if timing() {
        eprintln!("snapshot-timing: note: {what}");
    }
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
/// snapshot (WP-1394, spec/pch.md "Consumer"): the header's own
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

/// After a restored compile (WP-1396, spec/pch.md "Consumer"): a file of the prefix (one of the first `restored`
/// buffers) whose text the compile read and that no longer holds the
/// text the snapshot was built from ends the compile with a fatal error.
/// An edit that keeps a file's size and modification time passes
/// `mtime` validation; the restored state was built from the old text,
/// so the compile cannot go on. The buffer is unavailable (no line or
/// column of it was printed). True when it reported one.
pub(crate) fn changed_files(
    sm: &SourceManager,
    restored: usize,
    pch: &str,
    engine: &DiagEngine<'_>,
) -> bool {
    if timing() {
        let (n, bytes) = sm.lazy_reads(restored);
        eprintln!("snapshot-timing: prefix files read after the restore: {n} ({bytes} bytes)");
    }
    let changed = sm
        .lazy_fallbacks()
        .into_iter()
        .find(|&(f, how)| f.index() < restored && how == kiln_source::LazyOutcome::Unavailable);
    let Some((f, _)) = changed else { return false };
    kiln_diag::Diag::new(kiln_diag::DiagId::drv_pch_file_modified, kiln_source::Span::INVALID)
        .arg(sm.file_name(f))
        .arg(pch)
        .arg("content changed")
        .emit(engine);
    true
}

/// Why a consumer does not use a PCH's snapshot (it replays the header).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Refusal {
    /// The `-v` note's reason.
    pub why: String,
    /// A configuration reason (another kiln build, an identity component,
    /// a lookup or a recorded file that changed): the compile warns
    /// (`-Wpch-snapshot-unused`, spec/pch.md). The other
    /// refusals are by design or a section that cannot be read: `-v` only.
    pub configuration: bool,
}

impl Refusal {
    /// A refusal noted under `-v` only.
    pub(crate) fn quiet(why: impl Into<String>) -> Refusal {
        Refusal { why: why.into(), configuration: false }
    }

    /// A configuration refusal (warned of).
    fn configuration(why: impl Into<String>) -> Refusal {
        Refusal { why: why.into(), configuration: true }
    }
}

/// `components` without those named in `skip`.
fn without<'c>(components: &'c [(String, String)], skip: &[&str]) -> Vec<&'c (String, String)> {
    components.iter().filter(|(n, _)| !skip.contains(&n.as_str())).collect()
}

/// The text of the component `name` of `components`.
fn component<'c>(components: &'c [(String, String)], name: &str) -> Option<&'c str> {
    components.iter().find(|(n, _)| n == name).map(|(_, t)| t.as_str())
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
) -> Result<PchSnapshotUse, Refusal> {
    let started = Instant::now();
    let no_section = || Refusal::quiet("the PCH has no snapshot section");
    // The block's sections where they are in the record's image (no copy
    // of a section that may be tens of MB), or in the file when the
    // record left the image there (WP-1401).
    let (sections, img_len) = match (&rec.image, &rec.image_at) {
        (Some(img), _) => (bmi::section_ranges(img).map_err(|e| e.to_string()), img.len()),
        (None, Some(at)) => (at.sections(pch), at.len),
        (None, None) => return Err(no_section()),
    };
    let sections = sections.map_err(|e| Refusal::quiet(format!("{e} in its section block")))?;
    let Some((_, range)) = sections.into_iter().find(|s| s.0 == bmi::SECTION_SNAPSHOT) else {
        return Err(no_section());
    };
    // The section's start: all of it in memory, or from the file what the
    // header and the identity take (the whole section if they take more).
    let head;
    let section: &[u8] = match (&rec.image, &rec.image_at) {
        (Some(img), _) => &img[range.clone()],
        (None, Some(at)) => {
            head = at
                .read(pch, range.start, range.len().min(1 << 16))
                .map_err(|e| Refusal::quiet(e.to_string()))?;
            &head
        }
        (None, None) => return Err(no_section()),
    };
    let sections_done = started.elapsed();
    let head = match snapshot_head(args, tc, std, diag_opts, section, started, rec) {
        // A decoding error in a part of the section: read all of it.
        Err((_, true)) if section.len() < range.len() => {
            let whole = rec.image_at.as_ref().map_or_else(
                || Ok(Vec::new()),
                |at| {
                    at.read(pch, range.start, range.len())
                        .map_err(|e| Refusal::quiet(e.to_string()))
                },
            )?;
            snapshot_head(args, tc, std, diag_opts, &whole, started, rec).map_err(|e| e.0)?
        }
        r => r.map_err(|e| e.0)?,
    };
    let Head { start, hashed, lookups, list, same_cwd, same_list } = head;
    let checked = started.elapsed();
    // Every file the producer read must still be the one it read: the
    // restored source manager re-reads them lazily.
    if let Some((f, why)) =
        rec.files.iter().find_map(|f| file_changed(f, args.pch.validate).map(|w| (f, w)))
    {
        return Err(Refusal::configuration(format!("'{}' {why}", f.spelled)));
    }
    let validated = started.elapsed();
    // WP-1403: another search list (or working directory) gives the same
    // prefix when every lookup the prefix performed finds what it found.
    if !same_list {
        let known: std::collections::HashSet<&str> =
            rec.files.iter().map(|f| f.spelled.as_str()).collect();
        let checked = lookups.check(
            &list,
            &args.embed_dirs,
            &mut kiln_pp::RealFs,
            &|p| known.contains(p),
            same_cwd,
        );
        if let Err(why) = checked {
            return Err(Refusal::configuration(format!("{SEARCH_PATH}: {why}")));
        }
    }
    if timing() {
        let looked = started.elapsed();
        eprintln!(
            "snapshot-timing: find sections {:.2} ms, schema hash {:.2} ms, identity {:.2} ms, file check {:.2} ms ({} files), lookups {} ({} lookups), section {} bytes",
            ms(sections_done),
            ms(hashed.saturating_sub(sections_done)),
            ms(checked.saturating_sub(hashed)),
            ms(validated.saturating_sub(checked)),
            rec.files.len(),
            if same_list {
                "not repeated (the same search list)".to_owned()
            } else {
                format!("repeated {:.2} ms", ms(looked.saturating_sub(validated)))
            },
            lookups.lookups.len(),
            range.len()
        );
    }
    let block_sum = match (&rec.image, &rec.image_at) {
        (Some(img), _) => img.get(img_len.saturating_sub(8)..).and_then(|t| t.try_into().ok()),
        (None, Some(at)) => Some(at.block_sum),
        (None, None) => None,
    };
    let roots = match file_roots(pch, img_len, block_sum, range.clone(), start) {
        Some(r) => r,
        None => SnapshotRoots::Memory {
            bytes: Arc::new(match (&rec.image, &rec.image_at) {
                (Some(img), _) => img[range.start + start..range.end].to_vec(),
                (None, Some(at)) => at
                    .read(pch, range.start + start, range.len() - start)
                    .map_err(|e| Refusal::quiet(e.to_string()))?,
                (None, None) => return Err(no_section()),
            }),
            start: 0,
        },
    };
    Ok(PchSnapshotUse { pch: pch.to_owned(), roots, includes })
}

/// What [`snapshot_head`] read and decided.
struct Head {
    /// Where the roots start in the section.
    start: usize,
    /// When the schema hash was read.
    hashed: Duration,
    /// The prefix's lookups (WP-1403).
    lookups: kiln_pp::LookupLog,
    /// This compile's search list.
    list: kiln_pp::SearchList,
    /// The working directory is the producer's.
    same_cwd: bool,
    /// The search list and the working directory are the producer's: the
    /// lookups need not be repeated.
    same_list: bool,
}

/// The snapshot section's header, schema hash, identity and lookup log,
/// checked against this build and command line (the search list is
/// compared by the lookups afterwards, WP-1403). An error says why, and
/// whether it is one of decoding (which a part of the section may give
/// where the whole would not).
fn snapshot_head(
    args: &Args,
    tc: &crate::toolchain::Toolchain,
    std: crate::toolchain::LangStd,
    diag_opts: &kiln_diag::DiagOptions,
    section: &[u8],
    started: Instant,
    rec: &pch::Record,
) -> Result<Head, (Refusal, bool)> {
    let decode = |e: kiln_snap::SnapError| (Refusal::quiet(e.to_string()), true);
    let at = check_header(section).map_err(|why| {
        // Another format version is another kiln build; the rest cannot
        // be read.
        let configuration = why.contains("format version");
        (Refusal { why, configuration }, false)
    })?;
    let mut r = Reader::new(&section[at..]);
    let hash: u64 = r.get().map_err(decode)?;
    let hashed = started.elapsed();
    if hash != SCHEMA_HASH {
        return Err((Refusal::configuration("written by another build (schema hash)"), false));
    }
    let id: Vec<(String, String)> = r.get().map_err(decode)?;
    let lookups: kiln_pp::LookupLog = r.get().map_err(decode)?;
    // WP-1396: a damaged log is a section that cannot be read (`-v`
    // only, as a decoding error), not a configuration refusal.
    lookups.validate().map_err(|why| (Refusal::quiet(why), false))?;
    let (mine, list) = identity_and_list(args, tc, std, diag_opts);
    // WP-1403: the search list is compared by the prefix's lookups, and
    // the working directory only when the prefix names a file relatively
    // (spec/pch.md "Lookup-tolerant search path").
    let anywhere = lookups.absolute() && rec.files.iter().all(|f| f.spelled.starts_with('/'));
    let skip: &[&str] = if anywhere { &[SEARCH_PATH, WORKING_DIRECTORY] } else { &[SEARCH_PATH] };
    let (theirs, ours) = (without(&id, skip), without(&mine, skip));
    let owned = |v: Vec<&(String, String)>| v.into_iter().cloned().collect::<Vec<_>>();
    if let Some(why) = first_difference(&owned(theirs), &owned(ours)) {
        return Err((Refusal::configuration(why), false));
    }
    let same_cwd = component(&id, WORKING_DIRECTORY) == component(&mine, WORKING_DIRECTORY);
    let same_list = component(&id, SEARCH_PATH) == component(&mine, SEARCH_PATH) && same_cwd;
    Ok(Head { start: at + r.offset(), hashed, lookups, list, same_cwd, same_list })
}

/// The roots of the snapshot section at `section` in the block of
/// `img_len` bytes whose checksum is `sum` (its last section, ending the
/// PCH file `pch`) from `start`, as a range of the file, or `None` when
/// the file is not laid out so (the roots are then kept in memory).
fn file_roots(
    pch: &str,
    img_len: usize,
    sum: Option<[u8; 8]>,
    section: std::ops::Range<usize>,
    start: usize,
) -> Option<SnapshotRoots> {
    // The block ends with the snapshot section (the highest tag) and the
    // block's 8-byte checksum.
    let end = img_len.checked_sub(8)?;
    if section.end != end {
        return None;
    }
    let at = section.start;
    let meta = std::fs::metadata(pch).ok()?;
    let size = meta.len();
    let img_at = size.checked_sub(img_len as u64)?;
    let sum = sum?;
    Some(SnapshotRoots::File {
        path: std::path::PathBuf::from(pch),
        block: img_at,
        sum: u64::from_le_bytes(sum),
        offset: img_at + (at + start) as u64,
        len: (section.len() - start) as u64,
        size,
        modified: meta.modified().ok(),
    })
}

/// The process's peak RSS (`VmHWM` of `/proc/self/status`), where the
/// system has one, for `KILN_SNAP_TIMING`.
fn peak_rss() -> Option<String> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
    Some(line["VmHWM:".len()..].trim().to_owned())
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
        if timing() {
            kiln_ast::ast::set_field_timing(true);
            let _ = kiln_ast::ast::take_field_times();
        }
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
        if let Some(hwm) = peak_rss() {
            let _ = write!(line, ", peak RSS so far {hwm}");
        }
        eprintln!("{line}");
        kiln_ast::ast::set_field_timing(false);
        let mut fields = kiln_ast::ast::take_field_times();
        fields.sort_by_key(|f| std::cmp::Reverse(f.1));
        let mut line = String::from("snapshot-timing: fields");
        for (n, d, bytes) in fields.iter().filter(|f| f.1 >= Duration::from_micros(100)) {
            let _ = write!(
                line,
                " {} {:.2} ms ({}.{} MB),",
                n.trim_start_matches("Ast."),
                ms(*d),
                bytes / 1_000_000,
                bytes / 100_000 % 10
            );
        }
        eprintln!("{}", line.trim_end_matches(','));
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
    /// A streamed section block's checksum (compared once every root is
    /// decoded).
    verify: Option<BlockCheck>,
}

/// Opens a snapshot: the source manager and the interner, restored
/// first (before anything interns or loads a file), and the reader
/// positioned at the diagnostics engine.
pub(crate) fn open(
    s: &PchSnapshotUse,
) -> Result<(SourceManager, Arc<Interner>, Restore<'_>), String> {
    let mut timer = Timer::new();
    let (mut reader, verify) = match &s.roots {
        SnapshotRoots::Memory { bytes, start } => (Reader::new(&bytes[*start..]), None),
        SnapshotRoots::File { path, block, sum, offset, len, size, modified } => {
            let (r, check) = stream_roots(path, *block, *sum, *offset, *len, *size, *modified)?;
            (r, Some(check))
        }
    };
    let sm: SourceManager = reader.get().map_err(|e| e.to_string())?;
    timer.mark("SourceManager");
    let interner: Arc<Interner> = reader.get().map_err(|e| e.to_string())?;
    timer.mark("Interner");
    Ok((sm, interner, Restore { reader, includes: &s.includes, failed: None, timer, verify }))
}

/// A streamed section block's checksum as it is read, and the checksum
/// the block must have.
#[derive(Debug)]
struct BlockCheck {
    sum: Rc<RefCell<bmi::BlockChecksum>>,
    want: u64,
}

impl BlockCheck {
    /// Whether the bytes read so far are the checked block.
    fn matches(&self) -> bool {
        self.sum.borrow().finish() == self.want
    }
}

/// A reader of the `len` bytes of roots at `offset` in `path` (the file
/// still `size` bytes long and last modified at `modified`), which end the
/// section block at `block` whose checksum is `want`. The whole block
/// goes through the checksum: what comes before the roots (the other
/// sections and the snapshot's head) is read past here, the roots as they
/// are decoded. The caller compares the [`BlockCheck`] once every byte is
/// read, before anything restored is used (`restore_front`).
#[allow(clippy::too_many_arguments)]
fn stream_roots(
    path: &std::path::Path,
    block: u64,
    want: u64,
    offset: u64,
    len: u64,
    size: u64,
    modified: Option<std::time::SystemTime>,
) -> Result<(Reader<'static>, BlockCheck), String> {
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let meta = f.metadata().map_err(|e| e.to_string())?;
    if meta.len() != size || meta.modified().ok() != modified {
        return Err("the PCH file changed".to_owned());
    }
    std::io::Seek::seek(&mut f, std::io::SeekFrom::Start(block)).map_err(|e| e.to_string())?;
    let n = usize::try_from(len).map_err(|_| "the snapshot is too large".to_owned())?;
    let skip = offset.checked_sub(block).ok_or("the snapshot is outside its block")?;
    let sum = Rc::new(RefCell::new(bmi::BlockChecksum::new()));
    let mut src = Checked { inner: std::io::Read::take(f, skip + len), sum: Rc::clone(&sum) };
    let skipped = std::io::copy(&mut std::io::Read::take(&mut src, skip), &mut std::io::sink())
        .map_err(|e| e.to_string())?;
    if skipped != skip {
        return Err("the PCH file changed".to_owned());
    }
    // Offsets in errors count from the roots, as a memory reader's.
    Ok((Reader::streaming(src, n, 0), BlockCheck { sum, want }))
}

/// A source that adds every byte read to a [`bmi::BlockChecksum`].
struct Checked<R> {
    inner: R,
    sum: Rc<RefCell<bmi::BlockChecksum>>,
}

impl<R: std::io::Read> std::io::Read for Checked<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.sum.borrow_mut().update(&buf[..n]);
        Ok(n)
    }
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
        parse_options: impl FnOnce() -> kiln_parse::ParseOptions,
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
        let popts = parse_options();
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
        // Every byte of a streamed block has been read: it must be the
        // block whose checksum was checked (spec/snap.md "Streaming
        // reads"), or nothing restored is used.
        if restore.verify.as_ref().is_some_and(|c| !c.matches()) {
            restore.failed = Some("the section block read does not match its checksum".to_owned());
            return None;
        }
        Some((sema, pp, deps, popts))
    }
}

/// What the producer's snapshot pass made (WP-1393): the section, or why
/// there is none, and, when the pass stood in for the header pass, the
/// header pass's result (spec/snap.md "The producer's single pass").
#[derive(Debug)]
pub(crate) struct Produced {
    pub section: Result<Vec<u8>, String>,
    pub result: Option<crate::pp_cmd::JobResult>,
}

/// What a single pass gives the producer: the header pass's result, or
/// why the header pass is still needed (`KILN_SNAP_TIMING`).
pub(crate) type StandIn = Result<crate::pp_cmd::JobResult, String>;

/// A single pass's gate (WP-1393): the header, and the producer's
/// diagnostics output, counted instead of shown.
#[derive(Debug)]
pub(crate) struct Single<'h> {
    header: &'h str,
    written: Arc<std::sync::atomic::AtomicUsize>,
}

impl Single<'_> {
    /// Nothing was written to the pass's diagnostics output.
    pub(crate) fn quiet(&self) -> bool {
        self.written.load(std::sync::atomic::Ordering::Relaxed) == 0
    }

    /// The bytes written to the pass's diagnostics output.
    pub(crate) fn written(&self) -> usize {
        self.written.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// A writer that counts the bytes written to it.
struct Counted(Arc<std::sync::atomic::AtomicUsize>);

impl std::io::Write for Counted {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.fetch_add(buf.len(), std::sync::atomic::Ordering::Relaxed);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Why the producer may not try a single pass (WP-1393), or `None` when
/// it may: `KILN_PCH_SNAPSHOT_PASSES=2` is set, or the producer's command
/// line asks for something the header pass writes besides its
/// diagnostics and dependencies (`-v`, `-verify`, a time trace, a
/// report, a dump or a module file, `-MG`), or the
/// header is C (`std.c`): C's main-file-only diagnostics (an internal
/// declaration used in an extern inline function is quiet in a main file,
/// `kiln_sema` `c_decl.rs`) are not among those [`stands_in`] rules out,
/// so a C header takes two passes.
pub(crate) fn single_pass_blocker(
    args: &Args,
    std: crate::toolchain::LangStd,
) -> Option<&'static str> {
    let blockers = [
        (std.c.is_some(), "a C header"),
        (
            std::env::var_os("KILN_PCH_SNAPSHOT_PASSES").is_some_and(|v| v == "2"),
            "KILN_PCH_SNAPSHOT_PASSES=2",
        ),
        (args.verbose, "-v"),
        (args.verify.is_some(), "-verify"),
        (crate::time_trace::settings(args).ok().flatten().is_some(), "a time trace"),
        (args.memory_report, "-fkiln-memory-report"),
        (args.dump_ast.is_some(), "-fdump-ast"),
        (args.dump_record_layouts, "-fdump-record-layouts"),
        (args.dump_vtable_layouts, "-fdump-vtable-layouts"),
        (args.dump_cfg, "-fkiln-dump-cfg"),
        (args.cfg_stats, "-fkiln-cfg-stats"),
        (args.bmi_image_census || args.bmi_image_census_all, "-fkiln-bmi-image-census"),
        (args.constexpr.report.is_some(), "-fconstexpr-report"),
        (args.constexpr.profiling(), "constexpr profiling"),
        (args.deps.missing_generated, "-MG"),
    ];
    blockers.iter().find(|b| b.0).map(|b| b.1)
}

/// The header pass's result, from a single pass that ended without an
/// error or any output (WP-1393), or `None` when the header pass could
/// still differ from it. The two passes read the same files in the same
/// order and differ only in the header's place: the main file of one, the
/// last `-include` of an empty main file in the other. So they report the
/// same diagnostics except those only a main file gets (spec/snap.md
/// "The producer's single pass"), which are ruled out here:
///
/// - `#include_next`, `#pragma system_header` and module or import lines
///   in the header (warned of, or different, in a main file), and
///   `#pragma ... diagnostic` in it (which could turn on the next item
///   for the header alone);
/// - unused internal declarations of the main file (`-Wunused-variable`
///   and its kin, clang `isMainFileLoc`): the warnings are off, or the
///   header holds directives and comments only;
/// - `__INCLUDE_LEVEL__` and `__BASE_FILE__` in any file read (they
///   expand differently);
/// - a header found as a system header, or not listed under its own
///   name.
///
/// The dependency list is then the header pass's: the header moved to the
/// front, where the main file is listed (`<stdin>` is never listed).
pub(crate) fn stands_in(
    single: &Single<'_>,
    sm: &SourceManager,
    engine: &DiagEngine<'_>,
    deps: Option<crate::pp_cmd::SharedDeps>,
) -> Result<crate::pp_cmd::JobResult, String> {
    let deps = deps.ok_or("the pass has no dependency list")?;
    let header = std::path::Path::new(single.header);
    let Some(file) = sm.loaded_file(header) else {
        return Err(format!("the header '{}' is not a loaded file", single.header));
    };
    let text = sm.text(file);
    if let Some(line) = header_main_differs(text) {
        return Err(format!(
            "line {line} of the header is read differently in a main file (include_next, system_header, diagnostic pragma, module line, __INCLUDE_LEVEL__ or __BASE_FILE__)"
        ));
    }
    let unused = [
        kiln_diag::DiagId::sema_unused_variable,
        kiln_diag::DiagId::sema_unused_const_variable,
        kiln_diag::DiagId::sema_unneeded_internal_decl,
        kiln_diag::DiagId::sema_unused_variable_template,
    ];
    if unused.iter().any(|&id| engine.is_enabled(id)) && !directives_only(text) {
        return Err(
            "an unused-declaration warning is on and the header declares something".to_owned()
        );
    }
    {
        let list = deps.borrow();
        for name in list.files() {
            let Some(f) = sm.loaded_file(std::path::Path::new(name)) else {
                return Err(format!("dependency '{name}' is not a loaded file"));
            };
            let t = sm.text(f);
            if t.contains("__INCLUDE_LEVEL__") || t.contains("__BASE_FILE__") {
                return Err(format!("dependency '{name}' uses __INCLUDE_LEVEL__ or __BASE_FILE__"));
            }
        }
    }
    if !deps.borrow_mut().move_to_front(single.header) {
        return Err(format!(
            "the header '{}' is not listed under its own name in the dependencies",
            single.header
        ));
    }
    Ok(crate::pp_cmd::JobResult { dep_list: Some(deps), ..Default::default() })
}

/// The first line (1-based) of the header's text that a main file treats
/// differently from an included file (see [`stands_in`]). The directive
/// and module-line rules skip comments (a line of a block comment that
/// starts with `import` is not a module line), unless the text has a raw
/// string literal, whose contents the scan cannot tell from code; the
/// `__INCLUDE_LEVEL__` / `__BASE_FILE__` rule counts any occurrence.
fn header_main_differs(text: &str) -> Option<usize> {
    let skip_comments = !text.contains("R\"");
    let mut in_comment = false;
    for (i, raw) in text.lines().enumerate() {
        let code = if skip_comments { code_of(raw, &mut in_comment) } else { raw.to_owned() };
        let l = code.trim_start();
        let directive = l.starts_with('#')
            && ["include_next", "system_header", "diagnostic"].iter().any(|w| l.contains(w));
        let module = ["module", "import", "export"].iter().any(|w| l.starts_with(w));
        if directive || module || raw.contains("__INCLUDE_LEVEL__") || raw.contains("__BASE_FILE__")
        {
            return Some(i + 1);
        }
    }
    None
}

/// The line `line` with its comments replaced by a space (`in_comment`:
/// inside a block comment at its start; updated for the next line).
/// String and character literals are kept whole, so a `/*` in one does
/// not start a comment.
fn code_of(line: &str, in_comment: &mut bool) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut at = 0;
    let mut quote: Option<u8> = None;
    let mut from = 0;
    while at < bytes.len() {
        if *in_comment {
            if bytes[at] == b'*' && bytes.get(at + 1) == Some(&b'/') {
                *in_comment = false;
                out.push(' ');
                at += 2;
                from = at;
            } else {
                at += 1;
            }
            continue;
        }
        match (quote, bytes[at]) {
            (Some(_), b'\\') => at += 2,
            (Some(q), c) if c == q => {
                quote = None;
                at += 1;
            }
            (None, c @ (b'"' | b'\'')) => {
                quote = Some(c);
                at += 1;
            }
            (None, b'/') if bytes.get(at + 1) == Some(&b'*') => {
                out.push_str(&line[from..at]);
                *in_comment = true;
                at += 2;
            }
            (None, b'/') if bytes.get(at + 1) == Some(&b'/') => {
                out.push_str(&line[from..at]);
                return out;
            }
            // Inside a literal, or code.
            _ => at += 1,
        }
    }
    if !*in_comment {
        out.push_str(&line[from.min(line.len())..]);
    }
    out
}

/// Whether `text` holds only preprocessing directives, comments and
/// blank lines (so it declares nothing itself). Conservative.
fn directives_only(text: &str) -> bool {
    let mut in_comment = false;
    let mut continued = false;
    for l in text.lines() {
        let mut l = l.trim();
        if continued {
            continued = l.ends_with('\\');
            continue;
        }
        if in_comment {
            let Some(end) = l.find("*/") else { continue };
            l = l[end + 2..].trim();
            in_comment = false;
        }
        if l.is_empty() || l.starts_with("//") {
            continue;
        }
        if l.starts_with('#') {
            continued = l.ends_with('\\');
            if l.contains("/*") && !l.contains("*/") {
                in_comment = true;
            }
            continue;
        }
        if let Some(rest) = l.strip_prefix("/*") {
            match rest.find("*/") {
                Some(end) if rest[end + 2..].trim().is_empty() => continue,
                Some(_) => return false,
                None => {
                    in_comment = true;
                    continue;
                }
            }
        }
        return false;
    }
    true
}

/// The producer's section for `header` (WP-1383): a pass
/// over an empty main file with the producer's `-include`s, then the
/// header, parsed to the end of the last one (`kiln_parse::parse_prefix`)
/// and written root by root. Its diagnostics are not shown: the header
/// pass shows them.
///
/// With `single` (WP-1393, [`single_pass_allowed`]), the pass runs
/// before the header pass and goes on to the end of the translation
/// unit; when [`stands_in`] holds, [`Produced::result`] is the header
/// pass's result and the producer skips that pass.
pub(crate) fn produce(
    args: &Args,
    tc: &crate::toolchain::Toolchain,
    std: crate::toolchain::LangStd,
    header: &str,
    diag_opts: &kiln_diag::DiagOptions,
    single: bool,
) -> Produced {
    let mut a = args.clone();
    a.action = crate::args::Action::SyntaxOnly;
    a.includes.push(header.to_owned());
    a.deps.only = None;
    a.deps.side = single.then_some(crate::args::DepHeaders::All);
    a.deps.cdb_fragment = None;
    a.output = None;
    a.verbose = false;
    a.pch.header_main = false;
    a.pch.producing = false;
    a.pch.record_lookups = true;
    let written = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let gate = Single { header, written: Arc::clone(&written) };
    let stderr = if single {
        SharedWriter::new(Counted(written))
    } else {
        SharedWriter::new(std::io::sink())
    };
    let job = crate::pp_cmd::Job {
        args: &a,
        toolchain: tc,
        std,
        path: "-",
        stdin_text: Some(String::new()),
        diag_opts,
        stderr,
        asm: false,
    };
    let (roots, result) = crate::syntax_cmd::SyntaxJob(job).snapshot(single.then_some(&gate));
    if single && timing() {
        match &result {
            Ok(_) => {
                eprintln!("snapshot-timing: producer single pass: stands in for the header pass");
            }
            Err(why) => {
                eprintln!("snapshot-timing: producer single pass: header pass needed ({why})");
            }
        }
    }
    let result = result.ok();
    Produced { section: roots.map(|r| section(args, tc, std, diag_opts, &r)), result }
}

/// A standard error that keeps what is written (`=check`'s runs).
#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

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

/// `-fkiln-pch-snapshot=check` (spec/pch.md): the replay
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

    /// WP-1401: the compile-time [`SCHEMA_HASH`] keeps the runtime walk's
    /// property that any change of a snapshot type changes the hash. The
    /// walk is the oracle: every type it reaches (each field, element and
    /// variant type, by `type_name`, with its encoding attributes) must
    /// declare a constant, and two types whose descriptors differ must
    /// declare different constants. A hand impl that forgot its constant,
    /// or whose constant ignores a field, a type parameter or an
    /// attribute the descriptor names, fails here; a constant that misses
    /// a change no descriptor shows is caught by the derive's mutation
    /// tests (`kiln_snap/tests/snap.rs`). And every type the roots reach
    /// must be folded into their constant: a field that closes a cycle of
    /// types folds only its type's own items ([`kiln_snap::Schema::unfolded`]).
    #[test]
    fn schema_constants_cover_every_type() {
        let s = schema();
        let all = s.hashes();
        assert!(all.len() > 100, "the walk reached only {} types", all.len());
        // The constant names a type without its module (two copies of a
        // type in two modules encode alike), and a map's hasher does not
        // change its encoding: descriptors are compared without either.
        let no_paths = |d: &str| {
            let mut out = String::with_capacity(d.len());
            let mut ident = String::new();
            let mut chars = d.chars().peekable();
            while let Some(c) = chars.next() {
                if c.is_alphanumeric() || c == '_' {
                    ident.push(c);
                } else if c == ':' && chars.peek() == Some(&':') && !ident.is_empty() {
                    chars.next();
                    ident.clear();
                } else {
                    out.push_str(&ident);
                    ident.clear();
                    out.push(c);
                }
            }
            out + &ident
        };
        let no_hasher = |d: &str| {
            let mut d = d.replace(", std::hash::random::RandomState", "");
            while let Some(at) = d.find(", core::hash::BuildHasherDefault<") {
                let mut depth = 0;
                let end = d[at..]
                    .char_indices()
                    .find_map(|(i, c)| {
                        match c {
                            '<' => depth += 1,
                            '>' if depth == 1 => return Some(at + i + 1),
                            '>' => depth -= 1,
                            _ => {}
                        }
                        None
                    })
                    .unwrap_or(d.len());
                d.replace_range(at..end, "");
            }
            no_paths(&d)
        };
        let mut by_hash: std::collections::HashMap<u64, String> = std::collections::HashMap::new();
        for (desc, hash) in all {
            let hash = hash.unwrap_or_else(|| panic!("no schema constant for {desc}"));
            let desc = no_hasher(desc);
            if let Some(other) = by_hash.get(&hash) {
                assert_eq!(*other, desc, "two descriptors share the schema constant {hash:#x}");
            }
            by_hash.insert(hash, desc);
        }
        // Every type a root reaches is folded into the root's constant
        // (only a `#[snap(cycle)]` field can leave one out).
        assert_eq!(s.unfolded(), [], "types the schema constant does not fold");
    }

    /// WP-1393: a streamed block is checked as it is read: the bytes the
    /// roots reader hands out, and those before them, must be the block
    /// whose checksum was checked; a file changed in place (same size and
    /// modification time) is seen once the roots are read.
    #[test]
    fn a_streamed_block_is_checked_as_it_is_read() {
        let path = std::env::temp_dir().join(format!("kiln-wp1393-stream-{}", std::process::id()));
        let block: Vec<u8> = (0..5000u32).map(|i| (i * 7 % 253) as u8).collect();
        let mut file = b"head".to_vec();
        file.extend_from_slice(&block);
        file.extend_from_slice(b"checksum");
        std::fs::write(&path, &file).unwrap();
        let meta = std::fs::metadata(&path).unwrap();
        let (size, modified) = (meta.len(), meta.modified().ok());
        let want = bmi::block_checksum(&block);
        let read = |want: u64| {
            let (mut r, check) =
                stream_roots(&path, 4, want, 4 + 1000, 4000, size, modified).unwrap();
            let roots = r.bytes(4000).unwrap().to_vec();
            assert_eq!(r.remaining(), 0);
            (roots == block[1000..], check.matches())
        };
        assert_eq!(read(want), (true, true));
        assert_eq!(read(want ^ 1), (true, false));
        // A byte changed in place before the roots (only the checksum sees
        // it), then one inside them.
        for (at, roots_same) in [(500, true), (3000, false)] {
            let mut changed = file.clone();
            changed[4 + at] ^= 1;
            std::fs::write(&path, &changed).unwrap();
            let f = std::fs::File::options().write(true).open(&path).unwrap();
            f.set_modified(modified.unwrap()).unwrap();
            drop(f);
            assert_eq!(read(want), (roots_same, false), "byte {at}");
        }
        std::fs::write(&path, b"short").unwrap();
        assert!(stream_roots(&path, 4, want, 1004, 4000, size, modified).is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_header_of_directives_and_comments_declares_nothing() {
        assert!(directives_only("#include <vector>\n// c\n/* a\n b */\n#define X \\\n  1\n\n"));
        assert!(directives_only("/* one line */\n#pragma once\n"));
        assert!(!directives_only("#include <vector>\nstatic int x;\n"));
        assert!(!directives_only("/* c */ int x;\n"));
        assert!(!directives_only("#define M(x) x\nM(int y;)\n"));
    }

    #[test]
    fn lines_a_main_file_treats_differently_are_seen() {
        assert_eq!(header_main_differs("#include <vector>\n#pragma once\nint x;\n"), None);
        assert_eq!(header_main_differs("#include_next <map>\n"), Some(1));
        assert_eq!(header_main_differs("#  pragma GCC system_header\n"), Some(1));
        assert_eq!(
            header_main_differs("int a;\n#pragma clang diagnostic warning \"-Wunused\"\n"),
            Some(2)
        );
        assert_eq!(header_main_differs("#if __INCLUDE_LEVEL__ == 0\n#endif\n"), Some(1));
        assert_eq!(header_main_differs("const char *f = __BASE_FILE__;\n"), Some(1));
        assert_eq!(header_main_differs("export module m;\n"), Some(1));
        assert_eq!(header_main_differs("import std;\n"), Some(1));
        // WP-1403: comments do not count; a directive after one does; a
        // `/*` in a string does not open one.
        assert_eq!(header_main_differs("/* license\nimport this\nexport that */\nint x;\n"), None);
        assert_eq!(header_main_differs("// import x\n#define A 1 // module\n"), None);
        assert_eq!(header_main_differs("/* a */ #include_next <x.h>\n"), Some(1));
        assert_eq!(header_main_differs("const char *s = \"/*\";\nimport m;\n"), Some(2));
        assert_eq!(header_main_differs("/* __BASE_FILE__ */\n"), Some(1));
        // A raw string: comments are not skipped.
        assert_eq!(header_main_differs("auto s = R\"(/*)\";\n/*\nimport m;\n*/\n"), Some(3));
    }

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

    /// WP-1392: each component refuses, named (spec/pch.md
    /// "Identity").
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
        // WP-1404: `-O0` is the default level (a PCH built without it,
        // used with `-O0 -c`); another level still differs.
        same(&["-std=c++20", "t.cpp"], &["-std=c++20", "-O0", "-c", "t.cpp"]);
        same(&["-std=c++20", "-O0", "t.cpp"], &["-std=c++20", "t.cpp"]);
        assert_ne!(diff(&["-std=c++20", "t.cpp"], &["-std=c++20", "-O2", "t.cpp"]), None);
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
        assert_eq!(check_header(&old).unwrap_err(), "its section has format version 0, expected 3");
    }

    /// The checked-in format record (spec/pch.md "Format versioning").
    const RECORD: &str = include_str!("../pch-snapshot-format.txt");
    const RECORD_PATH: &str = "crates/kiln_driver/pch-snapshot-format.txt";

    /// Whether the record `text` ends with (`version`, `hash`), or what
    /// to do so it does: a schema hash that changed under the same
    /// version needs a version bump and a line; a bumped version, a line.
    /// The lines must be well formed, with versions increasing and no
    /// hash given two versions.
    fn check_format_record(text: &str, version: u32, hash: u64) -> Result<(), String> {
        let mut lines = Vec::new();
        for (n, l) in text.lines().enumerate() {
            let l = l.trim();
            if l.is_empty() || l.starts_with('#') {
                continue;
            }
            let parsed = l.split_once(' ').filter(|(_, h)| h.len() == 16).and_then(|(v, h)| {
                Some((v.parse::<u32>().ok()?, u64::from_str_radix(h, 16).ok()?))
            });
            let Some((v, h)) = parsed else {
                return Err(format!(
                    "{RECORD_PATH}:{}: not `<version> <16 hex digits>`: {l}",
                    n + 1
                ));
            };
            if lines.last().is_some_and(|&(pv, _)| pv >= v) {
                return Err(format!("{RECORD_PATH}:{}: versions must increase", n + 1));
            }
            if lines.iter().any(|&(_, ph)| ph == h) {
                return Err(format!(
                    "{RECORD_PATH}:{}: hash {h:016x} already has a version",
                    n + 1
                ));
            }
            lines.push((v, h));
        }
        let line = format!("{version} {hash:016x}");
        let Some(&(v, h)) = lines.last() else {
            return Err(format!("{RECORD_PATH} has no line: append `{line}`"));
        };
        if (v, h) == (version, hash) {
            return Ok(());
        }
        if v == version {
            return Err(format!(
                "the snapshot schema hash changed ({h:016x} -> {hash:016x}) without a format \
                 version bump: set FORMAT_VERSION in crates/kiln_driver/src/pch_snapshot.rs to {} \
                 and append `{} {hash:016x}` to {RECORD_PATH} (spec/pch.md \"Format versioning\")",
                version + 1,
                version + 1
            ));
        }
        if v < version {
            return Err(format!(
                "FORMAT_VERSION is {version} but {RECORD_PATH} ends at {v}: append `{line}` to it \
                 (spec/pch.md \"Format versioning\")"
            ));
        }
        Err(format!(
            "{RECORD_PATH} ends at version {v}, after this build's FORMAT_VERSION {version}: \
             a record line is never removed; set FORMAT_VERSION to {} and append its line",
            v + 1
        ))
    }

    /// WP-1397: the record ends with this build's (`FORMAT_VERSION`,
    /// `SCHEMA_HASH`). `SCHEMA_HASH` is folded from names only (spec/snap.md
    /// "Schema hash"), so the line is the same on every host.
    #[test]
    fn format_record() {
        if let Err(what) = check_format_record(RECORD, FORMAT_VERSION, SCHEMA_HASH) {
            panic!("{what}");
        }
    }

    /// The record check's messages.
    #[test]
    fn format_record_messages() {
        let rec = "# c\n1 00000000000000aa\n2 00000000000000bb\n";
        assert_eq!(check_format_record(rec, 2, 0xbb), Ok(()));
        let e = check_format_record(rec, 2, 0xcc).unwrap_err();
        assert!(e.starts_with("the snapshot schema hash changed (00000000000000bb -> 00000000000000cc) without a format version bump: set FORMAT_VERSION in crates/kiln_driver/src/pch_snapshot.rs to 3 and append `3 00000000000000cc`"), "{e}");
        let e = check_format_record(rec, 3, 0xcc).unwrap_err();
        assert!(e.starts_with("FORMAT_VERSION is 3 but crates/kiln_driver/pch-snapshot-format.txt ends at 2: append `3 00000000000000cc`"), "{e}");
        assert!(check_format_record(rec, 1, 0xaa).unwrap_err().contains("never removed"));
        assert!(check_format_record("", 1, 1).unwrap_err().contains("has no line"));
        assert!(check_format_record("2 aa\n", 2, 0xaa).unwrap_err().contains("16 hex digits"));
        let e = check_format_record("2 00000000000000aa\n1 00000000000000bb\n", 1, 0xbb);
        assert!(e.unwrap_err().contains("versions must increase"));
        let e = check_format_record("1 00000000000000aa\n2 00000000000000aa\n", 2, 0xaa);
        assert!(e.unwrap_err().contains("already has a version"));
    }
}

#[cfg(test)]
mod changed_file_tests {
    use kiln_diag::{DiagEngine, DiagOptions, SharedBuffer, TextSink};
    use kiln_source::hash::content_hash;
    use kiln_source::{LazyFile, SourceManager};

    use super::changed_files;

    /// A restored prefix buffer whose file changed after the consumer's
    /// file check (same size) ends the compile with clang's fatal error
    /// once its text is read; a buffer not read yet, or past the restored
    /// prefix, is not reported ([cpp.include]; spec/pch.md
    /// "Consumer"). WP-1396.
    #[test]
    fn a_prefix_file_changed_after_the_check_is_fatal() {
        let d = std::env::temp_dir().join(format!("kiln-wp1396-changed-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let path = d.join("p.h");
        let (old, new) = ("int f(int a);\n", "int f(int b);\n");
        std::fs::write(&path, new).unwrap();
        let sm = SourceManager::new();
        let len = u32::try_from(old.len()).unwrap();
        let LazyFile::Added(f) =
            sm.add_lazy_file("p.h", &path, len, content_hash(old.as_bytes()), None)
        else {
            panic!("the lazy buffer was not added")
        };
        let opts = DiagOptions::default();
        let out = SharedBuffer::new();
        let engine = DiagEngine::new(&sm, &opts, TextSink::new(out.clone(), &opts));
        assert!(!changed_files(&sm, f.index() + 1, "p.pch", &engine), "not read yet");
        let _ = sm.text(f);
        assert!(!changed_files(&sm, f.index(), "p.pch", &engine), "past the restored prefix");
        assert_eq!(out.contents(), "");
        assert!(changed_files(&sm, f.index() + 1, "p.pch", &engine));
        let text = out.contents();
        assert!(
            text.contains(
                "fatal error: file 'p.h' has been modified since the precompiled header 'p.pch' was built: content changed"
            ),
            "{text}"
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}
