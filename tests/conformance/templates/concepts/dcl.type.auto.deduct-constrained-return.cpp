//@ expect: compile-ok
// [dcl.spec.auto.general]/9: a constrained placeholder return type.
template <class T> concept Int = __is_same(T, int);
Int auto f() { return 1; }
Int decltype(auto) g() { return 2; }
static_assert(__is_same(decltype(f()), int));
static_assert(__is_same(decltype(g()), int));
