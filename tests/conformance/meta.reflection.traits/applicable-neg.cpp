// [meta.reflection.traits]/3.1.2, [meta.syn] (P1317): apply_result of a
// type that is not tuple-like, or of an invocation that is ill-formed, has
// no member `type`, so the call throws (not a constant expression); every
// operand must be a type ([meta.reflection.traits]/2).
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 22:15 error "static assertion expression is not an integral constant expression"
//@ diag: 22:15 note "'apply_result' has no result for these operands"
//@ diag: 23:15 error "static assertion expression is not an integral constant expression"
//@ diag: 23:15 note "'apply_result' has no result for these operands"
//@ diag: 24:15 error "static assertion expression is not an integral constant expression"
//@ diag: 24:15 note "'^^v' does not represent a type"
#include <tuple>
#include "kiln_meta_traits.h"
using namespace std::meta;

int add(int, long);
struct NotTuple { int a, b; };
int v;
static_assert(apply_result(^^decltype(add), ^^NotTuple) == ^^int);
static_assert(apply_result(^^decltype(add), ^^std::tuple<int>) == ^^int);
static_assert(is_applicable_type(^^v, ^^std::tuple<>));
