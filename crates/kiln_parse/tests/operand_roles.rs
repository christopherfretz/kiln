//! The operand-role table (`kiln_ast::roles`, spec/ast.md §20) against
//! real ASTs: every word of every expression, statement, attribute and
//! type node a translation unit builds holds what its kind's row says
//! (an id in range, 0 where the role is zero, an enum value at most its
//! maximum, bits only in the row's fields). A tooling client reads the
//! raw words through this table, so a builder that writes a word the
//! table does not describe fails here.
//!
//! `KILN_ROLES_DIR=<dir> cargo test -p kiln_parse --test operand_roles
//! -- --ignored` runs the same check over every `.cpp` file of a
//! directory tree.

use std::sync::Arc;

use kiln_ast::roles::{
    ATTR_FLAGS, Field, Role, Words, attr_roles, expr_roles, stmt_roles, type_roles,
};
use kiln_ast::{Ast, AttrId, ExprId, ListId, QualType, StmtId, TypeId};
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// Reduced from real code: templates and their instantiations, lambdas,
/// casts, new/delete, initialization forms, structured bindings, the
/// statement kinds, attributes on declarations and statements.
const SRC: &str = r#"
extern "C" int printf(const char *, ...);
extern "C" void *malloc(decltype(sizeof 0));
namespace std { using size_t = decltype(sizeof 0);
  template <class E> struct initializer_list { const E *b; size_t n;
    constexpr initializer_list() : b(nullptr), n(0) {}
    constexpr const E *begin() const { return b; } constexpr const E *end() const { return b + n; } }; }
void done(int *);
struct B { virtual ~B() {} int bx = 1; };
struct D : B { int dx; D(int x) : dx(x) {} D() : D(0) {} int get() const { return dx; } };
struct Agg { int a; double b[2]; };
union U { int i; float f; };
enum class Col : unsigned char { red, green = 4 };
template <class T, int N = 3> struct Arr {
  T v[N];
  constexpr T at(int i) const { return v[i]; }
  template <class U> static constexpr bool same = __is_same(T, U);
};
template <class... Ts> constexpr auto sum(Ts... ts) { return (ts + ... + 0); }
template <class T> concept Small = sizeof(T) <= 8 && requires(T t) { t + t; { t } -> Small; };
template <Small T> constexpr T twice(T t) noexcept(noexcept(t + t)) { return t + t; }
template <class T> typename T::type dep_fn(T t, decltype(t.x) *p = nullptr);
template <class T> struct Holder { T t; auto get() -> decltype(t.get()) { return t.get(); } };
__attribute__((visibility("hidden"))) int hidden_var;
__thread int tls_var __attribute__((tls_model("initial-exec")));
__attribute__((constructor(200))) void ctor_fn();
void nn(int *, int *) __attribute__((nonnull(2)));
[[nodiscard("why")]] int must();
[[deprecated("old")]] int old_var;
typedef int V4 __attribute__((vector_size(16)));
int run(int k, ...) {
  int v __attribute__((cleanup(done))) = 1;
  D d(k);
  B *bp = &d;
  D *dp = dynamic_cast<D *>(bp);
  const D &cr = static_cast<const D &>(*dp);
  long l = (long)k + reinterpret_cast<long>(nullptr) * 0;
  int *np = new int[3]{1, 2, 3};
  delete[] np;
  D *one = new D(2);
  delete one;
  Agg ag{1, {2.0, 3.0}};
  Agg ag2 = {.a = 4};
  U u{.f = 1.0f};
  auto [x1, x2] = ag;
  int arr[] = {1, 2, 3};
  for (int e : arr) k += e;
  for (int i = 0; i < 3; ++i) { if (i == 1) continue; k ^= i; }
  while (k > 100) k /= 2;
  do { --k; } while (k > 50);
  switch (k) { case 0: k = 1; [[fallthrough]]; case 1 ... 3: break; default: ; }
  if constexpr (sizeof(int) == 4) k += 1; else k -= 1;
  auto lam = [=, &v](int z) mutable { return z + k + v; };
  auto glam = []<class T>(T t) { return t; };
  k += lam(1) + glam(2) + sum(1, 2, 3) + twice(4) + Arr<int>{{1, 2, 3}}.at(1);
  k += Arr<int>::same<int> + sizeof(Agg) + alignof(D) + __builtin_offsetof(Agg, b[1]);
  k += noexcept(run(0)) + (k ? 1 : 2) + (k ?: 3) + "str"[0] + L'c' + u8'x';
  k += 0x1fULL + 1.5 + (int)Col::green + typeid(D).name()[0] + cr.get() + (int)l;
  V4 vec = {1, 2, 3, 4};
  k += vec[2] + __builtin_choose_expr(1, 2, 3) + ({ int s = 4; s; });
  __builtin_va_list ap;
  __builtin_va_start(ap, k);
  k += __builtin_va_arg(ap, int);
  __builtin_va_end(ap);
  int at = 0;
  __atomic_fetch_add(&at, 1, __ATOMIC_SEQ_CST);
  try { throw D(1); } catch (const D &e) { k += e.get(); } catch (...) { throw; }
  static_assert(__is_trivially_copyable(Agg));
  goto out;
out:
  [[assume(k > 0)]];
  asm volatile("" ::: "memory");
  return printf("%d %p", k, malloc(4)) + x1 + (int)x2 + ag2.a + u.i + (dp != nullptr);
}
"#;

fn parse(src: &str) -> Ast {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let target = Target::for_arch(Arch::X86_64);
    let opts = ParseOptions::default();
    parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default()).ast
}

/// Checks words against roles, collecting what does not fit.
struct Check<'a> {
    ast: &'a Ast,
    bad: Vec<String>,
    /// The words checked, per role kind (the test asserts coverage).
    seen: usize,
}

impl Check<'_> {
    fn in_range(w: u32, len: usize) -> bool {
        w != 0 && (w as usize) <= len
    }

    /// Whether `w` fits `r` (after resolving its conditions on the node's
    /// `flags` and `aux`).
    fn fits(&self, r: &Role, w: u32, cx: Words) -> bool {
        let ast = self.ast;
        match *r.resolve(cx) {
            Role::Zero => w == 0,
            Role::Expr => Self::in_range(w, ast.exprs.len()),
            Role::Stmt => Self::in_range(w, ast.stmts.len()),
            Role::Decl => Self::in_range(w, ast.decls.len()),
            Role::Attr => Self::in_range(w, ast.attrs.len()),
            Role::TypeId => Self::in_range(w, ast.types.len()),
            Role::Type => {
                QualType::from_raw(w).is_some_and(|q| q.ty().index() as usize <= ast.types.len())
            }
            Role::Opt(inner) => w == 0 || self.fits(inner, w, cx),
            Role::List(inner) => {
                (w as usize) < ast.lists.count() + 2
                    && w != 0
                    && ast.lists.get(ListId::new(w)).iter().all(|&e| self.fits(inner, e, cx))
            }
            Role::Enum { max, .. } => w <= max,
            Role::Bool => w <= 1,
            Role::Bits(fs) => {
                let all = fs.iter().fold(0, |m, f| m | f.mask());
                w & !all == 0 && fs.iter().all(|f| self.fits(f.role, (w & f.mask()) >> f.lo, cx))
            }
            _ => true,
        }
    }

    fn word(&mut self, what: &dyn Fn() -> String, r: &Role, w: u32, cx: Words) {
        self.seen += 1;
        if !self.fits(r, w, cx) {
            self.bad.push(format!("{}: {w:#x} is not {:?}", what(), r.resolve(cx)));
        }
    }

    /// The flag bits outside `fields` (within `mask`) are clear.
    fn flags(&mut self, what: &dyn Fn() -> String, fields: &[&[Field]], mask: u8, flags: u8) {
        let known = fields.iter().flat_map(|f| f.iter()).fold(0u8, |m, f| m | f.mask() as u8);
        if flags & mask & !known != 0 {
            self.bad.push(format!("{}: flags {flags:#x} outside the row's", what()));
        }
    }

    fn all(&mut self) {
        let ast = self.ast;
        for i in 1..=ast.exprs.len() {
            let n = *ast.exprs.get(ExprId::new(i as u32));
            let r = expr_roles(n.kind());
            let (f, aux) = (n.raw_bits(), u16::from(n.aux()));
            let cx = Words { flags: f, aux, a: n.a(), b: n.b() };
            let what = || format!("expr {i} {:?}", n.kind());
            self.word(&|| format!("{} a", what()), &r.a, n.a(), cx);
            self.word(&|| format!("{} b", what()), &r.b, n.b(), cx);
            self.word(&|| format!("{} aux", what()), &r.aux, u32::from(aux), cx);
            self.flags(&what, &[r.flags], 0xf0, f);
        }
        for i in 1..=ast.stmts.len() {
            let n = *ast.stmts.get(StmtId::new(i as u32));
            let r = stmt_roles(n.kind());
            // A statement has no flags byte (WP-2026): per-kind bits live in `aux`.
            let cx = Words { flags: 0, aux: n.aux(), a: n.a(), b: n.b() };
            let what = || format!("stmt {i} {:?}", n.kind());
            self.word(&|| format!("{} a", what()), &r.a, n.a(), cx);
            self.word(&|| format!("{} b", what()), &r.b, n.b(), cx);
            self.word(&|| format!("{} aux", what()), &r.aux, u32::from(n.aux()), cx);
        }
        for i in 1..=ast.attrs.len() {
            let n = *ast.attrs.get(AttrId::new(i as u32));
            let Some(r) = attr_roles(n.kind) else {
                self.bad.push(format!("attr {i}: kind {} has no row", n.kind.0));
                continue;
            };
            let cx = Words { flags: n.flags, aux: 0, a: n.a, b: n.b };
            let what = || format!("attr {i} kind {}", n.kind.0);
            self.word(&|| format!("{} a", what()), &r.a, n.a, cx);
            self.word(&|| format!("{} b", what()), &r.b, n.b, cx);
            self.flags(&what, &[ATTR_FLAGS, r.flags], 0xff, n.flags);
        }
        for i in 1..=ast.types.len() {
            let t = TypeId::new(i as u32);
            let n = ast.types.node(t);
            let r = type_roles(n.kind());
            let cx = Words { flags: n.flags().bits(), aux: n.aux(), a: 0, b: 0 };
            let what = || format!("type {i} {:?}", n.kind());
            self.word(&|| format!("{} aux", what()), &r.aux, u32::from(n.aux()), cx);
            if r.words.is_empty() {
                self.word(&|| format!("{} data", what()), &r.data, n.data(), cx);
            }
            for (j, (role, w)) in r.words.iter().zip(ast.types.payload_words(t)).enumerate() {
                self.word(&|| format!("{} word {j}", what()), role, w, cx);
            }
        }
    }
}

fn check(ast: &Ast) -> Check<'_> {
    let mut c = Check { ast, bad: Vec::new(), seen: 0 };
    c.all();
    c
}

/// Every word of the source's AST fits its kind's row.
#[test]
fn every_word_fits_its_role() {
    let ast = parse(SRC);
    let c = check(&ast);
    assert!(c.seen > 2000, "the source builds many nodes ({} words)", c.seen);
    assert!(c.bad.is_empty(), "{} words outside their roles:\n{}", c.bad.len(), c.bad.join("\n"));
}

/// The same check over the `.cpp` files of `KILN_ROLES_DIR`.
#[test]
#[ignore = "set KILN_ROLES_DIR to a directory of sources"]
fn every_word_fits_its_role_in_a_directory() {
    let Ok(dir) = std::env::var("KILN_ROLES_DIR") else { return };
    let mut files = Vec::new();
    let mut todo = vec![std::path::PathBuf::from(dir)];
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
    // The driver's compile thread has a large stack; a deep test needs one
    // here too (a debug build's frames are larger).
    let n = files.len();
    let worker = std::thread::Builder::new().stack_size(1 << 30).spawn(move || {
        let mut bad = Vec::new();
        for f in &files {
            let Ok(src) = std::fs::read_to_string(f) else { continue };
            // A file's own `//@ args:` are not applied, so a front-end
            // panic on one is reported, not fatal to the run.
            let Ok(ast) = std::panic::catch_unwind(|| parse(&src)) else {
                bad.push(format!("{}: the front end panicked", f.display()));
                continue;
            };
            let c = check(&ast);
            bad.extend(c.bad.into_iter().take(5).map(|b| format!("{}: {b}", f.display())));
        }
        bad
    });
    let bad = worker.expect("a thread").join().expect("no panic");
    eprintln!("{n} files");
    assert!(bad.is_empty(), "{} words outside their roles:\n{}", bad.len(), bad.join("\n"));
}
