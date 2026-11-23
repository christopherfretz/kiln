//! Identifier accounting for the written-reference table (spec/ast.md
//! §18.8): every identifier token of the main file that reaches the
//! parser is an entry's range, the own name of a declaration (the first
//! identifier of its §19 `name` span), or in the documented set of
//! identifiers that name nothing a rename edits: keywords and builtin
//! trait keywords, the contextual keywords and alternative tokens,
//! attribute names (`__builtin_has_attribute` operands included) and keyword
//! arguments, labels (`asm goto` labels included), `asm` symbolic
//! operand names and module names. Macro names, directive text and `#pragma` text never
//! reach the parser (the preprocessor consumes them), so they are not
//! counted. A name the table misses is a name a tooling client's rename
//! does not edit.
//!
//! The default test runs over the `.cpp` files of
//! `tests/conformance/written_refs/` and wants no identifier left over.
//! `KILN_REFS_DIR=<dir> cargo test -p kiln_parse --test
//! written_refs_accounting -- --ignored --nocapture` runs it over every
//! `.cpp` file of a directory tree and prints the leftovers, grouped by
//! the token before them; `KILN_REFS_STRICT=1` makes any leftover fail.
//! A file with an error (an include the test cannot resolve, a
//! diagnosed construct) is skipped: its recovery records what it can.
//!
//! `cargo test -p kiln_parse --test written_refs_accounting --
//! --ignored standard_library --nocapture` (WP-1934) accounts the host's
//! libstdc++ instead: a TU that includes `<bits/stdc++.h>`, configured as
//! the driver configures `-std=c++26`, every identifier of every header
//! the parser sees counted (`KILN_REFS_TU=<source text>` replaces the
//! TU). The default test also runs strictly over a pinned list of
//! `tests/conformance` directories (§18.8).

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_lex::Std;
use kiln_lex::{Punct, TokKind};
use kiln_parse::keywords::Keywords;
use kiln_parse::parse_preprocessed;
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_pp::{
    ClangFeatures, DirGroup, HeaderSearch, IncludeResolver, PpOptions, PredefConfig, Preprocessor,
    RealFs, SearchDirSpec, SearchList, build_predefines,
};
use kiln_sema::target::{Arch, Target};
use kiln_source::symbol::{Interner, Symbol};
use kiln_source::{FileId, Loc, SourceManager};

/// Where an identifier is, besides ordinary code.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    Code,
    /// Inside an attribute (`[[...]]`, `__attribute__((...))`,
    /// `__declspec(...)`, the attribute operand of
    /// `__builtin_has_attribute`), at parenthesis depth `depth` within
    /// its attribute list, in the argument list of attribute `owner` (the
    /// last name at the list's own level).
    Attr {
        depth: u32,
        owner: Option<Symbol>,
    },
    /// Inside the parentheses of an `asm` statement or declaration.
    Asm,
    /// After `module` / `import` in a module directive.
    Module,
    /// In a `__label__` declaration (GNU local labels), up to its `;`.
    LocalLabels,
}

/// Where a token is spelled: its file and offset.
type At = (FileId, u32);

/// One identifier token the parser saw, spelled in a counted file.
#[derive(Clone, Copy)]
struct Ident {
    at: At,
    sym: Symbol,
    /// The tokens just before and after it (by stream order).
    prev: Option<TokKind>,
    next: Option<TokKind>,
    place: Place,
}

/// What one file left over.
struct Report {
    /// The identifiers counted.
    seen: usize,
    /// `(line:col, spelling, token before)` of each leftover.
    left: Vec<(String, String, String)>,
    /// The file had errors and was skipped.
    skipped: bool,
}

/// Which files' identifiers are counted.
#[derive(Clone, Copy)]
enum Scope {
    /// The main file.
    Main(FileId),
    /// Every file read from disk: the headers of a TU whose main file
    /// is in memory (the predefines are in memory too).
    Headers,
}

/// `at` as `line:col`, after the file's name for [`Scope::Headers`].
fn line_col(sm: &SourceManager, scope: Scope, (file, off): At) -> String {
    let text = sm.text(file);
    let before = &text[..off as usize];
    let line = before.bytes().filter(|&b| b == b'\n').count() + 1;
    let col = off as usize - before.rfind('\n').map_or(0, |i| i + 1) + 1;
    match scope {
        Scope::Main(_) => format!("{line}:{col}"),
        Scope::Headers => format!("{}:{line}:{col}", sm.file_name(file)),
    }
}

/// Where `l` is spelled, when that is in a counted file.
fn spelled(sm: &SourceManager, scope: Scope, l: Loc) -> Option<At> {
    if !l.is_valid() {
        return None;
    }
    let (f, o) = sm.offset_of(sm.spelling_loc(l))?;
    let counted = match scope {
        Scope::Main(main) => f == main,
        Scope::Headers => !sm.is_virtual(f),
    };
    counted.then_some((f, o))
}

/// An attribute name without its `__x__` spelling's underscores.
fn normalized(interner: &Interner, s: Symbol) -> String {
    let t = interner.as_str(s);
    t.strip_prefix("__").and_then(|t| t.strip_suffix("__")).unwrap_or(t).to_owned()
}

/// The host's search list and predefines for `-std=c++26` with
/// libstdc++, as the driver builds them (`kiln_parse/tests/snap.rs`).
struct Host {
    triple: &'static str,
    dirs: Vec<SearchDirSpec>,
    predefines: String,
}

impl Host {
    /// `None` (the test prints why) on a host without the resource
    /// files or the headers.
    fn new() -> Option<Host> {
        let triple = match std::env::consts::ARCH {
            "aarch64" => "aarch64-unknown-linux-gnu",
            "x86_64" => "x86_64-unknown-linux-gnu",
            _ => return None,
        };
        let resource = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resource");
        let list =
            std::fs::read_to_string(resource.join(format!("include-paths/{triple}.txt"))).ok()?;
        let res_inc = resource.join("include").display().to_string();
        let mut dirs = Vec::new();
        for line in list.lines() {
            let (group, p) = if let Some(p) = line.strip_prefix("#   -internal-isystem ") {
                (DirGroup::System, p)
            } else if let Some(p) = line.strip_prefix("#   -internal-externc-isystem ") {
                (DirGroup::ExternCSystem, p)
            } else {
                continue;
            };
            let p =
                if p.ends_with("/lib/clang/19/include") { res_inc.clone() } else { p.to_owned() };
            dirs.push(SearchDirSpec::new(p, group));
        }
        if !dirs.iter().any(|d| Path::new(&d.path).join("bits/stdc++.h").is_file()) {
            eprintln!("skipped: no libstdc++ headers on this host");
            return None;
        }
        let base = std::fs::read_to_string(resource.join(format!("predefines/{triple}.c++26.txt")))
            .ok()?;
        let mut cfg = PredefConfig::new(true);
        cfg.gnu = false;
        cfg.aarch64 = triple.starts_with("aarch64");
        cfg.reflection = true;
        cfg.std_float_aliases = true;
        let predefines = build_predefines(&base, &cfg, &[]);
        Some(Host { triple, dirs, predefines })
    }

    fn target(&self) -> Target {
        Target::for_arch(if self.triple.starts_with("x86_64") {
            Arch::X86_64
        } else {
            Arch::AArch64
        })
    }
}

/// The parser options: the default ones, and for `host` the driver's
/// `-std=c++26` with libstdc++.
fn parse_options(host: Option<&Host>) -> ParseOptions {
    let opts = ParseOptions { written_refs: true, decl_extents: true, ..ParseOptions::default() };
    match host {
        None => opts,
        Some(_) => {
            ParseOptions { std: Std::Cxx26, reflection: true, gnu_lib_builtins: true, ..opts }
        }
    }
}

/// The predefined macros of `kiln -std=c++26 --target=x86_64-linux-gnu`
/// for `opts`, so `__SIZE_TYPE__` and the others are defined as on the
/// command line (without them a corpus file's `using size_t =
/// __SIZE_TYPE__;` is invalid, which once made the constant evaluator
/// panic, WP-2173).
fn x86_64_predefines(opts: &ParseOptions) -> String {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../resource/predefines/x86_64-unknown-linux-gnu.c++26.txt");
    let base = std::fs::read_to_string(base).expect("the predefines file");
    let mut cfg = PredefConfig::new(true);
    cfg.gnu = opts.gnu;
    cfg.aarch64 = false;
    cfg.reflection = opts.reflection;
    build_predefines(&base, &cfg, &[])
}

/// A preprocessor as `parse_tu` configures one, with predefines (the
/// x86-64 ones, or `host`'s) and `host`'s search list and feature
/// queries; its main file entered.
fn preprocessor<'a, 'd>(
    sm: &'a SourceManager,
    engine: &'d DiagEngine<'a>,
    interner: &'a Interner,
    main: FileId,
    host: Option<&Host>,
) -> Preprocessor<'a, 'd> {
    let opts = parse_options(host);
    let mut ppo = PpOptions::new(opts.std);
    ppo.predefines = match host {
        Some(h) => h.predefines.clone(),
        None => x86_64_predefines(&opts),
    };
    ppo.char8 = opts.char8;
    ppo.reflection = opts.reflection;
    ppo.parser_pragmas = true;
    ppo.modules = Some(kiln_pp::ModuleOptions::default());
    let mut pp = Preprocessor::new(sm, engine, interner, ppo);
    if let Some(h) = host {
        let mut fs = RealFs::new();
        let list = SearchList::build(&h.dirs, true, &mut fs);
        let r: Box<dyn IncludeResolver> = Box::new(HeaderSearch::new(fs, list));
        pp.set_include_resolver(r);
        let features = ClangFeatures::new(h.triple, Std::Cxx26, &[]).expect("a known triple");
        pp.set_feature_queries(Box::new(features));
    }
    pp.enter_main_file(main);
    pp
}

/// What the `(` after a keyword opens.
#[derive(Clone, Copy)]
enum Opening {
    /// An attribute list at this base depth.
    Attr(u32),
    HasAttribute,
    Asm,
}

/// The nesting state of the identifier scan.
#[derive(Default)]
struct Scan {
    /// Attribute nesting: `[[` opens with its list at depth 0 and closes
    /// at the `]]` that matches; `__attribute__((` has its list at
    /// parenthesis depth 2, `__declspec(` at 1. `(base, depth)`: `base`
    /// 0 for `[[`.
    attr: Option<(u32, u32)>,
    /// What the next `(` opens.
    opening: Option<Opening>,
    owner: Option<Symbol>,
    /// `__builtin_has_attribute(e, attr...)`: the parenthesis depth inside
    /// it until the `,` after which its attribute list starts (base 1).
    has_attr: Option<u32>,
    /// The parenthesis depth of an `asm` operand list.
    asm: Option<u32>,
    module: bool,
    local_labels: bool,
}

impl Scan {
    /// Tracks punctuator `p` between `prev` and `next`.
    fn punct(&mut self, p: Punct, prev: Option<TokKind>, next: Option<TokKind>) {
        let deeper = |d: Option<u32>| d.map(|d| d + 1);
        let shallower = |d: Option<u32>| d.and_then(|d| (d > 1).then(|| d - 1));
        match p {
            Punct::LSquare
                if self.attr.is_none() && next == Some(TokKind::Punct(Punct::LSquare)) =>
            {
                self.attr = Some((0, 0));
            }
            Punct::RSquare
                if self.attr == Some((0, 0)) && prev == Some(TokKind::Punct(Punct::RSquare)) =>
            {
                self.attr = None;
            }
            Punct::LParen => {
                let opening = self.opening.take();
                self.attr = match (opening, self.attr) {
                    (Some(Opening::Attr(base)), _) => Some((base, 1)),
                    (_, a) => a.map(|(b, d)| (b, d + 1)),
                };
                self.has_attr = match opening {
                    Some(Opening::HasAttribute) => Some(1),
                    _ => deeper(self.has_attr),
                };
                self.asm = match opening {
                    Some(Opening::Asm) => Some(1),
                    _ => deeper(self.asm),
                };
            }
            Punct::RParen => {
                if let Some((b, d)) = self.attr {
                    self.attr = if b > 0 && d <= 1 { None } else { Some((b, d.saturating_sub(1))) };
                }
                self.asm = shallower(self.asm);
                self.has_attr = shallower(self.has_attr);
            }
            Punct::Comma if self.has_attr == Some(1) && self.attr.is_none() => {
                self.has_attr = None;
                self.attr = Some((1, 1));
            }
            Punct::Semi => {
                self.module = false;
                self.local_labels = false;
            }
            _ => {}
        }
    }

    /// Where an identifier `s` at the current point is.
    fn place(&mut self, s: Symbol) -> Place {
        if let Some((b, d)) = self.attr {
            let depth = d.saturating_sub(b);
            if depth == 0 {
                self.owner = Some(s);
            }
            Place::Attr { depth, owner: self.owner }
        } else if self.asm.is_some() {
            Place::Asm
        } else if self.module {
            Place::Module
        } else if self.local_labels {
            Place::LocalLabels
        } else {
            Place::Code
        }
    }
}

/// The identifier tokens of the counted files the parser sees: the
/// preprocessed stream, as the parse builds it.
fn idents(
    sm: &SourceManager,
    interner: &Interner,
    main: FileId,
    scope: Scope,
    host: Option<&Host>,
) -> Vec<Ident> {
    let engine = DiagEngine::new(sm, &DiagOptions::default(), MemorySink::new());
    let mut pp = preprocessor(sm, &engine, interner, main, host);
    // `__attribute` is GCC's other spelling of `__attribute__`.
    let attributes = [interner.intern("__attribute__"), interner.intern("__attribute")];
    let declspec = interner.intern("__declspec");
    let has_attribute = interner.intern("__builtin_has_attribute");
    let local_label = interner.intern("__label__");
    let asms: Vec<Symbol> =
        ["asm", "__asm", "__asm__"].iter().map(|s| interner.intern(s)).collect();
    let module = interner.intern("module");
    let import = interner.intern("import");
    let mut toks = Vec::new();
    loop {
        let t = pp.next();
        if t.is_eof() {
            break;
        }
        toks.push(t);
    }
    let mut out = Vec::new();
    let mut scan = Scan::default();
    for (i, t) in toks.iter().enumerate() {
        let prev = i.checked_sub(1).map(|j| toks[j].kind);
        let next = toks.get(i + 1).map(|n| n.kind);
        let s = match t.kind {
            TokKind::Punct(p) => {
                scan.punct(p, prev, next);
                continue;
            }
            TokKind::Identifier(s) => s,
            _ => continue,
        };
        if scan.attr.is_none() && (attributes.contains(&s) || s == declspec) {
            scan.opening = Some(Opening::Attr(if s == declspec { 1 } else { 2 }));
            continue;
        }
        if scan.attr.is_none() && scan.has_attr.is_none() && s == has_attribute {
            scan.opening = Some(Opening::HasAttribute);
            continue;
        }
        if s == local_label {
            scan.local_labels = true;
            continue;
        }
        if scan.asm.is_none() && asms.contains(&s) {
            scan.opening = Some(Opening::Asm);
            continue;
        }
        let starts_directive = (s == module || s == import)
            && matches!(next, Some(TokKind::Identifier(_) | TokKind::Punct(_)))
            && prev.is_none_or(|p| {
                matches!(p, TokKind::Punct(Punct::Semi | Punct::RBrace | Punct::LBrace))
                    || matches!(p, TokKind::Identifier(_))
            });
        let Some(at) = spelled(sm, scope, t.span.lo) else { continue };
        let place = scan.place(s);
        out.push(Ident { at, sym: s, prev, next, place });
        scan.module |= starts_directive;
    }
    out
}

/// The identifier offsets covered by entries and declaration names.
fn covered(
    sm: &SourceManager,
    scope: Scope,
    tu: &kiln_parse::ParsedTu,
    ids: &[Ident],
) -> HashSet<At> {
    let mut sorted: Vec<At> = ids.iter().map(|i| i.at).collect();
    sorted.sort_unstable();
    sorted.dedup();
    let mut cov = HashSet::new();
    let mut span = |lo: Loc, hi: Loc, first_only: bool| {
        let Some(a) = spelled(sm, scope, lo) else { return };
        let b =
            spelled(sm, scope, hi).filter(|&b| b.0 == a.0 && b.1 > a.1).unwrap_or((a.0, a.1 + 1));
        let start = sorted.partition_point(|&o| o < a);
        for &o in &sorted[start..] {
            if o >= b {
                break;
            }
            cov.insert(o);
            if first_only {
                break;
            }
        }
    };
    for (_, r) in tu.ast.written_refs.iter() {
        span(r.range.lo, r.range.hi, false);
    }
    for (_, e) in tu.ast.decl_extents.iter() {
        if e.name.lo.is_valid() {
            span(e.name.lo, e.name.hi, true);
        }
    }
    cov
}

fn account(name: &str, src: &str) -> Report {
    account_tu(name, src, None)
}

/// Accounts the TU of main file `src` (named `name`): its main file,
/// or with `host` the headers it includes.
fn account_tu(name: &str, src: &str, host: Option<&Host>) -> Report {
    let sm = SourceManager::new();
    let main = sm.add_virtual(name, src.to_owned()).expect("location space");
    let scope = if host.is_some() { Scope::Headers } else { Scope::Main(main) };
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let opts = parse_options(host);
    // Not `parse_tu`: it preprocesses with no predefined macros, and the
    // test's preprocessor has the target's, as the command line.
    let pp = preprocessor(&sm, &engine, &interner, main, host);
    let target = host.map_or_else(|| Target::for_arch(Arch::X86_64), Host::target);
    let limits = ConstexprLimits::default();
    let (tu, _) = parse_preprocessed(pp, Arc::clone(&interner), &engine, target, &opts, limits);
    if engine.error_count() > 0 {
        if host.is_some() {
            for d in sink.diagnostics().iter().filter(|d| d.level >= kiln_diag::Level::Error) {
                let at = sm.offset_of(d.span.lo).map(|a| line_col(&sm, scope, a));
                eprintln!("{}: {}", at.unwrap_or_default(), d.message);
            }
        }
        return Report { seen: 0, left: Vec::new(), skipped: true };
    }
    let ids = idents(&sm, &interner, main, scope, host);
    let cov = covered(&sm, scope, &tu, &ids);
    let kw = Keywords::with_std(&interner, opts.gnu, true, true, opts.clang_compat);
    // The contextual keywords, and the alternative tokens ([lex.digraph]),
    // which the parser reads as operators.
    let contextual: HashSet<Symbol> = [
        "final", "override", "pre", "post", "import", "module", "and", "and_eq", "bitand", "bitor",
        "compl", "not", "not_eq", "or", "or_eq", "xor", "xor_eq",
    ]
    .iter()
    .map(|s| interner.intern(s))
    .collect();
    let keyword_args = [
        "enum_extensibility",
        "format",
        "mode",
        "visibility",
        "type_visibility",
        "pcs",
        "zero_call_used_regs",
        "function_return",
        "cpu_dispatch",
        "cpu_specific",
        // `access(read_only, 1)`: the access mode (GCC).
        "access",
        // A builtin's name: a builtin alias names no declaration.
        "__clang_arm_builtin_alias",
        "clang_builtin_alias",
        "builtin_alias",
    ];
    let goto_sym = interner.intern("goto");
    let mut left = Vec::new();
    let mut seen_offs = HashSet::new();
    for i in &ids {
        if !seen_offs.insert(i.at) || cov.contains(&i.at) {
            continue;
        }
        if kw.get(i.sym).is_some() || kw.builtin(i.sym).is_some() || contextual.contains(&i.sym) {
            continue;
        }
        // An attribute name: at the attribute list's own level, or a
        // scoped name's namespace.
        let attr = match i.place {
            Place::Attr { depth, owner } => Some((depth, owner)),
            _ => None,
        };
        if attr.is_some_and(|(d, _)| d == 0 || i.next == Some(TokKind::Punct(Punct::ColonColon))) {
            continue;
        }
        // An attribute's keyword argument: an identifier argument the
        // attribute reads as one of its own words (clang
        // `attributeHasIdentifierArg` and `VariadicIdentifierArgument`:
        // `mode(byte)`, `format(printf, 1, 2)`, `cpu_dispatch(atom)`).
        let lone = matches!(i.prev, Some(TokKind::Punct(Punct::LParen | Punct::Comma)))
            && matches!(i.next, Some(TokKind::Punct(Punct::RParen | Punct::Comma)));
        if let Some((1, Some(o))) = attr
            && lone
            && keyword_args.contains(&normalized(&interner, o).as_str())
        {
            continue;
        }
        // A label: `l:` (not `::`), `goto l`, `&&l`, `__label__ a, b`; an
        // `asm` statement's symbolic operand names and `asm goto` labels.
        let label = i.next == Some(TokKind::Punct(Punct::Colon))
            || i.prev == Some(TokKind::Identifier(goto_sym))
            || i.place == Place::LocalLabels
            || i.prev == Some(TokKind::Punct(Punct::AmpAmp));
        if label || matches!(i.place, Place::Module | Place::Asm) {
            continue;
        }
        let spelling = interner.as_str(i.sym).to_owned();
        let before = match i.prev {
            Some(TokKind::Identifier(s)) => interner.as_str(s).to_owned(),
            Some(TokKind::Punct(p)) => p.as_str().to_owned(),
            Some(k) => format!("{k:?}"),
            None => "(start)".to_owned(),
        };
        let before =
            if attr.is_some() { format!("attribute argument after {before}") } else { before };
        left.push((line_col(&sm, scope, i.at), spelling, before));
    }
    Report { seen: seen_offs.len(), left, skipped: false }
}

fn cpp_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).expect("a directory").flatten() {
            let p = e.path();
            if p.is_dir() {
                todo.push(p);
            } else if p.extension().is_some_and(|x| x == "cpp") {
                files.push(p);
            }
        }
    }
    files.sort();
    files
}

/// One leftover: `(file, line:col, spelling, token before)`.
type Leftover = (String, String, String, String);

/// Accounts every file on a thread with a large stack (a debug build's
/// frames are large); a front-end panic is reported, not fatal.
/// `(files accounted, files skipped, identifiers, leftovers)`.
fn run(dir: &Path) -> (usize, usize, usize, Vec<Leftover>) {
    let files = cpp_files(dir);
    let base = dir.to_path_buf();
    let worker = std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        let (mut counted, mut skipped, mut seen) = (0, 0, 0);
        let mut left = Vec::new();
        for f in &files {
            let Ok(src) = std::fs::read_to_string(f) else { continue };
            let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let shown = f.strip_prefix(&base).unwrap_or(f).display().to_string();
            let Ok(r) = std::panic::catch_unwind(|| account(&name, &src)) else {
                left.push((shown, String::new(), String::new(), "panic".into()));
                continue;
            };
            if r.skipped {
                skipped += 1;
                continue;
            }
            counted += 1;
            seen += r.seen;
            for (at, s, before) in r.left {
                left.push((shown.clone(), at, s, before));
            }
        }
        (counted, skipped, seen, left)
    });
    worker.expect("a thread").join().expect("no panic")
}

/// Prints `left` grouped by the token before each leftover, the largest
/// group first (all of a group with `KILN_REFS_ALL`, else 8).
fn print_groups(left: &[Leftover]) {
    let mut by_before: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for (f, at, s, b) in left {
        let f = if f.is_empty() { String::new() } else { format!("{f}:") };
        by_before.entry(b.as_str()).or_default().push(format!("{f}{at}: '{s}'"));
    }
    let mut groups: Vec<_> = by_before.into_iter().collect();
    groups.sort_by_key(|(_, v)| std::cmp::Reverse(v.len()));
    let all = std::env::var_os("KILN_REFS_ALL").is_some();
    for (b, v) in &groups {
        eprintln!("== after {b}: {}", v.len());
        for x in v.iter().take(if all { usize::MAX } else { 8 }) {
            eprintln!("  {x}");
        }
    }
}

fn conformance() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance")
}

/// The `tests/conformance` directories the default test also accounts
/// strictly (§18.8): declarations, classes, templates, overloading,
/// lookup and expressions, every identifier accounted for over all of
/// their files without errors, in a few seconds.
const PINNED: &[&str] = &[
    "basic.lookup.argdep",
    "class.derived",
    "class.friend",
    "class.mem",
    "class.virtual",
    "dcl.init.list",
    "expr.prim.lambda.capture",
    "namespace.udecl",
    "over.match.class.deduct",
    "temp.dep.type",
    "temp.expl.spec",
    "temp.friend",
    "temp.inst",
    "temp.names",
    "temp.variadic",
    "templates",
];

/// Every identifier of the written-reference corpus, and of the pinned
/// conformance directories, is accounted for.
#[test]
fn every_identifier_is_accounted_for() {
    let dir = conformance().join("written_refs");
    let (counted, _skipped, seen, mut left) = run(&dir);
    assert!(counted >= 20, "the corpus parses ({counted} files)");
    assert!(seen > 1000, "the corpus has identifiers ({seen})");
    let (mut pinned_counted, mut pinned_seen) = (0, 0);
    for d in PINNED {
        let (c, _, s, l) = run(&conformance().join(d));
        assert!(c > 0, "{d}: no file without errors");
        pinned_counted += c;
        pinned_seen += s;
        left.extend(l.into_iter().map(|(f, at, s, b)| (format!("{d}/{f}"), at, s, b)));
    }
    eprintln!("pinned: {pinned_counted} files, {pinned_seen} identifiers");
    assert!(pinned_counted >= 500, "the pinned directories parse ({pinned_counted} files)");
    assert!(pinned_seen > 40_000, "the pinned directories have identifiers ({pinned_seen})");
    let lines: Vec<String> =
        left.iter().map(|(f, at, s, b)| format!("{f}:{at}: '{s}' (after {b})")).collect();
    assert!(lines.is_empty(), "{} identifiers left over:\n{}", lines.len(), lines.join("\n"));
}

/// WP-2173: two corpus files whose enumeration's underlying type is an
/// alias of an invalid type ([dcl.typedef]/2, [dcl.enum]/2) made the
/// constant evaluator panic: `type-aware-direct-call.cpp` only without the
/// target's predefined macros (its `using size_t = __SIZE_TYPE__;`), as
/// `parse_tu` preprocesses. Both are diagnosed, not a panic, with and
/// without them.
#[test]
fn invalid_alias_enum_bases_are_diagnosed() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance");
    let worker = std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        for (f, bare) in [
            ("expr.const/type-aware-direct-call.cpp", 12),
            ("dcl.enum/invalid-alias-base-constexpr-ice.cpp", 3),
        ] {
            let src = std::fs::read_to_string(dir.join(f)).expect("the corpus file");
            assert!(account(f, &src).skipped, "{f}: diagnosed with the predefined macros");
            let sm = SourceManager::new();
            let main = sm.add_virtual(f, src).expect("location space");
            let interner = Arc::new(Interner::new());
            let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
            let opts = ParseOptions::default();
            let target = Target::for_arch(Arch::X86_64);
            let limits = ConstexprLimits::default();
            let _ = parse_tu(&sm, &interner, main, &engine, target, &opts, limits);
            assert_eq!(engine.error_count(), bare, "{f}: diagnosed without them");
        }
    });
    worker.expect("a thread").join().expect("no panic");
}

/// The same accounting over the `.cpp` files of `KILN_REFS_DIR`.
#[test]
#[ignore = "set KILN_REFS_DIR to a directory of sources"]
fn every_identifier_is_accounted_for_in_a_directory() {
    let Ok(dir) = std::env::var("KILN_REFS_DIR") else { return };
    let (counted, skipped, seen, left) = run(Path::new(&dir));
    eprintln!(
        "{counted} files accounted ({skipped} skipped with errors), {seen} identifiers, {} left over",
        left.len()
    );
    print_groups(&left);
    if std::env::var_os("KILN_REFS_STRICT").is_some() {
        assert!(left.is_empty(), "{} identifiers left over", left.len());
    }
}

/// The same accounting over the host's libstdc++: every identifier of
/// every header a TU that includes `<bits/stdc++.h>` reads (WP-1934).
#[test]
#[ignore = "parses <bits/stdc++.h> in-process (about a minute in a debug build)"]
fn every_identifier_is_accounted_for_in_the_standard_library() {
    let Some(host) = Host::new() else { return };
    let src = std::env::var("KILN_REFS_TU").unwrap_or_else(|_| "#include <bits/stdc++.h>\n".into());
    let worker = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || account_tu("stdcxx.cpp", &src, Some(&host)));
    let r = worker.expect("a thread").join().expect("no panic");
    assert!(!r.skipped, "the TU has errors");
    let left: Vec<Leftover> =
        r.left.into_iter().map(|(at, s, b)| (String::new(), at, s, b)).collect();
    eprintln!("{} identifiers, {} left over", r.seen, left.len());
    print_groups(&left);
    if std::env::var_os("KILN_REFS_STRICT").is_some() {
        assert!(left.is_empty(), "{} identifiers left over", left.len());
    }
}

// ---------------------------------------------------------------------
// The join (spec/ast.md §18.11.1, WP-2326): a name expression and its entry
// ---------------------------------------------------------------------

/// The name expressions of `src` (named `name`) without an entry they
/// join: every `DeclRefExpr` and `MemberExpr` of the code as written
/// (`kiln_ast::dump::for_each_written_expr`) whose declaration has a name
/// must have exactly one entry whose `name_lo` is the expression's
/// `loc()`, of role `expr`, `member`, `template` or `concept`, with the
/// expression's declaration as its entity. `None`: the file has errors.
/// Each mismatch is `(line:col, what)`.
fn join(name: &str, src: &str) -> Option<(usize, Vec<(String, String)>)> {
    use kiln_ast::expr::ExprKind;
    use kiln_ast::written_refs::RefRole;
    let sm = SourceManager::new();
    let main = sm.add_virtual(name, src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let opts = parse_options(None);
    let pp = preprocessor(&sm, &engine, &interner, main, None);
    let target = Target::for_arch(Arch::X86_64);
    let limits = ConstexprLimits::default();
    let (tu, _) = parse_preprocessed(pp, Arc::clone(&interner), &engine, target, &opts, limits);
    if engine.error_count() > 0 {
        return None;
    }
    let ast = &tu.ast;
    let mut at: std::collections::HashMap<Loc, Vec<kiln_ast::written_refs::WrittenRefId>> =
        std::collections::HashMap::new();
    for (id, _) in ast.written_refs.iter() {
        at.entry(ast.written_refs.name_lo(id)).or_default().push(id);
    }
    let dopts = kiln_ast::dump::DumpOptions {
        sources: Some(&sm),
        main_file: Some(main),
        ..Default::default()
    };
    let mut names = Vec::new();
    let mut parent = std::collections::HashMap::new();
    kiln_ast::dump::for_each_written_expr(ast, &dopts, |e, up| {
        if let Some(p) = up {
            parent.entry(e).or_insert(p);
        }
        let k = ast.exprs.get(e).kind();
        if matches!(k, ExprKind::DeclRefExpr | ExprKind::MemberExpr) {
            names.push(e);
        }
    });
    names.sort_unstable();
    names.dedup();
    let name_of = |d| {
        let mut s = String::new();
        kiln_ast::types::TypeNames::write_decl_name(ast, d, true, &mut s);
        s
    };
    let mut bad = Vec::new();
    let mut checked = 0;
    for e in names {
        let n = ast.exprs.get(e);
        let d = ast.exprs.decl(e);
        let loc = n.loc();
        let Some(a) = spelled(&sm, Scope::Main(main), loc) else { continue };
        if implicit_name(ast, &parent, e, src, a.1) {
            continue;
        }
        checked += 1;
        let ids = at.get(&loc).map_or(&[][..], Vec::as_slice);
        let ok: Vec<_> = ids
            .iter()
            .map(|&id| ast.written_refs.get(id))
            .filter(|r| {
                matches!(
                    r.role,
                    RefRole::Expr | RefRole::Member | RefRole::Template | RefRole::Concept
                )
            })
            .collect();
        // A member access in a template pattern that names an overload
        // set has the set's first member as its declaration and an entry
        // naming nothing yet: the set is resolved per specialization
        // (§18.11.1, §18.9.4).
        let overload = n.ty() == kiln_ast::QualType::OVERLOAD;
        let what = match ok.as_slice() {
            [r] if r.entity == Some(d) => continue,
            [r] if overload
                && r.entity.is_none()
                && r.flags & kiln_ast::written_refs::REF_DEPENDENT != 0 =>
            {
                continue;
            }
            [r] => format!(
                "{k:?} of {:?} '{}', entry {} names {:?}",
                ast.decls.get(d).kind(),
                name_of(d),
                r.role.spelling(),
                r.entity.map(|x| ast.decls.get(x).kind()),
                k = n.kind(),
            ),
            [] => format!(
                "{:?} of {:?} '{}': no entry ({} of other roles)",
                n.kind(),
                ast.decls.get(d).kind(),
                name_of(d),
                ids.len()
            ),
            _ => format!("{:?}: {} entries", n.kind(), ok.len()),
        };
        bad.push((line_col(&sm, Scope::Main(main), a), what));
    }
    Some((checked, bad))
}

/// True when name expression `e` (spelled at offset `off` of `src`) has
/// no name token of its own, so no entry joins it (spec/ast.md
/// §18.11.1's exceptions 2 to 6; the walk leaves out exception 1): a
/// declaration without a name or an implicit one (an anonymous union
/// member, a structured binding's variable and the member access on it,
/// a range-for's `__begin`), a condition variable's value, a reference
/// at a token that does not spell its name (a user-defined literal's
/// literal operator, an implicit conversion function, a structured
/// binding's `get`, a coroutine's implicit calls), an operator use's
/// callee (`a + b`, `f(x)` calling `operator()`, `a = b`), and an
/// implicit capture's initializer (a capture-default's).
fn implicit_name(
    ast: &kiln_ast::Ast,
    parent: &std::collections::HashMap<kiln_ast::ExprId, kiln_ast::ExprId>,
    e: kiln_ast::ExprId,
    src: &str,
    off: u32,
) -> bool {
    use kiln_ast::decl::{DeclFlags, DeclKind};
    use kiln_ast::expr::ExprKind;
    let d = ast.exprs.decl(e);
    let dn = ast.decls.get(d);
    if dn.name().is_empty() || dn.has(DeclFlags::IMPLICIT) {
        return true;
    }
    // A reference at the declaration's own name: a condition variable's
    // value (`if (auto t = e)`).
    if ast.exprs.get(e).loc() == dn.loc() {
        return true;
    }
    // A structured binding's member access on its hidden variable.
    if ast.exprs.get(e).kind() == ExprKind::MemberExpr {
        let mut b = ast.exprs.operand(e);
        while matches!(ast.exprs.get(b).kind(), ExprKind::ImplicitCastExpr) {
            b = ast.exprs.operand(b);
        }
        if ast.exprs.get(b).kind() == ExprKind::DeclRefExpr {
            let bd = ast.decls.get(ast.exprs.decl(b));
            if bd.name().is_empty() || bd.has(DeclFlags::IMPLICIT) {
                return true;
            }
        }
    }
    // No token spelling the name at the location: sema built the
    // reference at another token (a user-defined literal's literal
    // operator, a conversion function an implicit conversion calls, a
    // structured binding's `get`, a coroutine's `await_ready` at its
    // `co_await`).
    let text = src.get(off as usize..).unwrap_or("");
    let named = match (dn.kind(), dn.name().as_ident()) {
        (DeclKind::CXXDestructor, _) => text.starts_with('~'),
        (_, Some(n)) => text
            .strip_prefix(ast.symbols().as_str(n))
            .is_some_and(|rest| !rest.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')),
        _ => text.starts_with("operator"),
    };
    if !named {
        return true;
    }
    // The first ancestor that is not an implicit wrapper, and the child
    // of it on the way up.
    let wrapper = |x| {
        matches!(
            ast.exprs.get(x).kind(),
            ExprKind::ImplicitCastExpr
                | ExprKind::CXXConstructExpr
                | ExprKind::MaterializeTemporaryExpr
                | ExprKind::CXXBindTemporaryExpr
                | ExprKind::ExprWithCleanups
                | ExprKind::ConstantExpr
                | ExprKind::OpaqueValueExpr
                | ExprKind::ArrayInitLoopExpr
        )
    };
    let (mut child, mut up) = (e, parent.get(&e).copied());
    while let Some(p) = up
        && wrapper(p)
    {
        child = p;
        up = parent.get(&p).copied();
    }
    let Some(p) = up else { return false };
    match ast.exprs.get(p).kind() {
        ExprKind::CXXOperatorCallExpr => ast.exprs.callee(p) == child,
        ExprKind::LambdaExpr => {
            let l = ast.exprs.lambda(p);
            let inits = l.inits.map(|i| ast.lists.iter::<kiln_ast::ExprId>(i).collect::<Vec<_>>());
            inits
                .and_then(|v| v.iter().position(|&x| x == child))
                .is_some_and(|i| l.captures.get(i).is_some_and(|c| c.implicit))
        }
        _ => false,
    }
}

/// [`join`] over every `.cpp` file of `dir`: `(files, names, mismatches
/// as (file, line:col, what))`.
fn run_join(dir: &Path) -> (usize, usize, Vec<(String, String, String)>) {
    let files = cpp_files(dir);
    let base = dir.to_path_buf();
    let worker = std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        let (mut counted, mut names) = (0, 0);
        let mut bad = Vec::new();
        for f in &files {
            let Ok(src) = std::fs::read_to_string(f) else { continue };
            let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let shown = f.strip_prefix(&base).unwrap_or(f).display().to_string();
            let Ok(r) = std::panic::catch_unwind(|| join(&name, &src)) else {
                bad.push((shown, String::new(), "panic".into()));
                continue;
            };
            let Some((n, b)) = r else { continue };
            counted += 1;
            names += n;
            bad.extend(b.into_iter().map(|(at, w)| (shown.clone(), at, w)));
        }
        (counted, names, bad)
    });
    worker.expect("a thread").join().expect("no panic")
}

/// The join over the written-reference corpus and the pinned directories.
#[test]
fn every_name_expression_joins_its_entry() {
    let mut dirs = vec!["written_refs"];
    dirs.extend_from_slice(PINNED);
    let (mut files, mut names, mut bad) = (0, 0, Vec::new());
    for d in dirs {
        let (c, n, b) = run_join(&conformance().join(d));
        files += c;
        names += n;
        bad.extend(b.into_iter().map(|(f, at, w)| format!("{d}/{f}:{at}: {w}")));
    }
    eprintln!("join: {files} files, {names} name expressions, {} mismatches", bad.len());
    assert!(names > 1000, "the corpus has name expressions ({names})");
    assert!(bad.is_empty(), "{} mismatches:\n{}", bad.len(), bad.join("\n"));
}

/// The join over the `.cpp` files of `KILN_REFS_DIR`.
#[test]
#[ignore = "set KILN_REFS_DIR to a directory of sources"]
fn every_name_expression_joins_its_entry_in_a_directory() {
    let Ok(dir) = std::env::var("KILN_REFS_DIR") else { return };
    let (files, names, bad) = run_join(Path::new(&dir));
    eprintln!("join: {files} files, {names} name expressions, {} mismatches", bad.len());
    for (f, at, w) in &bad {
        eprintln!("{f}:{at}: {w}");
    }
    if std::env::var_os("KILN_REFS_STRICT").is_some() {
        assert!(bad.is_empty(), "{} mismatches", bad.len());
    }
}
