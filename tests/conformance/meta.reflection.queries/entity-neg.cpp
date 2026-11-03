// [meta.reflection.queries] P2996R13: the failure conditions of
// parent_of, object_of and constant_of. Each call that fails is not a
// constant expression, with a note in the shape of GCC 16.1's
// std::meta::exception what() for the same call (checked by hand):
// parent_of without has_parent; object_of of what is neither an object
// nor a variable with static storage duration (a non-static data
// member, a local variable); constant_of of a variable not usable in
// constant expressions ([expr.const]/3: not constexpr, nor const
// non-volatile integral with a constant initializer) and of what is not
// a valid splice-expression operand (a type).
//@ expect: compile-fail
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 44:15 error "static assertion expression is not an integral constant expression"
//@ diag: 44:15 note "reflection does not represent an entity with parent"
//@ diag: 45:15 error "static assertion expression is not an integral constant expression"
//@ diag: 45:15 note "reflection does not represent an entity with parent"
//@ diag: 46:15 error "static assertion expression is not an integral constant expression"
//@ diag: 46:15 note "reflection does not represent an entity with parent"
//@ diag: 47:15 error "static assertion expression is not an integral constant expression"
//@ diag: 47:15 note "reflection does not represent an object with static storage duration, or a reference to such an object"
//@ diag: 48:15 error "static assertion expression is not an integral constant expression"
//@ diag: 48:15 note "reflection does not represent an object with static storage duration, or a reference to such an object"
//@ diag: 51:17 error "static assertion expression is not an integral constant expression"
//@ diag: 51:17 note "reflection does not represent an object with static storage duration, or a reference to such an object"
//@ diag: 53:15 error "static assertion expression is not an integral constant expression"
//@ diag: 53:15 note "the value of 'nc' is not usable in a constant expression"
//@ diag: 54:15 error "static assertion expression is not an integral constant expression"
//@ diag: 54:15 note "the value of 'bad' is not usable in a constant expression"
//@ diag: 55:15 error "static assertion expression is not an integral constant expression"
//@ diag: 55:15 note "the value of 'cvv' is not usable in a constant expression"
//@ diag: 56:15 error "static assertion expression is not an integral constant expression"
//@ diag: 56:15 note "the value of 'dd' is not usable in a constant expression"
//@ diag: 57:15 error "static assertion expression is not an integral constant expression"
//@ diag: 57:15 note "reflection does not represent an annotation or a valid argument to a splice-expression"
#include "kiln_meta_queries.h"
using namespace std::meta;
struct S { int m; };
int nc = 2;
const int bad = nc;
const volatile int cvv = 3;
const double dd = 1.0;
static_assert(parent_of(^^::) == ^^::);
static_assert(parent_of(^^int) == ^^::);
static_assert(parent_of(info{}) == ^^::);
static_assert(object_of(^^S) == ^^S);
static_assert(object_of(^^S::m) == ^^S);
void h() {
  int loc;
  static_assert(object_of(^^loc) == ^^loc);
}
static_assert(constant_of(^^nc) == ^^S);
static_assert(constant_of(^^bad) == ^^S);
static_assert(constant_of(^^cvv) == ^^S);
static_assert(constant_of(^^dd) == ^^S);
static_assert(constant_of(^^S) == ^^S);
