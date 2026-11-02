// [temp.pre]/9, [temp.pre]/1 (constraint-logical-and-expression of
// primary-expressions): a builtin type trait `__trait(...)` is a
// primary-expression in a requires-clause (clang's
// ParseCastExpression(PrimaryExprOnly)), not a name followed by a call.
// Found reducing libc++'s prospective destructors (WP-276).
// Verified against clang 19.
//@ expect: compile-ok
//@ strict-diags
template <class T> struct A {
  int f() requires __is_trivially_destructible(T) { return 1; }
  int g() requires __is_same(T, int) && __is_integral(T) { return 2; }
  int h() requires __is_pointer(T) || __is_class(T) { return 3; }
  int h() requires __is_integral(T) { return 4; }
};
template <class T> requires __is_integral(T) int k(T) { return 5; }
template <class T> concept Int = __is_integral(T) && true;
static_assert(Int<int> && !Int<float>);
int u() { A<int> a; return a.f() + a.g() + a.h() + k(1); }
