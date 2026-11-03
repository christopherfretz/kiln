// [temp.res.general]/6, [expr.prim.splice]/2 P2996R13: a dependent splice
// is checked when it is instantiated: a template argument designating the
// wrong kind of entity for the splice's position is an error in that
// specialization, reported with the instantiation that requested it.
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 22:44 error "splice in a type position does not designate a type"
//@ diag: 26:1 note "in instantiation of template type alias 'TT' requested here"
//@ diag: 20:54 error "splice in an expression position does not designate a variable, function, enumerator, value or object"
//@ diag: 24:9 note "in instantiation of function template specialization 'value<^^int>' requested here"
//@ diag: 21:54 error "splice in a scope position does not designate a class, namespace or enumeration"
//@ diag: 25:9 note "in instantiation of function template specialization 'scope<^^N::x>' requested here"
//@ diag: 23:60 error "splice in a member access position does not designate a member of 'S'"
//@ diag: 27:9 note "in instantiation of function template specialization 'member<^^N::x>' requested here"
#include "kiln_meta.h"
namespace N { int x; }
struct S { int m; };
template <std::meta::info R> int value() { return [: R :]; }
template <std::meta::info R> int scope() { return [: R :]::x; }
template <std::meta::info R> using TT = [: R :];
template <std::meta::info R> int member(S s) { return s.[: R :]; }
int a = value<^^int>();
int b = scope<^^N::x>();
TT<^^N> c;
int d = member<^^N::x>(S{});
