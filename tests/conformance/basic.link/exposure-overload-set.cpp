// [basic.link]/14.5 (working draft /13.5): a declaration names a function
// E when it contains an id-expression that refers to a set of overloads
// containing E, whether or not overload resolution selects E. In a module
// interface, an overload set holding a TU-local function is an exposure
// where a use of that function would be (/15, /17): a call (the set found
// by lookup, and the functions argument-dependent lookup adds), a pointer
// conversion, a qualified name, an operator function named as a call, a
// function template in the set, a template-id, an unevaluated operand, a
// dependent call whose definition's set holds one (and a specialization
// made TU-local by its instantiated call, /15.5), a default argument, a
// default member initializer and a lambda in an inline variable's
// initializer. GCC 16.1 checks only the selected function (it rejects
// `dependent` alone) and clang 22 accepts each: a deviation,
// docs/status/deviations.md. The forms that do not name the set are in
// exposure-overload-set-accepted.cpp.
//@ expect: compile-fail
//@ std: c++20
//@ skip(clang): clang 22 does not check an overload set
//@ strict-diags
//@ diag: 51:19 error "declaration of 'call' exposes TU-local entity 'f'"
//@ diag: 52:19 error "declaration of 'address' exposes TU-local entity 'f'"
//@ diag: 53:19 error "declaration of 'qualified' exposes TU-local entity 'f'"
//@ diag: 54:19 error "declaration of 'by_adl' exposes TU-local entity 'adl'"
//@ diag: 55:17 error "declaration of 'operator_name' exposes TU-local entity 'operator+'"
//@ diag: 56:19 error "declaration of 'with_template' exposes TU-local entity 'ft'"
//@ diag: 57:19 error "declaration of 'template_id' exposes TU-local entity 'tf'"
//@ diag: 58:29 error "declaration of 'unevaluated' exposes TU-local entity 'f'"
//@ diag: 59:38 error "declaration of 'dependent' exposes TU-local entity 'f'"
//@ diag: 60:38 error "declaration of 'non_dependent' exposes TU-local entity 'f'"
//@ diag: 62:19 error "declaration of 'instantiation' exposes TU-local entity 'calls_adl<N::A>'"
//@ diag: 63:12 error "declaration of 'default_argument' exposes TU-local entity 'f'"
//@ diag: 64:15 error "declaration of 'Member' exposes TU-local entity 'f'"
//@ diag: 65:30 error "declaration of 'closure' exposes TU-local entity 'f'"
export module M;
static int f(int) { return 1; }
int f(double) { return 2; }
namespace N {
struct A {};
int adl(A) { return 3; }
static int adl(int) { return 4; }
}
int adl(double) { return 5; }
struct B {};
static B operator+(B, int) { return {}; }
B operator+(B, B) { return {}; }
template <class T> static int ft(T *) { return 6; }
int ft(double) { return 7; }
template <class T> static int tf(T) { return 8; }
template <class T> int tf(T, int) { return 9; }

export inline int call() { return f(1.0); }
export inline int address() { int (*p)(double) = f; return p(1.0); }
export inline int qualified() { return ::f(2.0); }
export inline int by_adl() { return adl(N::A{}); }
export inline B operator_name() { return operator+(B{}, B{}); }
export inline int with_template() { return ft(1.0); }
export inline int template_id() { return tf<int>(1, 2); }
export inline unsigned long unevaluated() { return sizeof(f(1.0)); }
export template <class T> inline int dependent(T x) { return f(x); }
export template <class T> inline int non_dependent(T) { return f(1.0); }
template <class T> inline int calls_adl(T x) { return adl(x); }
export inline int instantiation() { return calls_adl(N::A{}); }
export int default_argument(int = f(1.0));
export struct Member { int m = f(1.0); };
export inline auto closure = [] { return f(1.0); };
