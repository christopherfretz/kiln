// [temp.res.general]/4-5: a parameter of a function declared at namespace
// scope with an unqualified declarator-id needs `typename` before a
// dependent qualified name. After clang's "missing 'typename'" error the
// declaration is invalid ([dcl.meaning.general]/2: an out-of-line
// definition does not match it). [temp.deduct.guide]/3: a deduction-guide
// template's parameters must be deducible.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:42 error "missing 'typename' prior to dependent type name 'X<T>::m'"
//@ diag: 32:42 error "missing 'typename' prior to dependent type name 'X<T>::m'"
//@ diag: 34:42 error "missing 'typename' prior to dependent type name 'X<T>::m'"
//@ diag: 35:28 error "out-of-line definition of 'f3' does not match any declaration in namespace 'N'"
//@ diag: 36:28 error "missing 'typename' prior to dependent type name 'X<T>::m'"
//@ diag: 37:28 error "missing 'typename' prior to dependent type name 'X<T>::m'"
//@ diag: 42:20 error "deduction guide template contains a template parameter that cannot be deduced"
//@ diag: 42:17 note "non-deducible template parameter 'T'"
//@ diag: 43:25 error "missing 'typename' prior to dependent type name 'X<T>::m'"
//@ diag: 44:31 error "missing 'typename' prior to dependent type name 'X<T>::m'"
//@ diag: 52:25 error "variable has incomplete type 'void'"
//@ diag: 52:34 error "use of undeclared identifier 'fp'"
//@ diag: 52:51 error "expected '(' for function-style cast or type construction"
//@ requires: clang>=22
// clang 22 spelling of missing-typename-invalid-decl.cpp (WP-394).
template <class T> struct X { using m = int; };
template <class T> struct S { S(int); };

// The declaration is recovered as if `typename` were written, but it is
// invalid: no qualified (out-of-line) redeclaration matches it, whatever
// its parameter says. Calls and unqualified redeclarations still find it.
namespace N { template <class T> void f1(X<T>::m x); }
template <class T> void N::f1(X<T>::m x) {}
namespace N { template <class T> void f2(X<T>::m x, int); }
template <class T> void N::f2(typename X<T>::m x, int) {}
namespace N { template <class T> void f3(X<T>::m x); }
template <class T> void N::f3(int x) {}
template <class T> void f4(X<T>::m x);
template <class T> void f4(X<T>::m x) {}
void use() { f4<int>(1); }

// Deduction guides at namespace scope: only the first parameter takes
// `X<T>::m` as a type (clang); in a class, every parameter does.
template <class T> S(X<T>::m*) -> S<T>;
template <class T> S(T, X<T>::m) -> S<T>;
template <class T> S(X<T>::m, X<T>::m, T) -> S<T>;
struct G {
  template <class T> struct W { W(int); };
  template <class T> W(T, X<T>::m) -> W<T>;
};

// A parenthesized function type with a dependent parameter is an
// initializer: `h` is a variable.
template <class T> void h(void (*fp)(X<T>::m), int);
