// [dcl.fct]/11: a function shall not have a return type of type array or
// function. With a trailing-return-type ([dcl.fct]/2) clang diagnoses it at
// the `auto` type-specifier of the declarator that owns the trailing return
// (the inner `auto` of a nested `auto (*)() -> F`), also when the type comes
// from substitution; without one, at the function declarator's '('.
// A lambda has no `auto` to point at: its '(' (clang).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:5 error "function cannot return function type 'F' (aka 'int ()')"
//@ diag: 25:5 error "function cannot return array type 'A' (aka 'int[2]')"
//@ diag: 26:8 error "function cannot return function type 'int ()'"
//@ diag: 27:8 error "function cannot return function type 'F' (aka 'int ()')"
//@ diag: 28:1 error "function cannot return array type 'A' (aka 'int[2]')"
//@ diag: 30:10 error "function cannot return function type 'F' (aka 'int ()')"
//@ diag: 31:3 error "function cannot return array type 'A' (aka 'int[2]')"
//@ diag: 33:1 error "function cannot return function type 'F' (aka 'int ()')"
//@ diag: 34:8 error "function cannot return function type 'int ()'"
//@ diag: 35:14 error "function cannot return function type 'F' (aka 'int ()')"
//@ diag: 37:38 error "function cannot return function type 'int ()'"
//@ diag: 38:6 note "in instantiation of template class 'X<int ()>' requested here"
//@ diag: 39:14 error "function cannot return array type 'A' (aka 'int[2]')"
typedef int F();
typedef int A[2];
F g1();
A g2();
int (g3())();
static auto f1() -> F;
auto f2() -> A;
struct S {
  static auto m() -> F;
  auto n() const -> A;
};
auto (*fp)() -> F;
inline auto f3() -> int();
auto f4() -> auto (*)() -> F;
template <class T> auto f5() -> T;
template <class T> struct X { static auto m() -> T; };
X<F> x;
auto lam = []() -> A { return {}; };
