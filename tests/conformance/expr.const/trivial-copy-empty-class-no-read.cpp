// [expr.const]/10.9, [class.copy.ctor]/14: a trivial copy or assignment of
// a class holding no data does not read its source (clang's
// isReadByLvalueToRvalueConversion), so copying an `extern const` empty
// object without an initializer is a constant expression. libc++
// <__functional/bind.h>:62 (`extern const __ph<1> _1;`; func.bind.place
// placeholders, WP-276). Verified against clang 19.
//@ expect: compile-ok
//@ strict-diags
template <int N> struct ph {};
extern const ph<1> p1;
struct E {};
struct F { E e; };
extern const F f1;
extern E e1;
template <class T> constexpr bool test(const T &t) {
  T x = t;
  T y;
  y = t;
  (void)x;
  return true;
}
static_assert(test(p1));
static_assert(test(f1));
static_assert(test(e1));
