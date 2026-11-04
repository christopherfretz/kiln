// [expr.const]/16-18: a qualified dependent call of a consteval function
// is checked when its template is instantiated; an argument that is not a
// constant there makes it an invocation that is not a constant
// expression, diagnosed once, with the instantiation note.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:41 error "call to consteval function 'mv' is not a constant expression"
//@ diag: 16:23 note "in instantiation of function template specialization 'p2<1>' requested here"
//@ diag: 14:48 note "function parameter 'x' with unknown value cannot be used in a constant expression"
//@ diag: 14:30 note "declared here"
struct V { int a; };
consteval V mv(V x, V y) { return {x.a + y.a}; }
template <int H> consteval V wl() { return {H}; }
template <int H> void p2(int x) { V v = ::mv(V{x}, wl<H>()); (void)v; }
template <int H> void p1() { V v = ::mv(wl<H>(), V{1}); (void)v; }
void use() { p1<1>(); p2<1>(2); }
