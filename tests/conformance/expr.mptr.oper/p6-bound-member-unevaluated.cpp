// [expr.mptr.oper]/6: the function result of `.*` can only be called;
// [expr.prim.id.general]: a builtin function name likewise. The operand of
// `decltype` and `noexcept` is no exception.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:20 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 12:22 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 13:21 error "reference to non-static member function must be called"
//@ diag: 14:19 error "builtin functions must be directly called"
struct S { bool p() const; int q(int); } s, *ps;
bool b = noexcept(s.*&S::p);
using T = decltype(ps->*&S::p);
using U = decltype(s.*&S::q);
bool c = noexcept(__builtin_abs);
