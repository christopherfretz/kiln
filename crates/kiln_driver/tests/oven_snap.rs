//! The Oven as a snapshot root (spec/oven.md "Snapshots", spec/snap.md
//! "Hand-written roots"; WP-1381): continue-after-restore equivalence.
//!
//! Each prefix is preprocessed by the driver (`-E`, so the real headers
//! and predefined macros are used) and analysed by `kiln_parse`, which
//! evaluates the prefix's constant expressions in the TU's Oven. The Oven
//! is then snapshot and restored into a fresh Oven attached to a copy of
//! the same AST (decoded from the AST's own snapshot), and the same new
//! constant expressions (the `probe_N` functions' return values) are
//! evaluated on both. Results, diagnostics, failure notes, per-evaluation
//! counters and the code the evaluations compile must be identical, and
//! the two Ovens' canonical encodings equal right after the restore and
//! again at the end.
//!
//! The evaluations here go through a post-sema bridge (as the codegen
//! fold prepass's): reflection queries that need sema's `meta` bridge
//! fail, identically, on both Ovens.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use kiln_abi::Target;
use kiln_ast::consts::ConstStore;
use kiln_ast::types::RecordLayout;
use kiln_ast::{Ast, DeclId, ExprId, QualType, StmtId};
use kiln_diag::{Diag, DiagEngine, DiagOptions, Level, MemorySink, SharedBuffer};
use kiln_driver::{Io, SharedWriter, run};
use kiln_oven::{EvalContext, EvalKind, Oven, SemaBridge, Subobject, SubobjectId};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_sema::target::Arch;
use kiln_snap::{Reader, Writer};
use kiln_source::symbol::Interner;
use kiln_source::{Loc, SourceManager};

/// Preprocesses `src` as C++26 with `extra` driver flags; `None` with the
/// driver's diagnostics when that fails.
fn preprocess(src: &str, extra: &[String]) -> Result<String, String> {
    let out = SharedBuffer::new();
    let err = SharedBuffer::new();
    let io = Io {
        stdout: Box::new(SharedWriter::new(out.clone())),
        stderr: SharedWriter::new(err.clone()),
        stdin: Box::new(std::io::Cursor::new(src.as_bytes().to_vec())),
    };
    let mut args: Vec<String> = vec!["-std=c++26".into(), "-E".into()];
    args.extend(extra.iter().cloned());
    args.push("-".into());
    if run(&args, io) == 0 { Ok(out.contents()) } else { Err(err.contents()) }
}

/// The host's target (the preprocessor's predefined macros are its).
fn host_target() -> Target {
    Target::for_arch(if cfg!(target_arch = "x86_64") { Arch::X86_64 } else { Arch::AArch64 })
}

/// The bridge of an evaluation after sema (the codegen fold prepass's):
/// bodies and layouts from the AST, diagnostics recorded. Every body
/// request also asks the Oven for a snapshot (`probe`), which must be
/// refused: an evaluation is running.
struct Bridge<'x> {
    ast: &'x mut Ast,
    abi: &'x Target,
    cxx_year: u32,
    diags: Vec<String>,
    probe: Vec<Result<(), String>>,
}

impl SemaBridge for Bridge<'_> {
    fn ast(&self) -> &Ast {
        self.ast
    }

    fn cxx_year(&self) -> u32 {
        self.cxx_year
    }

    fn ensure_body(&mut self, f: DeclId, oven: &mut Oven) -> Option<StmtId> {
        self.probe.push(oven.snap_put(&mut Writer::new()));
        let d = self.ast.definition(f)?;
        self.ast.decls.function(d)?.body
    }

    fn ensure_layout(&mut self, record: DeclId) -> Option<&RecordLayout> {
        kiln_abi::ensure_layout(&mut *self.ast, record, self.abi)
    }

    fn opaque_size_align(&mut self, ty: QualType) -> (u64, u32) {
        let l = kiln_abi::type_size_align(self.ast, ty, self.abi);
        (l.size, u32::try_from(l.align).unwrap_or(1))
    }

    fn decl_align(&mut self, d: DeclId) -> u64 {
        kiln_abi::layout::decl_align(self.ast, d, self.abi, false)
    }

    fn alignof_decl(&mut self, d: DeclId) -> u64 {
        kiln_abi::layout::decl_align(self.ast, d, self.abi, true)
    }

    fn final_overrider(
        &mut self,
        most_derived: DeclId,
        at: SubobjectId,
        method: DeclId,
    ) -> Option<(DeclId, SubobjectId)> {
        let first = self.ast.first(most_derived);
        self.ast.virtuals.get(first)?.final_overrider(at, method)
    }

    fn subobjects(&mut self, record: DeclId) -> &[Subobject] {
        let first = self.ast.first(record);
        self.ast.virtuals.get(first).map_or(&[], |v| &v.subobjects)
    }

    fn consts(&mut self) -> &mut ConstStore {
        &mut self.ast.consts
    }

    fn diag(&mut self, d: Diag) {
        self.diags.push(format!("{d:?}"));
    }

    fn set_temporary_value(&mut self, t: DeclId, v: kiln_ast::ConstValueId) {
        let _ = self.ast.decls.set_lifetime_extended_value(t, v);
    }

    fn lvalue_ref_to(&mut self, t: QualType) -> Option<QualType> {
        Some(self.ast.types.lvalue_ref_to(t))
    }
}

/// The return value expressions of the functions `probe_0`, `probe_1`,
/// ... (each `{ return <expr>; }`), in order.
fn probes(ast: &Ast) -> Vec<ExprId> {
    let mut out = Vec::new();
    for i in 0.. {
        let Some(sym) = ast.symbols().get(&format!("probe_{i}")) else { break };
        let e = ast.decls.ids().find_map(|d| {
            if ast.decls.get(d).name().as_ident() != Some(sym) {
                return None;
            }
            let body = ast.decls.function(d)?.body?;
            let first = ast.lists.iter::<StmtId>(ast.stmts.compound_body(body)).next()?;
            ast.stmts.return_value(first)
        });
        out.push(e.unwrap_or_else(|| panic!("probe_{i} has no `return <expr>;` body")));
    }
    out
}

/// The Oven's row `name` of its memory report: (count, live bytes).
fn row(oven: &Oven, name: &str) -> u64 {
    oven.memory().rows.iter().find(|r| r.name == name).map_or(0, |r| r.count)
}

/// One probe's observable outcome on one Oven.
fn observe(
    ast: &mut Ast,
    oven: &mut Oven,
    abi: &Target,
    e: ExprId,
    refusals: &mut usize,
) -> (String, Vec<String>, String, (u64, u64)) {
    let code0 = row(oven, "oven.code");
    let funcs0 = row(oven, "oven.funcs");
    let cxx_year = oven.cxx_year();
    let mut b = Bridge { ast, abi, cxx_year, diags: Vec::new(), probe: Vec::new() };
    let r = oven.evaluate(&mut b, e, EvalContext::new(EvalKind::Init, Loc::INVALID));
    for s in &b.probe {
        let why = s.as_ref().expect_err("a snapshot during an evaluation is refused");
        assert!(why.starts_with("Oven.ctx_depth"), "refused for the wrong reason: {why}");
        *refusals += 1;
    }
    let result = match &r {
        Ok(v) => format!("ok {v:?}"),
        Err(f) => format!("err {f:?}"),
    };
    let rec = oven.stats().records().last().expect("the evaluation is recorded");
    let counters = format!(
        "kind {:?} steps {} bytes {} peak {} calls {} allocs {} arena {} frozen {} static {} \
         memo {} refl {} reclaimed {} ok {}",
        rec.kind,
        rec.steps,
        rec.bytes,
        rec.peak_bytes,
        rec.calls,
        rec.allocs,
        rec.peak_arena,
        rec.frozen_bytes,
        rec.static_bytes,
        rec.memo_hits,
        rec.refl_bytes,
        rec.reclaimed_bytes,
        rec.ok,
    );
    let grown = (row(oven, "oven.code") - code0, row(oven, "oven.funcs") - funcs0);
    (result, b.diags, counters, grown)
}

/// A snapshot of `oven`, canonical or not.
fn snap(oven: &Oven, canonical: bool) -> Vec<u8> {
    let mut w = if canonical { Writer::canonical() } else { Writer::new() };
    oven.snap_put(&mut w).expect("the Oven is at rest between evaluations");
    w.into_bytes()
}

/// An Oven configured as `like` (the consumer's options), not yet used.
fn fresh_like(like: &Oven) -> Oven {
    let mut o = Oven::new(like.budget());
    o.set_cxx_year(like.cxx_year());
    o
}

/// Restores `bytes` into an Oven configured as `like`.
fn restore(like: &Oven, bytes: &[u8]) -> Oven {
    let mut o = fresh_like(like);
    let mut r = Reader::new(bytes);
    o.snap_restore(&mut r).expect("the snapshot decodes");
    r.finish().expect("the snapshot is read to its end");
    o
}

/// Runs the equivalence check on `prefix` followed by `probes` (C++
/// source defining `probe_0`, ...), with `extra` driver flags; returns
/// the restored AST and Oven for further checks. With `cold`, `probe_0`
/// must call functions the prefix compiled.
fn equivalence(
    name: &str,
    prefix: &str,
    probe_src: &str,
    extra: &[String],
    cold: bool,
) -> (Ast, Oven) {
    let t0 = Instant::now();
    let src = format!("{prefix}\n// probes\n{probe_src}\n");
    let ii = preprocess(&src, extra).unwrap_or_else(|e| panic!("{name}: -E failed:\n{e}"));
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.ii", ii).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let opts = ParseOptions {
        std: kiln_lex::Std::Cxx26,
        exceptions: true,
        rtti: true,
        reflection: true,
        clang_compat: 22,
        gnu_lib_builtins: true,
        sized_deallocation: true,
        ..ParseOptions::default()
    };
    let abi = host_target();
    let tu =
        parse_tu(&sm, &interner, file, &engine, abi.clone(), &opts, ConstexprLimits::default());
    let errors: Vec<_> = sink.take().into_iter().filter(|d| d.level >= Level::Error).collect();
    assert!(errors.is_empty(), "{name}: the prefix has errors: {errors:#?}");
    let (mut ast_a, mut oven_a) = (tu.ast, tu.oven);
    let parsed = t0.elapsed();

    // The AST copy the restored Oven is attached to: the same AST.
    let mut ast_b: Ast = kiln_snap::from_bytes(&kiln_snap::to_bytes(&ast_a)).expect("AST copy");

    let t1 = Instant::now();
    let bytes = snap(&oven_a, false);
    let encode = t1.elapsed();
    let t2 = Instant::now();
    let mut oven_b = restore(&oven_a, &bytes);
    let decode = t2.elapsed();
    assert_eq!(snap(&oven_a, true), snap(&oven_b, true), "{name}: canonical bytes after restore");

    let sizes = oven_a.snap_sizes().expect("at rest");
    let mut top: Vec<_> = sizes.iter().filter(|(_, n)| *n > 64).collect();
    top.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    eprintln!(
        "{name}: parse {parsed:?}; oven snapshot {} bytes, encode {encode:?}, decode {decode:?}",
        bytes.len()
    );
    eprintln!("{name}: by table {top:?}");

    let exprs = probes(&ast_a);
    assert!(!exprs.is_empty(), "{name}: no probes");
    if cold {
        // The counter that shows recompilation: an Oven without the
        // snapshot compiles probe_0's callees; one restored from it does
        // not (and grows exactly as the original, checked below).
        let copy = |ast: &Ast| -> Ast {
            kiln_snap::from_bytes(&kiln_snap::to_bytes(ast)).expect("AST copy")
        };
        let (mut ast_c, mut ast_d) = (copy(&ast_a), copy(&ast_a));
        let mut oven_c = fresh_like(&oven_a);
        let mut oven_d = restore(&oven_a, &bytes);
        let c = observe(&mut ast_c, &mut oven_c, &abi, exprs[0], &mut 0);
        let d = observe(&mut ast_d, &mut oven_d, &abi, exprs[0], &mut 0);
        assert!(c.0.starts_with("ok") && d.0.starts_with("ok"), "{name}: {c:?} {d:?}");
        assert!(c.3.0 > d.3.0 && c.3.1 > d.3.1, "{name}: cold {:?}, restored {:?}", c.3, d.3);
        eprintln!("{name}: probe_0 compiles (code, funcs) +{:?} cold, +{:?} restored", c.3, d.3);
    }
    let mut refusals = 0;
    for (i, &e) in exprs.iter().enumerate() {
        let a = observe(&mut ast_a, &mut oven_a, &abi, e, &mut 0);
        let b = observe(&mut ast_b, &mut oven_b, &abi, e, &mut refusals);
        assert_eq!(a, b, "{name}: probe_{i} differs after the restore");
        eprintln!("{name}: probe_{i}: {} (code +{}, funcs +{})", a.0, a.3.0, a.3.1);
    }
    assert!(refusals > 0, "{name}: no snapshot was attempted during an evaluation");
    eprintln!("{name}: {refusals} snapshots during evaluations refused");
    assert_eq!(snap(&oven_a, true), snap(&oven_b, true), "{name}: canonical bytes at the end");
    assert_eq!(
        kiln_snap::to_bytes_canonical(&ast_a),
        kiln_snap::to_bytes_canonical(&ast_b),
        "{name}: the ASTs diverged"
    );
    (ast_b, oven_b)
}

/// A hand-written prefix: constexpr and consteval functions, a memoized
/// recursive function, virtual calls, lambdas, static locals, frozen
/// (constexpr) objects and pointers into them, transient heap use.
const HAND: &str = r#"
constexpr long fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
static_assert(fib(22) == 17711);

struct Base {
    constexpr virtual int f() const { return 1; }
    constexpr virtual ~Base() = default;
};
struct Der : Base {
    int k;
    constexpr explicit Der(int k) : k(k) {}
    constexpr int f() const override { return k * 2; }
};
constexpr int virt(int k) { Der d{k}; const Base& b = d; return b.f(); }
static_assert(virt(4) == 8);

consteval int triple(int x) { return x * 3; }
static_assert(triple(5) == 15);
constexpr int via_cev = triple(7);

constexpr int with_static() { static constexpr int s[] = {7, 8, 9}; return s[1]; }
static_assert(with_static() == 8);

constexpr auto add1 = [](int x) { return x + 1; };
static_assert(add1(1) == 2);

struct Arr { int v[4]; };
constexpr Arr frozen = {{1, 2, 3, 4}};
constexpr const int* fp = &frozen.v[2];
constexpr const char* str = "frozen string";

constexpr int heap_sum(int n) {
    int* p = new int[n];
    for (int i = 0; i < n; ++i) p[i] = i;
    int s = 0;
    for (int i = 0; i < n; ++i) s += p[i];
    delete[] p;
    return s;
}
static_assert(heap_sum(10) == 45);

template <int N> struct Fact { static constexpr long v = N * Fact<N - 1>::v; };
template <> struct Fact<0> { static constexpr long v = 1; };
static_assert(Fact<10>::v == 3628800);
"#;

const HAND_PROBES: &str = r"
constexpr int fresh(int x) { return x * x + via_cev; }
long probe_0() { return fib(23); }
long probe_1() { return fib(21) + fib(15); }
int probe_2() { return virt(21); }
int probe_3() { return add1(41); }
int probe_4() { return *fp + frozen.v[0] + str[1]; }
int probe_5() { return heap_sum(100); }
int probe_6() { return with_static(); }
int probe_7() { return fresh(9); }
int probe_8() { return Der{3}.f(); }
long probe_9() { return Fact<10>::v + Fact<12>::v; }
int probe_10() { return 1 / (fib(1) - 1); }
int probe_11() { return [] { int i = 0; while (true) ++i; return i; }(); }
int probe_12() { return [] { int* p = new int(1); return *p; }(); }
int probe_13() { return [] { int a[2] = {}; return a[2]; }(); }
int probe_14() { return via_cev + fresh(2); }
";

#[test]
fn hand_written_prefix() {
    let (ast, oven) = equivalence("hand", HAND, HAND_PROBES, &[], true);
    corruption("hand", &ast, &oven);
}

/// The GCC 16 install with `<meta>`, if the workspace has it.
fn gcc16() -> Option<PathBuf> {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    here.ancestors()
        .map(|a| a.join(".toolchain/gcc-16.1.0-image/usr/local"))
        .find(|p| p.join("include/c++/16.1.0/meta").is_file())
}

const META: &str = r#"
#include <meta>
#include <array>
struct S;
consteval {
    std::meta::define_aggregate(^^S, {
        std::meta::data_member_spec(^^int, {.name = "a"}),
        std::meta::data_member_spec(^^long, {.name = "b"}),
    });
}
constexpr std::meta::info rS = ^^S;
constexpr std::meta::info rInt = ^^int;
constexpr auto mems =
    std::define_static_array(std::meta::nonstatic_data_members_of(^^S, std::meta::access_context::unchecked()));
static_assert(mems.size() == 2);
static_assert(std::meta::identifier_of(mems[1]) == "b");
constexpr S s{1, 2};
constexpr std::array<std::meta::info, 2> types{^^int, ^^long};
static_assert(types[1] == std::meta::type_of(mems[1]));
"#;

const META_PROBES: &str = r"
consteval long probe_0() { return s.a + s.b; }
consteval unsigned long probe_1() { return sizeof(S); }
consteval unsigned long probe_2() { return mems.size(); }
consteval bool probe_3() { return mems[0] == mems[0] && rS != rInt; }
consteval bool probe_4() { return types[0] == rInt; }
consteval unsigned long probe_5() { return std::meta::identifier_of(mems[0]).size(); }
consteval bool probe_6() { return std::meta::is_type(rS); }
";

#[test]
fn reflection_prefix() {
    let Some(prefix) = gcc16() else {
        eprintln!("unsupported: the GCC 16 toolchain image (<meta>) is not in the workspace");
        return;
    };
    let flags = [format!("--gcc-toolchain={}", prefix.display())];
    let (ast, oven) = equivalence("meta", META, META_PROBES, &flags, false);
    corruption("meta", &ast, &oven);
}

const STDCXX: &str = r#"
#include <bits/stdc++.h>
constexpr std::array<int, 6> unsorted{5, 3, 6, 1, 4, 2};
constexpr auto sorted = [] { auto b = unsorted; std::sort(b.begin(), b.end()); return b; }();
static_assert(sorted[0] == 1 && sorted[5] == 6);
constexpr std::size_t vlen() { std::vector<int> v{1, 2, 3}; v.push_back(4); return v.size(); }
static_assert(vlen() == 4);
constexpr std::size_t slen() { std::string s = "abc"; s += "defgh"; return s.size(); }
static_assert(slen() == 8);
constexpr std::optional<int> opt = 3;
constexpr std::string_view sv = "hello, world";
"#;

const STDCXX_PROBES: &str = r"
std::size_t probe_0() { return vlen() + slen(); }
int probe_1() { return std::ranges::max(sorted); }
std::size_t probe_2() { return sv.find('w'); }
int probe_3() { return *opt + sorted[2]; }
std::size_t probe_4() { return [] { std::map<int, int> m; for (int i = 0; i < 50; ++i) m[i] = i * i; return m.size(); }(); }
int probe_5() { return [] { std::vector<int> v; return v.at(3); }(); }
std::size_t probe_6() { return [] { std::string s(100, 'x'); return std::ranges::count(s, 'x'); }(); }
";

#[test]
fn stdcxx_prefix() {
    let (ast, oven) = equivalence("stdc++", STDCXX, STDCXX_PROBES, &[], true);
    corruption("stdc++", &ast, &oven);
}

/// Truncations and bit flips of `oven`'s snapshot never panic at decode;
/// an Oven a decode refused is still fresh (a correct snapshot restores
/// into it afterwards). The restore of a used Oven is refused.
fn corruption(name: &str, _ast: &Ast, oven: &Oven) {
    let bytes = snap(oven, false);
    let n = bytes.len();
    let mut cuts: Vec<usize> = (0..n.min(256)).collect();
    cuts.extend((0..256).map(|i| i * n / 256));
    let (mut refused, mut accepted) = (0, 0);
    for &cut in &cuts {
        let mut o = fresh_like(oven);
        match o.snap_restore(&mut Reader::new(&bytes[..cut])) {
            Ok(()) => accepted += 1,
            Err(_) => refused += 1,
        }
    }
    // A pseudo-random walk of single-bit flips over the whole encoding.
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    for _ in 0..512 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let at = (x % (n as u64 * 8)) as usize;
        let mut bad = bytes.clone();
        bad[at / 8] ^= 1 << (at % 8);
        let mut o = fresh_like(oven);
        let mut r = Reader::new(&bad);
        if o.snap_restore(&mut r).is_err() {
            refused += 1;
            // Refused: the Oven is unchanged, still fresh.
            o.snap_restore(&mut Reader::new(&bytes)).expect("a refused decode leaves it fresh");
        } else if r.finish().is_err() {
            refused += 1;
        } else {
            accepted += 1;
        }
    }
    eprintln!("{name}: corrupted snapshots: {refused} refused, {accepted} decoded");
    // A used Oven refuses a restore.
    let mut used = restore(oven, &bytes);
    let err = used.snap_restore(&mut Reader::new(&bytes)).expect_err("a used Oven");
    eprintln!("{name}: restore into a used Oven: {err}");
}
