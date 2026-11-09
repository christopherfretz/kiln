// new-expressions as conditions outside a constant context ([expr.new]/14,
// [stmt.if], [expr.cond], [expr.unary.op]; WP-1108, spec/codegen.md §3):
// clang's codegen folds `(bool)new T` to `true` (`VisitCXXNewExpr`) when
// the allocation function is a replaceable global one, the nothrow form
// or the reserved placement form, and the initializer evaluates; the
// allocation is then never emitted (`CheckMemoryLeaks` is only a
// core-constant-expression note). A class-specific or user placement
// allocation function, a runtime array bound, a non-constexpr
// constructor or a non-constant initializer keeps the call.
namespace std { struct nothrow_t { explicit nothrow_t() = default; }; extern const nothrow_t nothrow; using size_t = decltype(sizeof 0); enum class align_val_t : size_t {}; }
void *operator new(std::size_t, const std::nothrow_t &) noexcept;
void *operator new(std::size_t, short, char);
int g(); int h();
struct C { static void *operator new(std::size_t); int x; };
struct K { constexpr K() : x(1) {} int x; };
struct N { N(); int x; };
struct alignas(32) A { int x[8]; };
int a1() { return new (std::nothrow) int(1) ? g() : h(); }
int a2() { return new (short(1), 'x') int(1) ? g() : h(); }
int a3() { return new C ? g() : h(); }
int a4(int n) { return new int[n] ? g() : h(); }
int a5() { return new int[3] ? g() : h(); }
int a6() { return new K ? g() : h(); }
int a7() { return new N ? g() : h(); }
int a8() { return new int(g()) ? g() : h(); }
int a9() { return new int[2]{1, 2} ? g() : h(); }
int b1() { return new A{} ? g() : h(); }
int b3() { return new K[2] ? g() : h(); }
int b4() { return new N[2] ? g() : h(); }
bool b5() { return !new int; }
int b6() { if (!!new int) return g(); else return h(); }
static bool sb = new int;
bool b7() { return sb; }
// A value that points at the allocation is not folded; a comparison of
// two allocations is.
int *gp = new int(3);
bool gb = new int(3) != nullptr;
bool gc = new int(3) == new int(4);
int *f() { return new int(7); }
int gv() { return *new int(5); }
bool hv() { int *p = new int(1); return p; }
