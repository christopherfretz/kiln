// [expr.call]/1: the postfix expression of a function call is a function
// lvalue or a pointer to function; a pointer to member function needs an
// object (`(o.*pmf)(a)`) and is not callable by itself. libc++'s
// `invocable<int (S::*)(int), int>` is false (invocable.compile.pass.cpp).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:25 error "called object type 'P' (aka 'int (S::*)(int)') is not a function or function pointer"
struct S { int f(int); };
using P = int (S::*)(int);
int call(P p) { return p(1); }
