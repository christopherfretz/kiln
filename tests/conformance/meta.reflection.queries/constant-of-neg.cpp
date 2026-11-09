// [meta.reflection.queries] P2996R13: the failure conditions of
// constant_of over values of class, array, floating-point and reference
// type and over objects. Each failing call is not a constant expression,
// with a note in the shape of GCC 16.1's std::meta::exception what()
// (checked by hand; GCC rejects every line): the complete object of an
// object, or the object a reference refers to, is not usable in constant
// expressions ([expr.const]/4: a non-const variable; a structured
// binding of a non-constexpr decomposition); a non-static member
// function or data member is no splice-expression operand; the value's
// type is not structural ([temp.param]/7: a mutable or private member);
// the value refers to a string literal ([temp.arg.nontype]/6).
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 46:15 error "static assertion expression is not an integral constant expression"
//@ diag: 46:15 note "the value of 'gv' is not usable in a constant expression"
//@ diag: 47:15 error "static assertion expression is not an integral constant expression"
//@ diag: 47:15 note "the value of 'gv' is not usable in a constant expression"
//@ diag: 48:15 error "static assertion expression is not an integral constant expression"
//@ diag: 48:15 note "the value of 'gs' is not usable in a constant expression"
//@ diag: 49:15 error "static assertion expression is not an integral constant expression"
//@ diag: 49:15 note "the value of 'n1' is not usable in a constant expression"
//@ diag: 50:15 error "static assertion expression is not an integral constant expression"
//@ diag: 50:15 note "reflection does not represent an annotation or a valid argument to a splice-expression"
//@ diag: 51:15 error "static assertion expression is not an integral constant expression"
//@ diag: 51:15 note "reflection does not represent an annotation or a valid argument to a splice-expression"
//@ diag: 52:15 error "static assertion expression is not an integral constant expression"
//@ diag: 52:15 note "'M' is not a structural type"
//@ diag: 53:15 error "static assertion expression is not an integral constant expression"
//@ diag: 53:15 note "'P' is not a structural type"
//@ diag: 54:15 error "static assertion expression is not an integral constant expression"
//@ diag: 54:15 note "the value refers to a string literal, which is not a valid constant template argument"
#include "kiln_meta_queries.h"
using namespace std::meta;
struct S { int m; int mf(); };
struct M { mutable int x; };
struct P { private: int x = 0; };
int gv;
int& rg = gv;
S gs{1};
constexpr M cm{};
constexpr P cp{};
constexpr const char* lit = "x";
auto [n1] = gs;
static_assert(constant_of(object_of(^^gv)) == ^^S);
static_assert(constant_of(^^rg) == ^^S);
static_assert(constant_of(reflect_object(gs.m)) == ^^S);
static_assert(constant_of(^^n1) == ^^S);
static_assert(constant_of(^^S::mf) == ^^S);
static_assert(constant_of(^^S::m) == ^^S);
static_assert(constant_of(^^cm) == ^^S);
static_assert(constant_of(^^cp) == ^^S);
static_assert(constant_of(^^lit) == ^^S);
