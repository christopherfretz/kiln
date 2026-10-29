//! Expression and statement lowering against the clang oracle
//! (spec/codegen.md §6 "Unit corpus").
//!
//! Each case is a small C++ program. It is parsed with `parse_tu` for
//! `aarch64-unknown-linux-gnu`, lowered, printed and run through
//! `opt-19 -S -passes=verify`. `Exact` cases are also compiled by
//! `clang++-19 -S -emit-llvm -O0 -std=c++26` and, after both texts are
//! normalized (see [`normalize`]), must be identical. `VerifyOnly` cases
//! have a documented deviation (spec/codegen.md "Deviations"). Without
//! `opt-19` / `clang++-19` the corresponding checks are skipped.
//!
//! `KILN_CASE=<name>` runs one case and prints both IR texts.

use std::fmt::Write as _;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Arc;

use kiln_abi::{Arch, Target};
use kiln_codegen::{CodegenOptions, emit};
use kiln_diag::{DiagEngine, DiagOptions, Level, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    /// Must match clang exactly after normalization.
    Exact,
    /// Must verify; the clang diff is a documented deviation.
    VerifyOnly,
}
use Mode::{Exact, VerifyOnly};

const TRIPLE: &str = "aarch64-unknown-linux-gnu";
const IDENT: &str = "kiln";

/// Lowers `src` with kiln.
fn kiln_ir(src: &str) -> Result<String, String> {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let target = Target::for_arch(Arch::AArch64);
    let mut tu = parse_tu(
        &sm,
        &interner,
        file,
        &engine,
        target.clone(),
        &ParseOptions::default(),
        ConstexprLimits::default(),
    );
    let errors: Vec<String> =
        sink.take().into_iter().filter(|d| d.level >= Level::Error).map(|d| d.message).collect();
    if !errors.is_empty() {
        return Err(format!("parse errors: {errors:?}"));
    }
    let opts = CodegenOptions {
        source_filename: "t.cpp".to_owned(),
        exceptions: true,
        ident: IDENT.to_owned(),
    };
    let m = emit(&mut tu.ast, &mut tu.oven, &target, &opts).map_err(|e| format!("codegen: {e}"))?;
    Ok(m.to_string())
}

/// Runs `prog args` with `input` on stdin; `None` when `prog` is missing.
fn pipe(prog: &str, args: &[&str], input: &str) -> Option<Result<String, String>> {
    let Ok(mut child) = Command::new(prog)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    else {
        return None;
    };
    {
        let mut stdin = child.stdin.take().expect("stdin");
        let _ = stdin.write_all(input.as_bytes());
    }
    let out = child.wait_with_output().expect("wait");
    if out.status.success() {
        Some(Ok(String::from_utf8_lossy(&out.stdout).into_owned()))
    } else {
        Some(Err(String::from_utf8_lossy(&out.stderr).into_owned()))
    }
}

fn opt(ir: &str) -> Option<Result<String, String>> {
    pipe("opt-19", &["-S", "-passes=verify", "-o", "-"], ir)
}

fn clang_ir(src: &str) -> Option<Result<String, String>> {
    let target = format!("--target={TRIPLE}");
    pipe(
        "clang++-19",
        &["-x", "c++", "-", "-S", "-emit-llvm", "-O0", "-std=c++26", &target, "-o", "-"],
        src,
    )
}

/// Normalizes `opt -S` output for comparison: drops the `ModuleID`
/// comment, `source_filename`, the `llvm.ident` named metadata and its
/// string node. `opt` itself renumbers attribute groups by first use and
/// canonicalizes value numbering.
fn normalize(s: &str) -> String {
    let mut out = String::new();
    for line in s.lines() {
        if line.starts_with("; ModuleID")
            || line.starts_with("source_filename")
            || line.starts_with("!llvm.ident")
            || (line.starts_with('!')
                && line.contains(" = !{!\"")
                && (line.contains("clang version")
                    || line.ends_with(&format!("!{{!\"{IDENT}\"}}"))))
        {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// One case's outcome.
enum Outcome {
    Exact,
    Differs(String),
    Skipped,
}

fn run_case(name: &str, src: &str) -> Result<Outcome, String> {
    let ir = kiln_ir(src)?;
    let Some(v) = opt(&ir) else { return Ok(Outcome::Skipped) };
    let ours = v.map_err(|e| format!("opt-19 rejects kiln IR:\n{e}\n{ir}"))?;
    let Some(c) = clang_ir(src) else { return Ok(Outcome::Skipped) };
    let c = c.map_err(|e| format!("clang++-19 failed: {e}"))?;
    let theirs =
        opt(&c).expect("opt-19 present").map_err(|e| format!("opt-19 rejects clang IR: {e}"))?;
    let (a, b) = (normalize(&ours), normalize(&theirs));
    if std::env::var("KILN_CASE").is_ok_and(|c| c == name) {
        eprintln!("==== kiln ====\n{a}\n==== clang ====\n{b}");
    }
    if a == b { Ok(Outcome::Exact) } else { Ok(Outcome::Differs(diff(&a, &b))) }
}

/// A minimal line diff: the first differing region with context.
fn diff(a: &str, b: &str) -> String {
    let al: Vec<&str> = a.lines().collect();
    let bl: Vec<&str> = b.lines().collect();
    let mut i = 0;
    while i < al.len() && i < bl.len() && al[i] == bl[i] {
        i += 1;
    }
    let lo = i.saturating_sub(3);
    let mut s = String::new();
    for l in &al[lo..al.len().min(i + 8)] {
        let _ = writeln!(s, "kiln : {l}");
    }
    for l in &bl[lo..bl.len().min(i + 8)] {
        let _ = writeln!(s, "clang: {l}");
    }
    s
}

#[test]
fn lowering_matches_clang() {
    let only = std::env::var("KILN_CASE").ok();
    let mut exact = 0;
    let mut total = 0;
    let mut failures = Vec::new();
    let mut deviations = Vec::new();
    for &(name, mode, src) in CASES {
        if only.as_deref().is_some_and(|o| o != name) {
            continue;
        }
        total += 1;
        match run_case(name, src) {
            Ok(Outcome::Exact) => {
                exact += 1;
                if mode == VerifyOnly {
                    deviations.push(format!("{name}: now exact; mark it Exact"));
                }
            }
            Ok(Outcome::Differs(d)) => match mode {
                Exact => failures.push(format!("{name}: differs from clang\n{d}")),
                VerifyOnly => deviations.push(format!("{name}: verify-only")),
            },
            Ok(Outcome::Skipped) => {}
            Err(e) => failures.push(format!("{name}: {e}")),
        }
    }
    eprintln!("lowering: {exact}/{total} exact");
    for d in &deviations {
        eprintln!("  {d}");
    }
    assert!(failures.is_empty(), "{} failing case(s):\n{}", failures.len(), failures.join("\n"));
}

/// (name, mode, program).
const CASES: &[(&str, Mode, &str)] = &[
    ("int_lit_return", Exact, "int f() { return 42; }"),
    ("params_add", Exact, "int f(int a, int b) { return a + b; }"),
    ("land_if", Exact, "int f(int a, int b) { if (a && b) return a + b; return 0; }"),
    ("lor_value", Exact, "bool f(int a, int b) { return a < 0 || b > 3; }"),
    (
        "unsigned_arith",
        Exact,
        "unsigned f(unsigned a, unsigned b) { return a * b - a / b + a % b; }",
    ),
    ("signed_div_rem", Exact, "int f(int a, int b) { return a / b + a % b - (a * b); }"),
    ("shifts", Exact, "int f(int a, unsigned b) { return (a << 3) + (a >> b) + (int)(b >> 2); }"),
    ("bitops", Exact, "long f(long a, long b) { return (a & b) | (a ^ ~b); }"),
    (
        "compare_all",
        Exact,
        "int f(int a, unsigned b, double d) { return (a < 1) + (b > 2u) + (d <= 3.0) + (a >= 4) + (a == 5) + (d != 6.0); }",
    ),
    ("float_arith", Exact, "double f(double a, float b) { return a * b + (a - b) / -a; }"),
    (
        "conversions",
        Exact,
        "double f(char c, short s, long l, unsigned char u, float x) { int i = c; long m = s; unsigned k = l; i = u; x = l; i = x; return i + m + k + x; }",
    ),
    (
        "bool_conv",
        Exact,
        "bool f(int a, double d, int* p) { bool x = a; bool y = d; bool z = p; return x && y && z; }",
    ),
    ("bool_arith", Exact, "int f(bool a, bool b) { return a + b + !a; }"),
    ("locals_assign", Exact, "int f(int a) { int x; int y = a; x = y = 3; return x + y; }"),
    (
        "compound_assign",
        Exact,
        "int f(int a, double d) { a += 2; a -= 1; a *= 3; a /= 2; a %= 5; a <<= 1; a >>= 1; a &= 7; a |= 8; a ^= 1; d += a; a += d; return a; }",
    ),
    (
        "inc_dec",
        Exact,
        "int f(int a) { int b = a++; int c = ++a; int d = a--; int e = --a; return b + c + d + e; }",
    ),
    (
        "inc_dec_float_bool_ptr",
        Exact,
        "double f(double d, int* p) { d++; --d; p++; --p; return d + *p; }",
    ),
    ("unary", Exact, "int f(int a, double d) { return -a + +a + ~a + (int)-d + !a; }"),
    ("ternary_scalar", Exact, "int f(int a, int b) { return a > b ? a : b; }"),
    ("ternary_lvalue", Exact, "void f(int a, int b, int c) { (c ? a : b) = 5; }"),
    ("comma", Exact, "int f(int a, int b) { return (a++, b); }"),
    (
        "globals",
        Exact,
        "int g = 5; static int h; extern int e; int f() { h = g; return g + h + e; }",
    ),
    (
        "enumerators",
        Exact,
        "enum E { A, B = 5, C }; enum class F : char { X = 3 }; int f(E e) { F x = F::X; return e + B + C + (int)x; }",
    ),
    (
        "calls_direct",
        Exact,
        "int add(int a, int b); short sh(short s); int f(int x) { return add(x, 2) + sh((short)x); }",
    ),
    (
        "calls_defined",
        Exact,
        "static int sq(int a) { return a * a; } int f(int x) { return sq(x) + sq(3); }",
    ),
    ("calls_void", Exact, "void g(int); void h(); void f() { g(1); h(); }"),
    (
        "calls_noexcept",
        Exact,
        "void g(int) noexcept; int h() noexcept(true); int k(); int f() { g(1); return h() + k(); }",
    ),
    (
        "noexcept_operator",
        Exact,
        "int may(); int wont() noexcept; bool f() { return noexcept(may()); } bool g() { return noexcept(wont()); }",
    ),
    (
        "calls_variadic",
        Exact,
        "extern \"C\" int printf(const char*, ...); int f(float x, char c, short s) { return printf(\"%f %d %d\\n\", x, c, s); }",
    ),
    (
        "calls_fnptr",
        Exact,
        "int add(int a, int b) { return a + b; } int f(int (*p)(int, int)) { int (*q)(int, int) = add; return p(1, 2) + q(3, 4) + (*q)(5, 6); }",
    ),
    (
        "calls_struct_ret",
        Exact,
        "struct P { int x, y; }; struct Big { long a[5]; }; P mk(int a); Big mkbig(); int use(P* p, Big* b); int f(Big* b) { P p = mk(1); return use(&p, b) + mk(2).x + (int)mkbig().a[3]; }",
    ),
    ("ret_struct", Exact, "struct P { int x, y; }; P mk(int a); P f(int a) { return mk(a + 1); }"),
    (
        "ret_big_struct",
        Exact,
        "struct B { long a, b, c; }; B mkb(long); B g(long x) { return mkb(x); } long h() { B c = mkb(2); return c.a + mkb(1).b; }",
    ),
    (
        "pointers",
        Exact,
        "int f(int* p, int i) { int* q = p + i; q = q - 2; q += 1; return *q + p[i] + i[p] + (int)(q - p); }",
    ),
    ("ptr_compare", Exact, "bool f(int* p, int* q) { return p < q && p != nullptr && q; }"),
    (
        "addr_deref",
        Exact,
        "int f() { int x = 1; int* p = &x; *p = 2; int** pp = &p; return **pp; }",
    ),
    ("void_ptr", Exact, "void* f(void* p, char* c, long n) { c = (char*)p + n; return c; }"),
    (
        "ptr_int_casts",
        Exact,
        "long f(int* p) { long x = (long)p; int* q = (int*)x; return (long)q + reinterpret_cast<unsigned long>(p); }",
    ),
    (
        "arrays_local",
        Exact,
        "int f(int i) { int a[4]; a[0] = 1; a[i] = 2; int m[2][3]; m[1][i] = 3; return a[i] + m[1][2]; }",
    ),
    (
        "array_init_const",
        Exact,
        "int f(int i) { int a[4] = {1, 2, 3, 4}; int z[8] = {}; int s[2] = {7}; return a[i] + z[i] + s[i]; }",
    ),
    (
        "array_init_big_const",
        Exact,
        "int f(int i) { int a[16] = {1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16}; int b[32] = {1}; return a[i] + b[i]; }",
    ),
    (
        "array_init_dynamic",
        Exact,
        "int f(int x) { int a[3] = {x, x + 1, 2}; int b[8] = {x}; return a[2] + b[7]; }",
    ),
    (
        "char_array_string",
        Exact,
        "int f(int i) { char s[] = \"hello\"; char t[10] = \"hi\"; const char* p = \"world\"; return s[i] + t[i] + p[i]; }",
    ),
    (
        "struct_member",
        Exact,
        "struct S { int a; double b; char c[3]; }; double f(S* p, int i) { S s; s.a = i; s.b = 2.0; p->c[1] = 'x'; return s.a + s.b + p->a + p->c[i]; }",
    ),
    (
        "struct_init",
        Exact,
        "struct S { int a; int b; long c; }; long f(int x) { S s = {1, 2, 3}; S t = {x, 2}; S u = {}; return s.c + t.a + u.b; }",
    ),
    (
        "struct_nested",
        Exact,
        "int x0(); struct In { short x; int y; }; struct Out { In i; In a[2]; char c; }; int f(Out* o) { Out p = {{1, 2}, {{3, 4}, {5, x0()}}, 'c'}; return o->i.y + o->a[1].x + p.a[0].y; }",
    ),
    (
        "struct_member_copy",
        Exact,
        "struct S { int a[5]; int n; }; void f(S* a, S* b) { a->n = b->n; a->a[2] = b->a[3]; S c; c.n = a->n; b->a[0] = c.n; }",
    ),
    (
        "unions",
        Exact,
        "union U { int i; float f; char c[8]; }; int f(float x) { U u; u.f = x; U v = {3}; return u.i + v.c[1]; }",
    ),
    (
        "bitfields",
        Exact,
        "struct B { unsigned a : 3; int b : 5; unsigned c : 9; }; int f(B* p, int x) { p->a = x; p->b = -1; p->c = p->c + 2; return p->a + p->b + p->c; }",
    ),
    // Deviation (kiln_sema): the compound-assignment computation type does
    // not promote a bit-field LHS narrower than int, so `c += 2` computes in
    // `unsigned` (`add`) where clang computes in `int` (`add nsw`).
    ("bitfields_compound", Exact, "struct B { unsigned c : 9; }; void f(B* p) { p->c += 2; }"),
    (
        "sizeof_alignof",
        Exact,
        "struct S { char c; double d; }; unsigned long f(int x) { return sizeof(int) + sizeof x + alignof(S) + sizeof(S[3]); }",
    ),
    (
        "string_literals",
        Exact,
        "const char* f(int i) { const char* a = \"abc\"; const char* b = \"abc\"; return i ? a : b; } const wchar_t* g() { return L\"wide\"; }",
    ),
    (
        "casts_named",
        Exact,
        "int f(double d, const int* p) { int* q = const_cast<int*>(p); return static_cast<int>(d) + *q + (int)d + int(d); }",
    ),
    ("nullptr_lit", Exact, "int* f(bool b) { int* p = nullptr; if (b) p = 0; return p; }"),
    (
        "constexpr_fold",
        Exact,
        "constexpr int sq(int x) { return x * x; } int f() { constexpr int a = sq(4); const int b = 3; int arr[b] = {}; return a + b + arr[0]; }",
    ),
    (
        "static_local",
        Exact,
        "int f() { static int n = 3; static const int k = 10; return ++n + k; }",
    ),
    (
        "static_local_discriminators",
        Exact,
        "int f() { int r = 0; { static int s = 1; r += s; } { static int s = 2; r += s; } { static int s = 3; r += s; } { static int s = 4; r += s; } { static int s = 5; r += s; } { static int s = 6; r += s; } { static int s = 7; r += s; } { static int s = 8; r += s; } { static int s = 9; r += s; } { static int s = 10; r += s; } { static int s = 11; r += s; } { static int s = 12; r += s; } { static int s = 13; r += s; } return r; }",
    ),
    (
        "if_else_chain",
        Exact,
        "int f(int a) { if (a < 0) return -1; else if (a == 0) return 0; else { a = a * 2; } return a; }",
    ),
    (
        "if_constexpr_true",
        Exact,
        "int f(int a) { if constexpr (sizeof(int) == 4) { return a + 1; } else { return a - 1; } }",
    ),
    (
        "if_constexpr_false",
        Exact,
        "int f(int a) { if constexpr (sizeof(int) == 8) { a = a * 3; } else { a = a - 1; } return a; }",
    ),
    (
        "if_consteval",
        Exact,
        "int f(int a) { if consteval { return 1; } else { return a; } } int g(int a) { if !consteval { a += 2; } return a; }",
    ),
    (
        "if_init",
        Exact,
        "int f(int a) { if (int b = a * 2; b > 4) return b; if (sizeof(int) == 4) return 1; else return 2; }",
    ),
    ("while_loop", Exact, "int f(int n) { int s = 0; while (n > 0) { s += n; n--; } return s; }"),
    ("do_while", Exact, "int f(int n) { int s = 0; do { s += n; } while (--n); return s; }"),
    (
        "for_loop",
        Exact,
        "int f(int n) { int s = 0; for (int i = 0; i < n; ++i) { if (i == 3) continue; if (i == 7) break; s += i; } return s; }",
    ),
    (
        "for_infinite",
        Exact,
        "int f(int n) { for (;;) { if (n > 10) break; n++; } while (1) { if (n) return n; } }",
    ),
    (
        "nested_loops",
        Exact,
        "int f(int n) { int s = 0; for (int i = 0; i < n; i++) for (int j = 0; j < i; j++) { if (j == 2) break; s += j; } return s; }",
    ),
    (
        "switch_basic",
        Exact,
        "int f(int x) { switch (x) { case 1: return 10; case 2: case 3: x += 1; break; default: x = 0; } return x; }",
    ),
    (
        "switch_fallthrough",
        Exact,
        "int f(int x) { int r = 0; switch (x) { case 0: r++; case 5: r += 2; break; case 9: { r = 9; break; } } return r; }",
    ),
    (
        "switch_char_enum",
        Exact,
        "enum E { A, B, C }; int f(E e, char c) { switch (e) { case A: return 1; case C: break; } switch (c) { case 'a': return 2; default: return 3; } }",
    ),
    (
        "goto_labels",
        Exact,
        "int f(int n) { int i = 0; top: if (i >= n) goto out; i++; goto top; out: return i; }",
    ),
    (
        "null_and_static_assert",
        Exact,
        "int f(int a) { ; static_assert(sizeof(a) == 4); {} ;; return a; }",
    ),
    ("void_return_falloff", Exact, "void g(int*); void f(int* p) { if (!p) return; g(p); }"),
    ("main_fn", Exact, "int main() { int x = 3; return x - 3; }"),
    ("main_no_return", Exact, "int puts(const char*); int main() { puts(\"hi\"); }"),
    (
        "fn_returning_ptr_to_array",
        Exact,
        "int a[3][4]; int (*f(int i))[4] { return &a[i]; } int g(int i, int j) { return a[i][j] + f(i)[0][j]; }",
    ),
    (
        "long_double_and_i128",
        Exact,
        "long double f(long double x, __int128 y) { return x * 2 + (long double)(y >> 3); }",
    ),
    (
        "char_literals",
        Exact,
        "int f() { char c = 'a'; wchar_t w = L'b'; char16_t u = u'c'; char32_t U = U'd'; return c + w + u + U; }",
    ),
    (
        "ignored_exprs",
        Exact,
        "int g(); void f(int a, int* p) { a; (void)a; g(); a + 1; *p; (void)g(); }",
    ),
];
