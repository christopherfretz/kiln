// [expr.const.imm]/2.1 (P4101R1): a potentially-evaluated immediate
// constant expression (`^^x`, whose value is a non-null reflection) that
// is not in an immediate function context nor a subexpression of an
// immediate invocation is immediate-escalating: it makes an immediate-escalating
// function (a lambda's call operator, a constexpr function template
// specialization) immediate ([expr.const.imm]/4), and is ill-formed in
// any other function (spec/sema.md §5.10). Manifestly constant-evaluated
// contexts (a constexpr variable's initializer, a template argument,
// static_assert) and unevaluated operands are exempt.
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 46:26 error "consteval-only value outside an immediate function context"
//@ diag: 47:18 error "consteval-only value outside an immediate function context"
//@ diag: 48:50 error "consteval-only value outside an immediate function context"
//@ diag: 50:22 error "cannot take address of immediate function 'is_refl_int<int>' outside of an immediate invocation"
//@ diag: 34:35 note "declared here"
//@ diag: 34:58 note "'is_refl_int<int>' is an immediate function because its body contains a consteval-only value of type 'std::meta::info'"
//@ diag: 51:22 error "cannot take address of immediate function 'operator()' outside of an immediate invocation"
//@ diag: 35:12 note "declared here"
#include "kiln_meta.h"
using namespace std::meta;
consteval bool is_int(info r) { return r == ^^int; }

// Exempt: manifestly constant-evaluated, unevaluated, an immediate
// invocation's argument or an immediate function context.
constexpr info a = ^^int;
template <info R> struct X {};
X<^^int> x;
static_assert(sizeof(^^int) == sizeof(info));
consteval info cf() { return ^^int; }
template <class T> constexpr info refl() { return ^^T; }
template <class T> constexpr bool is_refl_int() { return ^^T == ^^int; }
auto lam = [] { return ^^int == ^^int; };
constexpr int k() { if consteval { (void)^^int; } return 0; }
static_assert(refl<int>() == ^^int);
static_assert(is_refl_int<int>() && lam());
void ok() {
  bool c = is_int(^^int);
  constexpr info d = ^^char;
  static_assert(d == ^^char);
  (void)c;
}
// Ill-formed: not immediate-escalating.
bool plain_fn() { return ^^int == ^^char; }
void g() { (void)^^int; }
constexpr bool cx_fn() { return is_int(^^int) && ^^int == ^^char; }
// The escalated functions are immediate.
void h() { auto p = &is_refl_int<int>; (void)p;
           auto l = &decltype(lam)::operator(); (void)l; }
