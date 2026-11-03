// [expr.const.imm]/2 (P2996R13): a potentially-evaluated expression of
// consteval-only type (`^^x`, of type std::meta::info) that is not in an
// immediate function context nor a subexpression of an immediate
// invocation is immediate-escalating: it makes an immediate-escalating
// function (a lambda's call operator, a constexpr function template
// specialization) immediate ([expr.const.imm]/4), and is ill-formed in
// any other function (spec/sema.md §5.10). Manifestly constant-evaluated
// contexts (a constexpr variable's or a constant initializer, a
// template argument, static_assert) and unevaluated operands are exempt.
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 49:26 error "expression of consteval-only type 'std::meta::info' is not allowed outside an immediate function context"
//@ diag: 50:18 error "expression of consteval-only type 'std::meta::info' is not allowed outside an immediate function context"
//@ diag: 51:33 error "expression of consteval-only type 'std::meta::info' is not allowed outside an immediate function context"
//@ diag: 53:22 error "cannot take address of immediate function 'refl<int>' outside of an immediate invocation"
//@ diag: 37:35 note "declared here"
//@ diag: 37:51 note "'refl<int>' is an immediate function because its body contains an expression of consteval-only type 'std::meta::info'"
//@ diag: 54:22 error "cannot take address of immediate function 'refl_int<int>' outside of an immediate invocation"
//@ diag: 38:35 note "declared here"
//@ diag: 38:55 note "'refl_int<int>' is an immediate function because its body contains an expression of consteval-only type 'std::meta::info'"
//@ diag: 55:22 error "cannot take address of immediate function 'operator()' outside of an immediate invocation"
//@ diag: 39:12 note "declared here"
#include "kiln_meta.h"
using namespace std::meta;
consteval bool is_int(info r) { return r == ^^int; }

// Exempt: manifestly constant-evaluated, unevaluated, an immediate
// invocation's argument or an immediate function context.
constexpr info a = ^^int;
info b = ^^int;
template <info R> struct X {};
X<^^int> x;
static_assert(sizeof(^^int) == sizeof(info));
consteval info cf() { return ^^int; }
template <class T> constexpr info refl() { return ^^T; }
template <class T> constexpr info refl_int() { return ^^int; }
auto lam = [] { return ^^int; };
constexpr int k() { if consteval { (void)^^int; } return 0; }
static_assert(refl<int>() == ^^int);
static_assert(refl_int<char>() == (^^int) && lam() == ^^int);
void ok() {
  bool c = is_int(^^int);
  constexpr info d = ^^char;
  (void)c, (void)d;
}
// Ill-formed: not immediate-escalating.
info plain_fn() { return ^^int; }
void g() { (void)^^int; }
constexpr info cx_fn() { return ^^int; }
// The escalated functions are immediate.
void h() { auto p = &refl<int>; (void)p;
           auto q = &refl_int<int>; (void)q;
           auto l = &decltype(lam)::operator(); (void)l; }
