// [basic.link]/14, /17 (WP-1845): a concept is a template, and its
// constraint-expression is part of its concept-definition, neither a
// function body nor an initializer /14 ignores. In a module interface, a
// concept that is not TU-local naming a TU-local entity there is an
// exposure: a call (also in an unevaluated operand, a requires-expression
// and a dependent call's overload set), a TU-local type (in `sizeof`, a
// type-requirement, a requires-expression parameter), a TU-local concept's
// concept-name (/14.4 names "a concept-name denoting E"), a `decltype`
// operand, and a use in a lambda there (the error at the lambda). Exported
// or not. A concept-name denoting a TU-local concept in an inline function
// is an exposure too. GCC 16.1 rejects each concept here (but `C8`, and
// it accepts the inline function `names_concept`); clang 22 accepts all of
// it. The concepts that stay valid are in
// exposure-concept-definition-accepted.cpp.
//@ expect: compile-fail
//@ std: c++20
//@ skip(clang): clang 22 does not check concept definitions
//@ strict-diags
//@ diag: 34:35 error "declaration of 'C1' exposes TU-local entity 'helper'"
//@ diag: 35:35 error "declaration of 'C2' exposes TU-local entity 'helper'"
//@ diag: 36:35 error "declaration of 'C3' exposes TU-local entity 'S'"
//@ diag: 37:35 error "declaration of 'C4' exposes TU-local entity 'S'"
//@ diag: 38:35 error "declaration of 'C5' exposes TU-local entity 'Hid'"
//@ diag: 39:28 error "declaration of 'C6' exposes TU-local entity 'helper'"
//@ diag: 40:40 error "declaration of 'C7' exposes TU-local entity 'helper'"
//@ diag: 41:35 error "declaration of 'C8' exposes TU-local entity 'hidden'"
//@ diag: 42:35 error "declaration of 'C9' exposes TU-local entity 'S'"
//@ diag: 43:20 error "declaration of 'names_concept' exposes TU-local entity 'Hid'"
export module M;
static int helper() { return 1; }
static int hidden;
namespace { struct S { int v; }; template <class T> concept Hid = true; }
template <class T> struct Box { T t; };
export template <class T> concept C1 = sizeof(helper()) > 0;
export template <class T> concept C2 = requires { helper(); };
export template <class T> concept C3 = sizeof(S) > 0;
export template <class T> concept C4 = requires { typename Box<S>; };
export template <class T> concept C5 = Hid<T>;
template <class T> concept C6 = requires(T t) { helper(t); };
export template <class T> concept C7 = [] { return helper() > 0; }();
export template <class T> concept C8 = sizeof(decltype(hidden)) > 0;
export template <class T> concept C9 = requires(S s) { s.v; };
export inline bool names_concept() { return Hid<int>; }
