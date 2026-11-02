// [expr.call]/1, [expr.mptr.oper]/6: a pointer to member function is
// called through `.*`/`->*` with an object; calling it directly is
// ill-formed, so a requires-expression that tries it is not satisfied
// (libc++ invocable.compile.pass.cpp, regular_invocable.compile.pass.cpp).
//@ expect: compile-ok
//@ strict-diags
struct S { int f(int); };
template <class F, class... A> concept callable = requires(F f, A... a) { f(a...); };
static_assert(!callable<int (S::*)(int), int>);
static_assert(!callable<int (S::*)(int), S &, int>);
static_assert(callable<int (*)(int), int>);
int g(S &o, int (S::*p)(int)) { return (o.*p)(1); }
