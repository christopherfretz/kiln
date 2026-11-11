//! Precompiled headers (WP-1023, spec/driver.md "Precompiled headers").
//!
//! A kiln PCH is a *textual* record: the header's path, the files it
//! read with their sizes and content hashes, and the options that change
//! what preprocessing it produces. The consumer (`-include-pch`) checks
//! the record the way clang checks a PCH, with clang's diagnostics, and
//! then processes the header again as if it were `-include`d. Replaying
//! the header text under the consumer's own options is always sound; the
//! checks exist so builds that rely on clang rejecting a stale or
//! mismatched PCH see the same errors. In 1.0 a PCH buys correctness and
//! build-system compatibility, not speed.
//!
//! With `-fkiln-pch-image` (WP-1280) the record also carries, after its
//! `end` line, the header's preprocessor state (spec/modules.md §11.13;
//! `crate::pch_image`), framed by an `image <len>\t<sum>` line.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use kiln_diag::{Diag, DiagId, DiagOptions};
use kiln_pp::CmdMacro;
use kiln_pp::bmi;
use kiln_source::{SourceManager, Span};

use crate::args::{
    Action, Args, DepHeaders, IncludePchOrder, Input, PchCheckLoad, PchDeclsUse, PchImage,
    PchImageUse,
};
use crate::codegen_cmd::{self, Phase};
use crate::deps::DepList;
use crate::toolchain::{LangStd, Toolchain};
use crate::{Io, SharedWriter, input_std, make_engine, report_output_error, syntax_only};

/// The record's first word.
pub(crate) const MAGIC: &str = "KILNPCH";
/// The record format version this kiln writes and reads.
pub(crate) const FORMAT_VERSION: u32 = 1;

/// One recorded option value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum OptVal {
    /// clang's boolean language options ("X was enabled in PCH file...").
    Bool(bool),
    /// clang's valued options ("X differs in PCH file vs. current file").
    Value(String),
}

/// Which clang wording an option check belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Compat {
    /// Both.
    All,
    /// `-fclang-compat=19` (clang 19 checks `__OPTIMIZE__`).
    Below22,
    /// `-fclang-compat=22` (clang 22 checks `OptimizationLevel`).
    From22,
}

/// One file the header read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FileEntry {
    /// Size in bytes when the PCH was built.
    pub size: u64,
    /// `bmi::checksum` of the contents.
    pub hash: u64,
    /// Found as a system header (not checked, as clang does not check
    /// system input files by default).
    pub system: bool,
    /// The name as the dependency output spells it.
    pub spelled: String,
    /// The absolute path checked.
    pub abs: String,
}

/// A parsed PCH record (the format is spec/driver.md "Precompiled
/// headers").
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Record {
    /// `kiln <version> <sha> <clang-compat>`, compared as a whole.
    pub kiln: String,
    /// The producer's working directory.
    pub cwd: String,
    /// The precompiled input, as spelled.
    pub main: String,
    /// The producer's `-include` files, as spelled, in order.
    pub includes: Vec<String>,
    /// The checked options, in clang's check order.
    pub opts: Vec<(String, OptVal)>,
    /// The target triple.
    pub target: String,
    /// The command-line macros' final state, in first-appearance order
    /// (`None`: undefined).
    pub macros: Vec<(String, Option<String>)>,
    /// The files read, in dependency order.
    pub files: Vec<FileEntry>,
    /// The preprocessor-state record (`-fkiln-pch-image`, WP-1280): a
    /// module-file section block (spec/modules.md §11.13) after the
    /// text, its length and checksum on the text's `image` line.
    pub image: Option<Vec<u8>>,
}

/// Why a file is not a readable PCH record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ReadError {
    /// No kiln magic.
    NotPch,
    /// A clang PCH (`CPCH` magic).
    Clang,
    /// An older format version.
    Older,
    /// A newer format version.
    Newer,
    /// Kiln magic, but malformed (the detail says where).
    Corrupt(String),
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '\t' => o.push_str("\\t"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            c => o.push(c),
        }
    }
    o
}

fn unesc(s: &str) -> Option<String> {
    let mut o = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            o.push(match it.next()? {
                '\\' => '\\',
                't' => '\t',
                'n' => '\n',
                'r' => '\r',
                _ => return None,
            });
        } else {
            o.push(c);
        }
    }
    Some(o)
}

/// This kiln's identity line (a PCH from another one is rejected).
pub(crate) fn kiln_line(args: &Args) -> String {
    let sha = bmi::kiln_sha();
    let sha = if sha.is_empty() { "-" } else { sha };
    format!("{} {sha} -fclang-compat={}", bmi::KILN_VERSION, args.predef.clang_compat.major())
}

impl Record {
    /// The record: its text, then the image's bytes.
    pub(crate) fn write(&self) -> Vec<u8> {
        use std::fmt::Write as _;
        let mut s = format!("{MAGIC} {FORMAT_VERSION}\n");
        let _ = writeln!(s, "kiln {}", esc(&self.kiln));
        let _ = writeln!(s, "cwd {}", esc(&self.cwd));
        let _ = writeln!(s, "main {}", esc(&self.main));
        for i in &self.includes {
            let _ = writeln!(s, "include {}", esc(i));
        }
        for (d, v) in &self.opts {
            let _ = match v {
                OptVal::Bool(b) => writeln!(s, "opt b\t{}\t{}", esc(d), u8::from(*b)),
                OptVal::Value(v) => writeln!(s, "opt v\t{}\t{}", esc(d), esc(v)),
            };
        }
        let _ = writeln!(s, "target {}", esc(&self.target));
        for (n, d) in &self.macros {
            let _ = match d {
                Some(d) => writeln!(s, "macro D\t{}\t{}", esc(n), esc(d)),
                None => writeln!(s, "macro U\t{}", esc(n)),
            };
        }
        for f in &self.files {
            let _ = writeln!(
                s,
                "file {}\t{:016x}\t{}\t{}\t{}",
                f.size,
                f.hash,
                u8::from(f.system),
                esc(&f.spelled),
                esc(&f.abs)
            );
        }
        if let Some(img) = &self.image {
            let _ = writeln!(s, "image {}\t{:016x}", img.len(), bmi::checksum(img));
        }
        let sum = bmi::checksum(s.as_bytes());
        let _ = writeln!(s, "end {sum:016x}");
        let mut out = s.into_bytes();
        out.extend_from_slice(self.image.as_deref().unwrap_or_default());
        out
    }

    /// Parses a record.
    pub(crate) fn parse(bytes: &[u8]) -> Result<Record, ReadError> {
        if bytes.starts_with(&bmi::CLANG_MAGIC) {
            return Err(ReadError::Clang);
        }
        let head = format!("{MAGIC} ");
        if !bytes.starts_with(head.as_bytes()) {
            return Err(ReadError::NotPch);
        }
        let corrupt = |what: &str| ReadError::Corrupt(what.to_owned());
        // The text ends at its `end` line (no other line starts with
        // `end `: the values are escaped); the image's bytes follow.
        let text_len = bytes
            .windows(5)
            .position(|w| w == b"\nend ")
            .and_then(|at| bytes[at + 1..].iter().position(|&b| b == b'\n').map(|n| at + 2 + n))
            .unwrap_or(bytes.len());
        let (bytes, tail) = bytes.split_at(text_len);
        let text = std::str::from_utf8(bytes).map_err(|_| corrupt("not UTF-8"))?;
        let first = text.lines().next().unwrap_or("");
        let ver: u32 = first[head.len()..].parse().map_err(|_| corrupt("bad format version"))?;
        if ver < FORMAT_VERSION {
            return Err(ReadError::Older);
        }
        if ver > FORMAT_VERSION {
            return Err(ReadError::Newer);
        }
        let body = text.strip_suffix('\n').ok_or_else(|| corrupt("truncated"))?;
        let end_at = body.rfind('\n').ok_or_else(|| corrupt("truncated"))? + 1;
        let sum = body[end_at..].strip_prefix("end ").ok_or_else(|| corrupt("truncated"))?;
        if u64::from_str_radix(sum, 16).ok() != Some(bmi::checksum(&bytes[..end_at])) {
            return Err(corrupt("checksum mismatch"));
        }
        let mut r = Record::default();
        let mut seen_kiln = false;
        for line in body[..end_at].lines().skip(1) {
            let bad = || corrupt(&format!("bad line '{line}'"));
            let (key, rest) = line.split_once(' ').ok_or_else(bad)?;
            let f: Vec<&str> = rest.split('\t').collect();
            let u = |i: usize| f.get(i).and_then(|s| unesc(s)).ok_or_else(bad);
            match (key, f.len()) {
                ("kiln", 1) => (r.kiln, seen_kiln) = (u(0)?, true),
                ("cwd", 1) => r.cwd = u(0)?,
                ("main", 1) => r.main = u(0)?,
                ("include", 1) => r.includes.push(u(0)?),
                ("target", 1) => r.target = u(0)?,
                ("opt", 3) => {
                    let v = match f[0] {
                        "b" => OptVal::Bool(f[2] == "1"),
                        "v" => OptVal::Value(u(2)?),
                        _ => return Err(bad()),
                    };
                    r.opts.push((u(1)?, v));
                }
                ("macro", 3) if f[0] == "D" => r.macros.push((u(1)?, Some(u(2)?))),
                ("macro", 2) if f[0] == "U" => r.macros.push((u(1)?, None)),
                ("image", 2) if r.image.is_none() => {
                    let len: usize = f[0].parse().map_err(|_| bad())?;
                    let sum = u64::from_str_radix(f[1], 16).map_err(|_| bad())?;
                    if tail.len() != len || bmi::checksum(tail) != sum {
                        return Err(corrupt("image checksum mismatch"));
                    }
                    r.image = Some(tail.to_vec());
                }
                ("file", 5) => r.files.push(FileEntry {
                    size: f[0].parse().map_err(|_| bad())?,
                    hash: u64::from_str_radix(f[1], 16).map_err(|_| bad())?,
                    system: f[2] == "1",
                    spelled: u(3)?,
                    abs: u(4)?,
                }),
                _ => return Err(bad()),
            }
        }
        if !seen_kiln || r.main.is_empty() {
            return Err(corrupt("missing fields"));
        }
        if r.image.is_none() && !tail.is_empty() {
            return Err(corrupt("trailing bytes"));
        }
        Ok(r)
    }
}

/// The checked options of a compile, in clang's order (clang
/// `checkLanguageOptions`: the language options in `LangOptions.def`
/// order, the first mismatch reported), each with the wording it
/// belongs to. Kiln's own reflection options follow clang's.
fn options(args: &Args, tc: &Toolchain, std: LangStd) -> Vec<(&'static str, OptVal, Compat)> {
    use kiln_lex::Std as S;
    let sink = SharedWriter::new(std::io::sink());
    let diag_opts = DiagOptions::default();
    let job = crate::pp_cmd::Job {
        args,
        toolchain: tc,
        std,
        path: "",
        stdin_text: None,
        diag_opts: &diag_opts,
        stderr: sink,
    };
    let fp = job.fingerprint();
    let get = |k: &str| fp.iter().find(|(key, _)| key == k).map_or("", |(_, v)| v.as_str());
    let p = &args.predef;
    let b = |v: bool| OptVal::Bool(v);
    let mut v = Vec::new();
    for (d, lvl) in [
        ("C++", S::Cxx98),
        ("C++11", S::Cxx11),
        ("C++14", S::Cxx14),
        ("C++17", S::Cxx17),
        ("C++20", S::Cxx20),
        ("C++23", S::Cxx23),
        ("C++26", S::Cxx26),
    ] {
        v.push((d, b(std.cplusplus && std.lex >= lvl), Compat::All));
    }
    v.push(("char8_t keyword", b(get("char8_t") == "-fchar8_t"), Compat::All));
    v.push(("GNU extensions", b(std.gnu), Compat::All));
    v.push(("exception handling", b(get("exceptions") == "-fexceptions"), Compat::All));
    v.push(("run-time type information", b(get("rtti") == "-frtti"), Compat::All));
    v.push((
        "freestanding implementation",
        b(get("freestanding") == "-ffreestanding"),
        Compat::All,
    ));
    v.push(("__OPTIMIZE__ predefined macro", b(p.opt != kiln_pp::OptLevel::None), Compat::Below22));
    v.push((
        "__OPTIMIZE_SIZE__ predefined macro",
        b(p.opt == kiln_pp::OptLevel::Size),
        Compat::Below22,
    ));
    v.push(("signed char", b(get("char") == "-fsigned-char"), Compat::All));
    v.push((
        "sized deallocation",
        b(get("sized-deallocation") == "-fsized-deallocation"),
        Compat::All,
    ));
    v.push(("C++26 reflection", b(get("reflection") == "-freflection"), Compat::All));
    v.push((
        "reflection mangling",
        OptVal::Value(get("reflection-mangling").to_owned()),
        Compat::All,
    ));
    let flag = args.opt_flag.as_deref();
    v.push((
        "OptimizationLevel",
        OptVal::Value(codegen_cmd::opt_level(flag).to_string()),
        Compat::From22,
    ));
    v.push((
        "OptimizeSize",
        OptVal::Value(codegen_cmd::opt_size(flag).to_string()),
        Compat::From22,
    ));
    v
}

/// clang `collectMacroDefinitions`: the command line's macros, each
/// with its final state, in first-appearance order. A definition is the
/// text after the first `=` up to a line break, else `1`.
pub(crate) fn macros(args: &Args) -> Vec<(String, Option<String>)> {
    let mut out: Vec<(String, Option<String>)> = Vec::new();
    for m in &args.macros {
        let (name, def) = match m {
            CmdMacro::Undef(n) => (n.as_str(), None),
            CmdMacro::Define(d) => match d.split_once('=') {
                None => (d.as_str(), Some("1".to_owned())),
                Some((n, body)) => {
                    (n, Some(body.split(['\n', '\r']).next().unwrap_or("").to_owned()))
                }
            },
        };
        match out.iter_mut().find(|(n, _)| n == name) {
            Some(e) => e.1 = def,
            None => out.push((name.to_owned(), def)),
        }
    }
    out
}

fn cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_default()
}

/// What `-include-pch` changes in a compile: the arguments with the
/// header replayed as `-include`s and its input files seeded into the
/// dependency output. `Ok(None)` without `-include-pch`; `Err(())` after
/// reporting a rejected PCH (the compile stops, as clang's does).
// One decision per path (`-E`/`-M`, the image, replay), in the order
// the record and the image are checked: split, each part would take
// the same dozen locals.
#[allow(clippy::too_many_lines)]
pub(crate) fn consumer(
    args: &Args,
    tc: &Toolchain,
    inputs: &[&Input],
    diag_opts: &DiagOptions,
    stderr: &SharedWriter,
) -> Result<Option<Args>, ()> {
    let Some(pch) = args.pch.include.as_deref() else { return Ok(None) };
    let sm = SourceManager::new();
    let e = make_engine(&sm, diag_opts, stderr.clone(), None);
    let std = inputs
        .first()
        .and_then(|i| input_std(args, args.input_lang(i)).ok())
        .unwrap_or_else(|| LangStd::default_for(true));
    let finish = |e: kiln_diag::DiagEngine<'_>| {
        e.flush();
        if let Some(s) = e.summary()
            && diag_opts.format == kiln_diag::Format::Text
        {
            let mut err = stderr.clone();
            let _ = std::io::Write::write_fmt(&mut err, format_args!("{s}\n"));
        }
    };
    // WP-1037: `-include-pch <dir>` uses the first acceptable PCH in it
    // (clang `FrontendAction::BeginSourceFile`).
    let picked;
    let pch = if Path::new(pch).is_dir() {
        let Some(p) = pick_in_dir(args, tc, std, pch, diag_opts) else {
            Diag::new(DiagId::drv_pch_dir_no_pch, Span::INVALID).arg(pch).emit(&e);
            finish(e);
            return Err(());
        };
        picked = p;
        picked.as_str()
    } else {
        pch
    };
    let preprocess = args.action == Action::Preprocess || args.deps.only.is_some();
    let rec = match load(pch, preprocess) {
        Ok(rec) => rec,
        Err(diags) if preprocess => {
            // clang `-E` reports the PCH in each job and preprocesses
            // without it (WP-1153); the job emits the error.
            let mut out = args.clone();
            out.pch.unread = diags;
            return Ok(Some(out));
        }
        Err(diags) => {
            emit(&diags, &e);
            finish(e);
            return Err(());
        }
    };
    let here = cwd();
    let resolve = |spelled: &str| {
        if Path::new(spelled).is_absolute() || here == Path::new(&rec.cwd) {
            spelled.to_owned()
        } else {
            Path::new(&rec.cwd).join(spelled).display().to_string()
        }
    };
    let mut out = args.clone();
    let Ok(image) = image_section(args, pch, &rec, &e) else {
        finish(e);
        return Err(());
    };
    let load = args.pch.image == PchImage::Load;
    // `load` stands the record (and for a compile the declaration image)
    // in for the header, so the header must be the one it was built
    // from: the fingerprint's check, then every file's text (a stale
    // file, which `-fno-validate-pch` lets through, is replayed).
    let checked = load && image.is_some();
    let lines = u32::try_from(rec.includes.len() + 1).unwrap_or(u32::MAX);
    if checked && !args.pch.no_validate && !check(args, tc, std, pch, &rec, &e) {
        finish(e);
        return Err(());
    }
    let stale = || rec.files.iter().find_map(|f| stale_file(f)).map(|n| format!("'{n}' changed"));
    // `check` keeps replaying a stale header against the record (which
    // reports the difference) but builds no image beside it.
    let mut stale_why = None;
    let image = match image {
        Some(s) if !preprocess || load => match stale() {
            Some(why) => {
                note_path(args, stderr, pch, Err(&why));
                stale_why = Some(why);
                if load { None } else { Some(s) }
            }
            None => Some(s),
        },
        s => s,
    };
    if preprocess {
        if let Some(s) = &image
            && load
        {
            out.includes =
                args.includes.iter().filter(|i| !rec.includes.contains(i)).cloned().collect();
            let lines = u32::try_from(1 + args.includes.len() - out.includes.len()).unwrap_or(1);
            out.pch.image_use = Some(PchImageUse {
                pch: pch.to_owned(),
                bytes: std::sync::Arc::clone(&s.pp),
                install: true,
                checkpoint: 0,
                decls: None,
                lines,
            });
            out.pch.deps = rec.files.iter().map(|f| (resolve(&f.spelled), f.system)).collect();
            finish(e);
            return Ok(Some(out));
        }
        // clang `-E` does not load the PCH: it preprocesses the header the
        // PCH was built from, then the `-include`s (no checks).
        out.includes = std::iter::once(resolve(&rec.main)).chain(args.includes.clone()).collect();
        let image = image.filter(|_| !load);
        if let Some(s) = &image
            && !rec.includes.is_empty()
        {
            // `check`: the record covers the producer's `-include`s,
            // which `-E` does not replay; a replay of them and the
            // header with no output checks it.
            let mut includes: Vec<String> = rec.includes.iter().map(|i| resolve(i)).collect();
            includes.push(resolve(&rec.main));
            let record = PchImageUse {
                pch: pch.to_owned(),
                bytes: std::sync::Arc::clone(&s.pp),
                install: false,
                checkpoint: lines,
                decls: None,
                lines: 0,
            };
            let input =
                crate::pch_check::Input { tc, std, path: "-", stdin_text: None, diag_opts, stderr };
            let ok = crate::pch_check::preprocess_record(args, &input, includes, record);
            finish(e);
            return if ok { Ok(Some(out)) } else { Err(()) };
        }
        out.pch.image_use = image.map(|s| PchImageUse {
            pch: pch.to_owned(),
            bytes: s.pp,
            install: false,
            checkpoint: 1,
            decls: None,
            lines: 0,
        });
        finish(e);
        return Ok(Some(out));
    }
    if !checked && !args.pch.no_validate && !check(args, tc, std, pch, &rec, &e) {
        finish(e);
        return Err(());
    }
    out.pch.deps = rec.files.iter().map(|f| (resolve(&f.spelled), f.system)).collect();
    // WP-1036: the record and the declaration image stand in for the
    // producer's `-include`s and the header, or neither does (`check`:
    // the compile is also run so, and compared with replay).
    let variant = match &image {
        Some(s) if stale_why.is_none() => {
            match s.decls.as_deref().map(kiln_sema::PchDecls::decode) {
                Some(Ok(d)) => {
                    note_path(args, stderr, pch, Ok(d.decls()));
                    Some(PchCheckLoad {
                        includes: args
                            .includes
                            .iter()
                            .filter(|i| !rec.includes.contains(i))
                            .cloned()
                            .collect(),
                        image_use: PchImageUse {
                            pch: pch.to_owned(),
                            bytes: std::sync::Arc::clone(&s.pp),
                            install: true,
                            checkpoint: 0,
                            decls: Some(PchDeclsUse {
                                image: std::sync::Arc::new(d),
                                header: replay_name(&resolve(&rec.main)),
                            }),
                            lines,
                        },
                    })
                }
                Some(Err(why)) => {
                    Diag::new(DiagId::drv_pch_corrupt, Span::INVALID)
                        .arg(format!("{why} in kiln PCH '{pch}'"))
                        .emit(&e);
                    finish(e);
                    return Err(());
                }
                None => {
                    note_path(args, stderr, pch, Err("it has no declaration image"));
                    None
                }
            }
        }
        _ => None,
    };
    if load && let Some(v) = variant {
        out.includes = v.includes;
        out.pch.image_use = Some(v.image_use);
        finish(e);
        return Ok(Some(out));
    }
    // A compile replays the producer's `-include`s, then the header
    // (`check`: and compares the state reached with the record).
    let checkpoint = u32::try_from(rec.includes.len() + 1).unwrap_or(u32::MAX);
    out.pch.image_use = image.filter(|_| !load).map(|s| PchImageUse {
        pch: pch.to_owned(),
        bytes: s.pp,
        install: false,
        checkpoint,
        decls: None,
        lines: 0,
    });
    finish(e);
    let mut includes: Vec<String> = rec.includes.iter().map(|i| resolve(i)).collect();
    includes.push(resolve(&rec.main));
    includes.extend(args.includes.iter().filter(|i| !rec.includes.contains(i)).cloned());
    out.includes = includes;
    out.pch.check_load = variant.map(Box::new);
    Ok(Some(out))
}

/// The buffer name replay's `-include <spelled>` gives the header: found
/// from the predefines, so in the working directory (`kiln_pp`'s
/// resolver, `HeaderSearch::LookupFile`).
fn replay_name(spelled: &str) -> String {
    if spelled.starts_with('/') { spelled.to_owned() } else { kiln_pp::join_path(".", spelled) }
}

/// The name of `f` when its text is no longer the one recorded.
fn stale_file(f: &FileEntry) -> Option<&str> {
    let ok = std::fs::read(&f.abs)
        .is_ok_and(|b| b.len() as u64 == f.size && bmi::checksum(&b) == f.hash);
    (!ok).then_some(f.spelled.as_str())
}

/// `-v`: which path a consumer of `pch` takes (WP-1036): the
/// declaration image (`Ok`: its declaration count) or a replay of the
/// header, and why.
fn note_path(args: &Args, stderr: &SharedWriter, pch: &str, path: Result<usize, &str>) {
    if !args.verbose {
        return;
    }
    let mut err = stderr.clone();
    let _ = match path {
        Ok(n) => writeln!(err, "note: precompiled header '{pch}': declaration image ({n} records)"),
        Err(why) => writeln!(err, "note: precompiled header '{pch}' is replayed: {why}"),
    };
}

/// A PCH's `-fkiln-pch-image` sections.
struct Sections {
    /// The preprocessor-state record (`SECTION_PP`).
    pp: std::sync::Arc<Vec<u8>>,
    /// The declaration image (`SECTION_IMAGE`), when the producer
    /// carried every declaration of the header.
    decls: Option<Vec<u8>>,
}

/// The `-fkiln-pch-image` sections of `rec` for `check`/`load`, or
/// `None` (with a warning when one was asked for but cannot be used: the
/// header is then replayed unchecked); `Err` after reporting a damaged
/// section block.
fn image_section(
    args: &Args,
    pch: &str,
    rec: &Record,
    e: &kiln_diag::DiagEngine<'_>,
) -> Result<Option<Sections>, ()> {
    if !matches!(args.pch.image, PchImage::Check | PchImage::Load) {
        return Ok(None);
    }
    let unusable = |why: &str| {
        Diag::new(DiagId::drv_pch_image_unusable, Span::INVALID).arg(pch).arg(why).emit(e);
        None
    };
    let Some(img) = &rec.image else {
        return Ok(unusable("it was built without -fkiln-pch-image"));
    };
    let sections = match bmi::read_sections(img) {
        Ok(s) => s,
        Err(err) => {
            // The block is checksummed: a damaged PCH, not one to guess
            // around (WP-1036).
            Diag::new(DiagId::drv_pch_corrupt, Span::INVALID)
                .arg(format!("{err} in kiln PCH image sections '{pch}'"))
                .emit(e);
            return Err(());
        }
    };
    let mut pp = None;
    let mut decls = None;
    for s in sections {
        match s.tag {
            bmi::SECTION_PP => pp = Some(std::sync::Arc::new(s.bytes)),
            bmi::SECTION_IMAGE => decls = Some(s.bytes),
            _ => {}
        }
    }
    Ok(match pp {
        Some(pp) => Some(Sections { pp, decls }),
        None => unusable("it has no preprocessor-state section"),
    })
}

/// The first file in the directory `dir`, by name, that is a kiln PCH
/// whose configuration matches the compile ([`check_config`], silently;
/// any readable record with `-fno-validate-pch`). clang takes the
/// directory's own order, which is unspecified; kiln sorts by name so the
/// choice is reproducible.
fn pick_in_dir(
    args: &Args,
    tc: &Toolchain,
    std: LangStd,
    dir: &str,
    diag_opts: &DiagOptions,
) -> Option<String> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|d| d.path())
        .filter(|p| p.is_file())
        .collect();
    entries.sort();
    let sm = SourceManager::new();
    let quiet = make_engine(&sm, diag_opts, SharedWriter::new(std::io::sink()), None);
    entries.into_iter().map(|p| p.display().to_string()).find(|p| {
        let Ok(rec) = std::fs::read(p).map_err(drop).and_then(|b| Record::parse(&b).map_err(drop))
        else {
            return false;
        };
        args.pch.no_validate || check_config(args, tc, std, p, &rec, &quiet)
    })
}

/// Whether `path` starts with a PCH's magic, kiln's or clang's (clang's
/// `maybeHasClangPchSignature`; a clang PCH is then reported by the
/// consumer as one kiln cannot read, as an explicit `-include-pch` is).
fn has_pch_magic(path: &Path) -> bool {
    use std::io::Read;
    let mut head = [0u8; 8];
    let Ok(mut f) = std::fs::File::open(path) else { return false };
    let mut n = 0;
    while n < head.len() {
        match f.read(&mut head[n..]) {
            Ok(0) | Err(_) => break,
            Ok(k) => n += k,
        }
    }
    let head = &head[..n];
    head.starts_with(&bmi::CLANG_MAGIC) || head.starts_with(format!("{MAGIC} ").as_bytes())
}

/// A warning of the driver's `-include` probe.
enum ProbeWarning {
    /// `<h>.gch` is a file without a PCH's magic.
    GchFile(String),
    /// `<h>.gch` is a directory with no PCH in it.
    GchDir(String),
    /// The PCH found for an `-include` that is not the first: the PCH and
    /// the option as clang renders it.
    NotFirst(String, String),
}

/// clang's `gchProbe`: whether `gch` is a PCH (a file with a PCH's magic,
/// or a directory holding one), else the warning when it exists.
fn gch_probe(gch: &str, warnings: &mut Vec<ProbeWarning>) -> bool {
    let Ok(md) = std::fs::metadata(gch) else { return false };
    if md.is_dir() {
        let any = std::fs::read_dir(gch)
            .into_iter()
            .flatten()
            .flatten()
            .any(|d| has_pch_magic(&d.path()));
        if !any {
            warnings.push(ProbeWarning::GchDir(gch.to_owned()));
        }
        return any;
    }
    if has_pch_magic(Path::new(gch)) {
        return true;
    }
    warnings.push(ProbeWarning::GchFile(gch.to_owned()));
    false
}

/// clang's driver probe of `-include <h>` for a precompiled `<h>`
/// (WP-1037, Clang.cpp `AddPreprocessingOptions`; spec/driver.md
/// "Precompiled headers"). Each of the driver's own `-include`s is
/// looked up as given (not in the include path): `<h>.pch` if that path
/// exists, else `<h>.gch` if it is a PCH ([`gch_probe`]). The first
/// `-include` that finds one becomes `-include-pch` of it, unless it is
/// not the first `-include`, which is only a warning. As in clang's
/// frontend, the last `-include-pch` wins, so an explicit one after it
/// (or behind `-Xclang`) replaces it, and the `-include` is gone either
/// way. The warnings are the driver's, once per compile job (`jobs`).
/// `Ok(None)` when the arguments do not change; `Err(())` when a warning
/// was an error (`-Werror`).
pub(crate) fn probe(
    args: &Args,
    jobs: usize,
    diag_opts: &DiagOptions,
    stderr: &SharedWriter,
) -> Result<Option<Args>, ()> {
    if jobs == 0 || args.pch.probed.is_empty() {
        return Ok(None);
    }
    let mut warnings = Vec::new();
    let mut first = None;
    for (k, &(idx, spelling)) in args.pch.probed.iter().enumerate() {
        let h = &args.includes[idx];
        let pch = format!("{h}.pch");
        let found = if Path::new(&pch).exists() {
            Some(pch)
        } else {
            let gch = format!("{h}.gch");
            gch_probe(&gch, &mut warnings).then_some(gch)
        };
        match found {
            Some(p) if k == 0 => first = Some((idx, p)),
            Some(p) => warnings.push(ProbeWarning::NotFirst(p, format!("{spelling}{h}"))),
            None => {}
        }
    }
    if !warnings.is_empty() {
        let sm = SourceManager::new();
        let e = make_engine(
            &sm,
            &codegen_cmd::driver_diag_options(diag_opts),
            stderr.clone(),
            Some(&args.program),
        );
        for _ in 0..jobs {
            for w in &warnings {
                let d = match w {
                    ProbeWarning::GchFile(p) => {
                        Diag::new(DiagId::drv_pch_ignoring_gch_file, Span::INVALID).arg(p)
                    }
                    ProbeWarning::GchDir(p) => {
                        Diag::new(DiagId::drv_pch_ignoring_gch_dir, Span::INVALID).arg(p)
                    }
                    ProbeWarning::NotFirst(p, opt) => {
                        Diag::new(DiagId::drv_pch_not_first_include, Span::INVALID).arg(p).arg(opt)
                    }
                };
                d.emit(&e);
            }
        }
        e.flush();
        if e.error_count() > 0 {
            return Err(());
        }
    }
    let Some((idx, p)) = first else { return Ok(None) };
    let mut out = args.clone();
    out.includes.remove(idx);
    out.pch.probed.clear();
    if args.pch.include_order == IncludePchOrder::BeforeProbe {
        out.pch.include = Some(p);
    }
    Ok(Some(out))
}

/// One diagnostic of an unreadable PCH, kept to be emitted later: its id
/// and its arguments.
pub(crate) type PchDiag = (DiagId, Vec<String>);

/// Reads and parses `pch`; on failure, clang's errors for an unreadable
/// or foreign file. With `preprocess` (`-E`, `-M`), only the first of
/// them: clang's `-E` reads only the header's name from the PCH
/// (`ASTReader::getOriginalSourceFile`), reports a file it cannot read
/// (`unable to read PCH file`) or one that is not a PCH (`input is not a
/// PCH file`) and goes on (WP-1153).
fn load(pch: &str, preprocess: bool) -> Result<Record, Vec<PchDiag>> {
    let one = |id: DiagId, args: &[&str]| (id, args.iter().map(|&a| a.to_owned()).collect());
    let bytes = match std::fs::read(pch) {
        Ok(b) => b,
        Err(err) => {
            let why = if err.kind() == std::io::ErrorKind::NotFound {
                "No such file or directory".to_owned()
            } else {
                err.to_string()
            };
            let mut d = vec![one(DiagId::drv_pch_unreadable, &[pch, &why])];
            if !preprocess {
                d.push(one(DiagId::drv_pch_not_found, &[pch]));
            }
            return Err(d);
        }
    };
    Record::parse(&bytes).map_err(|err| match err {
        ReadError::NotPch if preprocess => vec![one(DiagId::drv_pch_not_pch, &[pch])],
        ReadError::NotPch => {
            vec![one(DiagId::drv_pch_not_pch, &[pch]), one(DiagId::drv_pch_invalid, &[pch])]
        }
        ReadError::Clang => vec![one(DiagId::drv_pch_clang, &[pch])],
        ReadError::Older => vec![one(DiagId::drv_pch_older, &[pch])],
        ReadError::Newer => vec![one(DiagId::drv_pch_newer, &[])],
        ReadError::Corrupt(why) => {
            vec![one(DiagId::drv_pch_corrupt, &[&format!("{why} in kiln PCH record '{pch}'")])]
        }
    })
}

/// Emits diagnostics [`load`] kept.
pub(crate) fn emit(diags: &[PchDiag], e: &kiln_diag::DiagEngine<'_>) {
    for (id, args) in diags {
        args.iter().fold(Diag::new(*id, Span::INVALID), |d, a| d.arg(a.as_str())).emit(e);
    }
}

/// clang's PCH checks, in clang's order: the compiler, the language
/// options (first mismatch), the target, the macros (first mismatch);
/// then, when those pass, the input files (first stale one).
fn check(
    args: &Args,
    tc: &Toolchain,
    std: LangStd,
    pch: &str,
    rec: &Record,
    e: &kiln_diag::DiagEngine<'_>,
) -> bool {
    check_config(args, tc, std, pch, rec, e) && check_files(pch, rec, e)
}

/// The configuration half of [`check`]: the compiler, the language
/// options, the target and the macros (clang's `isAcceptableASTFile`
/// with strict option matches, which picks a PCH from a directory).
fn check_config(
    args: &Args,
    tc: &Toolchain,
    std: LangStd,
    pch: &str,
    rec: &Record,
    e: &kiln_diag::DiagEngine<'_>,
) -> bool {
    let me = kiln_line(args);
    if rec.kiln != me {
        Diag::new(DiagId::drv_pch_different_branch, Span::INVALID).arg(&rec.kiln).arg(me).emit(e);
        return false;
    }
    let mut ok = true;
    let compat22 = args.predef.clang_compat.major() >= 22;
    for (desc, cur, compat) in options(args, tc, std) {
        let applies = match compat {
            Compat::All => true,
            Compat::Below22 => !compat22,
            Compat::From22 => compat22,
        };
        let Some((_, old)) = rec.opts.iter().find(|(d, _)| d == desc) else { continue };
        if !applies || *old == cur {
            continue;
        }
        let d = match (old, &cur) {
            (OptVal::Bool(o), OptVal::Bool(c)) => {
                Diag::new(DiagId::drv_pch_config_bool, Span::INVALID)
                    .arg(desc)
                    .arg(u8::from(*o))
                    .arg(pch)
                    .arg(u8::from(*c))
            }
            _ => Diag::new(DiagId::drv_pch_config_value, Span::INVALID).arg(desc).arg(pch),
        };
        d.emit(e);
        ok = false;
        break;
    }
    if rec.target != tc.target.triple {
        Diag::new(DiagId::drv_pch_target, Span::INVALID)
            .arg(pch)
            .arg(&rec.target)
            .arg(tc.target.triple)
            .emit(e);
        ok = false;
    }
    // clang `checkPreprocessorOptions`: the current command line's macros
    // in order; one the PCH does not mention is fine.
    for (name, cur) in macros(args) {
        let Some((_, old)) = rec.macros.iter().find(|(n, _)| *n == name) else { continue };
        match (old, &cur) {
            (None, None) => continue,
            (Some(o), Some(c)) if o == c => continue,
            (Some(o), Some(c)) => {
                Diag::new(DiagId::drv_pch_macro_def_differs, Span::INVALID)
                    .arg(&name)
                    .arg(pch)
                    .arg(o)
                    .arg(c)
                    .emit(e);
            }
            _ => {
                Diag::new(DiagId::drv_pch_macro_def_undef, Span::INVALID)
                    .arg(&name)
                    .arg(u8::from(old.is_none()))
                    .arg(pch)
                    .emit(e);
            }
        }
        ok = false;
        break;
    }
    ok
}

/// clang's input-file check (after [`check_config`] passed): the first
/// non-system file whose size or content changed, or that is gone, is
/// fatal.
fn check_files(pch: &str, rec: &Record, e: &kiln_diag::DiagEngine<'_>) -> bool {
    for f in rec.files.iter().filter(|f| !f.system) {
        let Ok(bytes) = std::fs::read(&f.abs) else {
            Diag::new(DiagId::drv_pch_corrupt, Span::INVALID)
                .arg(format!("could not find file '{}' referenced by AST file '{pch}'", f.abs))
                .emit(e);
            return false;
        };
        let why = if bytes.len() as u64 != f.size {
            format!("size changed (was {}, now {})", f.size, bytes.len())
        } else if bmi::checksum(&bytes) != f.hash {
            "content changed".to_owned()
        } else {
            continue;
        };
        Diag::new(DiagId::drv_pch_file_modified, Span::INVALID)
            .arg(&f.abs)
            .arg(pch)
            .arg(why)
            .with_note(Diag::new(DiagId::note_pch_rebuild, Span::INVALID).arg(pch))
            .emit(e);
        return false;
    }
    true
}

/// Whether the compile precompiles `input` (a header with `-c`, `-S` or
/// no action; every input with `-emit-pch`).
pub(crate) fn is_producer(args: &Args, input: &Input) -> bool {
    matches!(args.action, Action::Compile | Action::Link)
        && args.deps.only.is_none()
        && (args.pch.emit || args.is_header_input(input))
}

/// Precompiles each of `inputs` (all producers, see [`is_producer`]):
/// checks the header as `-fsyntax-only` does, writes its record and its
/// `-MD` dependency file. Returns the errors.
pub(crate) fn produce(
    args: &Args,
    tc: &Toolchain,
    inputs: &[&Input],
    all_inputs: usize,
    diag_opts: &DiagOptions,
    io: &mut Io,
) -> u32 {
    let phase = Phase::of(args);
    // `-o` names the PCH unless a link follows (clang: `-o` is the final
    // output, a PCH when only headers are given).
    let link_follows = phase == Phase::Link && all_inputs > inputs.len();
    if !link_follows && args.output.is_some() && all_inputs > 1 {
        let sm = SourceManager::new();
        let e = make_engine(
            &sm,
            &codegen_cmd::driver_diag_options(diag_opts),
            io.stderr.clone(),
            Some(&args.program),
        );
        Diag::new(DiagId::drv_out_file_multiple, Span::INVALID).emit(&e);
        e.flush();
        return 1;
    }
    let mut errors = 0;
    for input in inputs {
        errors += produce_one(args, tc, input, phase, link_follows, diag_opts, io);
    }
    errors
}

fn produce_one(
    args: &Args,
    tc: &Toolchain,
    input: &Input,
    phase: Phase,
    link_follows: bool,
    diag_opts: &DiagOptions,
    io: &mut Io,
) -> u32 {
    let lang = args.input_lang(input);
    let std = match input_std(args, lang) {
        Ok(s) => s,
        Err(d) => {
            let sm = SourceManager::new();
            let e = make_engine(&sm, diag_opts, io.stderr.clone(), None);
            (*d).emit(&e);
            e.flush();
            return 1;
        }
    };
    let mut check_args = args.clone();
    check_args.action = Action::SyntaxOnly;
    check_args.deps.only = None;
    check_args.deps.side = Some(DepHeaders::All);
    check_args.deps.cdb_fragment = None;
    check_args.output = None;
    check_args.pch.header_main = args.is_header_input(input);
    check_args.pch.producing = true;
    let stdin = None;
    let out = match &args.output {
        Some(o) if !link_follows => o.clone(),
        _ if args.is_header_input(input) || phase == Phase::Link => format!("{}.pch", input.path),
        _ => codegen_cmd::default_output(&input.path, phase),
    };
    // spec/driver.md "Time trace": the precompile job's trace (`Frontend`,
    // then `WriteAST`), named as clang's (`<-o or a>-<input>.json` when
    // the final phase is a link).
    let mut trace = crate::time_trace::settings(args).ok().flatten().map(|s| {
        let path = crate::time_trace::path(args, Some(phase), &input.path, &out);
        crate::time_trace::InputTrace::start(s, path)
    });
    let r = {
        let _frontend = trace.as_ref().map(|_| kiln_source::trace::Scope::new("Frontend"));
        syntax_only(&check_args, tc, std, &input.path, stdin, diag_opts, io)
    };
    let errors = write_pch(args, tc, input, std, r, &out, diag_opts, io);
    if let Some(mut t) = trace.take() {
        t.failed = errors > 0;
        if let Err((path, e)) = t.finish(&args.program) {
            // clang: reported, the exit status unchanged.
            report_output_error(diag_opts, &io.stderr, &path, &e);
        }
    }
    errors
}

/// Writes the PCH record of the checked header (`r`, its job's result)
/// to `out`, then the `-MD` file. The errors.
#[allow(clippy::too_many_arguments)]
fn write_pch(
    args: &Args,
    tc: &Toolchain,
    input: &Input,
    std: LangStd,
    r: crate::pp_cmd::JobResult,
    out: &str,
    diag_opts: &DiagOptions,
    io: &mut Io,
) -> u32 {
    if r.errors > 0 {
        return r.errors;
    }
    let image = match r.pch_image {
        Some(Ok(bytes)) => Some(bytes),
        Some(Err(why)) => {
            let sm = SourceManager::new();
            let e = make_engine(&sm, diag_opts, io.stderr.clone(), None);
            Diag::new(DiagId::drv_pch_image_not_written, Span::INVALID).arg(out).arg(why).emit(&e);
            e.flush();
            None
        }
        None => None,
    };
    if args.verbose && image.is_some() {
        // WP-1036: the path a `load` consumer takes.
        let mut err = io.stderr.clone();
        let _ = match r.pch_decls {
            Some(Ok(n)) => writeln!(
                err,
                "note: precompiled header '{out}': consumers use its declaration image ({n} records)"
            ),
            Some(Err(why)) => writeln!(
                err,
                "note: precompiled header '{out}': consumers replay the header: {why}"
            ),
            None => Ok(()),
        };
    }
    let Some(list) = r.dep_list else { return 0 };
    let list = list.borrow();
    let here = cwd();
    let files = list
        .files()
        .iter()
        .filter_map(|name| {
            let abs = here.join(name);
            let bytes = std::fs::read(&abs).ok()?;
            Some(FileEntry {
                size: bytes.len() as u64,
                hash: bmi::checksum(&bytes),
                system: list.is_system(name),
                spelled: name.clone(),
                abs: abs.display().to_string(),
            })
        })
        .collect();
    let rec = Record {
        kiln: kiln_line(args),
        cwd: here.display().to_string(),
        main: input.path.clone(),
        includes: args.includes.clone(),
        opts: options(args, tc, std).into_iter().map(|(d, v, _)| (d.to_owned(), v)).collect(),
        target: tc.target.triple.to_owned(),
        macros: macros(args),
        files,
        image,
    };
    let write_ast = kiln_source::trace::Scope::with_detail("WriteAST", || out.to_owned());
    if let Err(err) = bmi::write_file(Path::new(out), &rec.write()) {
        let why = if err.kind() == std::io::ErrorKind::NotFound {
            "No such file or directory".to_owned()
        } else {
            err.to_string()
        };
        report_output_error(diag_opts, &io.stderr, out, &why);
        return 1;
    }
    drop(write_ast);
    // The user's `-MD` / `-MMD` file: the same list, filtered.
    if let Some(h) = args.deps.headers() {
        let mut user = DepList::new(h == DepHeaders::All);
        for f in list.files() {
            user.add(f, list.is_system(f));
        }
        if !crate::deps::write_side_file(args, &input.path, &user, diag_opts, io) {
            return 1;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Record {
        Record {
            kiln: "1.0 - -fclang-compat=19".into(),
            cwd: "/w d".into(),
            main: "p\th.hpp".into(),
            includes: vec!["a\\b.h".into()],
            opts: vec![("C++".into(), OptVal::Bool(true)), ("O".into(), OptVal::Value("2".into()))],
            target: "aarch64-unknown-linux-gnu".into(),
            macros: vec![("A".into(), Some("1".into())), ("B".into(), None)],
            files: vec![FileEntry {
                size: 3,
                hash: 0xabc,
                system: false,
                spelled: "p\nh.hpp".into(),
                abs: "/w d/p.hpp".into(),
            }],
            image: None,
        }
    }

    /// The record round-trips, and damage is detected (WP-1023).
    #[test]
    fn record_round_trip() {
        let mut r = sample();
        let text = String::from_utf8(r.write()).unwrap();
        assert_eq!(Record::parse(text.as_bytes()), Ok(r.clone()));
        let broken = text.replace("C++", "C+-");
        assert!(matches!(Record::parse(broken.as_bytes()), Err(ReadError::Corrupt(_))));
        let mut trailing = text.clone().into_bytes();
        trailing.push(0);
        assert!(matches!(Record::parse(&trailing), Err(ReadError::Corrupt(_))));
        // WP-1280: the image follows the text, any bytes, checksummed.
        r.image = Some(vec![0, b'\n', 0xff, b'e', b'n', b'd', b' ', 7]);
        let with = r.write();
        assert_eq!(Record::parse(&with), Ok(r.clone()));
        for i in 0..with.len() {
            let mut b = with.clone();
            b[i] ^= 0x41;
            assert_ne!(Record::parse(&b), Ok(r.clone()), "byte {i}");
        }
        assert!(Record::parse(&with[..with.len() - 1]).is_err());
        assert_eq!(Record::parse(b"CPCH...."), Err(ReadError::Clang));
        assert_eq!(Record::parse(b"hello"), Err(ReadError::NotPch));
        assert_eq!(Record::parse(b"KILNPCH 2\n"), Err(ReadError::Newer));
        assert_eq!(Record::parse(b"KILNPCH 0\n"), Err(ReadError::Older));
    }

    /// clang `collectMacroDefinitions` (WP-1023).
    #[test]
    fn macro_states() {
        let a = Args::parse_as(
            "kiln++",
            &["-DA", "-DB=2", "-UA", "-DC=x\ny", "-DB=3"].map(String::from),
        );
        assert_eq!(
            macros(&a),
            [("A".into(), None), ("B".into(), Some("3".into())), ("C".into(), Some("x".into()))]
        );
    }
}
