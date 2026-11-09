// Coverage of a new-expression in a condition ([expr.new]/14,
// [stmt.if], [expr.cond], [expr.log.and], [expr.log.or],
// [expr.unary.op]; WP-1108, spec/codegen.md §5.4): clang's
// `ConditionFoldsToBool` folds `(bool)new T` to `true` when the
// allocation function is replaceable (or the nothrow form) and the
// initializer is constant, so the branch region of a folded condition
// is zeroed and its dead side gets no counter; a placement allocation
// function or a non-constant initializer is not folded.
//@ args: -fprofile-instr-generate -fcoverage-mapping -fcoverage-compilation-dir=/cov -Xclang -disable-llvm-passes
namespace std {
struct nothrow_t { explicit nothrow_t() = default; };
extern const nothrow_t nothrow;
using size_t = decltype(sizeof 0);
}
void *operator new(std::size_t, const std::nothrow_t &) noexcept;
void *operator new(std::size_t, short, char);
int g();
int h();
struct K { constexpr K() : x(1) {} int x; };
struct N { N(); int x; };
int i1() {
  if (new int(1))
    return g();
  return h();
}
int i2() {
  if (new (std::nothrow) int(1))
    return g();
  return h();
}
int i3() {
  if (new (short(1), 'x') int(1))
    return g();
  return h();
}
int i4() {
  if (!new int)
    return g();
  return h();
}
int c1() { return new int ? g() : h(); }
int c2() { return new N ? g() : h(); }
int c3() { return new int(g()) ? g() : h(); }
bool l1(bool c) { return c && new int(2); }
bool l2(bool c) { return c || new (std::nothrow) int(4); }
bool l3(bool c) { return c || new (short(1), 'x') int(4); }
bool l4(bool c) { return (new K) && c; }
bool n1(bool c) { return c && !new int; }
int w1(int n) {
  while (!new int[2]{1, 2})
    n = g();
  return n;
}
