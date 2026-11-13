//! Snapshots of the semantic state (spec/sema.md and spec/parse.md
//! "Snapshots", spec/snap.md): a prefix ([cpp.include]: a header read as
//! the main file, as `-emit-pch` reads it) parsed to its end by
//! [`parse_prefix`], written with the source manager and the interner,
//! read back into a fresh, configured [`Sema`] ([`new_sema`]) and
//! continued by [`continue_preprocessed`], goes on exactly as the
//! original does: the same diagnostics and the same AST for the main
//! file ([temp.inst], [expr.const], [class.default.ctor],
//! [basic.lookup.argdep], [basic.def.odr], [dcl.attr.deprecated],
//! [meta.reflection]).
//!
//! The prefixes are the pch bench group's header sets (spec/bench.md
//! "The pch group") and a GCC 16 `<meta>` set. They use the host's
//! standard library headers and are skipped (with a message) where the
//! host has none; the target is the host's.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use kiln_ast::dump::{DumpOptions, dump_to_string};
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_lex::Std;
use kiln_parse::{ConstexprLimits, ParseOptions, continue_preprocessed, new_sema, parse_prefix};
use kiln_pp::{
    ClangFeatures, DirGroup, HeaderSearch, IncludeResolver, ModuleOptions, PpOptions, PredefConfig,
    Preprocessor, RealFs, SearchDirSpec, SearchList, build_predefines,
};
use kiln_sema::Sema;
use kiln_sema::target::{Arch, Target};
use kiln_snap::{Reader, SnapError, Writer};
use kiln_source::symbol::Interner;
use kiln_source::{FileId, SourceManager};

// ----- configuration ----------------------------------------------------

/// The standard library a prefix is compiled against.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Lib {
    /// The host's libstdc++ (clang 19's search list).
    Gnu,
    /// The host's libc++ 19 (`-stdlib=libc++`).
    Cxx,
    /// GCC 16.1's libstdc++ (`--gcc-toolchain=`), for `<meta>`.
    Gcc16,
}

/// The host's normalized triple (the resource files are per target).
fn triple() -> Option<&'static str> {
    match std::env::consts::ARCH {
        "aarch64" => Some("aarch64-unknown-linux-gnu"),
        "x86_64" => Some("x86_64-unknown-linux-gnu"),
        _ => None,
    }
}

fn target() -> Target {
    if std::env::consts::ARCH == "x86_64" {
        Target::for_arch(Arch::X86_64)
    } else {
        Target::for_arch(Arch::AArch64)
    }
}

fn manifest() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn resource() -> PathBuf {
    manifest().join("../../resource")
}

/// GCC 16.1's install prefix, `.toolchain/gcc-16.1.0-image/usr/local` in
/// this checkout or an enclosing directory.
fn gcc16() -> Option<PathBuf> {
    manifest()
        .ancestors()
        .map(|d| d.join(".toolchain/gcc-16.1.0-image/usr/local"))
        .find(|p| p.join("include/c++/16.1.0/meta").is_file())
}

/// What a consumer configures from its command line: the search list,
/// the predefines and the parser options, as the driver builds them for
/// `-std=c++26` (`-freflection` on, the default there).
struct Config {
    triple: &'static str,
    dirs: Vec<SearchDirSpec>,
    predefines: String,
    lib: Lib,
}

impl Config {
    fn host(lib: Lib) -> Option<Config> {
        let triple = triple()?;
        let list_name = match lib {
            Lib::Cxx => format!("include-paths/{triple}.libc++.txt"),
            Lib::Gnu | Lib::Gcc16 => format!("include-paths/{triple}.txt"),
        };
        let list = fs::read_to_string(resource().join(list_name)).ok()?;
        let res_inc = resource().join("include").display().to_string();
        let mut dirs = Vec::new();
        let gcc = if lib == Lib::Gcc16 {
            let Some(g) = gcc16() else {
                println!("unsupported: no GCC 16.1 image (.toolchain/gcc-16.1.0-image)");
                return None;
            };
            let v = g.join("include/c++/16.1.0");
            let arch = if triple.starts_with("x86_64") { "x86_64" } else { "aarch64" };
            for d in [v.clone(), v.join(format!("{arch}-linux-gnu")), v.join("backward")] {
                dirs.push(SearchDirSpec::new(d.display().to_string(), DirGroup::System));
            }
            true
        } else {
            false
        };
        // The cc1 classification block: `-internal-isystem` and
        // `-internal-externc-isystem`, in order.
        for line in list.lines() {
            let (group, p) = if let Some(p) = line.strip_prefix("#   -internal-isystem ") {
                (DirGroup::System, p)
            } else if let Some(p) = line.strip_prefix("#   -internal-externc-isystem ") {
                (DirGroup::ExternCSystem, p)
            } else {
                continue;
            };
            if gcc && p.contains("/include/c++/") {
                continue;
            }
            let p =
                if p.ends_with("/lib/clang/19/include") { res_inc.clone() } else { p.to_owned() };
            dirs.push(SearchDirSpec::new(p, group));
        }
        if !dirs.iter().any(|d| Path::new(&d.path).join("vector").is_file()) {
            println!("skipped: no {lib:?} headers on this host");
            return None;
        }
        let base =
            fs::read_to_string(resource().join(format!("predefines/{triple}.c++26.txt"))).ok()?;
        let mut cfg = PredefConfig::new(true);
        cfg.gnu = false;
        cfg.aarch64 = triple.starts_with("aarch64");
        cfg.reflection = true;
        cfg.std_float_aliases = lib != Lib::Cxx;
        let predefines = build_predefines(&base, &cfg, &[]);
        Some(Config { triple, dirs, predefines, lib })
    }

    fn pp_options(&self, header: bool) -> PpOptions {
        let mut o = PpOptions::new(Std::Cxx26);
        o.predefines.clone_from(&self.predefines);
        o.reproducible_time = Some(("Jan  1 2000".to_owned(), "00:00:00".to_owned()));
        o.main_is_header = header;
        o.reflection = true;
        o.parser_pragmas = true;
        o.modules = Some(ModuleOptions::default());
        o
    }

    /// A preprocessor configured as the command line says, its main
    /// file entered.
    fn preprocessor<'a, 'd>(
        &self,
        sm: &'a SourceManager,
        engine: &'d DiagEngine<'a>,
        interner: &'a Interner,
        main: FileId,
        header: bool,
    ) -> Preprocessor<'a, 'd> {
        let mut pp = Preprocessor::new(sm, engine, interner, self.pp_options(header));
        let mut fs = RealFs;
        let list = SearchList::build(&self.dirs, true, &mut fs);
        let r: Box<dyn IncludeResolver> = Box::new(HeaderSearch::new(fs, list));
        pp.set_include_resolver(r);
        pp.set_feature_queries(Box::new(ClangFeatures::new(self.triple, Std::Cxx26, &[]).unwrap()));
        pp.enter_main_file(main);
        pp
    }

    fn parse_options(&self) -> ParseOptions {
        ParseOptions {
            reflection: true,
            gnu_lib_builtins: self.lib != Lib::Cxx,
            ..ParseOptions::default()
        }
    }
}

fn diag_options() -> DiagOptions {
    DiagOptions { warnings: vec!["all".to_owned()], ..DiagOptions::default() }
}

/// The diagnostics in `sink`, one line each (raw location, line:column,
/// level, message; notes indented), in emission order.
fn render(sm: &SourceManager, sink: &MemorySink) -> Vec<String> {
    fn one(sm: &SourceManager, d: &kiln_diag::Diagnostic, indent: &str, out: &mut Vec<String>) {
        let pos = sm
            .line_col(d.span.lo)
            .map_or_else(|| "-".to_owned(), |lc| format!("{}:{}", lc.line, lc.col));
        out.push(format!("{indent}{} {pos} {:?}: {}", d.span.lo.to_raw(), d.level, d.message));
        for n in &d.notes {
            one(sm, n, "  ", out);
        }
    }
    let mut out = Vec::new();
    for d in sink.take() {
        one(sm, &d, "", &mut out);
    }
    out
}

// ----- the snapshot stream ----------------------------------------------

/// Writes the parts this test restores: the source manager, the
/// interner (so the AST's `Arc` is a back-reference to it), then Sema
/// with its AST. WP-1383 adds the diagnostics engine and the
/// preprocessor (WP-1379) in this place.
fn write(
    sm: &SourceManager,
    interner: &Arc<Interner>,
    s: &Sema<'_, '_>,
    canonical: bool,
) -> Vec<u8> {
    let mut w = if canonical { Writer::canonical() } else { Writer::new() };
    w.put(sm);
    w.put(interner);
    s.snap_put(&mut w).expect("the prefix is at rest");
    w.into_bytes()
}

/// The source manager and the interner read back from `bytes`, and the
/// reader positioned at Sema.
fn read_front(bytes: &[u8]) -> Result<(SourceManager, Arc<Interner>, Reader<'_>), SnapError> {
    let mut r = Reader::new(bytes);
    let sm: SourceManager = r.get()?;
    let interner: Arc<Interner> = r.get()?;
    Ok((sm, interner, r))
}

/// Restores Sema from `r` into a fresh one configured as `cfg` says.
fn read_sema<'a, 'sm>(
    mut r: Reader<'_>,
    interner: &Arc<Interner>,
    engine: &'a DiagEngine<'sm>,
    cfg: &Config,
) -> Result<Sema<'a, 'sm>, SnapError> {
    let mut s = new_sema(
        Arc::clone(interner),
        engine,
        target(),
        &cfg.parse_options(),
        ConstexprLimits::default(),
    );
    s.snap_restore(&mut r)?;
    r.finish()?;
    Ok(s)
}

/// The prefix at `name` (`text`) parsed to its end: the source manager,
/// interner, diagnostics and Sema are handed to `then`.
fn with_prefix<R>(
    cfg: &Config,
    name: &str,
    text: &str,
    then: impl for<'a, 'sm> FnOnce(
        &'sm SourceManager,
        &'sm Arc<Interner>,
        &MemorySink,
        Result<Sema<'a, 'sm>, String>,
    ) -> R,
) -> R {
    let sm = SourceManager::new();
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &diag_options(), sink.clone());
    let file = sm.add_virtual(name, text.to_owned()).expect("location space");
    let pp = cfg.preprocessor(&sm, &engine, &interner, file, true);
    let t = Instant::now();
    let p = parse_prefix(
        pp,
        Arc::clone(&interner),
        &engine,
        target(),
        &cfg.parse_options(),
        ConstexprLimits::default(),
    );
    println!("{name}: prefix parsed in {:?}", t.elapsed());
    let ready = p.ready.map(|()| p.sema);
    then(&sm, &interner, &sink, ready)
}

fn bench_prefix(name: &str) -> String {
    fs::read_to_string(manifest().join("../../bench/pch").join(name)).expect("bench/pch header set")
}

// ----- encode, decode, encode -------------------------------------------

/// Writes the prefix's state canonically, reads it back, writes it again:
/// the bytes are equal. Prints the sizes for the handback table.
fn round_trip(cfg: &Config, name: &str, text: &str) {
    with_prefix(cfg, name, text, |sm, interner, sink, s| {
        let s = s.unwrap_or_else(|why| panic!("{name}: not at rest: {why}"));
        let diags = render(sm, sink);
        assert!(
            !diags.iter().any(|d| d.contains(" Error: ") || d.contains(" Fatal: ")),
            "{name}: the prefix has errors:\n{}",
            diags.join("\n")
        );
        let t = Instant::now();
        let first = write(sm, interner, &s, true);
        let enc = t.elapsed();
        let t = Instant::now();
        let (sm2, interner2, r) = read_front(&first).expect("decode");
        let sink2 = MemorySink::new();
        let engine2 = DiagEngine::new(&sm2, &diag_options(), sink2);
        let s2 = read_sema(r, &interner2, &engine2, cfg).expect("decode Sema");
        let dec = t.elapsed();
        let second = write(&sm2, &interner2, &s2, true);
        assert!(first == second, "{name}: the re-encoded snapshot differs");
        let sizes = s.snap_sizes();
        let total: usize = sizes.iter().map(|(_, n)| n).sum();
        let ast = sizes[0].1;
        let mut rest: Vec<_> = sizes[2..].to_vec();
        rest.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        let mut line = format!(
            "{name}: snapshot {} bytes (sm+interner+Sema), Sema {total} (ast {ast}, Sema alone {}); encode {enc:?}, decode {dec:?}; largest:",
            first.len(),
            total - ast
        );
        for (f, n) in rest.iter().take(10) {
            let _ = write!(line, " {f}={n}");
        }
        println!("{line}");
    });
}

#[test]
fn round_trip_mid() {
    if let Some(cfg) = Config::host(Lib::Gnu) {
        round_trip(&cfg, "mid.h", &bench_prefix("mid.h"));
    }
}

#[test]
fn round_trip_gnu() {
    if let Some(cfg) = Config::host(Lib::Gnu) {
        round_trip(&cfg, "gnu.h", &bench_prefix("gnu.h"));
    }
}

#[test]
fn round_trip_lcx() {
    if let Some(cfg) = Config::host(Lib::Cxx) {
        round_trip(&cfg, "lcx.h", &bench_prefix("lcx.h"));
    }
}

const META_PREFIX: &str = "#include <meta>
#include <vector>
#include <string>
#include <map>
#include <optional>
#include <variant>
#include <tuple>
#include <functional>
#include <algorithm>
#include <memory>
";

#[test]
fn round_trip_meta() {
    if let Some(cfg) = Config::host(Lib::Gcc16) {
        round_trip(&cfg, "meta.h", META_PREFIX);
    }
}

// ----- continue ---------------------------------------------------------

/// What a continuation produced: the diagnostics and the dump of the main
/// file's declarations (ids and locations included).
#[derive(PartialEq, Eq, Debug)]
struct Outcome {
    diags: Vec<String>,
    dump: String,
}

fn continue_main<'sm>(
    cfg: &Config,
    sm: &'sm SourceManager,
    interner: &'sm Arc<Interner>,
    s: Sema<'_, 'sm>,
    sink: &MemorySink,
    main: &str,
) -> Outcome {
    let engine = s.diags;
    let file = sm.add_virtual("main.cpp", main.to_owned()).expect("location space");
    let pp = cfg.preprocessor(sm, engine, interner, file, false);
    let (tu, _pp) = continue_preprocessed(s, pp, &target(), &cfg.parse_options());
    let opts = DumpOptions {
        sources: Some(sm),
        main_file: Some(file),
        ids: true,
        locs: true,
        implicit: true,
        ..DumpOptions::default()
    };
    Outcome { diags: render(sm, sink), dump: dump_to_string(&tu.ast, &opts) }
}

/// Continues the prefix twice, once with the original Sema and once
/// with the one restored from its snapshot, and compares.
fn continue_both(cfg: &Config, name: &str, prefix: &str, main: &str) -> Outcome {
    let (bytes, original) = with_prefix(cfg, name, prefix, |sm, interner, sink, s| {
        let s = s.unwrap_or_else(|why| panic!("{name}: not at rest: {why}"));
        let prefix_diags = render(sm, sink);
        assert!(
            !prefix_diags.iter().any(|d| d.contains(" Error: ")),
            "{name}: the prefix has errors:\n{}",
            prefix_diags.join("\n")
        );
        let bytes = write(sm, interner, &s, false);
        (bytes, continue_main(cfg, sm, interner, s, sink, main))
    });
    let (sm, interner, reader) = read_front(&bytes).expect("decode");
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &diag_options(), sink.clone());
    let sema = read_sema(reader, &interner, &engine, cfg).expect("decode Sema");
    let restored = continue_main(cfg, &sm, &interner, sema, &sink, main);
    if original != restored {
        let d = |a: &[String], b: &[String]| {
            a.iter().zip(b).position(|(x, y)| x != y).map_or(a.len().min(b.len()), |i| i)
        };
        let i = d(&original.diags, &restored.diags);
        let ol: Vec<_> = original.dump.lines().map(str::to_owned).collect();
        let rl: Vec<_> = restored.dump.lines().map(str::to_owned).collect();
        let j = d(&ol, &rl);
        panic!(
            "{name}: the restored state continues differently\n diags at {i}: {:?} vs {:?}\n dump line {j}: {:?} vs {:?}",
            original.diags.get(i),
            restored.diags.get(i),
            ol.get(j),
            rl.get(j)
        );
    }
    original
}

/// The prefix of the continue test: the `mid` header set and a namespace
/// of the entities the main file uses.
const CONTINUE_PREFIX: &str = r#"#include <algorithm>
#include <map>
#include <string>
#include <vector>
namespace pfx {
struct Widget {
  int a = 1;
  std::string name;
};
template <class T> struct Box {
  T v;
  constexpr T get() const { return v; }
};
constexpr int square(int x) { return x * x; }
template <class T> constexpr T twice(T t) { return t + t; }
struct Tag {};
inline int touch(Tag) { return 7; }
int declared_only(int);
[[deprecated("use square")]] inline int old_square(int x) { return x * x; }
inline int used_later() { return 3; }
}
#pragma pack(push, 2)
"#;

/// The main file: it compiles only with the prefix's declarations
/// ([basic.def.odr]: `declared_only` is used but never defined; that is
/// for the link to find).
const CONTINUE_MAIN: &str = r##"struct Packed {
  char c;
  int i;
};
static_assert(sizeof(Packed) == 6, "#pragma pack(2) from the prefix");
#pragma pack(pop)
static int unused_helper() { return 1; }
constexpr int k = pfx::square(9);
static_assert(k == 81);
static_assert(pfx::Box<long>{5}.get() == 5);
static_assert(pfx::twice(21) == 42);
int use_all(int n) {
  pfx::Widget w;
  pfx::Widget w2 = w;
  w2 = static_cast<pfx::Widget &&>(w);
  std::vector<pfx::Widget> ws;
  ws.push_back(w2);
  std::vector<int> v(n, 3);
  std::sort(v.begin(), v.end());
  std::map<std::string, int> m;
  m["a"] = touch(pfx::Tag{});
  int d = pfx::old_square(3);
  return pfx::declared_only(d) + pfx::used_later() + static_cast<int>(ws.size()) + m["a"];
}
"##;

#[test]
fn continue_mid() {
    let Some(cfg) = Config::host(Lib::Gnu) else { return };
    let out = continue_both(&cfg, "prefix.h", CONTINUE_PREFIX, CONTINUE_MAIN);
    let all = out.diags.join("\n");
    assert!(all.contains("'old_square' is deprecated"), "{all}");
    // `unused_helper` is an unused static function: kiln has no
    // -Wunused-function yet, so it shows only in the dump.
    assert!(out.dump.contains("unused_helper"));
    assert!(!all.contains(" Error: "), "{all}");
}

const META_CONTINUE_PREFIX: &str = r"#include <meta>
#include <vector>
#include <string>
#include <map>
#include <optional>
#include <variant>
#include <tuple>
#include <functional>
#include <algorithm>
#include <memory>
namespace pfx {
struct Point {
  int x;
  long y;
  void move(int);
};
enum class Color { red, green, blue };
}
";

const META_CONTINUE_MAIN: &str = r#"consteval std::size_t data_members() {
  return std::meta::nonstatic_data_members_of(^^pfx::Point,
                                              std::meta::access_context::unchecked())
      .size();
}
consteval bool third_is_function() {
  auto ms = std::meta::members_of(^^pfx::Point, std::meta::access_context::unchecked());
  return std::meta::is_function(ms[2]);
}
consteval std::string_view second_enumerator() {
  return std::meta::identifier_of(std::meta::enumerators_of(^^pfx::Color)[1]);
}
static_assert(std::meta::identifier_of(^^pfx::Point) == "Point");
static_assert(data_members() == 2);
static_assert(third_is_function());
static_assert(second_enumerator() == "green");
std::optional<std::variant<int, std::string>> use_meta() { return std::string("x"); }
"#;

#[test]
fn continue_meta() {
    let Some(cfg) = Config::host(Lib::Gcc16) else { return };
    let out = continue_both(&cfg, "meta-prefix.h", META_CONTINUE_PREFIX, META_CONTINUE_MAIN);
    let all = out.diags.join("\n");
    assert!(!all.contains(" Error: "), "{all}");
}

// ----- quiescence -------------------------------------------------------

/// A prefix without library headers, parsed to its end; its readiness.
fn ready_of(prefix: &str) -> Result<(), String> {
    let sm = SourceManager::new();
    let interner = Arc::new(Interner::new());
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let file = sm.add_virtual("q.h", prefix.to_owned()).expect("location space");
    let mut pp = Preprocessor::new(&sm, &engine, &interner, {
        let mut o = PpOptions::new(Std::Cxx26);
        o.parser_pragmas = true;
        o.main_is_header = true;
        o
    });
    pp.enter_main_file(file);
    let p = parse_prefix(
        pp,
        Arc::clone(&interner),
        &engine,
        target(),
        &ParseOptions::default(),
        ConstexprLimits::default(),
    );
    p.ready
}

/// [basic.namespace], [dcl.link], [class]: a prefix that ends inside a
/// namespace, a linkage specification or a class is refused, naming the
/// open context. A complete prefix is accepted.
#[test]
fn quiescence_refuses_open_contexts() {
    assert_eq!(ready_of("namespace n { int a; }\nint b;\n"), Ok(()));
    let ns = ready_of("namespace outer { namespace inner { int a;\n").unwrap_err();
    assert!(ns.contains("ends inside 2 '{'"), "{ns}");
    let c = ready_of("extern \"C\" {\nint f(int);\n").unwrap_err();
    assert!(c.contains("ends inside 1 '{'"), "{c}");
    let class = ready_of("struct S { int a;\n").unwrap_err();
    assert!(class.contains("ends inside 1 '{'"), "{class}");
    let t = ready_of("template <class T> struct S { T a;\n").unwrap_err();
    assert!(t.starts_with("Sema.") || t.starts_with("Parser."), "{t}");
}

/// [cpp.pragma] (`#pragma pack`): an unterminated `#pragma pack(push)` at
/// the end of a prefix is state, not an open construct: the snapshot is
/// taken and the main file packs with it (continue test above).
#[test]
fn quiescence_accepts_open_pragma_pack() {
    assert_eq!(ready_of("#pragma pack(push, 1)\nstruct A { char c; int i; };\n"), Ok(()));
}

/// Options a snapshot does not carry are refused.
#[test]
fn quiescence_refuses_recording_options() {
    let sm = SourceManager::new();
    let interner = Arc::new(Interner::new());
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let file = sm.add_virtual("q.h", "int a;\n".to_owned()).expect("location space");
    let mut pp = Preprocessor::new(&sm, &engine, &interner, PpOptions::new(Std::Cxx26));
    pp.enter_main_file(file);
    let opts = ParseOptions { record_carried: true, ..ParseOptions::default() };
    let p = parse_prefix(
        pp,
        Arc::clone(&interner),
        &engine,
        target(),
        &opts,
        ConstexprLimits::default(),
    );
    assert!(p.ready.unwrap_err().contains("carried"));
}

// ----- corruption -------------------------------------------------------

/// A truncated or bit-flipped snapshot of the `mid` prefix is an error or
/// (for a flip inside a value) a different state, never a panic.
#[test]
fn corruption_never_panics() {
    let Some(cfg) = Config::host(Lib::Gnu) else { return };
    let bytes = with_prefix(&cfg, "mid.h", &bench_prefix("mid.h"), |sm, interner, _, s| {
        write(sm, interner, &s.expect("at rest"), false)
    });
    let decode = |b: &[u8]| -> bool {
        let Ok((sm, interner, r)) = read_front(b) else { return false };
        let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
        read_sema(r, &interner, &engine, &cfg).is_ok()
    };
    assert!(decode(&bytes));
    let n = bytes.len();
    // A fixed pseudo-random sequence (xorshift), so a failure reproduces.
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    for k in 0..48 {
        let at = if k < 8 { k * n / 8 } else { (next() % n as u64) as usize };
        assert!(!decode(&bytes[..at]), "a snapshot truncated at {at} of {n} decoded");
    }
    let mut flipped = bytes.clone();
    for _ in 0..48 {
        let at = (next() % n as u64) as usize;
        let bit = 1u8 << (next() % 8);
        flipped[at] ^= bit;
        let _ = decode(&flipped);
        flipped[at] ^= bit;
    }
}
