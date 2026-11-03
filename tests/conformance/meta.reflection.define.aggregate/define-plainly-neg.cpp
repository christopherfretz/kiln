// [meta.reflection.define.aggregate]/7 (P2996R13; spec/reflection.md
// §6.2): define_aggregate is a constant subexpression only when it is
// plainly constant-evaluated (a consteval block's evaluation). Elsewhere
// it is not a constant expression -- a hard failure, not a
// std::meta::exception, so a try block does not catch it.
//@ expect: compile-fail
//@ args: -ferror-limit=0 -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 28:15 error "static assertion expression is not an integral constant expression"
//@ diag: 28:15 note "'std::meta::define_aggregate' can only be called from a 'consteval' block"
//@ diag: 30:20 error "call to consteval function 'def' is not a constant expression"
//@ diag: 29:37 note "'std::meta::define_aggregate' can only be called from a 'consteval' block"
//@ diag: 30:20 note "in call to 'def(^^A)'"
//@ diag: 30:16 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 29:37 note "'std::meta::define_aggregate' can only be called from a 'consteval' block"
//@ diag: 30:20 note "in call to 'def(^^A)'"
//@ diag: 32:5 error "non-type template argument is not a constant expression"
//@ diag: 29:37 note "'std::meta::define_aggregate' can only be called from a 'consteval' block"
//@ diag: 32:5 note "in call to 'def(^^A)'"
//@ diag: 41:15 error "static assertion expression is not an integral constant expression"
//@ diag: 29:37 note "'std::meta::define_aggregate' can only be called from a 'consteval' block"
//@ diag: 35:5 note "in call to 'def(^^A)'"
//@ diag: 41:15 note "in call to 'caught()'"
#include "kiln_meta.h"
using namespace std::meta;
struct A;
static_assert(define_aggregate(^^A, {}) == ^^A);
consteval info def(info c) { return define_aggregate(c, {}); }
constexpr info b = def(^^A);
template <info R> struct Tag {};
Tag<def(^^A)> t;
consteval bool caught() {
  try {
    def(^^A);
  } catch (...) {
    return true;
  }
  return false;
}
static_assert(caught());
// None of these defined A.
consteval { define_aggregate(^^A, {}); }
static_assert(sizeof(A) == 1);
