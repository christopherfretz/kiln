// [support.srcloc.obs], [lex.string]/9, [intro.object]/9, [expr.eq]/3:
// nothing ties the identity of a source-location string to its contents.
// kiln makes one string object per (content, role, declaration the use is
// evaluated in), so that each object has one maker (spec/ast.md §5.8.1),
// yet objects with equal contents keep one address, at run time and to
// the constant evaluator, as in clang 19 and 22 (one StringLiteral per
// content) and GCC 16.1: the "f" of f(int) and the "f" of f(long) compare
// equal, subtract to 0 and order as one object; so do two callers' names
// made through a default argument (evaluated in the caller), and a file
// named "f" against a function named "f".
//@ expect: compile-ok
//@ strict-diags

constexpr const char *f(int) { return __builtin_FUNCTION(); }
constexpr const char *f(long) { return __builtin_FUNCTION(); }
static_assert(f(1)[0] == 'f' && f(1L)[1] == '\0');
static_assert(f(1) == f(1L));
static_assert(!(f(1) != f(1L)));
static_assert(f(1) <= f(1L) && f(1) >= f(1L));
static_assert(f(1) - f(1L) == 0);
static_assert(f(1) + 1 == f(1L) + 1);
static_assert(f(1) < f(1L) + 1);
// One use evaluated twice.
static_assert(f(1) == f(2));

// A default argument evaluated at a call is a use in the caller.
constexpr const char *g(const char *s = __builtin_FUNCTION()) { return s; }
constexpr const char *h() { return g(); }
namespace x {
constexpr const char *h() { return g(); }
}
static_assert(h()[0] == 'h' && x::h()[0] == 'h');
static_assert(h() == x::h());

// Two lambdas of one function print one pretty name.
struct loc {
  const char *fn;
};
constexpr loc two() {
  auto a = [] { return __builtin_FUNCTION(); };
  auto b = [] { return __builtin_FUNCTION(); };
  return {a() == b() ? a() : nullptr};
}
static_assert(two().fn != nullptr);

#line 100 "f"
// A file name "f" and a function name "f": two roles, one address.
constexpr const char *file() { return __builtin_FILE(); }
static_assert(file()[0] == 'f' && file() == f(1));
