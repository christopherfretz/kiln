// [dcl.fct.def.delete]/2: a deleted function selected by overload
// resolution in an unevaluated operand is still ill-formed (outside a
// substitution): in decltype and sizeof, for a function and a function
// template specialization; a function template whose signature names a
// member without an object fails substitution, leaving no viable
// function. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:10 error "call to deleted function 'del'"
//@ diag: 18:6 note "candidate function has been explicitly deleted"
//@ diag: 19:6 note "candidate function"
//@ diag: 22:20 error "call to deleted function 'tdel'"
//@ diag: 21:24 note "candidate function [with T = int] has been explicitly deleted"
//@ diag: 24:19 error "attempt to use a deleted function"
//@ diag: 23:23 note "'g' has been explicitly marked deleted here"
//@ diag: 27:9 error "no matching function for call to 'u'"
//@ diag: 26:33 note "candidate template ignored: substitution failure [with C = bar]"
void del(int) = delete;
void del(double);
decltype(del(1)) *p;
template<class T> void tdel(T) = delete;
using U = decltype(tdel(1));
struct S { static int g(int) = delete; };
int n = sizeof(S::g(1));
struct bar { int x(); };
template<class C> constexpr int u(decltype(sizeof(C::x))) { return 1; }
int a = u<bar>(0);
